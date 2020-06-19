use super::crypto::{make_nonce, Crypto};
use super::Item;
use anyhow::{anyhow, Result};
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use reqwest::StatusCode;
use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct AuthParams {
    pub pw_cost: u32,
    pub pw_nonce: String,
    pub version: String,
}

#[derive(Deserialize)]
struct User {
    pub uuid: Uuid,
    pub email: String,
}

#[derive(Deserialize)]
struct ErrorResponse {
    pub errors: Vec<String>,
}

#[derive(Serialize)]
struct RegistrationRequest {
    pub email: String,
    pub password: String,
    pub pw_cost: u32,
    pub pw_nonce: String,
    pub version: String,
}

#[derive(Serialize)]
struct SignInRequest {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
struct SignInResponse {
    pub user: User,
    pub token: String,
}

#[derive(Serialize)]
struct SyncRequest {
    pub items: Vec<Item>,
    pub sync_token: Option<String>,
    pub cursor_token: Option<String>,
}

#[derive(Deserialize)]
struct SyncResponse {
    pub retrieved_items: Vec<Item>,
    pub saved_items: Vec<Item>,
    pub unsaved: Vec<Item>,
    pub sync_token: String,
}

/// Register a new user and return JWT on success.
pub fn register(host: &str, email: &str, password: &str) -> Result<String> {
    let nonce = make_nonce();
    let cost = 110000;
    let crypto = Crypto::new(email, cost, &nonce, password)?;
    let encoded_pw = crypto.password();
    let cost_str = cost.to_string();

    let request = RegistrationRequest {
        email: email.to_string(),
        password: encoded_pw,
        pw_cost: cost,
        pw_nonce: nonce,
        version: "003".to_string(),
    };

    let url = format!("{}/auth", host);
    let client = reqwest::blocking::Client::new();
    let response = client.post(&url).json(&request).send()?;

    match response.status() {
        StatusCode::OK => {
            let response = response.json::<SignInResponse>()?;
            Ok(response.token)
        }
        _ => {
            let response = response.json::<ErrorResponse>()?;
            Err(anyhow!("{}", response.errors[0]))
        }
    }
}

/// Sign in and return JWT on success.
pub fn sign_in(host: &str, email: &str, password: &str) -> Result<String> {
    let client = reqwest::blocking::Client::new();

    let url = format!("{}/auth/params?email={}", host, email);
    let response = client.get(&url).send()?.json::<AuthParams>()?;
    let crypto = Crypto::new(email, response.pw_cost, &response.pw_nonce, password)?;
    let encoded_pw = crypto.password();

    let request = SignInRequest {
        email: email.to_string(),
        password: encoded_pw,
    };

    let url = format!("{}/auth/sign_in", host);
    let response = client.post(&url).json(&request).send()?;

    match response.status() {
        StatusCode::OK => {
            let response = response.json::<SignInResponse>()?;
            Ok(response.token)
        }
        _ => {
            let response = response.json::<ErrorResponse>()?;
            Err(anyhow!("{}", response.errors[0]))
        }
    }
}

pub fn sync(host: &str, token: &str) -> Result<()> {
    let client = reqwest::blocking::Client::new();

    let url = format!("{}/items/sync", host);
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    let sync_request = SyncRequest {
        items: vec![],
        sync_token: None,
        cursor_token: None,
    };

    let response = client
        .post(&url)
        .headers(headers)
        .bearer_auth(token)
        .body(serde_json::to_string(&sync_request)?)
        .send()?
        .json::<SyncResponse>()?;

    Ok(())
}
