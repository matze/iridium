use crate::consts::{SHORTCUTS_UI, WINDOW_UI, BASE_CSS};
use crate::ui::state::{AppEvent, WindowEvent, User, RemoteAuth};
use crate::ui::model::Model;
use gtk::prelude::*;

pub struct Window {
    pub widget: gtk::ApplicationWindow,
    pub sender: glib::Sender<WindowEvent>,
}

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

fn setup_style_provider(window: &gtk::ApplicationWindow) {
    let style_provider = gtk::CssProvider::new();
    style_provider.load_from_resource(BASE_CSS);

    gtk::StyleContext::add_provider_for_screen(
        &window.get_screen().unwrap(),
        &style_provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

impl Window {
    pub fn new(app_sender: glib::Sender<AppEvent>) -> Self {
        let builder = gtk::Builder::new_from_resource(WINDOW_UI);
        let window: gtk::ApplicationWindow = builder.get_object("window").unwrap();

        window.set_help_overlay(Some(&get_shortcuts_window()));
        setup_style_provider(&window);

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
        let mut model = Model::new(note_list_box.clone(), title_entry.clone());

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
            clone!(@strong text_buffer, @strong builder => move |event| {
                match event {
                    WindowEvent::ShowMainContent => {
                        let stack = get_widget!(builder, gtk::Stack, "iridium-main-stack");
                        let main_box = get_widget!(builder, gtk::Box, "iridium-main-content");
                        stack.set_visible_child(&main_box);

                        // Do not show the right hand pane until we have a note to show.
                        if model.is_empty() {
                            right_hand_stack.set_visible_child(&right_hand_info);
                        }
                    }
                    WindowEvent::AddNote(uuid, title) => {
                        model.insert(&uuid, &title);
                        right_hand_stack.set_visible_child(&note_pane_box);
                        title_entry.grab_focus();
                    }
                    WindowEvent::DeleteNote(uuid) => {
                        model.delete(&uuid);

                        if model.is_empty() {
                            right_hand_stack.set_visible_child(&right_hand_info);
                        }
                    }
                    WindowEvent::SelectNote(row) => {
                        if let Some(uuid) = model.select(&row) {
                            app_sender.send(AppEvent::SelectNote(uuid)).unwrap();
                        }
                    }
                    WindowEvent::UpdateFilter(term) => {
                        if let Some(term) = term {
                            let term = term.to_lowercase();
                            model.show_matching_rows(&term);
                        }
                        else {
                            model.show_all_rows();
                        }
                    }
                    WindowEvent::UpdateTitle => {
                        let title = title_entry.get_text().unwrap();
                        let title = title.as_str().to_string();
                        app_sender.send(AppEvent::Update(Some(title), None)).unwrap();
                    }
                    WindowEvent::UpdateText => {
                        let start = text_buffer.get_start_iter();
                        let end = text_buffer.get_end_iter();
                        let text = text_buffer.get_text(&start, &end, false).unwrap();
                        let text = text.as_str().to_string();
                        app_sender.send(AppEvent::Update(None, Some(text))).unwrap();
                    }
                    WindowEvent::UpdateNote(title, content) => {
                        title_entry.set_text(&title);
                        text_buffer.set_text(&content);
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
        }
    }
}
