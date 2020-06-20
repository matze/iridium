use std::collections::HashMap;
use crate::ui::state::{AppEvent, WindowEvent, User, RemoteAuth};
use gio::prelude::*;
use gtk::prelude::*;
use uuid::Uuid;

pub struct Window {
    pub widget: gtk::ApplicationWindow,
    pub sender: glib::Sender<WindowEvent>,
    text_buffer: gtk::TextBuffer,
    title_entry: gtk::Entry,
}

fn get_shortcuts_window() -> gtk::ShortcutsWindow {
    let builder = gtk::Builder::new_from_resource("/net/bloerg/Iridium/data/resources/ui/shortcuts.ui");
    builder.get_object("shortcuts").unwrap()
}

fn get_user_details(builder: &gtk::Builder) -> User {
    let identifier_entry = builder.get_object::<gtk::Entry>("identifier-entry").unwrap();
    let password_entry = builder.get_object::<gtk::Entry>("password-entry").unwrap();

    User {
        identifier: identifier_entry.get_text().unwrap().to_string(),
        password: password_entry.get_text().unwrap().to_string(),
    }
}

fn get_auth_details(builder: &gtk::Builder) -> RemoteAuth {
    let server_combo_box = builder.get_object::<gtk::ComboBoxText>("server-combo").unwrap();

    RemoteAuth {
        server: server_combo_box.get_active_text().unwrap().to_string(),
        user: get_user_details(&builder),
    }
}

fn new_note_row(title: &str) -> (gtk::ListBoxRow, gtk::Label) {
    let label = gtk::Label::new(None);
    label.set_halign(gtk::Align::Start);
    label.set_margin_start(9);
    label.set_margin_end(9);
    label.set_margin_top(9);
    label.set_margin_bottom(9);
    label.set_widget_name("iridium-note-row-label");
    label.set_text(&title);

    let row_widget = gtk::ListBoxRow::new();
    row_widget.add(&label);
    row_widget.set_widget_name("iridium-note-row");
    row_widget.show_all();
    (row_widget, label)
}

impl Window {
    pub fn new(app_sender: glib::Sender<AppEvent>) -> Self {
        let builder =
            gtk::Builder::new_from_resource("/net/bloerg/Iridium/data/resources/ui/window.ui");
        let window: gtk::ApplicationWindow = builder.get_object("window").unwrap();

        window.set_help_overlay(Some(&get_shortcuts_window()));

        let style_provider = gtk::CssProvider::new();
        style_provider.load_from_resource("/net/bloerg/Iridium/data/resources/css/base.css");
        gtk::StyleContext::add_provider_for_screen(
            &window.get_screen().unwrap(),
            &style_provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        let note_list_box = builder.get_object::<gtk::ListBox>("iridium-note-list").unwrap();
        let title_entry = builder.get_object::<gtk::Entry>("iridium-title-entry").unwrap();
        let search_bar = builder.get_object::<gtk::SearchBar>("iridium-search-bar").unwrap();
        let search_entry = builder.get_object::<gtk::SearchEntry>("iridium-search-entry").unwrap();
        let text_view = builder.get_object::<gtk::TextView>("iridium-text-view").unwrap();
        let identifier_entry = builder.get_object::<gtk::Entry>("identifier-entry").unwrap();
        let local_button = builder.get_object::<gtk::Button>("create-local-button").unwrap();
        let signup_button = builder.get_object::<gtk::Button>("signup-button").unwrap();
        let login_button = builder.get_object::<gtk::Button>("login-button").unwrap();
        let text_buffer = text_view.get_buffer().unwrap();

        let (win_sender, win_receiver) = glib::MainContext::channel::<WindowEvent>(glib::PRIORITY_DEFAULT);

        let mut current_binding: Option<glib::Binding> = None;
        let mut current_uuid: Option<Uuid> = None;
        let mut row_map: HashMap<gtk::ListBoxRow, (Uuid, gtk::Label)> = HashMap::new();

        search_bar.connect_entry(&search_entry);

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
            clone!(@strong builder, @strong app_sender as sender => move |_| {
                let main_box = builder.get_object::<gtk::Box>("iridium-main-content").unwrap();
                let stack = builder.get_object::<gtk::Stack>("iridium-main-stack").unwrap();
                stack.set_visible_child(&main_box);

                let user = get_user_details(&builder);
                sender.send(AppEvent::CreateStorage(user)).unwrap();
            })
        );

        signup_button.connect_clicked(
            clone!(@strong builder, @strong app_sender as sender => move |_| {
                sender.send(AppEvent::Register(get_auth_details(&builder))).unwrap();
            })
        );

        login_button.connect_clicked(
            clone!(@strong builder, @strong app_sender as sender => move |_| {
                sender.send(AppEvent::SignIn(get_auth_details(&builder))).unwrap();
            })
        );

        search_entry.connect_search_changed(
            clone!(@weak search_entry, @strong win_sender => move |_| {
                let text = search_entry.get_text().unwrap();

                if text != "" {
                    win_sender.send(WindowEvent::UpdateFilter(Some(text.as_str().to_string()))).unwrap();
                }
                else {
                    win_sender.send(WindowEvent::UpdateFilter(None)).unwrap();
                }
            })
        );

        title_entry.connect_changed(
            clone!(@strong win_sender as sender => move|_| {
                sender.send(WindowEvent::UpdateTitle).unwrap();
            })
        );

        text_buffer.connect_changed(
            clone!(@strong win_sender as sender => move|_| {
                sender.send(WindowEvent::UpdateText).unwrap();
            })
        );

        note_list_box.connect_row_selected(
            clone!(@strong win_sender as sender => move |_, row| {
                if let Some(row) = row {
                    sender.send(WindowEvent::SelectNote(row.clone())).unwrap();
                }
            })
        );

        win_receiver.attach(None,
            clone!(@strong note_list_box, @strong text_buffer, @strong builder => move |event| {
                match event {
                    WindowEvent::ShowMainContent => {
                        let stack = builder.get_object::<gtk::Stack>("iridium-main-stack").unwrap();
                        let main_box = builder.get_object::<gtk::Box>("iridium-main-content").unwrap();
                        stack.set_visible_child(&main_box);
                    }
                    WindowEvent::AddNote(uuid, title) => {
                        let (row, label) = new_note_row(&title);
                        note_list_box.add(&row);

                        note_list_box.select_row(Some(&row));
                        title_entry.grab_focus();
                        row_map.insert(row, (uuid, label));
                        current_uuid = Some(uuid);
                    }
                    WindowEvent::SelectNote(row) => {
                        if let Some(binding) = &current_binding {
                            binding.unbind();
                        }

                        if let Some((uuid, label)) = row_map.get(&row) {
                            app_sender.send(AppEvent::SelectNote(*uuid)).unwrap();
                            current_binding = Some(title_entry.bind_property("text", label, "label").build().unwrap());
                            current_uuid = Some(*uuid);
                        }
                    }
                    WindowEvent::UpdateFilter(text) => {
                        match text {
                            Some(_) => {
                                note_list_box.set_filter_func(Some(Box::new(|_| -> bool {
                                    true
                                })));
                            }
                            None => {
                                note_list_box.set_filter_func(None);
                            }
                        }
                    }
                    WindowEvent::UpdateTitle => {
                        if let Some(uuid) = current_uuid {
                            let title = title_entry.get_text().unwrap();
                            let title = title.as_str().to_string();
                            app_sender.send(AppEvent::Update(uuid, Some(title), None)).unwrap();
                        }
                    }
                    WindowEvent::UpdateText => {
                        if let Some(uuid) = current_uuid {
                            let start = text_buffer.get_start_iter();
                            let end = text_buffer.get_end_iter();
                            let text = text_buffer.get_text(&start, &end, false).unwrap();
                            let text = text.as_str().to_string();
                            app_sender.send(AppEvent::Update(uuid, None, Some(text))).unwrap();
                        }
                    }
                    WindowEvent::ToggleSearchBar => {
                        search_bar.set_search_mode(!search_bar.get_search_mode());
                    }
                    WindowEvent::ShowNotification(message) => {
                        let revealer = builder.get_object::<gtk::Revealer>("iridium-notification-revealer").unwrap();
                        let label = builder.get_object::<gtk::Label>("iridium-notification-label").unwrap();
                        let close_button = builder.get_object::<gtk::Button>("iridium-notification-button").unwrap();

                        label.set_text(&message);
                        revealer.set_reveal_child(true);

                        close_button.connect_clicked(move |_| {
                            revealer.set_reveal_child(false);
                        });
                    }
                }

                glib::Continue(true)
            })
        );

        Window {
            widget: window,
            sender: win_sender,
            text_buffer: text_buffer,
            title_entry: builder.get_object("iridium-title-entry").unwrap(),
        }
    }

    pub fn load_note(&self, title: &str, content: &str) {
        self.title_entry.set_text(title);
        self.text_buffer.set_text(content);
    }
}
