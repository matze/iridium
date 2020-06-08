use data_encoding::HEXLOWER;
use directories::BaseDirs;
use ring::digest;
use std::path::PathBuf;

pub struct Storage {
    path: PathBuf,
}

impl Storage {
    pub fn new(email: &str) -> Storage {
        let name = HEXLOWER.encode(digest::digest(&digest::SHA256, &email.as_bytes()).as_ref());
        let dirs = BaseDirs::new().unwrap();
        let mut path = PathBuf::from(dirs.cache_dir());
        path.push("iridium");
        path.push(name);

        Self { path: path }
    }

    pub fn flush(&self) {}
}
