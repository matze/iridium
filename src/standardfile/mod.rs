use anyhow::Result;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

pub mod crypto;

#[derive(Deserialize)]
pub struct AuthParams {
    pub identifier: String,
    pub pw_cost: u32,
    pub pw_nonce: String,
    pub version: String,
}

#[derive(Serialize, Deserialize)]
pub struct Item {
    pub uuid: Uuid,
    pub content: String,
    pub content_type: String,
    pub enc_item_key: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct Exported {
    pub auth_params: AuthParams,
    items: Vec<Item>,
}

#[derive(Serialize, Deserialize)]
pub struct Note {
    pub title: Option<String>,
    pub text: String,
}

impl Exported {
    pub fn encrypted_notes(&self) -> Vec<&Item> {
        self
            .items
            .iter()
            .filter(|x| x.content_type == "Note")
            .collect::<Vec<&Item>>()
    }
}
