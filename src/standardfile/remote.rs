use super::crypto::Crypto;
use super::{RemoteAuthParams, RemoteSignInResponse, RemoteSyncRequest, RemoteSyncResponse};
use anyhow::Result;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use std::collections::HashMap;

/// Sign in and return JWT on success.
pub fn sign_in(host: &str, email: &str, password: &str) -> Result<(String)> {
    let client = reqwest::blocking::Client::new();

    let url = format!("{}/auth/params?email={}", host, email);
    let response = client.get(&url).send()?.json::<RemoteAuthParams>()?;
    let crypto = Crypto::new_from_remote(&response, email, password)?;

    let mut params = HashMap::new();
    let encoded_pw = crypto.password();
    params.insert("email", email);
    params.insert("password", encoded_pw.as_str());

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
