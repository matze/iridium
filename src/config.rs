use anyhow::Result;
use directories::BaseDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::fs::{create_dir_all, read_to_string, write};

pub static APP_ID: &str = "net.bloerg.Iridium";

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub identifier: String,
}

fn get_path() -> PathBuf {
    let dirs = BaseDirs::new().unwrap();
    let mut path = PathBuf::from(dirs.config_dir());
    path.push("iridium");
    path.push("config.toml");
    path
}

impl Config {
    pub fn new(identifier: &str) -> Config {
        Self {
            identifier: identifier.to_string(),
        }
    }

    pub fn new_from_file() -> Result<Option<Config>> {
        let path = get_path();

        if path.exists() {
            let contents = read_to_string(path)?;
            Ok(Some(toml::from_str(contents.as_str())?))
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
