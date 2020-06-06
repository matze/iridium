use anyhow::Result;
use gio::prelude::*;
use gtk::prelude::*;
use std::env;

use crate::config::APP_ID;
use crate::ui::window::Window;
use crate::ui::state::UiEvent;
use crate::standardfile::NoteItem;

pub struct Application {
    app: gtk::Application,
}

impl Application {
    pub fn new(notes: Vec<NoteItem>) -> Result<Self> {
        let app = gtk::Application::new(Some(APP_ID), gio::ApplicationFlags::FLAGS_NONE)?;

        let (sender, receiver) = glib::MainContext::channel::<UiEvent>(glib::PRIORITY_DEFAULT);
        let window = Window::new(sender, notes);

        app.connect_activate(clone!(@weak window.widget as window => move |app| {
            window.set_application(Some(app));
            app.add_window(&window);
            window.present();
        }));

        action!(app, "quit", clone!(@strong app => move |_, _| {
            app.quit();
        }));

        receiver.attach(None, move |event| {
            match event {
                UiEvent::NoteSelected(uuid) => {
                    window.load_note(uuid.as_str());
                }
            }

            glib::Continue(true)
        });

        Ok(Self { app })
    }

    pub fn run(&self) {
        let args: Vec<String> = env::args().collect();
        self.app.run(&args);
    }
}
