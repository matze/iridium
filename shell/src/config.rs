use anyhow::Result;
use crate::secret;
use directories::BaseDirs;
use serde::{Deserialize, Serialize};
use standardfile::Credentials;
use std::path::PathBuf;
use std::fs;
use std::fs::{create_dir_all, read_to_string};

#[derive(Serialize, Deserialize)]
pub struct Geometry {
    pub width: i32,
    pub height: i32,
    pub x: i32,
    pub y: i32,
    pub maximized: bool,
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub identifier: String,
    pub nonce: String,
    pub cost: u32,
    pub server: Option<String>,
    pub geometry: Option<Geometry>,
}

fn get_path() -> PathBuf {
    let dirs = BaseDirs::new().unwrap();
    let mut path = PathBuf::from(dirs.config_dir());
    path.push("iridium");
    path.push("config.toml");
    path
}

impl Config {
    pub fn new() -> Result<Option<Config>> {
        let path = get_path();

        if path.exists() {
            let contents = read_to_string(path)?;
            Ok(Some(toml::from_str(&contents)?))
        }
        else {
            Ok(None)
        }
    }

    pub fn new_from_credentials(credentials: &Credentials) -> Config {
        Self {
            identifier: credentials.identifier.clone(),
            nonce: credentials.nonce.clone(),
            cost: credentials.cost,
            server: None,
            geometry: None,
        }
    }

    pub fn set_server(&mut self, server: &str) {
        self.server = Some(server.to_string());
    }

    pub fn to_credentials(&self) -> Result<Credentials> {
        Ok(Credentials {
            password: secret::load(&self.identifier, None)?,
            identifier: self.identifier.clone(),
            cost: self.cost,
            nonce: self.nonce.clone(),
        })
    }

    pub fn write(&self) -> Result<()> {
        let path = get_path();

        if !path.exists() {
            create_dir_all(path.parent().unwrap())?;
        }

        fs::write(path, toml::to_string(self)?)?;
        Ok(())
    }
}
