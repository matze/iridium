use anyhow::{anyhow, Result};
use crate::secret;
use directories::BaseDirs;
use serde::{Deserialize, Serialize};
use standardfile::Credentials;
use std::path::PathBuf;
use std::fs;
use std::collections::HashMap;
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
struct Root {
    pub identifier: String,
    pub nonce: String,
    pub cost: u32,
    pub server: Option<String>,
    pub geometry: Option<Geometry>,
}

struct Identity {
    pub identifier: String,
    pub nonce: String,
    pub cost: u32,
    pub server: Option<String>,
}

pub struct Config {
    pub identifier: Option<String>,
    identities: HashMap<String, Identity>,
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
    /// Create a new Config and load from filesystem if possible.
    pub fn new() -> Result<Self> {
        let path = get_path();

        if path.exists() {
            let contents = read_to_string(path)?;
            let root: Root = toml::from_str(&contents)?;

            let identity = Identity {
                identifier: root.identifier.clone(),
                nonce: root.nonce,
                cost: root.cost,
                server: root.server,
            };

            let mut config = Self {
                identifier: Some(root.identifier.clone()),
                identities: HashMap::new(),
                geometry: root.geometry,
            };

            config.identities.insert(root.identifier, identity);
            Ok(config)
        }
        else {
            Ok(Self {
                identifier: None,
                identities: HashMap::new(),
                geometry: None,
            })
        }
    }

    /// Add a new identity and switch to it.
    fn add_identity(&mut self, identity: Identity) {
        self.identifier = Some(identity.identifier.clone());
        self.identities.insert(identity.identifier.clone(), identity);
    }

    /// Add a new identity from credentials and switch to it.
    pub fn add(&mut self, credentials: &Credentials, server: Option<String>) {
        let identity = Identity {
            identifier: credentials.identifier.clone(),
            nonce: credentials.nonce.clone(),
            cost: credentials.cost,
            server: server,
        };

        self.add_identity(identity);
    }

    /// Return credentials for current identity.
    pub fn credentials(&self) -> Result<Credentials> {
        let identifier = self.identifier.as_ref().unwrap();

        if let Some(identity) = self.identities.get(identifier) {
            Ok(Credentials {
                password: secret::load(&identity.identifier, &None)?,
                identifier: identity.identifier.clone(),
                cost: identity.cost,
                nonce: identity.nonce.clone(),
            })
        }
        else {
            Err(anyhow!("Current identity not found"))
        }
    }

    /// Get server for current identity.
    pub fn server(&self) -> Option<String> {
        let identifier = self.identifier.as_ref().unwrap();

        if let Some(identity) = self.identities.get(identifier) {
            // FIXME: this looks very suspicious
            Some(identity.server.as_ref().unwrap().clone())
        }
        else {
            None
        }
    }

    /// Write configuration to disk.
    pub fn write(&self) -> Result<()> {
        let identifier = self.identifier.as_ref().unwrap();

        if let Some(identity) = self.identities.get(identifier) {
            let path = get_path();

            if !path.exists() {
                create_dir_all(path.parent().unwrap())?;
            }

            let geometry = match &self.geometry {
                Some(geometry) => Some(Geometry {
                    width: geometry.width,
                    height: geometry.height,
                    x: geometry.x,
                    y: geometry.y,
                    maximized: geometry.maximized,
                }),
                None => None,
            };

            let root = Root {
                identifier: identifier.clone(),
                nonce: identity.nonce.clone(),
                cost: identity.cost,
                server: identity.server.clone(),
                geometry: geometry,
            };

            fs::write(path, toml::to_string(&root)?)?;
        }
        else {
            // FIXME: return an error
        }
        Ok(())
    }
}
