use anyhow::Result;
use gio::prelude::*;
use gtk::prelude::*;
use glib::translate::{ToGlib, from_glib};
use std::env;
use std::path::PathBuf;
use crate::config;
use crate::config::{Config, Geometry};
use crate::consts::{APP_ID, APP_VERSION, ABOUT_UI, BASE_CSS, IMPORT_UI, SETUP_UI, SHORTCUTS_UI, WINDOW_UI};
use crate::secret;
use crate::storage::Storage;
use crate::ui::controller::Controller;
use standardfile::{remote, Exported, Credentials};

pub struct Application {
    app: gtk::Application,
}

enum AppEvent {
    AddNote,
    DeleteNote,
    SelectNote,
    Register(String, Credentials),
    SignIn(String, Credentials),
    Import(PathBuf, String, Option<String>),
    Update(Option<String>, Option<String>),
    UpdateFilter(Option<String>),
    CreateStorage(Credentials),
    FlushDirty,
    Quit,
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

fn get_user_details(builder: &gtk::Builder) -> Credentials {
    let identifier_entry = get_widget!(builder, gtk::Entry, "identifier-entry");
    let password_entry = get_widget!(builder, gtk::Entry, "password-entry");

    Credentials::from_defaults(&identifier_entry.get_text(), &password_entry.get_text())
}

fn get_auth_details(builder: &gtk::Builder) -> (String, Credentials) {
    let server_combo_box = get_widget!(builder, gtk::ComboBoxText, "server-combo");

    (server_combo_box.get_active_text().unwrap().to_string(), get_user_details(&builder))
}

fn show_main_content(builder: &gtk::Builder) {
    let stack = get_widget!(builder, gtk::Stack, "iridium-main-stack");
    let main_box = get_widget!(builder, gtk::Box, "iridium-main-content");
    stack.set_visible_child(&main_box);
}

fn show_notification(builder: &gtk::Builder, message: &str) {
    let revealer = get_widget!(builder, gtk::Revealer, "iridium-notification-revealer");
    let label = get_widget!(builder, gtk::Label, "iridium-notification-label");
    let close_button = get_widget!(builder, gtk::Button, "iridium-notification-button");

    label.set_text(&message);
    revealer.set_reveal_child(true);

    close_button.connect_clicked(move |_| {
        revealer.set_reveal_child(false);
    });
}

fn setup_style_provider(window: &gtk::ApplicationWindow) {
    let style_provider = gtk::CssProvider::new();
    style_provider.load_from_resource(BASE_CSS);

    gtk::StyleContext::add_provider_for_screen(
        &window.get_screen().unwrap(),
        &style_provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

fn setup_overlay_help(window: &gtk::ApplicationWindow) {
    let builder = gtk::Builder::from_resource(SHORTCUTS_UI);
    let shortcuts_window = get_widget!(builder, gtk::ShortcutsWindow,"shortcuts");
    window.set_help_overlay(Some(&shortcuts_window));
}

fn write_config(window: &gtk::ApplicationWindow, credentials: &Credentials, server: Option<String>) {
    let (width, height) = window.get_size();
    let (x, y) = window.get_position();

    let mut config = Config::new(credentials);

    config.geometry = Some(Geometry {
        x: x,
        y: y,
        width: width,
        height: height,
        maximized: false,
    });

    config.server = server;
    config.write().unwrap();
}

fn restore_geometry(config: &Config, window: &gtk::ApplicationWindow) {
    if let Some(geometry) = &config.geometry {
        window.move_(geometry.x, geometry.y);
        window.resize(geometry.width, geometry.height);
    }
}

impl Application {
    pub fn new() -> Result<Self> {
        let app = gtk::Application::new(Some(APP_ID), gio::ApplicationFlags::FLAGS_NONE)?;

        let (sender, receiver) = glib::MainContext::channel::<AppEvent>(glib::PRIORITY_DEFAULT);

        let builder = gtk::Builder::from_resource(WINDOW_UI);
        let window = get_widget!(builder, gtk::ApplicationWindow, "window");
        let note_list_box = get_widget!(builder, gtk::ListBox, "iridium-note-list");
        let title_entry = get_widget!(builder, gtk::Entry, "iridium-title-entry");
        let note_popover = get_widget!(builder, gtk::PopoverMenu, "note_menu");
        let identifier_entry = get_widget!(builder, gtk::Entry, "identifier-entry");
        let local_button = get_widget!(builder, gtk::Button, "create-local-button");
        let signup_button = get_widget!(builder, gtk::Button, "signup-button");
        let login_button = get_widget!(builder, gtk::Button, "login-button");

        let text_view = get_widget!(builder, gtk::TextView, "iridium-text-view");
        let text_buffer = text_view.get_buffer().unwrap();

        let search_bar = get_widget!(builder, gtk::SearchBar, "iridium-search-bar");
        let search_entry = get_widget!(builder, gtk::SearchEntry, "iridium-search-entry");

        let mut model = Controller::new(&builder);

        setup_overlay_help(&window);
        setup_style_provider(&window);

        let config = config::Config::new_from_file()?;

        let mut storage = match config {
            Some(config) => {
                restore_geometry(&config, &window);

                let password = secret::load(&config.identifier, config.server.as_deref())?;
                let credentials = Credentials::from_defaults(&config.identifier, &password);

                if let Some(server) = &config.server {
                    sender.send(AppEvent::SignIn(server.to_string(), credentials)).unwrap();
                }

                show_main_content(&builder);

                let credentials = config.to_credentials()?;
                let storage = Storage::new(&credentials, None)?;

                for note in storage.notes.values() {
                    model.insert(&note);
                }

                Some(storage)
            }
            None => None
        };

        app.connect_activate(
            clone!(@weak window => move |app| {
                window.set_application(Some(app));
                app.add_window(&window);
                window.present();
            })
        );

        window.connect_destroy(
            clone!(@strong sender as sender => move |_| {
                sender.send(AppEvent::Quit).unwrap();
            })
        );

        identifier_entry.bind_property("text-length", &local_button, "sensitive")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        identifier_entry.bind_property("text-length", &login_button, "sensitive")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        identifier_entry.bind_property("text-length", &signup_button, "sensitive")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        local_button.connect_clicked(
            clone!(@strong builder, @strong sender => move |_| {
                let main_box = get_widget!(builder, gtk::Box, "iridium-main-content");
                let stack = get_widget!(builder, gtk::Stack, "iridium-main-stack");
                stack.set_visible_child(&main_box);

                let user = get_user_details(&builder);
                sender.send(AppEvent::CreateStorage(user)).unwrap();
            })
        );

        signup_button.connect_clicked(
            clone!(@strong builder, @strong sender => move |_| {
                let (server, credentials) = get_auth_details(&builder);
                sender.send(AppEvent::Register(server, credentials)).unwrap();
            })
        );

        login_button.connect_clicked(
            clone!(@strong builder, @strong sender => move |_| {
                let (server, credentials) = get_auth_details(&builder);
                sender.send(AppEvent::SignIn(server, credentials)).unwrap();
            })
        );

        search_bar.connect_entry(&search_entry);

        search_entry.connect_search_changed(
            clone!(@weak search_entry, @strong sender => move |entry| {
                let text = entry.get_text();

                if text.len() > 2 {
                    sender.send(AppEvent::UpdateFilter(Some(text.as_str().to_string()))).unwrap();
                }
                else {
                    sender.send(AppEvent::UpdateFilter(None)).unwrap();
                }
            })
        );

        note_list_box.connect_row_selected(
            clone!(@strong sender, @strong note_popover => move |_, row| {
                if let Some(row) = row {
                    note_popover.set_relative_to(Some(row));
                    sender.send(AppEvent::SelectNote).unwrap();
                }
            })
        );

        note_list_box.connect_button_press_event(
            clone!(@strong note_popover => move |_, event_button| {
                if event_button.get_button() == 3 {
                    note_popover.popup();
                }
                glib::signal::Inhibit(false)
            })
        );

        action!(app, "quit",
            clone!(@strong sender as sender => move |_, _| {
                sender.send(AppEvent::Quit).unwrap();
            })
        );

        action!(app, "about",
            clone!(@weak window => move |_, _| {
                let builder = gtk::Builder::from_resource(ABOUT_UI);
                let dialog = get_widget!(builder, gtk::AboutDialog, "about-dialog");
                dialog.set_version(Some(APP_VERSION));
                dialog.set_logo_icon_name(Some(APP_ID));
                dialog.set_transient_for(Some(&window));
                dialog.connect_response(|dialog, _| dialog.close());
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
            clone!(@strong search_bar => move |_, _| {
                search_bar.set_search_mode(!search_bar.get_search_mode());
            })
        );

        action!(app, "import",
            clone!(@weak  window, @strong sender => move |_, _| {
                let builder = gtk::Builder::from_resource(IMPORT_UI);
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
                            let server = server_entry.get_text().to_string();

                            sender.send(AppEvent::Import(filename, password_entry.get_text().to_string(), Some(server))).unwrap();
                        }
                    }
                    _ => {}
                }

                dialog.close();
            })
        );

        action!(app, "setup",
            clone!(@weak window => move |_, _| {
                let builder = gtk::Builder::from_resource(SETUP_UI);
                let dialog = get_widget!(builder, gtk::Dialog, "setup-dialog");

                setup_server_dialog(&builder);
                dialog.set_transient_for(Some(&window));
                dialog.set_modal(true);
                dialog.connect_response(|dialog, _| dialog.close());
                dialog.show();
            })
        );

        app.set_accels_for_action("app.quit", &["<primary>q"]);
        app.set_accels_for_action("app.search", &["<primary>f"]);

        let mut flush_timer_running = false;
        let mut title_entry_handler: Option<u64> = None;
        let mut text_buffer_handler: Option<u64> = None;

        receiver.attach(None,
            clone!(@strong sender, @strong app, @strong window => move |event| {
                match event {
                    AppEvent::Quit => {
                        if let Some(storage) = &mut storage {
                            storage.flush_dirty().unwrap();
                        }

                        app.quit();
                    }
                    AppEvent::CreateStorage(user) => {
                        let credentials = Credentials::from_defaults(&user.identifier, &user.password);

                        match Storage::new(&credentials, None) {
                            Ok(s) => {
                                storage = Some(s);
                                write_config(&window, &credentials, None);
                                secret::store(&credentials, None);
                            }
                            Err(message) => {
                                show_notification(&builder, &format!("Error: {}.", message));
                            }
                        };
                    }
                    AppEvent::Register(server, credentials) => {
                        log::info!("Registering with {}", server);
                        let client = remote::Client::new_register(&server, credentials);

                        match client {
                            Ok(client) => {
                                let credentials = client.credentials.clone();
                                storage = Some(Storage::new(&credentials, Some(client)).unwrap());
                                write_config(&window, &credentials, Some(server.clone()));
                                secret::store(&credentials, Some(&server));
                                show_main_content(&builder);
                            }
                            Err(message) => {
                                let message = format!("Registration failed: {}.", message);
                                show_notification(&builder, &message);
                            }
                        };
                    }
                    AppEvent::SignIn(server, credentials) => {
                        log::info!("Signing in to {}", server);
                        let client = remote::Client::new_sign_in(&server, &credentials);

                        match client {
                            Ok(client) => {
                                // We have to use the clients credentials because encryption
                                // parameters such as nonce and number of iterations might have
                                // changed.
                                let credentials = client.credentials.clone();

                                // Switch storage, read local files and show them in the UI.
                                storage = Some(Storage::new(&credentials, Some(client)).unwrap());
                                write_config(&window, &credentials, Some(server.clone()));

                                for note in storage.as_ref().unwrap().notes.values() {
                                    model.insert(&note);
                                }

                                // Store the encryption password and auth token in the keyring.
                                secret::store(&credentials, Some(&server));

                                show_main_content(&builder);
                            }
                            Err(message) => {
                                let message = format!("Login failed: {}.", message);
                                show_notification(&builder, &message);
                            }
                        }
                    }
                    AppEvent::Import(path, password, server) => {
                        let filename = path.file_name().unwrap().to_string_lossy();

                        if let Ok(contents) = std::fs::read_to_string(&path) {
                            if let Ok(exported) = Exported::from_str(&contents) {
                                let credentials = Credentials::from_exported(&exported, &password);

                                write_config(&window, &credentials, None);
                                secret::store(&credentials, server.as_deref());

                                storage = Some(Storage::new_from_items(&credentials, &exported.items).unwrap());

                                if let Some(storage) = &storage {
                                    for note in storage.notes.values() {
                                        model.insert(&note);
                                    }
                                }
                            }
                            else {
                                let message = format!("{} is not exported JSON.", filename);
                                show_notification(&builder, &message);
                            }
                        }
                        else {
                            let message = format!("{} does not contain UTF-8 data.", filename);
                            show_notification(&builder, &message);
                        }
                    }
                    AppEvent::AddNote => {
                        if let Some(storage) = &mut storage {
                            let uuid = storage.create_note();
                            let note = storage.notes.get(&uuid).unwrap();
                            model.insert(&note);
                        }
                    }
                    AppEvent::DeleteNote => {
                        if let Some(storage) = &mut storage {
                            if let Some(uuid) = storage.current {
                                log::info!("Deleting {}", uuid);
                                model.delete(&uuid);
                                storage.delete(&uuid).unwrap();
                            }
                        }
                    }
                    AppEvent::SelectNote => {
                        let row = note_list_box.get_selected_row().unwrap();

                        if let Some(uuid) = model.select(&row) {
                            if let Some(storage) = &mut storage {
                                storage.set_current_uuid(&uuid).unwrap();

                                // We first disconnect the change handlers before setting the text
                                // and content to avoid updating the storage and model which would
                                // unnecessarily cause row movement and a server sync.

                                if let Some(handler) = title_entry_handler {
                                    title_entry.disconnect(from_glib(handler));
                                }

                                if let Some(handler) = text_buffer_handler {
                                    text_buffer.disconnect(from_glib(handler));
                                }

                                title_entry.set_text(&storage.get_title());
                                text_buffer.set_text(&storage.get_text());

                                title_entry_handler = Some(title_entry.connect_changed(
                                    clone!(@strong sender => move |entry| {
                                        sender.send(AppEvent::Update(Some(entry.get_text().to_string()), None)).unwrap();
                                    })
                                ).to_glib());

                                text_buffer_handler = Some(text_buffer.connect_changed(
                                    clone!(@strong sender => move |text_buffer| {
                                        let start = text_buffer.get_start_iter();
                                        let end = text_buffer.get_end_iter();
                                        let text = text_buffer.get_text(&start, &end, false).unwrap();
                                        let text = text.as_str().to_string();

                                        sender.send(AppEvent::Update(None, Some(text))).unwrap();
                                    })
                                ).to_glib());
                            }
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

                            model.updated(&storage.current.unwrap());

                            if !flush_timer_running {
                                glib::source::timeout_add_seconds(5,
                                    clone!(@strong sender => move || {
                                        sender.send(AppEvent::FlushDirty).unwrap();
                                        glib::Continue(false)
                                    })
                                );

                                flush_timer_running = true;
                            }
                        }
                    }
                    AppEvent::UpdateFilter(term) => {
                        if let Some(term) = term {
                            let term = term.to_lowercase();
                            model.show_matching_rows(&term);
                        }
                        else {
                            model.show_all_rows();
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
