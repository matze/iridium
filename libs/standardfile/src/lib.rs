#![forbid(unsafe_code)]

use anyhow::Result;
use block_modes::{BlockModeError, InvalidKeyIvLength};
use chrono::{DateTime, Utc};
use data_encoding::DecodeError;
use uuid::Uuid;
use serde::{Serialize, Deserialize};
use thiserror::Error;
use std::str::Utf8Error;

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

#[derive(Serialize, Deserialize)]
pub struct AuthParams {
    pub identifier: String,
    pub pw_cost: u32,
    pub pw_nonce: String,
    pub version: String,
}

#[derive(Serialize, Deserialize)]
pub struct Exported {
    pub auth_params: AuthParams,
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

pub enum Item {
    Note(Note),
    Tag(Tag),
}

#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("unknown item content type `{0}'")]
    UnknownContentType(String),
    #[error("unsupported encryption scheme {0}")]
    UnsupportedScheme(String),
    #[error("uuid mismatch")]
    UuidMismatch,
    #[error("uuid decode error")]
    UuidDecode(#[from] uuid::Error),
    #[error("verification issue")]
    Verification,
    #[error("block mode error")]
    BlockMode(#[from] BlockModeError),
    #[error("iv length error")]
    IvLength(#[from] InvalidKeyIvLength),
    #[error("decode error")]
    Decode(#[from] DecodeError),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
    #[error("utf8 decode error")]
    Utf8Decode(#[from] Utf8Error),
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

impl AuthParams {
    pub fn from_credentials(credentials: &Credentials) -> Self {
        Self {
            identifier: credentials.identifier.clone(),
            pw_cost: credentials.cost,
            pw_nonce: credentials.nonce.clone(),
            version: "003".to_string(),
        }
    }
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

    /// Decrypt Envelope to an Item.
    pub fn decrypt(&self, crypto: &crypto::Crypto) -> Result<Item, CryptoError> {
        if self.content_type == "Note" {
            Ok(Note::decrypt(crypto, &self)?)
        }
        else if self.content_type == "Tag" {
            Ok(Tag::decrypt(crypto, &self)?)
        }
        else {
            Err(CryptoError::UnknownContentType(self.content_type.clone()))
        }
    }
}

impl Item {
    /// Encrypt Item to an Envelope.
    pub fn encrypt(&self, crypto: &crypto::Crypto) -> Result<Envelope> {
        match self {
            Item::Note(note) => note.encrypt(crypto),
            Item::Tag(tag) => tag.encrypt(crypto),
        }
    }

    /// Get uuid.
    pub fn uuid(&self) -> Uuid {
        match self {
            Item::Note(note) => note.uuid,
            Item::Tag(tag) => tag.uuid,
        }
    }
}

impl Exported {
    /// Deserialize Exported from JSON string.
    pub fn from_str(s: &str) -> Result<Self> {
        Ok(serde_json::from_str(s)?)
    }

    pub fn to_str(&self) -> Result<String> {
        Ok(serde_json::to_string(&self)?)
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

impl Note {
    fn encrypt(&self, crypto: &crypto::Crypto) -> Result<Envelope> {
        let content = NoteContent {
            title: Some(self.title.clone()),
            text: self.text.clone(),
        };

        let to_encrypt = serde_json::to_string(&content)?;
        let encrypted = crypto.encrypt(&to_encrypt, &self.uuid)?;

        Ok(Envelope {
            uuid: self.uuid,
            content: Some(encrypted.content),
            content_type: "Note".to_owned(),
            enc_item_key: Some(encrypted.enc_item_key),
            created_at: self.created_at,
            updated_at: self.updated_at,
            deleted: Some(false),
        })
    }

    fn decrypt(crypto: &crypto::Crypto, item: &Envelope) -> Result<Item> {
        let decrypted = crypto.decrypt(item)?;
        let content = serde_json::from_str::<NoteContent>(&decrypted)?;

        Ok(Item::Note(Note {
            title: content.title.unwrap_or("".to_string()),
            text: content.text,
            created_at: item.created_at,
            updated_at: item.updated_at,
            uuid: item.uuid,
        }))
    }
}

impl Tag {
    fn encrypt(&self, crypto: &crypto::Crypto) -> Result<Envelope> {
        let content = TagContent {
            title: self.title.clone(),
            references: self.references
                .iter()
                .map(|uuid| Reference {
                    uuid: uuid.clone(),
                    content_type: "Note".to_string(),
                })
                .collect::<_>()
        };

        let to_encrypt = serde_json::to_string(&content)?;
        let encrypted = crypto.encrypt(&to_encrypt, &self.uuid)?;

        Ok(Envelope {
            uuid: self.uuid,
            content: Some(encrypted.content),
            content_type: "Note".to_owned(),
            enc_item_key: Some(encrypted.enc_item_key),
            created_at: self.created_at,
            updated_at: self.updated_at,
            deleted: Some(false),
        })
    }

    fn decrypt(crypto: &crypto::Crypto, item: &Envelope) -> Result<Item> {
        let decrypted = crypto.decrypt(item)?;
        let content = serde_json::from_str::<TagContent>(&decrypted)?;
        let references = content.references
            .iter()
            .map(|reference| reference.uuid)
            .collect::<_>();

        Ok(Item::Tag(Tag {
            title: content.title,
            references: references,
            created_at: item.created_at,
            updated_at: item.updated_at,
            uuid: item.uuid,
        }))
    }
}
