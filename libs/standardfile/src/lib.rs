#![forbid(unsafe_code)]

use anyhow::{anyhow, Result};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

pub mod crypto;
pub mod remote;

#[derive(Serialize, Deserialize, Debug)]
pub struct Envelope {
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
    pub items: Vec<Envelope>,
}

#[derive(Serialize, Deserialize)]
struct NoteContent {
    pub title: Option<String>,
    pub text: String,
}

#[derive(Serialize, Deserialize)]
pub struct Reference {
    pub uuid: Uuid,
    pub content_type: String,
}

#[derive(Serialize, Deserialize)]
struct TagContent {
    pub title: String,
    pub references: Vec<Reference>,
}

pub struct Note {
    pub title: String,
    pub text: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub uuid: Uuid,
}

pub struct Tag {
    pub title: String,
    pub references: Vec<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub uuid: Uuid,
}

pub struct EncryptedItem {
    pub content: String,
    pub enc_item_key: String,
}

pub trait Cipher<T> {
    fn encrypt(crypto: &crypto::Crypto, item: &T) -> Result<Envelope>;
    fn decrypt(crypto: &crypto::Crypto, item: &Envelope) -> Result<T>;
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

impl Envelope {
    /// Deserialize Envelope from JSON string.
    pub fn from_str(s: &str) -> Result<Self> {
        Ok(serde_json::from_str(s)?)
    }

    /// Serialize Envelope as JSON string.
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

fn decrypt(crypto: &crypto::Crypto, item: &Envelope, content_type: &str) -> Result<String> {
    if item.content_type != content_type {
        return Err(anyhow!("{} is not {}", item.content_type, content_type));
    }

    Ok(crypto.decrypt(item)?)
}

impl Cipher<Note> for Note {
    fn encrypt(crypto: &crypto::Crypto, note: &Note) -> Result<Envelope> {
        let content = NoteContent {
            title: Some(note.title.clone()),
            text: note.text.clone(),
        };

        let to_encrypt = serde_json::to_string(&content)?;
        let encrypted = crypto.encrypt(&to_encrypt, &note.uuid)?;

        Ok(Envelope {
            uuid: note.uuid,
            content: Some(encrypted.content),
            content_type: "Note".to_owned(),
            enc_item_key: Some(encrypted.enc_item_key),
            created_at: note.created_at,
            updated_at: note.updated_at,
            deleted: Some(false),
        })
    }

    fn decrypt(crypto: &crypto::Crypto, item: &Envelope) -> Result<Note> {
        let decrypted = decrypt(crypto, item, "Note")?;
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

impl Cipher<Tag> for Tag {
    fn encrypt(crypto: &crypto::Crypto, tag: &Tag) -> Result<Envelope> {
        let content = TagContent {
            title: tag.title.clone(),
            references: tag.references
                .iter()
                .map(|uuid| Reference {
                    uuid: uuid.clone(),
                    content_type: "Note".to_string(),
                })
                .collect::<_>()
        };

        let to_encrypt = serde_json::to_string(&content)?;
        let encrypted = crypto.encrypt(&to_encrypt, &tag.uuid)?;

        Ok(Envelope {
            uuid: tag.uuid,
            content: Some(encrypted.content),
            content_type: "Note".to_owned(),
            enc_item_key: Some(encrypted.enc_item_key),
            created_at: tag.created_at,
            updated_at: tag.updated_at,
            deleted: Some(false),
        })
    }

    fn decrypt(crypto: &crypto::Crypto, item: &Envelope) -> Result<Tag> {
        let decrypted = decrypt(crypto, item, "Tag")?;
        let content = serde_json::from_str::<TagContent>(&decrypted)?;
        let references = content.references
            .iter()
            .map(|reference| reference.uuid)
            .collect::<_>();

        Ok(Tag {
            title: content.title,
            references: references,
            created_at: item.created_at,
            updated_at: item.updated_at,
            uuid: item.uuid,
        })
    }
}
