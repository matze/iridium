use crate::{Item, NoteContent, Note, Credentials};
use aes::Aes256;
use anyhow::{anyhow, Result};
use block_modes::block_padding::Pkcs7;
use block_modes::{BlockMode, Cbc};
use data_encoding::{BASE64, HEXLOWER};
use rand::prelude::*;
use ring::{digest, hmac};
use std::str;
use uuid::Uuid;

pub type Key = [u8; 768 / 8 / 3];

pub struct Crypto {
    pw: Key,
    mk: Key,
    ak: Key,
}

type Aes256Cbc = Cbc<Aes256, Pkcs7>;

fn decrypt(s: &str, ek: &Key, ak: &Key, check_uuid: &Uuid) -> Result<String> {
    let s: Vec<&str> = s.split(':').collect();
    let version = s[0];
    let auth_hash = s[1];
    let uuid = Uuid::parse_str(s[2])?;
    let iv = s[3];
    let ciphertext = s[4];

    if version != "003" {
        return Err(anyhow!("No support for encryption scheme < 003"));
    }

    if &uuid != check_uuid {
        return Err(anyhow!("UUIDs do not match"));
    }

    let to_auth = std::format!("003:{}:{}:{}", uuid, iv, ciphertext);
    let auth_hash_bytes = HEXLOWER.decode(&auth_hash.as_bytes())?;
    let key = hmac::Key::new(hmac::HMAC_SHA256, ak);
    hmac::verify(&key, to_auth.as_bytes(), &auth_hash_bytes).expect("foo");

    let iv_bytes = HEXLOWER.decode(iv.as_bytes())?;
    let cipher = Aes256Cbc::new_var(ek, &iv_bytes)?;
    let ciphertext_bytes = BASE64.decode(ciphertext.as_bytes())?;
    let decrypted = cipher.decrypt_vec(ciphertext_bytes.as_ref())?;
    Ok(str::from_utf8(decrypted.as_ref())?.to_string())
}

fn encrypt(s: &str, ek: &Key, ak: &Key, uuid: &Uuid) -> Result<String> {
    let mut rng = rand_chacha::ChaCha20Rng::from_entropy();
    let mut iv_bytes = [0u8; 16];
    rng.fill_bytes(&mut iv_bytes);

    let uuid_encoded = uuid.to_hyphenated_ref();
    let cipher = Aes256Cbc::new_var(ek, &iv_bytes)?;
    let encrypted = cipher.encrypt_vec(s.as_ref());
    let encrypted_encoded = BASE64.encode(encrypted.as_slice());
    let iv_encoded = HEXLOWER.encode(iv_bytes.as_ref());
    let to_auth = std::format!("003:{}:{}:{}", uuid_encoded, iv_encoded, encrypted_encoded);
    let key = hmac::Key::new(hmac::HMAC_SHA256, ak.as_ref());
    let to_auth_bytes = to_auth.as_bytes();
    let auth_hash_bytes = hmac::sign(&key, to_auth_bytes);
    let auth_hash = HEXLOWER.encode(auth_hash_bytes.as_ref());

    Ok(std::format!(
        "003:{}:{}:{}:{}",
        auth_hash,
        uuid_encoded,
        iv_encoded,
        encrypted_encoded
    ))
}

fn get_nonzero_cost(credentials: &Credentials) -> Result<std::num::NonZeroU32> {
    let cost = std::num::NonZeroU32::new(credentials.cost);

    match cost {
        Some(cost) => {
            Ok(cost)
        },
        None => {
            Err(anyhow!("Cost must be larger than zero"))
        }
    }
}

/// Create random nonce.
pub fn make_nonce() -> String {
    let mut rng = rand_chacha::ChaCha20Rng::from_entropy();
    let mut nonce = [0u8; 32];
    rng.fill_bytes(&mut nonce);
    HEXLOWER.encode(nonce.as_ref())
}

impl Crypto {
    pub fn new(credentials: &Credentials) -> Result<Self> {
        let cost = get_nonzero_cost(&credentials)?;
        let salt_input = std::format!("{}:SF:003:{}:{}", credentials.identifier, credentials.cost, credentials.nonce);
        let salt = digest::digest(&digest::SHA256, salt_input.as_bytes());
        let hex_salt = HEXLOWER.encode(salt.as_ref());
        let mut hashed = [0u8; 768 / 8];

        ring::pbkdf2::derive(
            ring::pbkdf2::PBKDF2_HMAC_SHA512,
            cost,
            &hex_salt.as_bytes(),
            credentials.password.as_bytes(),
            &mut hashed,
        );

        let mut pw: Key = [0u8; 32];
        let mut mk: Key = [0u8; 32];
        let mut ak: Key = [0u8; 32];

        pw.clone_from_slice(&hashed[0..32]);
        mk.clone_from_slice(&hashed[32..64]);
        ak.clone_from_slice(&hashed[64..]);

        Ok(Crypto { pw: pw, mk: mk, ak: ak })
    }

    pub fn password(&self) -> String {
        HEXLOWER.encode(&self.pw)
    }

    pub fn decrypt(&self, item: &Item) -> Result<Note> {
        if item.enc_item_key.is_none() || item.content.is_none() {
            return Err(anyhow!("Cannot decrypt without key"));
        }

        let enc_item_key = item.enc_item_key.as_ref().ok_or(anyhow!("Encrypted item key required"))?;
        let content = item.content.as_ref().ok_or(anyhow!("Encrypted content required"))?;
        let item_key = decrypt(&enc_item_key, &self.mk, &self.ak, &item.uuid)?;
        let mut item_ek: Key = [0; 32];
        let mut item_ak: Key = [0; 32];

        HEXLOWER
            .decode_mut(item_key[..64].as_bytes(), &mut item_ek)
            .expect("foo");
        HEXLOWER
            .decode_mut(item_key[64..].as_bytes(), &mut item_ak)
            .expect("foo");

        let decrypted = decrypt(&content, &item_ek, &item_ak, &item.uuid)?;

        if item.content_type == "Note" {
            let content = serde_json::from_str::<NoteContent>(&decrypted)?;

            Ok(Note {
                title: content.title.unwrap_or("".to_string()),
                text: content.text,
                created_at: item.created_at,
                updated_at: item.updated_at,
                uuid: item.uuid,
            })
        } else {
            Err(anyhow!("Not a note"))
        }
    }

    pub fn encrypt(&self, note: &Note, uuid: &Uuid) -> Result<Item> {
        let content = NoteContent {
            title: Some(note.title.clone()),
            text: note.text.clone(),
        };

        let mut rng = rand_chacha::ChaCha20Rng::from_entropy();
        let mut item_key = [0u8; 64];
        rng.fill_bytes(&mut item_key);

        let mut item_ek: Key = [0; 32];
        let mut item_ak: Key = [0; 32];

        item_ek.clone_from_slice(&item_key[..32]);
        item_ak.clone_from_slice(&item_key[32..]);

        let to_encrypt = serde_json::to_string(&content)?;

        let mut iv_bytes = [0u8; 16];
        rng.fill_bytes(&mut iv_bytes);

        let item_key_encoded = HEXLOWER.encode(item_key.as_ref());

        Ok(Item {
            uuid: uuid.clone(),
            content: Some(encrypt(to_encrypt.as_ref(), &item_ek, &item_ak, &uuid)?),
            content_type: "Note".to_owned(),
            enc_item_key: Some(encrypt(item_key_encoded.as_ref(), &self.mk, &self.ak, &uuid)?),
            created_at: note.created_at,
            updated_at: note.updated_at,
            deleted: Some(false),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_encrypt_decrypt() {
        let now = Utc::now();
        let uuid = Uuid::new_v4();

        let note = Note {
            title: "Title".to_owned(),
            text: "Text".to_owned(),
            created_at: now,
            updated_at: now,
            uuid: uuid,
        };

        let nonce = "3f8ea1ffd8067c1550ca3ad78de71c9b6e68b5cb540e370c12065eca15d9a049";
        let credentials = Credentials {
            identifier: "foo@bar.com".to_string(),
            cost: 110000,
            nonce: nonce.to_string(),
            password: "secret".to_string(),
        };
        let crypto = Crypto::new(&credentials).unwrap();
        let encrypted = crypto.encrypt(&note, &uuid).unwrap();
        let decrypted = crypto.decrypt(&encrypted).unwrap();

        assert_eq!(decrypted.title, note.title);
        assert_eq!(decrypted.text, note.text);
    }
}
