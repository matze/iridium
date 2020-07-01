use anyhow::{anyhow, Result};
use crate::config::Config;
use crate::secret;
use standardfile::Note;
use standardfile::crypto::Crypto;
use chrono::Utc;
use data_encoding::HEXLOWER;
use directories::BaseDirs;
use ring::digest;
use std::collections::HashMap;
use std::fs::{create_dir_all, write, read_dir, read_to_string, remove_file};
use std::path::PathBuf;
use uuid::Uuid;

pub struct Storage {
    path: PathBuf,
    pub notes: HashMap<Uuid, Note>,
    crypto: Option<Crypto>,
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
                let encrypted_item = standardfile::Item::from_str(&contents)?;
                assert_eq!(uuid, encrypted_item.uuid);
                storage.decrypt(&encrypted_item);
            }

            storage.read_from_disk(&path)?;
        }
        Ok(storage)
    }

    pub fn reset(&mut self, credentials: &standardfile::Credentials) -> Result<()> {
        let path = data_path_from_identifier(&credentials.identifier);
        log::info!("reset path to {:?}", path);
        self.crypto = Some(Crypto::new(&credentials)?);
        self.read_from_disk(&path)?;
        self.path = path;
        Ok(())
    }

    /// Decrypt item and add it to the storage.
    pub fn decrypt(&mut self, item: &standardfile::Item) -> Option<Uuid> {
        let note = self.crypto.as_ref().unwrap().decrypt(item).unwrap();
        self.notes.insert(item.uuid, note);
        Some(item.uuid)
    }

    /// Encrypt an item and return it.
    pub fn encrypt(&self, uuid: &Uuid) -> Result<standardfile::Item> {
        if let Some(note) = self.notes.get(&uuid) {
            assert!(self.crypto.is_some());

            let crypto = self.crypto.as_ref().unwrap();
            Ok(crypto.encrypt(&note, &uuid)?)
        }
        else {
            Err(anyhow!("Note {} does not exist", uuid))
        }
    }

    /// Encrypts item and writes it to disk.
    pub fn flush(&self, uuid: &Uuid) -> Result<()> {
        if let Some(item) = self.notes.get(uuid) {
            let encrypted = self.crypto.as_ref().unwrap().encrypt(item, uuid)?;
            let path = self.path_from_uuid(&uuid);
            self.ensure_path_exists()?;
            write(&path, encrypted.to_string()?)?;
        }

        Ok(())
    }

    fn ensure_path_exists(&self) -> Result<()> {
        if !self.path.exists() {
            create_dir_all(&self.path)?;
        }

        Ok(())
    }

    /// Delete note from storage.
    pub fn delete(&mut self, uuid: &Uuid) -> Result<()> {
        let path = self.path_from_uuid(&uuid);
        log::info!("Deleting {:?}", path);
        remove_file(path)?;
        self.notes.remove(&uuid);
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
            let encrypted_item = standardfile::Item::from_str(&contents)?;
            assert_eq!(uuid, encrypted_item.uuid);
            self.decrypt(&encrypted_item);
        }

        Ok(())
    }
}
