use anyhow::Result;
use directories::BaseDirs;
use serde::{Deserialize, Serialize};
use crate::standardfile::ExportedAuthParams;
use std::path::PathBuf;
use std::fs::{create_dir_all, read_to_string, write};

pub static APP_ID: &str = "net.bloerg.Iridium";
pub static APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub identifier: String,
    pub nonce: String,
    pub cost: u32,
}

fn get_path() -> PathBuf {
    let dirs = BaseDirs::new().unwrap();
    let mut path = PathBuf::from(dirs.config_dir());
    path.push("iridium");
    path.push("config.toml");
    path
}

impl Config {
    pub fn new(params: &ExportedAuthParams) -> Config {
        Self {
            identifier: params.identifier.clone(),
            nonce: params.pw_nonce.clone(),
            cost: params.pw_cost,
        }
    }

    pub fn new_from_file() -> Result<Option<Config>> {
        let path = get_path();

        if path.exists() {
            let contents = read_to_string(path)?;
            Ok(Some(toml::from_str(&contents)?))
        }
        else {
            Ok(None)
        }
    }

    pub fn write(&self) -> Result<()> {
        let path = get_path();

        if !path.exists() {
            create_dir_all(path.parent().unwrap())?;
        }

        write(path, toml::to_string(self)?)?;

        Ok(())
    }
}
