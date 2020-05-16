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
}

#[derive(Deserialize, Debug)]
pub struct Root {
    pub auth_params: AuthParams,
    pub items: Vec<Item>,
}
