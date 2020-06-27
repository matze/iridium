use anyhow::Result;
use crate::config::Config;
use crate::secret;
use crate::standardfile;
use crate::standardfile::crypto::Crypto;
use chrono::{DateTime, Utc};
use data_encoding::HEXLOWER;
use directories::BaseDirs;
use ring::digest;
use std::collections::HashMap;
use std::fs::{create_dir_all, write, read_dir, read_to_string};
use std::path::PathBuf;
use uuid::Uuid;

pub struct Note {
    pub title: String,
    pub text: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub uuid: Uuid,
}

pub struct Storage {
    path: PathBuf,
    pub notes: HashMap<Uuid, Note>,
    crypto: Option<Crypto>,
}

pub enum Decrypted {
    Note(standardfile::Note),
    None,
}

fn data_path_from_identifier(identifier: &str) -> PathBuf {
    let name = HEXLOWER.encode(digest::digest(&digest::SHA256, identifier.as_bytes()).as_ref());
    let dirs = BaseDirs::new().unwrap();
    let mut path = PathBuf::from(dirs.data_dir());
    path.push("iridium");
    path.push(name);
    path
}

impl Storage {
    pub fn new() -> Storage {
        Self {
            // FIXME: find a better solution, Option<PathBuf> is not ...
            path: PathBuf::from("/tmp"),
            notes: HashMap::new(),
            crypto: None,
        }
    }

    pub fn new_from_config(config: &Config) -> Result<Self> {
        let path = data_path_from_identifier(&config.identifier);

        let credentials = standardfile::Credentials {
            identifier: config.identifier.clone(),
            cost: config.cost,
            nonce: config.nonce.clone(),
            password: secret::load(&config.identifier, None)?,
        };

        let crypto = Crypto::new(&credentials)?;

        let mut storage = Self {
            path: path.clone(),
            notes: HashMap::new(),
            crypto: Some(crypto),
        };

        if path.exists() {
            for entry in read_dir(&path)? {
                let file_path = entry?.path();
                let uuid = Uuid::parse_str(file_path.file_name().unwrap().to_string_lossy().as_ref())?;
                let contents = read_to_string(file_path)?;
                let encrypted_item = serde_json::from_str::<standardfile::Item>(&contents)?;
                assert_eq!(uuid, encrypted_item.uuid);
                storage.decrypt(&encrypted_item);
            }

            storage.read_from_disk(&path)?;
        }
        Ok(storage)
    }

    pub fn reset(&mut self, credentials: &standardfile::Credentials) {
        let path = data_path_from_identifier(&credentials.identifier);
        log::info!("reset path to {:?}", path);
        self.crypto = Some(Crypto::new(&credentials).unwrap());
        self.read_from_disk(&path).unwrap();
        self.path = path;
    }

    /// Decrypt item and add it to the storage.
    pub fn decrypt(&mut self, item: &standardfile::Item) -> Option<Uuid> {
        if let Decrypted::Note(decrypted) = self.crypto.as_ref().unwrap().decrypt(item).unwrap() {
            let note = Note {
                title: decrypted.title.unwrap_or("".to_owned()),
                text: decrypted.text,
                created_at: item.created_at,
                updated_at: item.updated_at,
                uuid: item.uuid,
            };

            self.notes.insert(item.uuid, note);
            Some(item.uuid)
        }
        else {
            None
        }
    }

    /// Encrypt an item and return it.
    pub fn encrypt(&self, uuid: &Uuid) -> Result<standardfile::Item> {
        let note = self.notes.get(&uuid);

        assert!(note.is_some());
        assert!(self.crypto.is_some());

        let note = note.unwrap();
        let crypto = self.crypto.as_ref().unwrap();
        Ok(crypto.encrypt(&note, &uuid)?)
    }

    /// Encrypts item and writes it to disk.
    pub fn flush(&self, uuid: &Uuid) -> Result<()> {
        if let Some(item) = self.notes.get(uuid) {
            let encrypted = self.crypto.as_ref().unwrap().encrypt(item, uuid)?;
            let serialized = serde_json::to_string(&encrypted)?;
            let path = self.path_from_uuid(&uuid);
            self.ensure_path_exists()?;
            write(&path, serialized)?;
        }

        Ok(())
    }

    fn ensure_path_exists(&self) -> Result<()> {
        if !self.path.exists() {
            create_dir_all(&self.path)?;
        }

        Ok(())
    }

    fn path_from_uuid(&self, uuid: &Uuid) -> PathBuf {
        let mut path = PathBuf::from(&self.path);
        path.push(uuid.to_hyphenated().to_string());
        path
    }

    /// Create a new note and return its new uuid.
    pub fn create_note(&mut self) -> Uuid {
        let now = Utc::now();
        let uuid = Uuid::new_v4();

        let note = Note {
            title: "".to_owned(),
            text: "".to_owned(),
            created_at: now,
            updated_at: now,
            uuid: uuid,
        };

        self.notes.insert(uuid, note);

        uuid
    }

    /// Update the contents of a note.
    pub fn update_text(&mut self, uuid: &Uuid, text: &str) {
        if let Some(item) = self.notes.get_mut(uuid) {
            item.updated_at = Utc::now();
            item.text = text.to_owned();
        }

        // Returning an error?
    }

    /// Update the title of a note.
    pub fn update_title(&mut self, uuid: &Uuid, title: &str) {
        if let Some(item) = self.notes.get_mut(uuid) {
            item.updated_at = Utc::now();
            item.title = title.to_owned();
        }

        // Returning an error?
    }

    fn read_from_disk(&mut self, path: &PathBuf) -> Result<()> {
        if !path.exists() {
            return Ok(())
        }

        for entry in read_dir(&path)? {
            let file_path = entry?.path();
            let uuid = Uuid::parse_str(file_path.file_name().unwrap().to_string_lossy().as_ref())?;
            let contents = read_to_string(file_path)?;
            let encrypted_item = serde_json::from_str::<standardfile::Item>(&contents)?;
            assert_eq!(uuid, encrypted_item.uuid);
            self.decrypt(&encrypted_item);
        }

        Ok(())
    }
}
