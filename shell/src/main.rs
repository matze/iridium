#![forbid(unsafe_code)]

#[macro_use]
extern crate glib;
extern crate secret_service;

mod config;
mod consts;
mod secret;
mod storage;
mod ui;

use anyhow::Result;
use gio::{resources_register, Resource};
use glib::Bytes;
use ui::application::Application;

fn init_resources() -> Result<()> {
    let data: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/resources.gresource"));
    let gbytes = Bytes::from_static(data.as_ref());
    let resource = Resource::from_data(&gbytes)?;
    resources_register(&resource);

    Ok(())
}

fn main() -> Result<()> {
    env_logger::init();
    gtk::init()?;
    init_resources()?;
    let app = Application::new()?;
    app.run();

    Ok(())
}
