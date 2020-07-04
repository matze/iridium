use gio::prelude::*;
use gtk::prelude::*;
use std::cmp;
use uuid::Uuid;

pub struct Model {
    items: Vec<(gtk::ListBoxRow, Uuid, gtk::Label)>,
    list_box: gtk::ListBox,
    title_entry: gtk::Entry,
    binding: Option<glib::Binding>,
}

impl Model {
    pub fn new(list_box: gtk::ListBox, title_entry: gtk::Entry) -> Self {
        Self {
            items: Vec::new(),
            list_box: list_box,
            title_entry: title_entry,
            binding: None,
        }
    }

    pub fn insert(&mut self, uuid: &Uuid, title: &str) {
        if self.have(uuid) {
            return;
        }

        let label = gtk::Label::new(None);
        label.set_halign(gtk::Align::Start);
        label.set_margin_start(9);
        label.set_margin_end(9);
        label.set_margin_top(9);
        label.set_margin_bottom(9);
        label.set_widget_name("iridium-note-row-label");
        label.set_text(&title);

        let row = gtk::ListBoxRow::new();
        row.add(&label);
        row.set_widget_name("iridium-note-row");
        row.show_all();

        self.items.push((row.clone(), *uuid, label.clone()));

        self.list_box.add(&row);
        self.list_box.select_row(Some(&row));
    }

    pub fn delete(&mut self, uuid: &Uuid) {
        let mut index = 0;

        for (row, row_uuid, _) in &self.items {
            if row_uuid == uuid {
                index = cmp::max(0, row.get_index() - 1);
                self.list_box.remove(row);
            }
        }

        self.items.retain(|(_, row_uuid, _)| uuid != row_uuid);

        if self.items.len() > 0 {
            let new_selected_row = self.list_box.get_row_at_index(index).unwrap();
            self.list_box.select_row(Some(&new_selected_row));
        }
    }

    pub fn select(&mut self, selected_row: &gtk::ListBoxRow) -> Option<Uuid> {
        if let Some(binding) = &self.binding {
            binding.unbind();
        }

        for (row, uuid, label) in &self.items {
            if row == selected_row {
                self.binding = Some(self.title_entry.bind_property("text", label, "label").build().unwrap());
                return Some(uuid.clone());
            }
        }

        None
    }

    pub fn is_empty(&self) -> bool {
        self.items.len() == 0
    }

    pub fn show_matching_rows(&self, term: &str) {
        for (row, _, label) in &self.items {
            let label_text = label.get_text().unwrap().to_string().to_lowercase();

            if label_text.contains(&term) {
                row.show();
            }
            else {
                row.hide();
            }
        }
    }

    pub fn show_all_rows(&self) {
        for (row, _, _) in &self.items {
            row.show();
        }
    }

    fn have(&self, uuid: &Uuid) -> bool {
        self.items.iter().any(|item| item.1 == *uuid)
    }
}

