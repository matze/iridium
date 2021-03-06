use anyhow::Result;
use gio::prelude::*;
use gtk::prelude::*;
use glib::translate::{ToGlib, from_glib};
use std::env;
use std::path::PathBuf;
use crate::config::{Config, Geometry};
use crate::consts::{APP_DOMAIN, APP_ID, APP_VERSION, ABOUT_UI, BASE_CSS, IMPORT_UI, SHORTCUTS_UI, WINDOW_UI};
use crate::secret;
use crate::storage::Storage;
use crate::ui::controller::Controller;
use standardfile::{remote, Exported, Credentials};

pub struct Application {
    app: gtk::Application,
    window: gtk::ApplicationWindow,
    sender: glib::Sender<AppEvent>,
    builder: gtk::Builder,
    search_bar: gtk::SearchBar,
    tag_entry: gtk::Entry,
    setup_create_button: gtk::Button,
    setup_signup_button: gtk::Button,
    setup_login_button: gtk::Button,
    note_list_box: gtk::ListBox,
    note_popover: gtk::PopoverMenu,
}

enum AppEvent {
    AddNote,
    DeleteNote,
    SelectNote,
    Register(String, Credentials),
    SignIn(String, Credentials),
    Import(PathBuf, String, Option<String>),
    Export(PathBuf),
    Update(Option<String>, Option<String>),
    UpdateFilter(Option<String>),
    UpdateGeometry(Geometry),
    CreateStorage(Credentials),
    Switch(String),
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

fn show_header_buttons(builder: &gtk::Builder, visible: bool) {
    let menu_button = get_widget!(builder, gtk::MenuButton, "appmenu-button");
    let add_button = get_widget!(builder, gtk::Button, "add-button");
    menu_button.set_visible(visible);
    add_button.set_visible(visible);
}

fn show_setup_content(builder: &gtk::Builder) {
    let stack = get_widget!(builder, gtk::Stack, "main-stack");
    let setup_box = get_widget!(builder, gtk::Box, "main-setup");
    show_header_buttons(builder, false);
    stack.set_visible_child(&setup_box);
}

fn show_main_content(builder: &gtk::Builder) {
    let stack = get_widget!(builder, gtk::Stack, "main-stack");
    let main_box = get_widget!(builder, gtk::Box, "main-content");
    show_header_buttons(builder, true);
    stack.set_visible_child(&main_box);
}

fn show_notification(builder: &gtk::Builder, message: &str) {
    let revealer = get_widget!(builder, gtk::Revealer, "notification-revealer");
    let label = get_widget!(builder, gtk::Label, "notification-label");
    let close_button = get_widget!(builder, gtk::Button, "notification-button");

    label.set_text(&message);
    revealer.set_reveal_child(true);

    close_button.connect_clicked(move |_| {
        revealer.set_reveal_child(false);
    });
}

impl Application {
    fn setup_overlay_help(&self) {
        let builder = gtk::Builder::from_resource(SHORTCUTS_UI);
        let shortcuts_window = get_widget!(builder, gtk::ShortcutsWindow, "shortcuts");
        self.window.set_help_overlay(Some(&shortcuts_window));
    }

    fn setup_style_provider(&self) {
        let style_provider = gtk::CssProvider::new();
        style_provider.load_from_resource(BASE_CSS);

        gtk::StyleContext::add_provider_for_screen(
            &self.window.get_screen().unwrap(),
            &style_provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    fn setup_actions(&self) {
        action!(self.app, "quit",
            clone!(@strong self.sender as sender => move |_, _| {
                sender.send(AppEvent::Quit).unwrap();
            })
        );

        action!(self.app, "about",
            clone!(@weak self.window as window => move |_, _| {
                let builder = gtk::Builder::from_resource(ABOUT_UI);
                let dialog = get_widget!(builder, gtk::AboutDialog, "about-dialog");
                dialog.set_version(Some(APP_VERSION));
                dialog.set_logo_icon_name(Some(APP_ID));
                dialog.set_transient_for(Some(&window));
                dialog.connect_response(|dialog, _| dialog.close());
                dialog.show();
            })
        );

        action!(self.app, "add",
            clone!(@strong self.sender as sender => move |_, _| {
                sender.send(AppEvent::AddNote).unwrap();
            })
        );

        action!(self.app, "delete",
            clone!(@strong self.sender as sender => move |_, _| {
                sender.send(AppEvent::DeleteNote).unwrap();
            })
        );

        action!(self.app, "setup",
            clone!(@weak self.builder as builder => move |_, _| {
                show_setup_content(&builder);
            })
        );

        action!(self.app, "search",
            clone!(@strong self.search_bar as search_bar => move |_, _| {
                search_bar.set_search_mode(!search_bar.get_search_mode());
            })
        );

        action!(self.app, "tags",
            clone!(@strong self.tag_entry as tag_entry => move |_, _| {
                // Replace with tag_entry.set_action_name et al.
                tag_entry.set_visible(!tag_entry.get_visible());
            })
        );

        action!(self.app, "import",
            clone!(@weak self.window as window, @strong self.sender as sender => move |_, _| {
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
                            let server = if server != "" { Some(server) } else { None };

                            sender.send(AppEvent::Import(filename, password_entry.get_text().to_string(), server)).unwrap();
                        }
                    }
                    _ => {}
                }

                dialog.close();
            })
        );

        action!(self.app, "export",
            clone!(@weak self.window as window, @strong self.sender as sender => move |_, _| {
                let dialog = gtk::FileChooserDialog::with_buttons::<gtk::ApplicationWindow>(
                    Some("Export JSON"),
                    Some(&window),
                    gtk::FileChooserAction::Save,
                    &[("_Cancel", gtk::ResponseType::Cancel), ("_Save", gtk::ResponseType::Accept)]
                );

                match dialog.run() {
                    gtk::ResponseType::Accept => {
                        if let Some(filename) = dialog.get_filename() {
                            sender.send(AppEvent::Export(filename)).unwrap();
                        }
                    },
                    _ => {}
                }

                dialog.close();
            })
        );

        self.app.set_accels_for_action("app.quit", &["<primary>q"]);
        self.app.set_accels_for_action("app.search", &["<primary>f"]);
        self.app.set_accels_for_action("app.tags", &["<primary>t"]);
    }

    fn setup_signals(&self) {
        let search_entry = get_widget!(self.builder, gtk::SearchEntry, "search-entry");

        search_entry.connect_search_changed(
            clone!(@strong self.sender as sender => move |entry| {
                let text = entry.get_text();

                if text.len() > 2 {
                    sender.send(AppEvent::UpdateFilter(Some(text.as_str().to_string()))).unwrap();
                }
                else {
                    sender.send(AppEvent::UpdateFilter(None)).unwrap();
                }
            })
        );

        self.search_bar.connect_entry(&search_entry);

        self.app.connect_activate(
            clone!(@weak self.window as window => move |app| {
                window.set_application(Some(app));
                app.add_window(&window);
                window.present();
            })
        );

        self.window.connect_destroy(
            clone!(@strong self.sender as sender => move |_| {
                sender.send(AppEvent::Quit).unwrap();
            })
        );

        self.window.connect_configure_event(
            clone!(@strong self.sender as sender => move |window, event| {
                let (width, height) = event.get_size();
                let (x, y) = window.get_position();

                sender.send(AppEvent::UpdateGeometry(Geometry {
                    x: x,
                    y: y,
                    width: width,
                    height: height,
                    maximized: false,
                })).unwrap();

                false
            })
        );

        self.setup_create_button.connect_clicked(
            clone!(@strong self.builder as builder, @strong self.sender as sender => move |_| {
                let user = get_user_details(&builder);
                sender.send(AppEvent::CreateStorage(user)).unwrap();
            })
        );

        self.setup_signup_button.connect_clicked(
            clone!(@strong self.builder as builder, @strong self.sender as sender => move |_| {
                let (server, credentials) = get_auth_details(&builder);
                sender.send(AppEvent::Register(server, credentials)).unwrap();
            })
        );

        self.setup_login_button.connect_clicked(
            clone!(@strong self.builder as builder, @strong self.sender as sender => move |_| {
                let (server, credentials) = get_auth_details(&builder);
                sender.send(AppEvent::SignIn(server, credentials)).unwrap();
            })
        );

        self.note_list_box.connect_row_selected(
            clone!(@strong self.sender as sender, @strong self.note_popover as popover => move |_, row| {
                if let Some(row) = row {
                    popover.set_relative_to(Some(row));
                    sender.send(AppEvent::SelectNote).unwrap();
                }
            })
        );

        self.note_list_box.connect_button_press_event(
            clone!(@strong self.note_popover as popover => move |_, event_button| {
                if event_button.get_button() == 3 {
                    popover.popup();
                }
                glib::signal::Inhibit(false)
            })
        );
    }

    fn setup_binds(&self) {
        let setup_identifier_entry = get_widget!(self.builder, gtk::Entry, "identifier-entry");

        setup_identifier_entry.bind_property("text-length", &self.setup_create_button, "sensitive")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        setup_identifier_entry.bind_property("text-length", &self.setup_login_button, "sensitive")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();

        setup_identifier_entry.bind_property("text-length", &self.setup_signup_button, "sensitive")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .build();
    }

    fn restore_geometry(&self, geometry: &Geometry) {
        self.window.move_(geometry.x, geometry.y);
        self.window.resize(geometry.width as i32, geometry.height as i32);
    }

    pub fn new() -> Result<Self> {
        let app = gtk::Application::new(Some(APP_ID), gio::ApplicationFlags::FLAGS_NONE)?;
        let builder = gtk::Builder::from_resource(WINDOW_UI);

        let (sender, receiver) = glib::MainContext::channel::<AppEvent>(glib::PRIORITY_DEFAULT);

        let window = get_widget!(builder, gtk::ApplicationWindow, "window");
        let note_list_box = get_widget!(builder, gtk::ListBox, "note-list");
        let note_popover = get_widget!(builder, gtk::PopoverMenu, "note-menu");
        let profile_menu = get_widget!(builder, gtk::Box, "profile-menu");
        let title_entry = get_widget!(builder, gtk::Entry, "title-entry");
        let text_view = get_widget!(builder, gtk::TextView, "text-view");
        let text_buffer = text_view.get_buffer().unwrap();

        let application = Self {
            app: app.clone(),
            window: window.clone(),
            sender: sender.clone(),
            builder: builder.clone(),
            tag_entry: get_widget!(builder, gtk::Entry, "tag-entry"),
            search_bar: get_widget!(builder, gtk::SearchBar, "search-bar"),
            setup_create_button: get_widget!(builder, gtk::Button, "create-local-button"),
            setup_signup_button: get_widget!(builder, gtk::Button, "signup-button"),
            setup_login_button: get_widget!(builder, gtk::Button, "login-button"),
            note_list_box: note_list_box.clone(),
            note_popover: note_popover.clone(),
        };

        let mut controller = Controller::new(&builder);
        let mut config = Config::new()?;

        for identifier in config.identifiers() {
            let button = gtk::ModelButton::new();
            button.set_property_text(Some(&identifier));
            button.show();
            profile_menu.pack_end(&button, false, true, 0);

            button.connect_clicked(
                clone!(@strong sender => move |button| {
                    let identifier = button.get_property_text().unwrap().to_string();
                    sender.send(AppEvent::Switch(identifier)).unwrap();
                })
            );
        }

        let mut storage = match &config.identifier() {
            Some(identifier) => {
                if let Some(geometry) = &config.geometry {
                    application.restore_geometry(&geometry);
                }

                let server = config.server();
                let password = secret::load(&identifier, &server)?;
                let credentials = Credentials::from_defaults(&identifier, &password);

                if let Some(server) = server {
                    sender.send(AppEvent::SignIn(server.to_string(), credentials)).unwrap();
                }

                show_main_content(&builder);

                let credentials = config.credentials()?;
                let storage = Storage::new(&credentials, None)?;

                for item in storage.items.values() {
                    controller.insert(&item);
                }

                controller.select_first();

                Some(storage)
            }
            None => None
        };

        application.setup_overlay_help();
        application.setup_style_provider();
        application.setup_actions();
        application.setup_signals();
        application.setup_binds();

        let mut flush_timer_running = false;
        let mut title_entry_handler: Option<u64> = None;
        let mut text_buffer_handler: Option<u64> = None;

        receiver.attach(None,
            clone!(@strong sender, @strong app, @strong window => move |event| {
                match event {
                    AppEvent::Quit => {
                        if let Some(storage) = &mut storage {
                            if let Err(err) = storage.flush_dirty() {
                                g_error!(APP_DOMAIN, "Could not flush: {}", err);
                            }
                        }

                        if let Err(err) = config.write() {
                            g_warning!(APP_DOMAIN, "Could not write config: {}", err);
                        }

                        app.quit();
                    }
                    AppEvent::UpdateGeometry(geometry) => {
                        config.geometry = Some(geometry);
                    }
                    AppEvent::CreateStorage(user) => {
                        let credentials = Credentials::from_defaults(&user.identifier, &user.password);

                        match Storage::new(&credentials, None) {
                            Ok(s) => {
                                storage = Some(s);
                                config.add(&credentials, None);
                                if let Err(err) = secret::store(&credentials, None) {
                                    show_notification(&builder, &format!("{}", err));
                                }
                                else {
                                    controller.clear();
                                    show_main_content(&builder);
                                }
                            }
                            Err(message) => {
                                show_notification(&builder, &format!("Error: {}.", message));
                            }
                        };
                    }
                    AppEvent::Register(server, credentials) => {
                        g_info!(APP_DOMAIN, "Registering with {}", server);
                        let client = remote::Client::new_register(&server, credentials);

                        match client {
                            Ok(client) => {
                                let credentials = client.credentials.clone();
                                storage = Some(Storage::new(&credentials, Some(client)).unwrap());

                                if let Err(err) = secret::store(&credentials, Some(&server)) {
                                    show_notification(&builder, &format!("{}", err));
                                }
                                else {
                                    config.add(&credentials, Some(server));
                                    show_main_content(&builder);
                                }
                            }
                            Err(message) => {
                                let message = format!("Registration failed: {}.", message);
                                show_notification(&builder, &message);
                            }
                        };
                    }
                    AppEvent::SignIn(server, credentials) => {
                        g_info!(APP_DOMAIN, "Signing in to {}", server);
                        let client = remote::Client::new_sign_in(&server, &credentials);

                        match client {
                            Ok(client) => {
                                // We have to use the clients credentials because encryption
                                // parameters such as nonce and number of iterations might have
                                // changed.
                                let credentials = client.credentials.clone();

                                // Switch storage, read local files and show them in the UI.
                                storage = Some(Storage::new(&credentials, Some(client)).unwrap());

                                for item in storage.as_ref().unwrap().items.values() {
                                    controller.insert(&item);
                                }

                                // Store the encryption password and auth token in the keyring.
                                if let Err(err) = secret::store(&credentials, Some(&server)) {
                                    show_notification(&builder, &format!("{}", err));
                                }
                                else {
                                    config.add(&credentials, Some(server));
                                    show_main_content(&builder);
                                }
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

                                if let Err(err) = secret::store(&credentials, server.as_deref()) {
                                    show_notification(&builder, &format!("{}", err));
                                }

                                config.add(&credentials, server);
                                let new_storage = Storage::new_from_items(&credentials, &exported.items);

                                match new_storage {
                                    Err(err) => {
                                        let message = format!("Could not decrypt: {}", err);
                                        show_notification(&builder, &message);
                                    }
                                    Ok(s) => {
                                        for item in s.items.values() {
                                            controller.insert(&item);
                                        }

                                        storage = Some(s);
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
                    AppEvent::Export(path) => {
                        if let Some(storage) = &storage {
                            let exported = storage.export().unwrap();
                            std::fs::write(path, exported.to_str().unwrap()).unwrap();
                        }
                    }
                    AppEvent::Switch(identifier) => {
                        controller.clear();
                        config.switch(&identifier).unwrap();

                        // FIXME: do something about the unwraps
                        let credentials = config.credentials().unwrap();
                        let new_storage = Storage::new(&credentials, None).unwrap();

                        for item in new_storage.items.values() {
                            controller.insert(&item);
                        }

                        storage = Some(new_storage);
                    }
                    AppEvent::AddNote => {
                        if let Some(storage) = &mut storage {
                            let uuid = storage.create_note();
                            let item = storage.items.get(&uuid).unwrap();

                            controller.insert(&item);
                        }
                    }
                    AppEvent::DeleteNote => {
                        if let Some(storage) = &mut storage {
                            if let Some(uuid) = storage.current {
                                g_info!(APP_DOMAIN, "Deleting {}", uuid);
                                controller.delete(&uuid);
                                storage.delete(&uuid).unwrap();
                            }
                        }
                    }
                    AppEvent::SelectNote => {
                        let row = note_list_box.get_selected_row().unwrap();

                        if let Some(uuid) = controller.select(&row) {
                            if let Some(storage) = &mut storage {
                                storage.set_current_uuid(&uuid).unwrap();

                                // We first disconnect the change handlers before setting the text
                                // and content to avoid updating the storage and controller which would
                                // unnecessarily cause row movement and a server sync.

                                if let Some(handler) = title_entry_handler {
                                    title_entry.disconnect(from_glib(handler));
                                }

                                if let Some(handler) = text_buffer_handler {
                                    text_buffer.disconnect(from_glib(handler));
                                }

                                let title = storage.get_title().unwrap();
                                let text = storage.get_text().unwrap();

                                title_entry.set_text(&title);
                                text_buffer.set_text(&text);

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
                                storage.set_title(&title).unwrap();
                            }

                            if let Some(text) = text {
                                storage.set_text(&text).unwrap();
                            }

                            if let Some(uuid) = storage.current {
                                controller.updated(&uuid);
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
                        controller.filter_rows(term);
                    }
                    AppEvent::FlushDirty => {
                        if let Some(storage) = &mut storage {
                            if let Err(err) = storage.flush_dirty() {
                                g_error!(APP_DOMAIN, "Could not flush: {}", err);
                            }
                            else {
                                flush_timer_running = false;
                            }
                        }
                    }
                }

                glib::Continue(true)
            })
        );

        Ok(application)
    }

    pub fn run(&self) {
        let args: Vec<String> = env::args().collect();
        self.app.run(&args);
    }
}
