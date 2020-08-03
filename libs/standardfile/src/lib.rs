#![forbid(unsafe_code)]

use anyhow::{anyhow, Result};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

pub mod crypto;
pub mod remote;

#[derive(Serialize, Deserialize, Debug)]
pub struct Item {
    pub uuid: Uuid,
    pub content: Option<String>,
    pub content_type: String,
    pub enc_item_key: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted: Option<bool>,
}

#[derive(Deserialize)]
pub struct ExportedAuthParams {
    pub identifier: String,
    pub pw_cost: u32,
    pub pw_nonce: String,
    pub version: String,
}

#[derive(Deserialize)]
pub struct Exported {
    pub auth_params: ExportedAuthParams,
    pub items: Vec<Item>,
}

#[derive(Serialize, Deserialize)]
pub struct NoteContent {
    pub title: Option<String>,
    pub text: String,
}

pub struct Note {
    pub title: String,
    pub text: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub uuid: Uuid,
}

pub trait Encrypted<T> {
    fn from_encrypted(crypto: &crypto::Crypto, item: &Item) -> Result<T>;
}

/// Authentication parameters constructed locally, from a remote server or an imported file and
/// passed to construct the crypto used in the storage.
#[derive(Clone)]
pub struct Credentials {
    pub identifier: String,
    pub cost: u32,
    pub nonce: String,
    pub password: String,
}

/// Retrieve all items of content_type Note.
pub fn encrypted_notes(items: &Vec<Item>) -> Vec<&Item> {
    items
    .iter()
    .filter(|x| x.content_type == "Note")
    .collect::<Vec<&Item>>()
}

impl Item {
    /// Deserialize Item from JSON string.
    pub fn from_str(s: &str) -> Result<Self> {
        Ok(serde_json::from_str(s)?)
    }

    /// Serialize Item as JSON string.
    pub fn to_string(&self) -> Result<String> {
        Ok(serde_json::to_string(&self)?)
    }
}

impl Exported {
    /// Deserialize Exported from JSON string.
    pub fn from_str(s: &str) -> Result<Self> {
        Ok(serde_json::from_str(s)?)
    }
}

impl Credentials {
    pub fn from_exported(exported: &Exported, password: &str) -> Self {
        Self {
            identifier: exported.auth_params.identifier.clone(),
            cost: exported.auth_params.pw_cost,
            nonce: exported.auth_params.pw_nonce.clone(),
            password: password.to_string(),
        }
    }

    pub fn from_defaults(identifier: &str, password: &str) -> Self {
        Self {
            identifier: identifier.to_string(),
            cost: 110000,
            nonce: crypto::make_nonce(),
            password: password.to_string(),
        }
    }
}

impl Encrypted<Note> for Note {
    fn from_encrypted(crypto: &crypto::Crypto, item: &Item) -> Result<Note> {
        if item.content_type != "Note" {
            return Err(anyhow!("Not a note"));
        }

        let decrypted = crypto.decrypt(item)?;
        let content = serde_json::from_str::<NoteContent>(&decrypted)?;

        Ok(Note {
            title: content.title.unwrap_or("".to_string()),
            text: content.text,
            created_at: item.created_at,
            updated_at: item.updated_at,
            uuid: item.uuid,
        })
    }
}
