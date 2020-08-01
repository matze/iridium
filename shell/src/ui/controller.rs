use chrono::{DateTime, Utc};
use gio::prelude::*;
use gtk::prelude::*;
use standardfile::Note;
use std::cmp;
use uuid::Uuid;

struct Item {
    uuid: Uuid,
    row: gtk::ListBoxRow,
    label: gtk::Label,
    last_updated: DateTime<Utc>,
}

pub struct Controller {
    items: Vec<Item>,
    list_box: gtk::ListBox,
    title_entry: gtk::Entry,
    note_stack: gtk::Stack,
    note_info: gtk::Label,
    note_content: gtk::Box,
    binding: Option<glib::Binding>,
}

impl Controller {
    pub fn new(builder: &gtk::Builder) -> Self {
        Self {
            items: Vec::new(),
            list_box: get_widget!(builder, gtk::ListBox, "iridium-note-list"),
            title_entry: get_widget!(builder, gtk::Entry, "iridium-title-entry"),
            note_stack: get_widget!(builder, gtk::Stack, "right-hand-stack"),
            note_info: get_widget!(builder, gtk::Label, "right-hand-info-label"),
            note_content: get_widget!(builder, gtk::Box, "iridium-entry-box"),
            binding: None,
        }
    }

    pub fn insert(&mut self, note: &Note) {
        if self.have(&note.uuid) {
            return;
        }

        let label = gtk::Label::new(None);
        label.set_halign(gtk::Align::Start);
        label.set_margin_start(9);
        label.set_margin_end(9);
        label.set_margin_top(9);
        label.set_margin_bottom(9);
        label.set_widget_name("iridium-note-row-label");
        label.set_text(&note.title);

        let row = gtk::ListBoxRow::new();
        row.add(&label);
        row.set_widget_name("iridium-note-row");
        row.show_all();

        // Do stupid insertion sort until we figured out how gtk::ListBox::set_sort_func's closure
        // could use the model itself.
        let mut position = self.items.len();

        for (i, item) in self.items.iter().enumerate() {
            if item.last_updated < note.updated_at {
                position = i;
                break;
            }
        }

        self.list_box.insert(&row, position as i32);
        self.list_box.select_row(Some(&row));

        self.items.insert(position, Item {
            uuid: note.uuid,
            row: row.clone(),
            label: label.clone(),
            last_updated: note.updated_at,
        });

        if self.items.len() == 1 {
            self.note_stack.set_visible_child(&self.note_content);
        }
    }

    pub fn delete(&mut self, uuid: &Uuid) {
        let mut index = 0;

        for item in self.items.iter().filter(|item| item.uuid == *uuid) {
            index = cmp::max(0, item.row.get_index() - 1);
            self.list_box.remove(&item.row);
        }

        self.items.retain(|item| item.uuid != *uuid);

        if self.items.len() > 0 {
            let new_selected_row = self.list_box.get_row_at_index(index).unwrap();
            self.list_box.select_row(Some(&new_selected_row));
        }
        else {
            self.note_stack.set_visible_child(&self.note_info);
        }
    }

    pub fn clear(&mut self) {
        if let Some(binding) = &self.binding {
            binding.unbind();
        }

        for item in &self.items {
            self.list_box.remove(&item.row);
        }

        self.items.clear();
        self.note_stack.set_visible_child(&self.note_info);
    }

    pub fn select(&mut self, selected_row: &gtk::ListBoxRow) -> Option<Uuid> {
        if let Some(binding) = &self.binding {
            binding.unbind();
        }

        for item in self.items.iter().filter(|item| item.row == *selected_row) {
            self.binding = Some(self.title_entry.bind_property("text", &item.label, "label").build().unwrap());
            return Some(item.uuid);
        }

        None
    }

    pub fn updated(&mut self, uuid: &Uuid) {
        for item in self.items.iter_mut().filter(|item| item.uuid == *uuid) {
            item.last_updated = Utc::now();

            if item.row.get_index() > 0 {
                self.list_box.remove(&item.row);
                self.list_box.insert(&item.row, 0);
            }
        }
    }

    pub fn show_matching_rows(&self, term: &str) {
        for item in &self.items {
            let label_text = item.label.get_text().to_string().to_lowercase();

            if label_text.contains(&term) {
                item.row.show();
            }
            else {
                item.row.hide();
            }
        }
    }

    pub fn show_all_rows(&self) {
        for item in &self.items {
            item.row.show();
        }
    }

    fn have(&self, uuid: &Uuid) -> bool {
        self.items.iter().any(|item| item.uuid == *uuid)
    }
}
