use anyhow::Result;
use gio::prelude::*;
use gtk::prelude::*;
use std::env;
use std::collections::HashSet;
use crate::config::{APP_ID, APP_VERSION, Config};
use crate::secret;
use crate::storage::Storage;
use crate::standardfile::{crypto, remote, Item, Exported, Credentials, encrypted_notes};
use crate::ui::state::{AppEvent, WindowEvent};
use crate::ui::window::Window;
use uuid::Uuid;

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
            }
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

        window.widget.connect_destroy(
            clone!(@strong sender as sender => move |_| {
                sender.send(AppEvent::Quit).unwrap();
            })
        );

        action!(app, "quit",
            clone!(@strong sender as sender => move |_, _| {
                sender.send(AppEvent::Quit).unwrap();
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
                    }
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

        let mut to_flush: HashSet<Uuid> = HashSet::new();
        let mut client: Option<remote::Client> = None;

        receiver.attach(None,
            clone!(@strong sender as app_sender, @strong window.sender as sender, @strong app => move |event| {
                match event {
                    AppEvent::Quit => {
                        for uuid in &to_flush {
                            storage.flush(&uuid).unwrap();
                        }

                        app.quit();
                    }
                    AppEvent::CreateStorage(user) => {
                        let credentials = Credentials {
                            identifier: user.identifier,
                            cost: 110000,
                            nonce: crypto::make_nonce(),
                            password: user.password,
                            token: None,
                        };

                        storage.reset(&credentials);

                        let config = Config::new(&credentials);
                        config.write().unwrap();

                        secret::store(&credentials, None);
                    }
                    AppEvent::Register(auth) => {
                        let new_client = remote::Client::new_register(&auth.server, &auth.user.identifier, &auth.user.password);

                        match new_client {
                            Ok(new_client) => {
                                let credentials = &new_client.credentials;
                                storage.reset(&credentials);

                                let config = Config::new(&credentials);
                                config.write().unwrap();

                                secret::store(&credentials, Some(&auth.server));
                                sender.send(WindowEvent::ShowMainContent).unwrap();

                                // Replace the shared client.
                                client = Some(new_client);
                            }
                            Err(message) => {
                                let message = format!("Registration failed: {}.", message);
                                sender.send(WindowEvent::ShowNotification(message)).unwrap();
                            }
                        };
                    }
                    AppEvent::SignIn(auth) => {
                        let new_client = remote::Client::new_sign_in(&auth.server, &auth.user.identifier, &auth.user.password);

                        match new_client {
                            Ok(new_client) => {
                                let credentials = &new_client.credentials;

                                // Switch storage, read local files and show them in the UI.
                                storage.reset(&credentials);

                                let config = Config::new(&credentials);
                                config.write().unwrap();

                                for (uuid, note) in &storage.notes {
                                    sender.send(WindowEvent::AddNote(uuid.clone(), note.title.clone())).unwrap();
                                }

                                // Find all items we haven't synced yet. For now pretend we have
                                // never synced an item.
                                let mut unsynced_items: Vec<Item> = Vec::new();

                                for (uuid, _) in &storage.notes {
                                    unsynced_items.push(storage.encrypt(&uuid).unwrap());
                                }

                                // Decrypt, flush and show notes we have retrieved from the initial
                                // sync.
                                let items = new_client.sync(unsynced_items).unwrap();

                                for item in items {
                                    if item.content_type == "Note" {
                                        if let Some(uuid) = storage.decrypt(&item) {
                                            storage.flush(&uuid).unwrap();

                                            if let Some(note) = storage.notes.get(&uuid) {
                                                sender.send(WindowEvent::AddNote(uuid, note.title.clone())).unwrap();
                                            }
                                        }
                                    }
                                }

                                // Store the encryption password and auth token in the keyring.
                                secret::store(&credentials, Some(&auth.server));

                                sender.send(WindowEvent::ShowMainContent).unwrap();
                            }
                            Err(message) => {
                                let message = format!("Login failed: {}.", message);
                                sender.send(WindowEvent::ShowNotification(message)).unwrap();
                            }
                        }
                    }
                    AppEvent::Import(path, password) => {
                        let filename = path.file_name().unwrap().to_string_lossy();

                        if let Ok(contents) = std::fs::read_to_string(&path) {
                            if let Ok(exported) = serde_json::from_str::<Exported>(&contents) {
                                let credentials = Credentials {
                                    identifier: exported.auth_params.identifier,
                                    cost: exported.auth_params.pw_cost,
                                    nonce: exported.auth_params.pw_nonce,
                                    password: password,
                                    token: None,
                                };

                                storage.reset(&credentials);

                                let config = Config::new(&credentials);
                                config.write().unwrap();

                                for note in encrypted_notes(&exported.items) {
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
                    }
                    AppEvent::AddNote => {
                        let uuid = storage.create_note();
                        let note = storage.notes.get(&uuid).unwrap();
                        sender.send(WindowEvent::AddNote(uuid, note.title.clone())).unwrap();
                    }
                    AppEvent::SelectNote(uuid) => {
                        if let Some(item) = storage.notes.get(&uuid) {
                            window.load_note(&item.title, &item.text);
                        }
                    }
                    AppEvent::Update(uuid, title, text) => {
                        if let Some(title) = title {
                            storage.update_title(&uuid, &title);
                        }

                        if let Some(text) = text {
                            storage.update_text(&uuid, &text);
                        }

                        if !to_flush.contains(&uuid) {
                            to_flush.insert(uuid);

                            glib::source::timeout_add_seconds(5,
                                clone!(@strong app_sender as sender => move ||{
                                    sender.send(AppEvent::Flush(uuid)).unwrap();
                                    glib::Continue(false)
                                })
                            );
                        }
                    }
                    AppEvent::Flush(uuid) => {
                        storage.flush(&uuid).unwrap();
                        to_flush.remove(&uuid);
                    }
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
