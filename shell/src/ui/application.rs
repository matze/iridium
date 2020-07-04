use anyhow::Result;
use gio::prelude::*;
use gtk::prelude::*;
use std::env;
use crate::config;
use crate::consts::{APP_ID, APP_VERSION, ABOUT_UI, BASE_CSS, IMPORT_UI, SETUP_UI, SHORTCUTS_UI, WINDOW_UI};
use crate::secret;
use crate::storage::Storage;
use crate::ui::model::Model;
use crate::ui::state::{User, RemoteAuth, AppEvent};
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

fn get_user_details(builder: &gtk::Builder) -> User {
    let identifier_entry = get_widget!(builder, gtk::Entry, "identifier-entry");
    let password_entry = get_widget!(builder, gtk::Entry, "password-entry");

    User {
        identifier: identifier_entry.get_text().unwrap().to_string(),
        password: password_entry.get_text().unwrap().to_string(),
    }
}

fn get_auth_details(builder: &gtk::Builder) -> RemoteAuth {
    let server_combo_box = get_widget!(builder, gtk::ComboBoxText, "server-combo");

    RemoteAuth {
        server: server_combo_box.get_active_text().unwrap().to_string(),
        user: get_user_details(&builder),
    }
}

fn show_main_content(builder: &gtk::Builder, model: &Model) {
    let stack = get_widget!(builder, gtk::Stack, "iridium-main-stack");
    let main_box = get_widget!(builder, gtk::Box, "iridium-main-content");
    stack.set_visible_child(&main_box);

    // Do not show the right hand pane until we have a note to show.
    if model.is_empty() {
        let right_hand_stack = get_widget!(builder, gtk::Stack, "right-hand-stack");
        let right_hand_info = get_widget!(builder, gtk::Label, "right-hand-info-label");
        right_hand_stack.set_visible_child(&right_hand_info);
    }
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
    let builder = gtk::Builder::new_from_resource(SHORTCUTS_UI);
    let shortcuts_window = get_widget!(builder, gtk::ShortcutsWindow,"shortcuts");
    window.set_help_overlay(Some(&shortcuts_window));
}

impl Application {
    pub fn new() -> Result<Self> {
        let app = gtk::Application::new(Some(APP_ID), gio::ApplicationFlags::FLAGS_NONE)?;

        let (sender, receiver) = glib::MainContext::channel::<AppEvent>(glib::PRIORITY_DEFAULT);

        let builder = gtk::Builder::new_from_resource(WINDOW_UI);
        let window = get_widget!(builder, gtk::ApplicationWindow, "window");
        let note_list_box = get_widget!(builder, gtk::ListBox, "iridium-note-list");
        let title_entry = get_widget!(builder, gtk::Entry, "iridium-title-entry");
        let right_hand_stack = get_widget!(builder, gtk::Stack, "right-hand-stack");
        let right_hand_info = get_widget!(builder, gtk::Label, "right-hand-info-label");
        let note_pane_box = get_widget!(builder, gtk::Box, "iridium-entry-box");
        let note_popover = get_widget!(builder, gtk::PopoverMenu, "note_menu");
        let identifier_entry = get_widget!(builder, gtk::Entry, "identifier-entry");
        let local_button = get_widget!(builder, gtk::Button, "create-local-button");
        let signup_button = get_widget!(builder, gtk::Button, "signup-button");
        let login_button = get_widget!(builder, gtk::Button, "login-button");

        let text_view = get_widget!(builder, gtk::TextView, "iridium-text-view");
        let text_buffer = text_view.get_buffer().unwrap();

        let search_bar = get_widget!(builder, gtk::SearchBar, "iridium-search-bar");
        let search_entry = get_widget!(builder, gtk::SearchEntry, "iridium-search-entry");

        let mut model = Model::new(note_list_box.clone(), title_entry.clone());

        setup_overlay_help(&window);
        setup_style_provider(&window);

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
                    sender.send(AppEvent::SignIn(auth)).unwrap();
                }

                show_main_content(&builder, &model);

                let credentials = config.to_credentials()?;
                let storage = Storage::new(&credentials, None)?;

                for (uuid, note) in &storage.notes {
                    model.insert(&uuid, &note.title);
                }

                if !model.is_empty() {
                    right_hand_stack.set_visible_child(&note_pane_box);
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
                sender.send(AppEvent::Register(get_auth_details(&builder))).unwrap();
            })
        );

        login_button.connect_clicked(
            clone!(@strong builder, @strong sender => move |_| {
                sender.send(AppEvent::SignIn(get_auth_details(&builder))).unwrap();
            })
        );

        search_bar.connect_entry(&search_entry);

        search_entry.connect_search_changed(
            clone!(@weak search_entry, @strong sender => move |entry| {
                if let Some(text) = entry.get_text() {
                    if text.len() > 2 {
                        sender.send(AppEvent::UpdateFilter(Some(text.as_str().to_string()))).unwrap();
                    }
                    else {
                        sender.send(AppEvent::UpdateFilter(None)).unwrap();
                    }
                }
            })
        );

        title_entry.connect_changed(
            clone!(@strong sender => move |entry| {
                if let Some(text) = entry.get_text() {
                    sender.send(AppEvent::Update(Some(text.to_string()), None)).unwrap();
                }
            })
        );

        text_buffer.connect_changed(
            clone!(@strong sender => move |text_buffer| {
                let start = text_buffer.get_start_iter();
                let end = text_buffer.get_end_iter();
                let text = text_buffer.get_text(&start, &end, false).unwrap();
                let text = text.as_str().to_string();

                sender.send(AppEvent::Update(None, Some(text))).unwrap();
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
            clone!(@strong search_bar => move |_, _| {
                search_bar.set_search_mode(!search_bar.get_search_mode());
            })
        );

        action!(app, "import",
            clone!(@weak  window, @strong sender => move |_, _| {
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
            clone!(@weak window => move |_, _| {
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
            clone!(@strong sender, @strong app => move |event| {
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
                                show_notification(&builder, &format!("Error: {}.", message));
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
                                show_main_content(&builder, &model);
                            }
                            Err(message) => {
                                let message = format!("Registration failed: {}.", message);
                                show_notification(&builder, &message);
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
                                    model.insert(&uuid, &note.title);
                                }

                                // Store the encryption password and auth token in the keyring.
                                secret::store(&credentials, Some(&auth.server));

                                show_main_content(&builder, &model);
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
                                    model.insert(&uuid, &note.title);
                                }

                                storage = Some(temp);
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
                            model.insert(&uuid, "");
                        }
                    }
                    AppEvent::DeleteNote => {
                        if let Some(storage) = &mut storage {
                            if let Some(uuid) = storage.current {
                                log::info!("Deleting {}", uuid);
                                model.delete(&uuid);
                                storage.delete(&uuid).unwrap();

                                if model.is_empty() {
                                    right_hand_stack.set_visible_child(&right_hand_info);
                                }
                            }
                        }
                    }
                    AppEvent::SelectNote => {
                        let row = note_list_box.get_selected_row().unwrap();

                        if let Some(uuid) = model.select(&row) {
                            if let Some(storage) = &mut storage {
                                storage.set_current_uuid(&uuid).unwrap();
                                title_entry.set_text(&storage.get_title());
                                text_buffer.set_text(&storage.get_text());
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
