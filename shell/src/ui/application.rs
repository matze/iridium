use anyhow::Result;
use gio::prelude::*;
use gtk::prelude::*;
use std::env;
use std::collections::HashSet;
use crate::config;
use crate::consts::{ABOUT_UI, IMPORT_UI, SETUP_UI};
use crate::consts::{APP_ID, APP_VERSION};
use crate::secret;
use crate::storage::Storage;
use crate::ui::state::{User, RemoteAuth, AppEvent, WindowEvent};
use crate::ui::window::Window;
use standardfile::{crypto, remote, Item, Exported, Credentials, encrypted_notes};
use uuid::Uuid;

pub struct Application {
    app: gtk::Application,
}

fn setup_server_dialog(builder: &gtk::Builder) {
    let server_box = get_widget!(builder, gtk::ComboBoxText, "server-box");
    let server_entry = server_box.get_child().unwrap().downcast::<gtk::Entry>().unwrap();
    let sync_button = get_widget!(builder, gtk::Switch, "sync-switch");

    server_entry.set_input_purpose(gtk::InputPurpose::Url);
    server_entry.set_icon_from_icon_name(gtk::EntryIconPosition::Primary, Some("network-server-symbolic"));
    server_entry.set_placeholder_text(Some("Server address"));
    sync_button.bind_property("active", &server_box, "sensitive").flags(glib::BindingFlags::SYNC_CREATE).build();
    sync_button.bind_property("active", &server_entry, "sensitive").flags(glib::BindingFlags::SYNC_CREATE).build();
}

fn decrypt_and_store(storage: &mut Storage, item: &Item, sender: &glib::Sender<WindowEvent>) -> Result<()> {
    if let Some(uuid) = storage.decrypt(&item) {
        storage.flush(&uuid).unwrap();

        if let Some(note) = storage.notes.get(&uuid) {
            sender.send(WindowEvent::AddNote(uuid, note.title.clone())).unwrap();
        }
    }
    Ok(())
}

impl Application {
    pub fn new() -> Result<Self> {
        let app = gtk::Application::new(Some(APP_ID), gio::ApplicationFlags::FLAGS_NONE)?;

        let (sender, receiver) = glib::MainContext::channel::<AppEvent>(glib::PRIORITY_DEFAULT);
        let window = Window::new(sender.clone());

        let config = config::Config::new_from_file()?;

        let mut storage = match config {
            Some(config) => {
                let user = User {
                    password: secret::load(&config.identifier, config.server.as_deref()).unwrap(),
                    identifier: config.identifier.clone(),
                };

                if let Some(server) = &config.server {
                    let auth = RemoteAuth {
                        user: user,
                        server: server.clone(),
                    };
                    sender.send(AppEvent::SignIn(auth)).unwrap();
                }
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
                let builder = gtk::Builder::new_from_resource(ABOUT_UI);
                let dialog = get_widget!(builder, gtk::AboutDialog, "about-dialog");
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

        action!(app, "delete",
            clone!(@strong sender as sender => move |_, _| {
                sender.send(AppEvent::DeleteNote).unwrap();
            })
        );

        action!(app, "search",
            clone!(@strong window.sender as sender => move |_, _| {
                sender.send(WindowEvent::ToggleSearchBar).unwrap();
            })
        );

        action!(app, "import",
            clone!(@weak window.widget as window, @strong sender as sender => move |_, _| {
                let builder = gtk::Builder::new_from_resource(IMPORT_UI);
                let dialog = get_widget!(builder, gtk::Dialog, "import-dialog");

                setup_server_dialog(&builder);
                dialog.set_transient_for(Some(&window));
                dialog.set_modal(true);

                match dialog.run() {
                    gtk::ResponseType::Ok => {
                        let file_chooser = get_widget!(builder, gtk::FileChooserButton, "import-file-button");

                        if let Some(filename) = file_chooser.get_filename() {
                            let password_entry = get_widget!(builder, gtk::Entry, "import-password");
                            let server_box = get_widget!(builder, gtk::ComboBoxText, "server-box");
                            let server_entry = server_box.get_child().unwrap().downcast::<gtk::Entry>().unwrap();
                            let server = server_entry.get_text().as_deref().unwrap().to_string();

                            if let Some(password) = password_entry.get_text() {
                                sender.send(AppEvent::Import(filename, password.as_str().to_string(), Some(server))).unwrap();
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
                let builder = gtk::Builder::new_from_resource(SETUP_UI);
                let dialog = get_widget!(builder, gtk::Dialog, "setup-dialog");

                setup_server_dialog(&builder);
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
        let mut selected: Option<Uuid> = None;

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
                        };

                        storage.reset(&credentials);
                        config::write(&credentials).unwrap();
                        secret::store(&credentials, None);
                    }
                    AppEvent::Register(auth) => {
                        log::info!("Registering with {}", auth.server);
                        let new_client = remote::Client::new_register(&auth.server, &auth.user.identifier, &auth.user.password);

                        match new_client {
                            Ok(new_client) => {
                                let credentials = &new_client.credentials;
                                storage.reset(&credentials);
                                config::write(&credentials).unwrap();
                                secret::store(&credentials, Some(&auth.server));
                                sender.send(WindowEvent::ShowMainContent).unwrap();

                                // Replace the shared client.
                                client = Some(new_client);
                            }
                            Err(message) => {
                                let message = format!("Registration failed: {}.", message);
                                sender.send(WindowEvent::ShowNotification(message)).unwrap();
                                client = None;
                            }
                        };
                    }
                    AppEvent::SignIn(auth) => {
                        log::info!("Signing in to {}", auth.server);

                        let new_client = remote::Client::new_sign_in(&auth.server, &auth.user.identifier, &auth.user.password);

                        match new_client {
                            Ok(new_client) => {
                                let credentials = &new_client.credentials;

                                // Switch storage, read local files and show them in the UI.
                                storage.reset(&credentials);
                                config::write_with_server(&credentials, &auth.server).unwrap();

                                for (uuid, note) in &storage.notes {
                                    sender.send(WindowEvent::AddNote(uuid.clone(), note.title.clone())).unwrap();
                                }

                                // Store the encryption password and auth token in the keyring.
                                secret::store(&credentials, Some(&auth.server));

                                sender.send(WindowEvent::ShowMainContent).unwrap();

                                // Replace the shared client.
                                client = Some(new_client);
                            }
                            Err(message) => {
                                let message = format!("Login failed: {}.", message);
                                sender.send(WindowEvent::ShowNotification(message)).unwrap();
                                client = None;
                            }
                        }

                        if let Some(client) = &mut client {
                            // Find all items we haven't synced yet. For now pretend we have
                            // never synced an item.
                            let mut unsynced_items: Vec<Item> = Vec::new();

                            for (uuid, _) in &storage.notes {
                                unsynced_items.push(storage.encrypt(&uuid).unwrap());
                            }

                            // Decrypt, flush and show notes we have retrieved from the initial
                            // sync.
                            let items = client.sync(unsynced_items).unwrap();

                            for item in encrypted_notes(&items) {
                                if !item.deleted.unwrap_or(false) {
                                    decrypt_and_store(&mut storage, &item, &sender).unwrap();
                                }
                            }
                        }
                    }
                    AppEvent::Import(path, password, server) => {
                        let filename = path.file_name().unwrap().to_string_lossy();

                        if let Ok(contents) = std::fs::read_to_string(&path) {
                            if let Ok(exported) = serde_json::from_str::<Exported>(&contents) {
                                let credentials = Credentials {
                                    identifier: exported.auth_params.identifier,
                                    cost: exported.auth_params.pw_cost,
                                    nonce: exported.auth_params.pw_nonce,
                                    password: password,
                                };

                                storage.reset(&credentials);
                                config::write(&credentials).unwrap();
                                secret::store(&credentials, server.as_deref());

                                for item in encrypted_notes(&exported.items) {
                                    decrypt_and_store(&mut storage, &item, &sender).unwrap();
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
                    AppEvent::DeleteNote => {
                        if let Some(uuid) = selected {
                            log::info!("Deleting {}", uuid);

                            if to_flush.contains(&uuid) {
                                to_flush.remove(&uuid);
                            }

                            if let Some(client) = &mut client {
                                let mut encrypted = storage.encrypt(&uuid).unwrap();
                                encrypted.deleted = Some(true);

                                // Apparently, we do not receive the item back as marked deleted
                                // but on subsequent syncs only.
                                client.sync(vec![encrypted]).unwrap();
                            }

                            sender.send(WindowEvent::DeleteNote(uuid)).unwrap();
                            storage.delete(&uuid).unwrap();
                        }
                    }
                    AppEvent::SelectNote(uuid) => {
                        if let Some(item) = storage.notes.get(&uuid) {
                            window.load_note(&item.title, &item.text);
                            selected = Some(uuid);
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
                                clone!(@strong app_sender as sender => move || {
                                    sender.send(AppEvent::Flush(uuid)).unwrap();
                                    glib::Continue(false)
                                })
                            );
                        }
                    }
                    AppEvent::Flush(uuid) => {
                        if let Some(client) = &mut client {
                            // Ideally we use this to store on the server and the local storage.
                            let encrypted = storage.encrypt(&uuid).unwrap();
                            client.sync(vec![encrypted]).unwrap();
                        };

                        if to_flush.contains(&uuid) {
                            storage.flush(&uuid).unwrap();
                            to_flush.remove(&uuid);
                        }
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
