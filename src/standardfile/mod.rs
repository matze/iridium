use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Deserialize;

pub mod crypto;

#[derive(Deserialize, Debug)]
pub struct AuthParams {
    pub identifier: String,
    pub pw_cost: u32,
    pub pw_nonce: String,
    pub version: String,
}

#[derive(Deserialize, Debug)]
pub struct Item {
    pub uuid: String,
    pub content: String,
    pub content_type: String,
    pub enc_item_key: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Deserialize, Debug)]
pub struct Root {
    pub auth_params: AuthParams,
    pub items: Vec<Item>,
}

#[derive(Deserialize)]
pub struct Note {
    pub title: Option<String>,
    pub text: String,
}

impl Root {
    pub fn notes(&self, password: &str) -> Result<Vec<Note>> {
        let crypto = crypto::Crypto::new(&self.auth_params, password)?;

        let notes = self
            .items
            .iter()
            .filter(|x| x.content_type == "Note")
            .map(|x| serde_json::from_str(&crypto.decrypt(x).unwrap()).unwrap())
            .collect::<Vec<Note>>();

        Ok(notes)
    }
}
