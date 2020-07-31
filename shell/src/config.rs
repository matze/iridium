use anyhow::{anyhow, Result};
use crate::secret;
use directories::BaseDirs;
use serde::{Deserialize, Serialize};
use standardfile::Credentials;
use std::path::PathBuf;
use std::fs;
use std::collections::HashMap;
use std::fs::{create_dir_all, read_to_string};

#[derive(Serialize, Deserialize, Debug)]
pub struct Geometry {
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub maximized: bool,
}

#[derive(Serialize, Deserialize, Clone)]
struct Identity {
    pub identifier: String,
    pub nonce: String,
    pub cost: u32,
    pub server: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Root {
    pub current: String,
    pub identities: Vec<Identity>,
    pub geometry: Option<Geometry>,
}

pub struct Config {
    identifier: Option<String>,
    identities: HashMap<String, Identity>,
    pub geometry: Option<Geometry>,
}

fn get_path() -> Result<PathBuf> {
    let dirs = BaseDirs::new().ok_or(anyhow!("Could not get XDG config dir"))?;
    let mut path = PathBuf::from(dirs.config_dir());
    path.push("iridium");
    path.push("config.toml");
    Ok(path)
}

impl Config {
    /// Create a new Config and load from filesystem if possible.
    pub fn new() -> Result<Self> {
        let path = get_path()?;

        if path.exists() {
            let contents = read_to_string(path)?;
            let root: Root = toml::from_str(&contents)?;

            let mut config = Self {
                identifier: Some(root.current.clone()),
                identities: HashMap::new(),
                geometry: root.geometry,
            };

            for identity in root.identities {
                config.identities.insert(identity.identifier.clone(), identity);
            }

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

    /// Switch identities and return an error if it does not exist.
    pub fn switch(&mut self, identifier: &str) -> Result<()> {
        if !self.identities.contains_key(identifier) {
            Err(anyhow!("Identifier does not exist"))
        }
        else {
            self.identifier = Some(identifier.to_string());
            Ok(())
        }
    }

    /// Return credentials for current identity.
    pub fn credentials(&self) -> Result<Credentials> {
        let identifier = self.identifier.as_ref().ok_or(anyhow!("No identifier set"))?;
        let identity = self.identities.get(identifier).ok_or(anyhow!("No identity found for current identifier"))?;

        Ok(Credentials {
            password: secret::load(&identity.identifier, &None)?,
            identifier: identity.identifier.clone(),
            cost: identity.cost,
            nonce: identity.nonce.clone(),
        })
    }

    /// Get server for current identity.
    pub fn server(&self) -> Option<String> {
        let identifier = self.identifier.as_ref().unwrap();

        self.identities
            .get(identifier)
            .map_or(None, |identity| identity.server.as_ref())
            .map_or(None, |server| Some(server.clone()))
    }

    /// Get existing identifiers.
    pub fn identifiers(&self) -> Vec<String> {
        self.identities.keys().map(|s| s.clone()).collect()
    }

    pub fn identifier(&self) -> Option<&String> {
        self.identifier.as_ref()
    }

    /// Write configuration to disk.
    pub fn write(&self) -> Result<()> {
        let identifier = self.identifier.as_ref().ok_or(anyhow!("No identifier set"))?;
        let identity = self.identities.get(identifier).ok_or(anyhow!("No identity found for current identifier"))?;
        let path = get_path()?;

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

        let identities = self.identities
            .values()
            .map(|identity| identity.clone())
            .collect();

        let root = Root {
            current: identity.identifier.clone(),
            identities: identities,
            geometry: geometry,
        };

        fs::write(path, toml::to_string(&root)?)?;
        Ok(())
    }
}
