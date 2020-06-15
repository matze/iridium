use anyhow::Result;
use gio::prelude::*;
use gtk::prelude::*;
use std::env;
use secret_service::{EncryptionType, SecretService};

use crate::config::APP_ID;
use crate::models::Storage;
use crate::standardfile::Exported;
use crate::ui::state::{AppEvent, WindowEvent};
use crate::ui::window::Window;

pub struct Application {
    app: gtk::Application,
}

pub fn get_password(email: &str) -> Result<String> {
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

impl Application {
    pub fn new() -> Result<Self> {
        let app = gtk::Application::new(Some(APP_ID), gio::ApplicationFlags::FLAGS_NONE)?;

        let (sender, receiver) = glib::MainContext::channel::<AppEvent>(glib::PRIORITY_DEFAULT);
        let window = Window::new(sender.clone());
        let mut storage = Storage::new();

        app.connect_activate(
            clone!(@weak window.widget as window => move |app| {
                window.set_application(Some(app));
                app.add_window(&window);
                window.present();
            })
        );

        action!(app, "quit",
            clone!(@strong app => move |_, _| {
                app.quit();
            })
        );

        action!(app, "about",
            clone!(@weak window.widget as window => move |_, _| {
                let builder = gtk::Builder::new_from_resource("/net/bloerg/Iridium/data/resources/ui/about.ui");
                let dialog = builder.get_object::<gtk::AboutDialog>("about-dialog").unwrap();
                dialog.set_transient_for(Some(&window));
                dialog.connect_response(|dialog, _| dialog.destroy());
                dialog.show();
            })
        );

        action!(app, "add",
            clone!(@strong sender as sender => move |_, _| {
                sender.send(AppEvent::AddNote).unwrap();
            })
        );

        action!(app, "search",
            clone!(@strong window.sender as sender => move |_, _| {
                sender.send(WindowEvent::ToggleSearchBar).unwrap();
            })
        );

        action!(app, "import",
            clone!(@weak window.widget as window, @strong sender as sender => move |_, _| {
                let dialog = gtk::FileChooserDialog::with_buttons(
                    Some("Import notes"),
                    Some(&window),
                    gtk::FileChooserAction::Open,
                    &[("_Cancel", gtk::ResponseType::Cancel), ("_Open", gtk::ResponseType::Accept)]);

                dialog.set_transient_for(Some(&window));

                match dialog.run() {
                    gtk::ResponseType::Accept => {
                        if let Some(filename) = dialog.get_filename() {
                            sender.send(AppEvent::Import(filename)).unwrap();
                        }
                    },
                    _ => {}
                }

                dialog.destroy();
            })
        );

        action!(app, "setup",
            clone!(@weak window.widget as window => move |_, _| {
                let builder = gtk::Builder::new_from_resource("/net/bloerg/Iridium/data/resources/ui/setup.ui");
                let dialog = builder.get_object::<gtk::Dialog>("setup-dialog").unwrap();
                let server_box = builder.get_object::<gtk::ComboBoxText>("setup-server").unwrap();
                let server_entry = server_box.get_child().unwrap().downcast::<gtk::Entry>().unwrap();
                let sync_button = builder.get_object::<gtk::Switch>("setup-sync").unwrap();

                server_entry.set_input_purpose(gtk::InputPurpose::Url);
                sync_button.bind_property("active", &server_entry, "sensitive").flags(glib::BindingFlags::SYNC_CREATE).build();

                dialog.set_transient_for(Some(&window));
                dialog.connect_response(|dialog, _| dialog.destroy());
                dialog.show();
            })
        );

        app.set_accels_for_action("app.quit", &["<primary>q"]);
        app.set_accels_for_action("app.search", &["<primary>f"]);

        receiver.attach(None,
            clone!(@strong window.sender as sender => move |event| {
                match event {
                    AppEvent::Import(filename) => {
                        let contents = std::fs::read_to_string(filename).unwrap();
                        let exported = serde_json::from_str::<Exported>(&contents).unwrap();
                        let email = exported.auth_params.identifier.as_str();
                        let pass = get_password(email).unwrap();

                        storage.reset(&exported.auth_params, pass.as_str());

                        for note in exported.encrypted_notes() {
                            if let Some(uuid) = storage.decrypt(note) {
                                if let Some(note) = storage.notes.get(&uuid) {
                                    sender.send(WindowEvent::AddNote(uuid, note.title.clone())).unwrap();
                                }
                            }
                        }
                    },
                    AppEvent::AddNote => {
                        let uuid = storage.create_note();
                        let note = storage.notes.get(&uuid).unwrap();
                        sender.send(WindowEvent::AddNote(uuid, note.title.clone())).unwrap();
                    },
                    AppEvent::SelectNote(uuid) => {
                        if let Some(item) = storage.notes.get(&uuid) {
                            window.load_note(item.title.as_str(), item.text.as_str());
                        }
                    },
                    AppEvent::UpdateTitle(uuid, text) => {
                        storage.update_title(&uuid, text.as_str());
                    },
                    AppEvent::UpdateText(uuid, text) => {
                        storage.update_text(&uuid, text.as_str());
                    },
                }

                glib::Continue(true)
            })
        );

        Ok(Self { app })
    }

    pub fn run(&self) {
        let args: Vec<String> = env::args().collect();
        self.app.run(&args);
    }
}
