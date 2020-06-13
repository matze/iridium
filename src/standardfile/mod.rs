use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

pub mod crypto;
pub mod remote;

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
pub struct RemoteAuthParams {
    pub pw_cost: u32,
    pub pw_nonce: String,
    pub version: String,
}

#[derive(Deserialize)]
struct RemoteUser {
    pub uuid: Uuid,
    pub email: String,
}

#[derive(Deserialize)]
struct RemoteSignInResponse {
    pub user: RemoteUser,
    pub token: String,
}

#[derive(Serialize)]
struct RemoteSyncRequest {
    pub items: Vec<Item>,
    pub sync_token: String,
}

#[derive(Deserialize)]
struct RemoteSyncResponse {
    pub retrieved_items: Vec<Item>,
    pub saved_items: Vec<Item>,
    pub unsaved: Vec<Item>,
    pub sync_token: String,
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
