use anyhow::Result;
use gio::prelude::*;
use gtk::prelude::*;
use std::env;
use crate::config::{APP_ID, APP_VERSION, Config};
use crate::secret;
use crate::storage::Storage;
use crate::standardfile::{crypto, Exported};
use crate::ui::state::{AppEvent, WindowEvent};
use crate::ui::window::Window;

pub struct Application {
    app: gtk::Application,
}

impl Application {
    pub fn new() -> Result<Self> {
        let app = gtk::Application::new(Some(APP_ID), gio::ApplicationFlags::FLAGS_NONE)?;

        let (sender, receiver) = glib::MainContext::channel::<AppEvent>(glib::PRIORITY_DEFAULT);
        let window = Window::new(sender.clone());

        let config = Config::new_from_file()?;

        let mut storage = match config {
            Some(config) => {
                window.sender.send(WindowEvent::ShowMainContent).unwrap();
                Storage::new_from_config(&config)?
            },
            None => { Storage::new() },
        };

        for (uuid, note) in &storage.notes {
            window.sender.send(WindowEvent::AddNote(*uuid, note.title.clone())).unwrap();
        }

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
                dialog.set_version(Some(APP_VERSION));
                dialog.set_logo_icon_name(Some(APP_ID));
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
                let builder = gtk::Builder::new_from_resource("/net/bloerg/Iridium/data/resources/ui/import.ui");
                let dialog = builder.get_object::<gtk::Dialog>("import-dialog").unwrap();
                let server_box = builder.get_object::<gtk::ComboBoxText>("import-server").unwrap();
                let server_entry = server_box.get_child().unwrap().downcast::<gtk::Entry>().unwrap();
                let sync_button = builder.get_object::<gtk::Switch>("import-sync").unwrap();

                server_entry.set_input_purpose(gtk::InputPurpose::Url);
                server_entry.set_icon_from_icon_name(gtk::EntryIconPosition::Primary, Some("network-server-symbolic"));
                server_entry.set_placeholder_text(Some("Server address"));
                sync_button.bind_property("active", &server_box, "sensitive").flags(glib::BindingFlags::SYNC_CREATE).build();
                sync_button.bind_property("active", &server_entry, "sensitive").flags(glib::BindingFlags::SYNC_CREATE).build();

                dialog.set_transient_for(Some(&window));
                dialog.set_modal(true);

                match dialog.run() {
                    gtk::ResponseType::Ok => {
                        let file_chooser = builder.get_object::<gtk::FileChooserButton>("import-file-button").unwrap();

                        if let Some(filename) = file_chooser.get_filename() {
                            let password_entry = builder.get_object::<gtk::Entry>("import-password").unwrap();

                            if let Some(password) = password_entry.get_text() {
                                sender.send(AppEvent::Import(filename, password.as_str().to_string())).unwrap();
                            }
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
                server_entry.set_icon_from_icon_name(gtk::EntryIconPosition::Primary, Some("network-server-symbolic"));
                server_entry.set_placeholder_text(Some("Server address"));
                sync_button.bind_property("active", &server_box, "sensitive").flags(glib::BindingFlags::SYNC_CREATE).build();
                sync_button.bind_property("active", &server_entry, "sensitive").flags(glib::BindingFlags::SYNC_CREATE).build();

                dialog.set_transient_for(Some(&window));
                dialog.set_modal(true);
                dialog.connect_response(|dialog, _| dialog.destroy());
                dialog.show();
            })
        );

        app.set_accels_for_action("app.quit", &["<primary>q"]);
        app.set_accels_for_action("app.search", &["<primary>f"]);

        receiver.attach(None,
            clone!(@strong window.sender as sender => move |event| {
                match event {
                    AppEvent::CreateStorage(identifier, password, _) => {
                        let nonce = crypto::make_nonce();
                        let cost = 110000;

                        storage.reset(&identifier, cost, &nonce, &password);

                        let config = Config::new(&identifier, cost, &nonce);
                        config.write().unwrap();

                        secret::store(&identifier, &password);
                    }
                    AppEvent::Import(path, password) => {
                        let filename = path.file_name().unwrap().to_string_lossy();

                        if let Ok(contents) = std::fs::read_to_string(&path) {
                            if let Ok(exported) = serde_json::from_str::<Exported>(&contents) {
                                let params = &exported.auth_params;
                                storage.reset(&params.identifier, params.pw_cost, &params.pw_nonce, &password);
                                let config = Config::new(&params.identifier, params.pw_cost, &params.pw_nonce);
                                config.write().unwrap();

                                for note in exported.encrypted_notes() {
                                    if let Some(uuid) = storage.decrypt(note) {
                                        storage.flush(&uuid).unwrap();

                                        if let Some(note) = storage.notes.get(&uuid) {
                                            sender.send(WindowEvent::AddNote(uuid, note.title.clone())).unwrap();
                                        }
                                    }
                                }
                            }
                            else {
                                let message = format!("{} is not exported JSON.", filename);
                                sender.send(WindowEvent::ShowNotification(message)).unwrap();
                            }
                        }
                        else {
                            let message = format!("{} does not contain UTF-8 data.", filename);
                            sender.send(WindowEvent::ShowNotification(message)).unwrap();
                        }
                    },
                    AppEvent::AddNote => {
                        let uuid = storage.create_note();
                        let note = storage.notes.get(&uuid).unwrap();
                        sender.send(WindowEvent::AddNote(uuid, note.title.clone())).unwrap();
                    },
                    AppEvent::SelectNote(uuid) => {
                        if let Some(item) = storage.notes.get(&uuid) {
                            window.load_note(&item.title, &item.text);
                        }
                    },
                    AppEvent::UpdateTitle(uuid, text) => {
                        storage.update_title(&uuid, &text);
                        // TODO: do not write on each keypress
                        storage.flush(&uuid).unwrap();
                    },
                    AppEvent::UpdateText(uuid, text) => {
                        storage.update_text(&uuid, &text);
                        // TODO: do not write on each keypress
                        storage.flush(&uuid).unwrap();
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
