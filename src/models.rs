use crate::standardfile;
use crate::standardfile::crypto::Crypto;
use chrono::{DateTime, Utc};
use data_encoding::HEXLOWER;
use directories::BaseDirs;
use ring::digest;
use std::collections::HashMap;
use std::fs::{create_dir_all, File};
use std::io::prelude::*;
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

    pub fn reset(&mut self, auth_params: &standardfile::AuthParams, password: &str) {
        let name = HEXLOWER
            .encode(digest::digest(&digest::SHA256, &auth_params.identifier.as_bytes()).as_ref());
        let dirs = BaseDirs::new().unwrap();
        let mut path = PathBuf::from(dirs.cache_dir());
        path.push("iridium");
        path.push(name);

        self.path = Some(path);
        self.crypto = Some(Crypto::new(auth_params, password).unwrap());
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
    pub fn flush(&self, uuid: &Uuid) {
        if let Some(item) = self.notes.get(uuid) {
            let mut path = PathBuf::from(&self.path.as_ref().unwrap());

            if !path.exists() {
                create_dir_all(&path).unwrap();
            }

            path.push(uuid.to_hyphenated().to_string());

            let encrypted = self.crypto.as_ref().unwrap().encrypt(item, uuid).unwrap();
            let serialized = serde_json::to_string(&encrypted).unwrap();
            let mut file = File::create(path).unwrap();
            file.write_all(serialized.as_ref()).unwrap();
        }
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
