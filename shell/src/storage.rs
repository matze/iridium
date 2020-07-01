use anyhow::{anyhow, Result};
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
    crypto: Crypto,
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
    pub fn new(credentials: &standardfile::Credentials) -> Result<Self> {
        let mut storage = Self {
            path: data_path_from_identifier(&credentials.identifier),
            notes: HashMap::new(),
            crypto: Crypto::new(&credentials)?,
        };

        if storage.path.exists() {
            for entry in read_dir(&storage.path)? {
                let file_path = entry?.path();

                if let Some(file_name) = file_path.file_name() {
                    let uuid = Uuid::parse_str(file_name.to_string_lossy().as_ref())?;
                    let contents = read_to_string(file_path)?;
                    let encrypted_item = standardfile::Item::from_str(&contents)?;

                    if uuid != encrypted_item.uuid {
                        return Err(anyhow!("File is corrupted"));
                    }

                    storage.decrypt(&encrypted_item)?;
                }
            }
        }

        Ok(storage)
    }

    /// Decrypt item and add it to the storage.
    pub fn decrypt(&mut self, item: &standardfile::Item) -> Result<Uuid> {
        let note = self.crypto.decrypt(item)?;
        self.notes.insert(item.uuid, note);
        Ok(item.uuid)
    }

    /// Encrypt an item and return it.
    pub fn encrypt(&self, uuid: &Uuid) -> Result<standardfile::Item> {
        if let Some(note) = self.notes.get(&uuid) {
            Ok(self.crypto.encrypt(&note, &uuid)?)
        }
        else {
            Err(anyhow!("Note {} does not exist", uuid))
        }
    }

    /// Encrypts item and writes it to disk.
    pub fn flush(&self, uuid: &Uuid) -> Result<()> {
        if let Some(item) = self.notes.get(uuid) {
            let encrypted = self.crypto.encrypt(item, uuid)?;
            let path = self.path_from_uuid(&uuid);

            if !path.exists() {
                create_dir_all(&path)?;
            }

            write(&path, encrypted.to_string()?)?;
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
}
