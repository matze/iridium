use anyhow::Result;
use gio::prelude::*;
use gtk::prelude::*;
use std::env;

use crate::config::APP_ID;
use crate::ui::window::Window;

pub struct Application {
    app: gtk::Application,
}

impl Application {
    pub fn new() -> Result<Self> {
        let app = gtk::Application::new(Some(APP_ID), gio::ApplicationFlags::FLAGS_NONE)?;
        let window = Window::new();

        app.connect_activate(clone!(@weak window.widget as window => move |app| {
            window.set_application(Some(app));
            app.add_window(&window);
            window.present();
        }));

        action!(app, "quit", clone!(@strong app => move |_, _| {
            app.quit();
        }));

        Ok(Self { app })
    }

    pub fn run(&self) {
        let args: Vec<String> = env::args().collect();
        self.app.run(&args);
    }
}
