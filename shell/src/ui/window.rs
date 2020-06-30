use crate::consts::{SHORTCUTS_UI, WINDOW_UI};
use crate::ui::state::{AppEvent, WindowEvent, User, RemoteAuth};
use gio::prelude::*;
use gtk::prelude::*;
use std::cmp;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

pub struct Window {
    pub widget: gtk::ApplicationWindow,
    pub sender: glib::Sender<WindowEvent>,
    text_buffer: gtk::TextBuffer,
    title_entry: gtk::Entry,
}

type RowMap = HashMap<gtk::ListBoxRow, (Uuid, gtk::Label)>;

fn get_shortcuts_window() -> gtk::ShortcutsWindow {
    let builder = gtk::Builder::new_from_resource(SHORTCUTS_UI);
    builder.get_object("shortcuts").unwrap()
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

fn show_matching_rows(row_map: &RowMap, note_list_box: &gtk::ListBox, term: &str) {
    let count = row_map.len();

    for index in 0..count as i32 {
        let row = note_list_box.get_row_at_index(index).unwrap();

        if let Some((_, label)) = row_map.get(&row) {
            let label_text = label.get_text().unwrap().to_string().to_lowercase();

            if label_text.contains(&term) {
                row.show();
            }
            else {
                row.hide();
            }
        }
    }
}

fn show_all_rows(row_map: &RowMap, note_list_box: &gtk::ListBox) {
    let count = row_map.len();

    for index in 0..count as i32 {
        let row = note_list_box.get_row_at_index(index).unwrap();
        row.show();
    }
}

impl Window {
    pub fn new(app_sender: glib::Sender<AppEvent>) -> Self {
        let builder =
            gtk::Builder::new_from_resource(WINDOW_UI);
        let window: gtk::ApplicationWindow = builder.get_object("window").unwrap();

        window.set_help_overlay(Some(&get_shortcuts_window()));

        let style_provider = gtk::CssProvider::new();
        style_provider.load_from_resource("/net/bloerg/Iridium/data/resources/css/base.css");
        gtk::StyleContext::add_provider_for_screen(
            &window.get_screen().unwrap(),
            &style_provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        let note_list_box = get_widget!(builder, gtk::ListBox, "iridium-note-list");
        let title_entry = get_widget!(builder, gtk::Entry, "iridium-title-entry");
        let search_bar = get_widget!(builder, gtk::SearchBar, "iridium-search-bar");
        let search_entry = get_widget!(builder, gtk::SearchEntry, "iridium-search-entry");
        let text_view = get_widget!(builder, gtk::TextView, "iridium-text-view");
        let identifier_entry = get_widget!(builder, gtk::Entry, "identifier-entry");
        let local_button = get_widget!(builder, gtk::Button, "create-local-button");
        let signup_button = get_widget!(builder, gtk::Button, "signup-button");
        let login_button = get_widget!(builder, gtk::Button, "login-button");
        let text_buffer = text_view.get_buffer().unwrap();
        let note_popover = get_widget!(builder, gtk::PopoverMenu, "note_menu");

        let right_hand_stack = get_widget!(builder, gtk::Stack, "right-hand-stack");
        let right_hand_info = get_widget!(builder, gtk::Label, "right-hand-info-label");
        let note_pane_box = get_widget!(builder, gtk::Box, "iridium-entry-box");

        let (win_sender, win_receiver) = glib::MainContext::channel::<WindowEvent>(glib::PRIORITY_DEFAULT);

        // This auxiliary variable helps us break the binding between the title entry widget and
        // the selected listbox row.
        let mut current_binding: Option<glib::Binding> = None;
        let mut current_uuid: Option<Uuid> = None;
        let mut known_uuids: HashSet<Uuid> = HashSet::new();
        let mut row_map: RowMap = HashMap::new();

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
                let main_box = get_widget!(builder, gtk::Box, "iridium-main-content");
                let stack = get_widget!(builder, gtk::Stack, "iridium-main-stack");
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
                if let Some(text) = search_entry.get_text() {
                    if text.len() > 2 {
                        win_sender.send(WindowEvent::UpdateFilter(Some(text.as_str().to_string()))).unwrap();
                    }
                    else {
                        win_sender.send(WindowEvent::UpdateFilter(None)).unwrap();
                    }
                }
            })
        );

        title_entry.connect_changed(
            clone!(@strong win_sender as sender => move |_| {
                sender.send(WindowEvent::UpdateTitle).unwrap();
            })
        );

        text_buffer.connect_changed(
            clone!(@strong win_sender as sender => move |_| {
                sender.send(WindowEvent::UpdateText).unwrap();
            })
        );

        note_list_box.connect_row_selected(
            clone!(@strong win_sender as sender, @strong note_popover => move |_, row| {
                if let Some(row) = row {
                    note_popover.set_relative_to(Some(row));
                    sender.send(WindowEvent::SelectNote(row.clone())).unwrap();
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

        win_receiver.attach(None,
            clone!(@strong note_list_box, @strong text_buffer, @strong builder => move |event| {
                match event {
                    WindowEvent::ShowMainContent => {
                        let stack = get_widget!(builder, gtk::Stack, "iridium-main-stack");
                        let main_box = get_widget!(builder, gtk::Box, "iridium-main-content");
                        stack.set_visible_child(&main_box);

                        // Do not show the right hand pane until we have a note to show.
                        if known_uuids.len() == 0 {
                            right_hand_stack.set_visible_child(&right_hand_info);
                        }
                    }
                    WindowEvent::AddNote(uuid, title) => {
                        right_hand_stack.set_visible_child(&note_pane_box);

                        if !known_uuids.contains(&uuid) {
                            let (row, label) = new_note_row(&title);
                            note_list_box.add(&row);

                            note_list_box.select_row(Some(&row));
                            title_entry.grab_focus();
                            row_map.insert(row, (uuid, label));
                            current_uuid = Some(uuid);
                            known_uuids.insert(uuid);
                        }
                    }
                    WindowEvent::DeleteNote(uuid) => {
                        known_uuids.remove(&uuid);

                        // Feels dirty to get the index of the previous row but okay ...
                        let mut index = 0;

                        // Disgusting but works for now ...
                        for row in row_map.iter()
                            .filter(|&(_, (row_uuid, _))| uuid == *row_uuid)
                            .map(|(row, _)| row) {
                                index = cmp::max(0, row.get_index() - 1);
                                note_list_box.remove(row);
                        }

                        // If we have no more notes, hide the note entry part otherwise switch to
                        // the previous note.
                        if known_uuids.len() == 0 {
                            right_hand_stack.set_visible_child(&right_hand_info);
                        }
                        else {
                            let new_row = note_list_box.get_row_at_index(index).unwrap();
                            note_list_box.select_row(Some(&new_row));
                        }
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
                    WindowEvent::UpdateFilter(term) => {
                        if let Some(term) = term {
                            let term = term.to_lowercase();
                            show_matching_rows(&row_map, &note_list_box, &term);
                        }
                        else {
                            show_all_rows(&row_map, &note_list_box);
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
                        let revealer = get_widget!(builder, gtk::Revealer, "iridium-notification-revealer");
                        let label = get_widget!(builder, gtk::Label, "iridium-notification-label");
                        let close_button = get_widget!(builder, gtk::Button, "iridium-notification-button");

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
