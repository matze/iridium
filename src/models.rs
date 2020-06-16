use anyhow::Result;
use crate::config::Config;
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
use secret_service::{EncryptionType, SecretService};

pub struct Note {
    pub title: String,
    pub text: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub uuid: Uuid,
}

pub struct Storage {
    path: Option<PathBuf>,
    pub notes: HashMap<Uuid, Note>,
    crypto: Option<Crypto>,
}

pub enum Decrypted {
    Note(standardfile::Note),
    None,
}

impl Storage {
    pub fn new() -> Storage {
        Self {
            path: None,
            notes: HashMap::new(),
            crypto: None,
        }
    }

    pub fn new_from_config(config: &Config) -> Result<Self> {
        let service = SecretService::new(EncryptionType::Dh).unwrap();
        // TODO: rename email to identifier
        // TODO: select by server as well
        let query = vec![("service", "standardnotes"), ("email", config.identifier.as_str())];
        let items = service.search_items(query).unwrap();
        let item = items.get(0).unwrap();
        let password = String::from_utf8(item.get_secret().unwrap()).unwrap();

        // TODO: refactor with reset
        let name = HEXLOWER
            .encode(digest::digest(&digest::SHA256, &config.identifier.as_bytes()).as_ref());
        let dirs = BaseDirs::new().unwrap();
        let mut path = PathBuf::from(dirs.data_dir());
        path.push("iridium");
        path.push(name);

        let crypto = Crypto::new(
            config.identifier.as_str(),
            config.cost,
            config.nonce.as_str(),
            password.as_str())?;

        let mut storage = Self {
            path: Some(path.clone()),
            notes: HashMap::new(),
            crypto: Some(crypto),
        };

        for entry in read_dir(&path)? {
            let file_path = entry?.path();
            let uuid = Uuid::parse_str(file_path.file_name().unwrap().to_string_lossy().as_ref())?;
            let contents = read_to_string(file_path)?;
            let encrypted_item = serde_json::from_str::<standardfile::Item>(contents.as_str())?;
            assert_eq!(uuid, encrypted_item.uuid);
            storage.decrypt(&encrypted_item);
        }

        Ok(storage)
    }

    pub fn reset(&mut self, auth_params: &standardfile::ExportedAuthParams, password: &str) {
        let name = HEXLOWER
            .encode(digest::digest(&digest::SHA256, &auth_params.identifier.as_bytes()).as_ref());
        let dirs = BaseDirs::new().unwrap();
        let mut path = PathBuf::from(dirs.data_dir());
        path.push("iridium");
        path.push(name);

        self.path = Some(path);
        self.crypto = Some(Crypto::new_from_exported(auth_params, password).unwrap());
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

    /// Encrypts item and writes it to disk.
    pub fn flush(&self, uuid: &Uuid) -> Result<()> {
        if let Some(item) = self.notes.get(uuid) {
            let mut path = PathBuf::from(&self.path.as_ref().unwrap());

            if !path.exists() {
                create_dir_all(&path)?;
            }

            path.push(uuid.to_hyphenated().to_string());

            let encrypted = self.crypto.as_ref().unwrap().encrypt(item, uuid)?;
            let serialized = serde_json::to_string(&encrypted)?;
            write(path, serialized)?;
        }

        Ok(())
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
}
