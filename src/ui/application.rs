use anyhow::Result;
use gio::prelude::*;
use gtk::prelude::*;
use std::env;
use uuid::Uuid;

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
        let sender_ = sender.clone();
        let window = Window::new(sender, &notes);

        app.connect_activate(clone!(@weak window.widget as window => move |app| {
            window.set_application(Some(app));
            app.add_window(&window);
            window.present();
        }));

        action!(app, "quit", clone!(@strong app => move |_, _| {
            app.quit();
        }));

        action!(app, "about", clone!(@weak window.widget as window => move |_, _| {
            let builder = gtk::Builder::new_from_resource("/net/bloerg/Iridium/data/resources/ui/about.ui");
            let dialog = builder.get_object::<gtk::AboutDialog>("about-dialog").unwrap();
            dialog.set_transient_for(Some(&window));
            dialog.connect_response(|dialog, _| dialog.destroy());
            dialog.show();
        }));

        action!(app, "search", move |_, _| {
            sender_.send(UiEvent::ToggleSearchBar).unwrap();
        });

        app.set_accels_for_action("app.quit", &["<primary>q"]);
        app.set_accels_for_action("app.search", &["<primary>f"]);

        receiver.attach(None, move |event| {
            match event {
                UiEvent::NoteSelected(uuid) => {
                    let uuid = Uuid::parse_str(uuid.as_str()).unwrap();
                    let item = notes.iter().filter(|&x| x.item.uuid == uuid).collect::<Vec<_>>()[0];
                    window.load_note(item.note.title.as_deref().unwrap_or(""), item.note.text.as_str());
                },
                UiEvent::ToggleSearchBar => {
                    window.toggle_search_bar();
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
