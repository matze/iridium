use anyhow::{anyhow, Result};
use chrono::Utc;
use standardfile::{remote, Cipher, Item, Note, Tag, Credentials, crypto::Crypto};
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
    pub tags: HashMap<Uuid, Tag>,
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

fn filter_encrypted<'a>(items: &'a Vec<Item>, content_type: &str) -> Vec<&'a Item> {
    items
    .iter()
    .filter(|x| x.content_type == content_type)
    .collect::<Vec<&Item>>()
}

impl Storage {
    pub fn new(credentials: &Credentials, client: Option<remote::Client>) -> Result<Self> {
        let mut storage = Self {
            path: data_path_from_identifier(&credentials.identifier)?,
            notes: HashMap::new(),
            tags: HashMap::new(),
            crypto: Crypto::new(&credentials)?,
            current: None,
            dirty: HashSet::new(),
            client: client,
        };

        let mut items: Vec<Item> = Vec::new();

        if storage.path.exists() {
            log::info!("Loading {:?}", storage.path);

            for entry in read_dir(&storage.path)? {
                let file_path = entry?.path();

                if let Some(file_name) = file_path.file_name() {
                    let uuid = Uuid::parse_str(file_name.to_string_lossy().as_ref())?;
                    let contents = read_to_string(file_path)?;
                    let item = Item::from_str(&contents)?;

                    if uuid != item.uuid {
                        return Err(anyhow!("File is corrupted"));
                    }

                    if item.content_type == "Note" {
                        storage.notes.insert(uuid, Note::decrypt(&storage.crypto, &item)?);
                    }
                    else if item.content_type == "Tag" {
                        storage.tags.insert(uuid, Tag::decrypt(&storage.crypto, &item)?);
                    }
                    else {
                        Err(anyhow!("Cannot handle {}", item.content_type))?;
                    }

                    items.push(item);
                }
            }
        }

        if let Some(client) = &mut storage.client {
            log::info!("Syncing with remote");

            // Use all items we haven't synced yet. For now pretend we have never synced an item.
            // Decrypt, flush and show notes we have retrieved from the initial sync.
            let items = client.sync(items)?;
            storage.insert_encrypted_items(&items)?;
        }

        Ok(storage)
    }

    /// Create storage from vector of encrypted items.
    pub fn new_from_items(credentials: &Credentials, items: &Vec<Item>) -> Result<Self> {
        let mut storage = Storage::new(credentials, None)?;
        storage.insert_encrypted_items(items)?;
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

    fn insert_encrypted_items(&mut self, items: &Vec<Item>) -> Result<()> {
        for item in filter_encrypted(&items, "Note").iter().filter(|x| !x.deleted.unwrap_or(false)) {
            self.notes.insert(item.uuid, Note::decrypt(&self.crypto, &item)?);
            self.flush(&item)?;
        }

        for item in filter_encrypted(&items, "Tag").iter().filter(|x| !x.deleted.unwrap_or(false)) {
            self.tags.insert(item.uuid, Tag::decrypt(&self.crypto, &item)?);
            self.flush(&item)?;
        }

        Ok(())
    }

    fn get_uuid(&self) -> Result<Uuid> {
        Ok(self.current.ok_or(anyhow!("No current uuid set"))?)
    }

    fn get_note(&self) -> Result<&Note> {
        Ok(self.notes.get(&self.get_uuid()?).ok_or(anyhow!("uuid mapping not found"))?)
    }

    /// Update the contents of the currently selected item.
    pub fn set_text(&mut self, text: &str) -> Result<()> {
        let uuid = self.get_uuid()?;
        let note = self.notes.get_mut(&uuid).ok_or(anyhow!("uuid mapping not found"))?;
        note.updated_at = Utc::now();
        note.text = text.to_owned();

        self.dirty.insert(note.uuid);
        Ok(())
    }

    /// Get text of the currently selected item.
    pub fn get_text(&self) -> Result<String> {
        Ok(self.get_note()?.text.clone())
    }

    /// Update the title of the currently selected item.
    pub fn set_title(&mut self, title: &str) -> Result<()> {
        let uuid = self.get_uuid()?;
        let note = self.notes.get_mut(&uuid).ok_or(anyhow!("uuid mapping not found"))?;
        note.updated_at = Utc::now();
        note.title = title.to_owned();

        self.dirty.insert(note.uuid);
        Ok(())
    }

    /// Get title of the currently selected item.
    pub fn get_title(&self) -> Result<String> {
        Ok(self.get_note()?.title.clone())
    }

    fn flush_to_disk(&self, uuid: &Uuid, item: &Item) -> Result<()> {
        let path = self.path_from_uuid(&uuid);

        if let Some(parent) = path.parent() {
            if !parent.exists() {
                create_dir_all(&parent)?;
            }
        }

        write(&path, item.to_string()?)?;

        Ok(())
    }

    /// Write encrypted item to disk and sync with remote.
    fn flush(&mut self, item: &Item) -> Result<()> {
        self.flush_to_disk(&item.uuid, &item)?;

        if let Some(client) = &mut self.client {
            log::info!("Syncing {}", item.uuid);

            let copy = Item {
                uuid: item.uuid,
                content: item.content.clone(),
                content_type: item.content_type.clone(),
                enc_item_key: item.enc_item_key.clone(),
                created_at: item.created_at,
                updated_at: item.updated_at,
                deleted: item.deleted,
            };

            client.sync(vec![copy])?;
        }

        Ok(())
    }

    /// Encrypt all dirty items, write them to disk and sync with remote.
    pub fn flush_dirty(&mut self) -> Result<()> {
        let mut items: Vec<Item> = Vec::new();

        for uuid in &self.dirty {
            let note = self.notes.get(uuid).ok_or(anyhow!("uuid dirty but not found"))?;
            let item = Note::encrypt(&self.crypto, note)?;

            self.flush_to_disk(&uuid, &item)?;
            items.push(item);
        }

        if let Some(client) = &mut self.client {
            log::info!("Syncing dirty items");
            client.sync(items)?;
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
                let mut item = Note::encrypt(&self.crypto, note)?;
                item.deleted = Some(true);

                // Apparently, we do not receive the item back as marked deleted
                // but on subsequent syncs only.
                client.sync(vec![item])?;
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
