use super::{RemoteAuthParams, RemoteSignInResponse};
use super::crypto::Crypto;
use std::collections::HashMap;
use anyhow::Result;
use reqwest::blocking;
use serde::Deserialize;

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
    let response = client.post(&url).form(&params).send()?.json::<RemoteSignInResponse>()?;

    Ok(response.token)
}
