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

#[derive(Deserialize, Debug, Clone)]
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

#[derive(Deserialize, Clone)]
pub struct Note {
    pub title: Option<String>,
    pub text: String,
}

pub struct NoteItem {
    pub item: Item,
    pub note: Note,
}

impl Root {
    pub fn notes(&self, password: &str) -> Result<Vec<NoteItem>> {
        let crypto = crypto::Crypto::new(&self.auth_params, password)?;

        let notes = self
            .items
            .iter()
            .filter(|x| x.content_type == "Note")
            .map(|x| NoteItem {item: x.clone(), note: serde_json::from_str(&crypto.decrypt(x).unwrap()).unwrap()})
            .collect::<Vec<NoteItem>>();

        Ok(notes)
    }
}
