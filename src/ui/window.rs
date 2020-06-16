use std::collections::HashMap;
use crate::ui::state::{AppEvent, WindowEvent};
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
        let text_buffer = text_view.get_buffer().unwrap();

        let (win_sender, win_receiver) = glib::MainContext::channel::<WindowEvent>(glib::PRIORITY_DEFAULT);

        let mut current_binding: Option<glib::Binding> = None;
        let mut current_uuid: Option<Uuid> = None;
        let mut row_map: HashMap<gtk::ListBoxRow, (Uuid, gtk::Label)> = HashMap::new();

        search_bar.connect_entry(&search_entry);

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
                    WindowEvent::AddNote(uuid, title) => {
                        let label = gtk::Label::new(None);
                        label.set_halign(gtk::Align::Start);
                        label.set_margin_start(9);
                        label.set_margin_end(9);
                        label.set_margin_top(9);
                        label.set_margin_bottom(9);
                        label.set_widget_name("iridium-note-row-label");
                        label.set_text(title.as_str());

                        let row_widget = gtk::ListBoxRow::new();
                        row_widget.add(&label);
                        row_widget.set_widget_name("iridium-note-row");
                        row_widget.show_all();
                        note_list_box.add(&row_widget);

                        note_list_box.select_row(Some(&row_widget));
                        title_entry.grab_focus();
                        row_map.insert(row_widget, (uuid, label));
                        current_uuid = Some(uuid);
                    },
                    WindowEvent::SelectNote(row) => {
                        if let Some(binding) = &current_binding {
                            binding.unbind();
                        }

                        if let Some((uuid, label)) = row_map.get(&row) {
                            app_sender.send(AppEvent::SelectNote(*uuid)).unwrap();
                            current_binding = Some(title_entry.bind_property("text", label, "label").build().unwrap());
                            current_uuid = Some(*uuid);
                        }
                    },
                    WindowEvent::UpdateFilter(text) => {
                        match text {
                            Some(_) => {
                                note_list_box.set_filter_func(Some(Box::new(|_| -> bool {
                                    true
                                })));
                            },
                            None => {
                                note_list_box.set_filter_func(None);
                            }
                        }
                    },
                    WindowEvent::UpdateTitle => {
                        if let Some(uuid) = current_uuid {
                            // Should we actually get that from the model?
                            let text = title_entry.get_text().unwrap();
                            let text = text.as_str();
                            app_sender.send(AppEvent::UpdateTitle(uuid.clone(), text.to_owned())).unwrap();
                        }
                    },
                    WindowEvent::UpdateText => {
                        if let Some(uuid) = current_uuid {
                            let start = text_buffer.get_start_iter();
                            let end = text_buffer.get_end_iter();
                            let text = text_buffer.get_text(&start, &end, false).unwrap();
                            let text = text.as_str();
                            app_sender.send(AppEvent::UpdateText(uuid.clone(), text.to_owned())).unwrap();
                        }
                    },
                    WindowEvent::ToggleSearchBar => {
                        search_bar.set_search_mode(!search_bar.get_search_mode());
                    },
                    WindowEvent::ShowNotification(message) => {
                        let revealer = builder.get_object::<gtk::Revealer>("iridium-notification-revealer").unwrap();
                        let label = builder.get_object::<gtk::Label>("iridium-notification-label").unwrap();
                        label.set_text(message.as_str());
                        revealer.set_reveal_child(true);
                    },
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
