#[macro_use]
extern crate glib;

mod config;
mod standardfile;
mod ui;

use gio::{resources_register, Resource};
use glib::Bytes;
use anyhow::{Context, Result};
use standardfile::Root;
use ui::application::Application;

fn init_resources() -> Result<()> {
    let data: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/resources.gresource"));
    let gbytes = Bytes::from_static(data.as_ref());
    let resource = Resource::new_from_data(&gbytes)?;
    resources_register(&resource);

    Ok(())
}

fn main() -> Result<()> {
    let filename = "test.json";
    let contents = std::fs::read_to_string(filename)
        .with_context(|| format!("Could not open {}.", filename))?;

    let root: Root = serde_json::from_str(&contents)?;
    let pass = std::env::var("SF_PASS").unwrap();

    for item in root.notes(&pass)? {
        println!("{}", item.item.updated_at);
        match item.note.title {
            None => println!("n/a"),
            Some(x) => println!("{}", x),
        }
    }

    init_resources()?;
    let app = Application::new()?;
    app.run();

    Ok(())
}
