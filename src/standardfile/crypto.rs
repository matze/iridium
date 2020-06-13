use super::{ExportedAuthParams, RemoteAuthParams, Item, Note};
use crate::models;
use crate::standardfile;
use aes::Aes256;
use anyhow::Result;
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

    assert!(version == "003");
    assert!(check_uuid == &uuid);

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

impl Crypto {
    fn new(identifier: &str, cost: u32, nonce: &str, password: &str) -> Result<Self> {
        let cost = std::num::NonZeroU32::new(cost).unwrap();
        let salt_input = std::format!("{}:SF:003:{}:{}", identifier, cost, nonce);
        let salt = digest::digest(&digest::SHA256, salt_input.as_bytes());
        let hex_salt = HEXLOWER.encode(&salt.as_ref());
        let mut hashed = [0u8; 768 / 8];

        ring::pbkdf2::derive(
            ring::pbkdf2::PBKDF2_HMAC_SHA512,
            cost,
            &hex_salt.as_bytes(),
            password.as_bytes(),
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

    /// Construct crypto manager from local, exported JSON.
    pub fn new_from_exported(params: &ExportedAuthParams, password: &str) -> Result<Self> {
        Self::new(params.identifier.as_str(), params.pw_cost, params.pw_nonce.as_str(), password)
    }

    /// Construct crypto manager from remote signin process.
    pub fn new_from_remote(params: &RemoteAuthParams, identifier: &str, password: &str) -> Result<Self> {
        Self::new(identifier, params.pw_cost, params.pw_nonce.as_str(), password)
    }

    pub fn password(&self) -> String {
        HEXLOWER.encode(&self.pw)
    }

    pub fn decrypt(&self, item: &Item) -> Result<models::Decrypted> {
        let item_key = decrypt(&item.enc_item_key, &self.mk, &self.ak, &item.uuid)?;
        let mut item_ek: Key = [0; 32];
        let mut item_ak: Key = [0; 32];

        HEXLOWER
            .decode_mut(item_key[..64].as_bytes(), &mut item_ek)
            .expect("foo");
        HEXLOWER
            .decode_mut(item_key[64..].as_bytes(), &mut item_ak)
            .expect("foo");

        let decrypted = decrypt(&item.content, &item_ek, &item_ak, &item.uuid)?;

        if item.content_type == "Note" {
            Ok(models::Decrypted::Note(serde_json::from_str::<standardfile::Note>(decrypted.as_str())?))
        } else {
            Ok(models::Decrypted::None)
        }
    }

    pub fn encrypt(&self, note: &models::Note, uuid: &Uuid) -> Result<Item> {
        let json_note = Note {
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

        let to_encrypt = serde_json::to_string(&json_note)?;

        let mut iv_bytes = [0u8; 16];
        rng.fill_bytes(&mut iv_bytes);

        let item_key_encoded = HEXLOWER.encode(item_key.as_ref());

        Ok(Item {
            uuid: uuid.clone(),
            content: encrypt(to_encrypt.as_ref(), &item_ek, &item_ak, &uuid)?,
            content_type: "Note".to_owned(),
            enc_item_key: encrypt(item_key_encoded.as_ref(), &self.mk, &self.ak, &uuid)?,
            created_at: note.created_at,
            updated_at: note.updated_at,
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

        let note = models::Note {
            title: "Title".to_owned(),
            text: "Text".to_owned(),
            created_at: now,
            updated_at: now,
            uuid: uuid,
        };

        let auth_params = ExportedAuthParams {
            identifier: "foo@bar.com".to_owned(),
            pw_cost: 110000,
            pw_nonce: "3f8ea1ffd8067c1550ca3ad78de71c9b6e68b5cb540e370c12065eca15d9a049".to_owned(),
            version: "003".to_owned(),
        };

        let crypto = Crypto::new_from_exported(&auth_params, "foobar").unwrap();
        let encrypted = crypto.encrypt(&note, &uuid).unwrap();

        match crypto.decrypt(&encrypted).unwrap() {
            models::Decrypted::Note(decrypted) => {
                assert_eq!(decrypted.title.unwrap(), note.title);
                assert_eq!(decrypted.text, note.text);
            },
            models::Decrypted::None => {
                assert!(false);
            }
        }
    }
}
