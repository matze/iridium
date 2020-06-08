use data_encoding::HEXLOWER;
use directories::BaseDirs;
use ring::digest;
use std::collections::HashMap;
use std::fs::{create_dir_all, File};
use std::path::PathBuf;
use std::io::prelude::*;
use uuid::Uuid;

pub struct Storage {
    path: PathBuf,
    notes: HashMap<Uuid, i32>,
}

impl Storage {
    pub fn new(email: &str) -> Storage {
        let name = HEXLOWER.encode(digest::digest(&digest::SHA256, &email.as_bytes()).as_ref());
        let dirs = BaseDirs::new().unwrap();
        let mut path = PathBuf::from(dirs.cache_dir());
        path.push("iridium");
        path.push(name);

        let mut notes = HashMap::new();
        notes.insert(Uuid::new_v4(), 1234);

        Self {
            path: path,
            notes: notes,
        }
    }

    pub fn flush(&self) {
        for (uuid, _) in &self.notes {
            let mut path = PathBuf::from(&self.path);

            if !path.exists() {
                create_dir_all(&path).unwrap();
            }

            path.push(uuid.to_hyphenated().to_string());
            // let mut file = File::create(path).unwrap();
            // file.write_all(s.as_bytes()).unwrap();
        }
    }
}
