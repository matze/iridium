use super::crypto::{make_nonce, Crypto};
use super::{
    RemoteAuthParams, RemoteErrorResponse, RemoteRegistrationRequest, RemoteRegistrationResponse, RemoteSignInResponse,
    RemoteSyncRequest, RemoteSyncResponse,
};
use anyhow::{anyhow, Result};
use reqwest::StatusCode;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use std::collections::HashMap;

/// Register a new user and return JWT on success.
pub fn register(host: &str, email: &str, password: &str) -> Result<String> {
    let nonce = make_nonce();
    let cost = 110000;
    let crypto = Crypto::new(email, cost, &nonce, password)?;
    let encoded_pw = crypto.password();
    let cost_str = cost.to_string();

    let request = RemoteRegistrationRequest {
        email: email.to_string(),
        password: encoded_pw,
        pw_cost: cost,
        pw_nonce: nonce,
        version: "003".to_string(),
    };

    let url = format!("{}/auth", host);
    let client = reqwest::blocking::Client::new();
    let response = client
        .post(&url)
        .json(&request)
        .send()?;

    match response.status() {
        StatusCode::OK => {
            let response = response.json::<RemoteRegistrationResponse>()?;
            Ok(response.token)
        }
        _ => {
            let response = response.json::<RemoteErrorResponse>()?;
            Err(anyhow!("{}", response.errors[0]))
        }
    }
}

/// Sign in and return JWT on success.
pub fn sign_in(host: &str, email: &str, password: &str) -> Result<String> {
    let client = reqwest::blocking::Client::new();

    let url = format!("{}/auth/params?email={}", host, email);
    let response = client.get(&url).send()?.json::<RemoteAuthParams>()?;
    let crypto = Crypto::new(email, response.pw_cost, &response.pw_nonce, password)?;

    let mut params = HashMap::new();
    let encoded_pw = crypto.password();
    params.insert("email", email);
    params.insert(password, &encoded_pw);

    let url = format!("{}/auth/sign_in", host);
    let response = client
        .post(&url)
        .form(&params)
        .send()?
        .json::<RemoteSignInResponse>()?;

    Ok(response.token)
}

pub fn sync(host: &str, token: &str) -> Result<()> {
    let client = reqwest::blocking::Client::new();

    let url = format!("{}/items/sync", host);
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    let sync_request = RemoteSyncRequest {
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
        .json::<RemoteSyncResponse>()?;

    Ok(())
}
