use chrono::{DateTime, Utc};
use gio::prelude::*;
use gtk::prelude::*;
use standardfile::Item as StandardItem;
use std::{cell::RefCell, cmp, cmp::{Ord, Ordering}, collections::HashMap, rc::Rc};
use uuid::Uuid;

struct Item {
    uuid: Uuid,
    label: gtk::Label,
    last_updated: DateTime<Utc>,
}

pub struct Controller {
    items: Rc<RefCell<HashMap<gtk::ListBoxRow, Item>>>,
    list_box: gtk::ListBox,
    title_entry: gtk::Entry,
    note_stack: gtk::Stack,
    note_info: gtk::Label,
    note_content: gtk::Box,
    binding: Option<glib::Binding>,
}

impl Ord for Item {
    fn cmp(&self, other: &Self) -> Ordering {
        self.last_updated.cmp(&other.last_updated)
    }
}

impl PartialOrd for Item {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Item {
    fn eq(&self, other: &Self) -> bool {
        self.last_updated == other.last_updated
    }
}

impl Eq for Item {}

impl Controller {
    pub fn new(builder: &gtk::Builder) -> Self {
        let controller = Self {
            items: Rc::new(RefCell::new(HashMap::new())),
            list_box: get_widget!(builder, gtk::ListBox, "iridium-note-list"),
            title_entry: get_widget!(builder, gtk::Entry, "iridium-title-entry"),
            note_stack: get_widget!(builder, gtk::Stack, "right-hand-stack"),
            note_info: get_widget!(builder, gtk::Label, "right-hand-info-label"),
            note_content: get_widget!(builder, gtk::Box, "iridium-entry-box"),
            binding: None,
        };

        controller.list_box.set_sort_func(Some(Box::new(
            clone!(@strong controller.items as items => move |row_a, row_b| {
                let items = items.borrow();
                let item_a = &items[row_a];
                let item_b = &items[row_b];
                (item_a < item_b) as i32
            })
        )));

        controller
    }

    pub fn insert(&mut self, item: &StandardItem) {
        if let StandardItem::Note(note) = item {
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

            {
                let mut items = self.items.borrow_mut();

                items.insert(row.clone(), Item {
                    uuid: note.uuid,
                    label: label.clone(),
                    last_updated: note.updated_at,
                });

                if items.len() == 1 {
                    self.note_stack.set_visible_child(&self.note_content);
                }
            }

            self.list_box.insert(&row, 0);
            self.list_box.select_row(Some(&row));
        }
    }

    pub fn delete(&mut self, uuid: &Uuid) {
        let mut index = 0;
        let mut items = self.items.borrow_mut();

        for (row, _) in items.iter().filter(|&(_, item)| item.uuid == *uuid) {
            index = cmp::max(0, row.get_index() - 1);
            self.list_box.remove(row);
        }

        items.retain(|_, item| item.uuid != *uuid);

        if items.len() > 0 {
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

        let mut items = self.items.borrow_mut();

        for row in items.keys() {
            self.list_box.remove(row);
        }

        items.clear();
        self.note_stack.set_visible_child(&self.note_info);
    }

    pub fn select_first(&self) {
        let items = self.items.borrow();
        let most_recent = items.iter().max_by(|(_, x), (_, y)| x.cmp(y));

        if let Some((row, _)) = most_recent {
            self.list_box.select_row(Some(row));
        }
    }

    pub fn select(&mut self, selected_row: &gtk::ListBoxRow) -> Option<Uuid> {
        if let Some(binding) = &self.binding {
            binding.unbind();
        }

        if let Some(item) = self.items.borrow().get(selected_row) {
            self.binding = Some(self.title_entry.bind_property("text", &item.label, "label").build().unwrap());
            return Some(item.uuid);
        }

        None
    }

    pub fn updated(&mut self, uuid: &Uuid) {
        for item in self.items.borrow_mut()
            .iter_mut()
            .filter(|(_, item)| item.uuid == *uuid)
            .map(|(_, item)| item) {
            item.last_updated = Utc::now();
        }

        for row in self.items.borrow()
            .iter()
            .filter(|(row, item)| item.uuid == *uuid && row.get_index() > 0)
            .map(|(row, _)| row) {
            self.list_box.remove(row);
            self.list_box.insert(row, 0);
        }
    }

    pub fn filter_rows(&self, term: Option<String>) {
        if let Some(term) = term {
            self.list_box.set_filter_func(Some(Box::new(
                clone!(@strong self.items as items => move |row| {
                    let items = items.borrow();
                    let label_text = items[row].label.get_text().to_string().to_lowercase();
                    label_text.contains(&term)
                })
            )));
        }
        else {
            self.list_box.set_filter_func(None);
        }
    }

    fn have(&self, uuid: &Uuid) -> bool {
        self.items.borrow().iter().any(|(_, item)| item.uuid == *uuid)
    }
}
