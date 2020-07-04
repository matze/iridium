use anyhow::Result;
use gio::prelude::*;
use gtk::prelude::*;
use std::env;
use crate::config;
use crate::consts::{ABOUT_UI, IMPORT_UI, SETUP_UI};
use crate::consts::{APP_ID, APP_VERSION};
use crate::secret;
use crate::storage::Storage;
use crate::ui::state::{User, RemoteAuth, AppEvent, WindowEvent};
use crate::ui::window::Window;
use standardfile::{crypto, remote, Exported, Credentials};

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

impl Application {
    pub fn new() -> Result<Self> {
        let app = gtk::Application::new(Some(APP_ID), gio::ApplicationFlags::FLAGS_NONE)?;

        let (sender, receiver) = glib::MainContext::channel::<AppEvent>(glib::PRIORITY_DEFAULT);
        let window = Window::new(sender.clone());

        let config = config::Config::new_from_file()?;

        let mut storage = match config {
            Some(config) => {
                let user = User {
                    password: secret::load(&config.identifier, config.server.as_deref())?,
                    identifier: config.identifier.clone(),
                };

                if let Some(server) = &config.server {
                    let auth = RemoteAuth {
                        user: user,
                        server: server.clone(),
                    };
                    sender.send(AppEvent::SignIn(auth))?;
                }
                window.sender.send(WindowEvent::ShowMainContent).unwrap();

                let credentials = config.to_credentials()?;
                let storage = Storage::new(&credentials, None)?;

                for (uuid, note) in &storage.notes {
                    window.sender.send(WindowEvent::AddNote(*uuid, note.title.clone())).unwrap();
                }

                Some(storage)
            }
            None => None
        };

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

        let mut flush_timer_running = false;

        receiver.attach(None,
            clone!(@strong sender as app_sender, @strong window.sender as sender, @strong app => move |event| {
                match event {
                    AppEvent::Quit => {
                        if let Some(storage) = &mut storage {
                            storage.flush_dirty().unwrap();
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

                        match Storage::new(&credentials, None) {
                            Ok(s) => {
                                storage = Some(s);
                                config::write(&credentials).unwrap();
                                secret::store(&credentials, None);
                            }
                            Err(message) => {
                                sender.send(WindowEvent::ShowNotification(format!("Error: {}.", message))).unwrap();
                            }
                        };
                    }
                    AppEvent::Register(auth) => {
                        log::info!("Registering with {}", auth.server);
                        let client = remote::Client::new_register(&auth.server, &auth.user.identifier, &auth.user.password);

                        match client {
                            Ok(client) => {
                                let credentials = client.credentials.clone();
                                storage = Some(Storage::new(&credentials, Some(client)).unwrap());
                                config::write_with_server(&credentials, &auth.server).unwrap();
                                secret::store(&credentials, Some(&auth.server));
                                sender.send(WindowEvent::ShowMainContent).unwrap();
                            }
                            Err(message) => {
                                let message = format!("Registration failed: {}.", message);
                                sender.send(WindowEvent::ShowNotification(message)).unwrap();
                            }
                        };
                    }
                    AppEvent::SignIn(auth) => {
                        log::info!("Signing in to {}", auth.server);
                        let client = remote::Client::new_sign_in(&auth.server, &auth.user.identifier, &auth.user.password);

                        match client {
                            Ok(client) => {
                                let credentials = client.credentials.clone();

                                // Switch storage, read local files and show them in the UI.
                                storage = Some(Storage::new(&credentials, Some(client)).unwrap());
                                config::write_with_server(&credentials, &auth.server).unwrap();

                                for (uuid, note) in &storage.as_ref().unwrap().notes {
                                    sender.send(WindowEvent::AddNote(uuid.clone(), note.title.clone())).unwrap();
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
                    AppEvent::Import(path, password, server) => {
                        let filename = path.file_name().unwrap().to_string_lossy();

                        if let Ok(contents) = std::fs::read_to_string(&path) {
                            if let Ok(exported) = Exported::from_str(&contents) {
                                let credentials = Credentials {
                                    identifier: exported.auth_params.identifier,
                                    cost: exported.auth_params.pw_cost,
                                    nonce: exported.auth_params.pw_nonce,
                                    password: password,
                                };

                                let temp = Storage::new_from_items(&credentials, &exported.items).unwrap();

                                config::write(&credentials).unwrap();
                                secret::store(&credentials, server.as_deref());

                                for (uuid, note) in &temp.notes {
                                    sender.send(WindowEvent::AddNote(*uuid, note.title.clone())).unwrap();
                                }

                                storage = Some(temp);
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
                        if let Some(storage) = &mut storage {
                            let uuid = storage.create_note();
                            let note = storage.notes.get(&uuid).unwrap();
                            sender.send(WindowEvent::AddNote(uuid, note.title.clone())).unwrap();
                        }
                    }
                    AppEvent::DeleteNote => {
                        if let Some(storage) = &mut storage {
                            if let Some(uuid) = storage.current {
                                log::info!("Deleting {}", uuid);
                                sender.send(WindowEvent::DeleteNote(uuid)).unwrap();
                                storage.delete(&uuid).unwrap();
                            }
                        }
                    }
                    AppEvent::SelectNote(uuid) => {
                        if let Some(storage) = &mut storage {
                            storage.set_current_uuid(&uuid).unwrap();
                            sender.send(WindowEvent::UpdateNote(storage.get_title(), storage.get_text())).unwrap();
                        }
                    }
                    AppEvent::Update(title, text) => {
                        if let Some(storage) = &mut storage {
                            if let Some(title) = title {
                                storage.set_title(&title);
                            }

                            if let Some(text) = text {
                                storage.set_text(&text);
                            }

                            if !flush_timer_running {
                                glib::source::timeout_add_seconds(5,
                                    clone!(@strong app_sender as sender => move || {
                                        sender.send(AppEvent::FlushDirty).unwrap();
                                        glib::Continue(false)
                                    })
                                );

                                flush_timer_running = true;
                            }
                        }
                    }
                    AppEvent::FlushDirty => {
                        if let Some(storage) = &mut storage {
                            storage.flush_dirty().unwrap();
                            flush_timer_running = false;
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
