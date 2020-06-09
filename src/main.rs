#[macro_use]
extern crate glib;
extern crate secret_service;

mod config;
mod models;
mod standardfile;
mod ui;

use anyhow::{Context, Result};
use gio::{resources_register, Resource};
use glib::Bytes;
use models::Storage;
use secret_service::{EncryptionType, SecretService};
use standardfile::Exported;
use ui::application::Application;

fn init_resources() -> Result<()> {
    let data: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/resources.gresource"));
    let gbytes = Bytes::from_static(data.as_ref());
    let resource = Resource::new_from_data(&gbytes)?;
    resources_register(&resource);

    Ok(())
}

fn get_password(email: &str) -> Result<String> {
    let service = SecretService::new(EncryptionType::Dh).unwrap();

    let items = service
        .search_items(vec![
            ("service", "standardnotes"),
            ("email", email),
            ("server", "https://app.standardnotes.org"),
        ])
        .unwrap();

    let item = items.get(0).unwrap();
    let pass = item.get_secret().unwrap();

    Ok(String::from_utf8(pass)?)
}

fn main() -> Result<()> {
    let filename = "test.json";
    let contents = std::fs::read_to_string(filename)
        .with_context(|| format!("Could not open {}.", filename))?;

    let root = serde_json::from_str::<Exported>(&contents)?;
    let email = &std::env::var("SF_EMAIL")?;
    let pass = get_password(email)?;
    let notes = root.notes(&pass)?;
    let mut storage = Storage::new(email);

    let uuid = storage.create_note();
    storage.update_title(&uuid, "foo");
    storage.update_text(&uuid, "# Header\n\nText");

    // init_resources()?;
    // let app = Application::new(notes?)?;
    // app.run();

    Ok(())
}
