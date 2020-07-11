use anyhow::{anyhow, Result};
use chrono::Utc;
use standardfile::{encrypted_notes, remote, Item, Note, Credentials};
use standardfile::crypto::Crypto;
use data_encoding::HEXLOWER;
use directories::BaseDirs;
use ring::digest;
use std::collections::{HashSet, HashMap};
use std::fs::{create_dir_all, write, read_dir, read_to_string, remove_file};
use std::path::PathBuf;
use uuid::Uuid;

pub struct Storage {
    path: PathBuf,
    pub notes: HashMap<Uuid, Note>,
    crypto: Crypto,
    pub current: Option<Uuid>,

    /// Contains uuids of notes that have not been flushed yet
    dirty: HashSet<Uuid>,

    /// The storage automatically syncs with the client if it exists.
    pub client: Option<remote::Client>,
}

fn data_path_from_identifier(identifier: &str) -> Result<PathBuf> {
    let name = HEXLOWER.encode(digest::digest(&digest::SHA256, identifier.as_bytes()).as_ref());

    if let Some(dirs) = BaseDirs::new() {
        let mut path = PathBuf::from(dirs.data_dir());
        path.push("iridium");
        path.push(name);
        Ok(path)
    }
    else {
        Err(anyhow!("Could not determine XDG data dir"))
    }
}

impl Storage {
    pub fn new(credentials: &Credentials, client: Option<remote::Client>) -> Result<Self> {
        let mut storage = Self {
            path: data_path_from_identifier(&credentials.identifier)?,
            notes: HashMap::new(),
            crypto: Crypto::new(&credentials)?,
            current: None,
            dirty: HashSet::new(),
            client: client,
        };

        let mut encrypted_items: Vec<Item> = Vec::new();

        if storage.path.exists() {
            log::info!("Loading {:?}", storage.path);

            for entry in read_dir(&storage.path)? {
                let file_path = entry?.path();

                if let Some(file_name) = file_path.file_name() {
                    let uuid = Uuid::parse_str(file_name.to_string_lossy().as_ref())?;
                    let contents = read_to_string(file_path)?;
                    let encrypted_item = Item::from_str(&contents)?;

                    if uuid != encrypted_item.uuid {
                        return Err(anyhow!("File is corrupted"));
                    }

                    storage.decrypt_and_add(&encrypted_item)?;
                    encrypted_items.push(encrypted_item);
                }
            }
        }

        if let Some(client) = &mut storage.client {
            log::info!("Syncing with remote");

            // Use all items we haven't synced yet. For now pretend we have never synced an item.
            // Decrypt, flush and show notes we have retrieved from the initial sync.
            let items = client.sync(encrypted_items)?;

            for item in encrypted_notes(&items) {
                if !item.deleted.unwrap_or(false) {
                    let uuid = storage.decrypt_and_add(&item)?;
                    storage.flush(&uuid)?;
                }
            }
        }

        Ok(storage)
    }

    /// Create storage from vector of encrypted items.
    pub fn new_from_items(credentials: &Credentials, items: &Vec<Item>) -> Result<Self> {
        let mut storage = Storage::new(credentials, None)?;

        for item in encrypted_notes(items) {
            let uuid = storage.decrypt_and_add(&item)?;
            storage.flush(&uuid)?;
        }

        Ok(storage)
    }

    /// Set the currently note to update.
    pub fn set_current_uuid(&mut self, uuid: &Uuid) -> Result<()> {
        if !self.notes.contains_key(&uuid) {
            return Err(anyhow!(format!("{} does not exist", uuid)));
        }

        self.current = Some(*uuid);
        Ok(())
    }

    /// Update the contents of the currently selected item.
    pub fn set_text(&mut self, text: &str) {
        let uuid = &self.current.unwrap();

        if let Some(item) = self.notes.get_mut(uuid) {
            item.updated_at = Utc::now();
            item.text = text.to_owned();
        }

        self.dirty.insert(*uuid);

        // Returning an error?
    }

    /// Get text of the currently selected item.
    pub fn get_text(&self) -> String {
        // FIXME: for obvious reasons
        self.notes.get(&self.current.unwrap()).unwrap().text.clone()
    }

    /// Update the title of the currently selected item.
    pub fn set_title(&mut self, title: &str) {
        let uuid = &self.current.unwrap();

        if let Some(item) = self.notes.get_mut(uuid) {
            item.updated_at = Utc::now();
            item.title = title.to_owned();
        }

        self.dirty.insert(*uuid);

        // Returning an error?
    }

    /// Get title of the currently selected item.
    pub fn get_title(&self) -> String {
        // FIXME: for obvious reasons
        self.notes.get(&self.current.unwrap()).unwrap().title.clone()
    }

    /// Decrypt item and add it to the storage.
    pub fn decrypt_and_add(&mut self, item: &Item) -> Result<Uuid> {
        let note = self.crypto.decrypt(item)?;
        self.notes.insert(item.uuid, note);
        Ok(item.uuid)
    }

    fn flush_to_disk(&self, uuid: &Uuid, encrypted: &Item) -> Result<()> {
        let path = self.path_from_uuid(&uuid);

        if let Some(parent) = path.parent() {
            if !parent.exists() {
                create_dir_all(&parent)?;
            }
        }

        write(&path, encrypted.to_string()?)?;

        Ok(())
    }

    /// Encrypt single item, write it to disk and sync with remote.
    pub fn flush(&mut self, uuid: &Uuid) -> Result<()> {
        let item = self.notes.get(uuid).ok_or(anyhow!("uuid does not exist"))?;
        let encrypted = self.crypto.encrypt(item, uuid)?;

        self.flush_to_disk(&uuid, &encrypted)?;

        if let Some(client) = &mut self.client {
            log::info!("Syncing {}", uuid);
            client.sync(vec![encrypted])?;
        }

        Ok(())
    }

    /// Encrypt all dirty items, write them to disk and sync with remote.
    pub fn flush_dirty(&mut self) -> Result<()> {
        let mut encrypted_items: Vec<Item> = Vec::new();

        for uuid in &self.dirty {
            let item = self.notes.get(uuid).ok_or(anyhow!("uuid dirty but not found"))?;
            let encrypted = self.crypto.encrypt(item, uuid)?;

            self.flush_to_disk(&uuid, &encrypted)?;
            encrypted_items.push(encrypted);
        }

        if let Some(client) = &mut self.client {
            log::info!("Syncing dirty items");
            client.sync(encrypted_items)?;
        }

        self.dirty.clear();

        Ok(())
    }

    /// Delete note from storage.
    pub fn delete(&mut self, uuid: &Uuid) -> Result<()> {
        if self.dirty.contains(uuid) {
            self.dirty.remove(&uuid);
        }

        if let Some(client) = &mut self.client {
            if let Some(note) = self.notes.get(&uuid) {
                let mut encrypted = self.crypto.encrypt(&note, &uuid)?;
                encrypted.deleted = Some(true);

                // Apparently, we do not receive the item back as marked deleted
                // but on subsequent syncs only.
                client.sync(vec![encrypted])?;
            }
        }

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
}
