use crate::ui::state::{AppEvent, WindowEvent};
use gio::prelude::*;
use gtk::prelude::*;
use uuid::Uuid;

use row_data::RowData;

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

        let row_model = gio::ListStore::new(RowData::static_type());
        let note_list_box = builder.get_object::<gtk::ListBox>("iridium-note-list").unwrap();
        let title_entry = builder.get_object::<gtk::Entry>("iridium-title-entry").unwrap();
        let search_bar = builder.get_object::<gtk::SearchBar>("iridium-search-bar").unwrap();
        let search_entry = builder.get_object::<gtk::SearchEntry>("iridium-search-entry").unwrap();
        let text_view = builder.get_object::<gtk::TextView>("iridium-text-view").unwrap();
        let text_buffer = text_view.get_buffer().unwrap();

        let (win_sender, win_receiver) = glib::MainContext::channel::<WindowEvent>(glib::PRIORITY_DEFAULT);

        let mut current_binding: Option<glib::Binding> = None;
        let mut current_uuid: Option<Uuid> = None;

        search_bar.connect_entry(&search_entry);

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
                    sender.send(WindowEvent::SelectNote(row.get_index())).unwrap();
                }
            })
        );

        note_list_box.bind_model(Some(&row_model),
            clone!(@weak window => @default-panic, move |item| {
                let item = item.downcast_ref::<RowData>().unwrap();

                let label = gtk::Label::new(None);
                label.set_halign(gtk::Align::Start);
                label.set_margin_start(9);
                label.set_margin_end(9);
                label.set_margin_top(9);
                label.set_margin_bottom(9);
                label.set_widget_name("iridium-note-row-label");

                item.bind_property("title", &label, "label")
                    .flags(glib::BindingFlags::DEFAULT |
                           glib::BindingFlags::SYNC_CREATE |
                           glib::BindingFlags::BIDIRECTIONAL)
                    .build();

                let row_widget = gtk::ListBoxRow::new();
                row_widget.add(&label);
                row_widget.set_widget_name("iridium-note-row");
                row_widget.show_all();
                row_widget.upcast::<gtk::Widget>()
            }),
        );

        win_receiver.attach(None,
            clone!(@strong note_list_box, @strong text_buffer => move |event| {
                match event {
                    WindowEvent::AddNote(uuid, title) => {
                        let row = RowData::new(
                            title.as_str(),
                            uuid.to_hyphenated().to_string().as_str()
                        );
                        row_model.append(&row);

                        let row = note_list_box.get_row_at_index((row_model.get_n_items() - 1) as i32).unwrap();
                        note_list_box.select_row(Some(&row));
                        current_uuid = Some(uuid);
                    },
                    WindowEvent::SelectNote(row_index) => {
                        if let Some(binding) = &current_binding {
                            binding.unbind();
                        }

                        let item = row_model.get_object(row_index as u32).unwrap();
                        let item = item.downcast_ref::<RowData>().unwrap();
                        let uuid = item.get_property("uuid").unwrap().get::<String>().unwrap().unwrap();
                        let binding = title_entry.bind_property("text", item, "title").build();
                        current_binding = Some(binding.unwrap());
                        current_uuid = Some(Uuid::parse_str(uuid.as_str()).unwrap());
                        app_sender.send(AppEvent::SelectNote(uuid)).unwrap();
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

mod row_data {
    use super::*;

    use glib::subclass;
    use glib::subclass::prelude::*;
    use glib::translate::*;

    mod imp {
        use super::*;
        use std::cell::RefCell;

        pub struct RowData {
            title: RefCell<Option<String>>,
            uuid: RefCell<Option<String>>,
        }

        static PROPERTIES: [subclass::Property; 2] = [
            subclass::Property("title", |title| {
                glib::ParamSpec::string(title, "Title", "Title", None, glib::ParamFlags::READWRITE)
            }),
            subclass::Property("uuid", |title| {
                glib::ParamSpec::string(title, "UUID", "UUID", None, glib::ParamFlags::READWRITE)
            }),
        ];

        impl ObjectSubclass for RowData {
            const NAME: &'static str = "RowData";
            type ParentType = glib::Object;
            type Instance = subclass::simple::InstanceStruct<Self>;
            type Class = subclass::simple::ClassStruct<Self>;

            glib_object_subclass!();

            fn class_init(klass: &mut Self::Class) {
                klass.install_properties(&PROPERTIES);
            }

            fn new() -> Self {
                Self {
                    title: RefCell::new(None),
                    uuid: RefCell::new(None),
                }
            }
        }

        impl ObjectImpl for RowData {
            glib_object_impl!();

            fn set_property(&self, _obj: &glib::Object, id: usize, value: &glib::Value) {
                let prop = &PROPERTIES[id];

                match *prop {
                    subclass::Property("title", ..) => {
                        let title = value.get().expect("type conformity checked");
                        self.title.replace(title);
                    }
                    subclass::Property("uuid", ..) => {
                        let uuid = value.get().expect("type conformity checked");
                        self.uuid.replace(uuid);
                    }
                    _ => unimplemented!(),
                }
            }

            fn get_property(&self, _obj: &glib::Object, id: usize) -> Result<glib::Value, ()> {
                let prop = &PROPERTIES[id];

                match *prop {
                    subclass::Property("title", ..) => Ok(self.title.borrow().to_value()),
                    subclass::Property("uuid", ..) => Ok(self.uuid.borrow().to_value()),
                    _ => unimplemented!(),
                }
            }
        }
    }

    glib_wrapper! {
        pub struct RowData(
            Object<
                subclass::simple::InstanceStruct<imp::RowData>,
                subclass::simple::ClassStruct<imp::RowData>,
                RowDataClass
            >
        );

        match fn {
            get_type => || imp::RowData::get_type().to_glib(),
        }
    }

    impl RowData {
        pub fn new(title: &str, uuid: &str) -> RowData {
            glib::Object::new(Self::static_type(), &[("title", &title), ("uuid", &uuid)])
                .expect("Failed to create row data")
                .downcast()
                .expect("Created row data is of wrong type")
        }
    }
}
