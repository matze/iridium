use super::{AuthParams, Item};
use aes::Aes256;
use anyhow::Result;
use block_modes::block_padding::Pkcs7;
use block_modes::{BlockMode, Cbc};
use data_encoding::{BASE64, HEXLOWER};
use ring::{digest, hmac};
use std::str;

pub type Key = [u8; 768 / 8 / 3];

pub struct Crypto {
    pub pw: Key,
    pub mk: Key,
    pub ak: Key,
}

pub fn decrypt<S: AsRef<str>>(s: S, ek: &Key, ak: &Key, check_uuid: S) -> Result<String> {
    let s: Vec<&str> = s.as_ref().split(':').collect();
    let version = s[0];
    let auth_hash = s[1];
    let uuid = s[2];
    let iv = s[3];
    let ciphertext = s[4];

    assert!(version == "003");
    assert!(check_uuid.as_ref() == uuid);

    let to_auth = std::format!("003:{}:{}:{}", uuid, iv, ciphertext);
    let auth_hash_bytes = HEXLOWER.decode(&auth_hash.as_bytes())?;
    let key = hmac::Key::new(hmac::HMAC_SHA256, ak);
    hmac::verify(&key, to_auth.as_bytes(), &auth_hash_bytes).expect("foo");

    type Aes256Cbc = Cbc<Aes256, Pkcs7>;
    let iv_bytes = HEXLOWER.decode(iv.as_bytes())?;
    let cipher = Aes256Cbc::new_var(ek, &iv_bytes)?;
    let ciphertext_bytes = BASE64.decode(ciphertext.as_bytes())?;
    let decrypted = cipher.decrypt_vec(ciphertext_bytes.as_ref())?;
    Ok(str::from_utf8(decrypted.as_ref())?.to_string())
}

impl Crypto {
    pub fn new(params: &AuthParams, password: &str) -> Result<Self> {
        let cost = std::num::NonZeroU32::new(params.pw_cost).unwrap();
        let salt_input = std::format!("{}:SF:003:{}:{}", params.identifier, cost, params.pw_nonce);
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

        Ok(Crypto {
            pw: pw,
            mk: mk,
            ak: ak,
        })
    }

    pub fn decrypt(&self, item: &Item) -> Result<String> {
        let item_key = decrypt(&item.enc_item_key, &self.mk, &self.ak, &item.uuid)?;
        let mut item_ek: Key = [0; 32];
        let mut item_ak: Key = [0; 32];

        HEXLOWER
            .decode_mut(item_key[..64].as_bytes(), &mut item_ek)
            .expect("foo");
        HEXLOWER
            .decode_mut(item_key[64..].as_bytes(), &mut item_ak)
            .expect("foo");

        Ok(decrypt(&item.content, &item_ek, &item_ak, &item.uuid)?)
    }
}
