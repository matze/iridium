use crate::standardfile::NoteItem;
use crate::ui::state::UiEvent;
use gio::prelude::*;
use gtk::prelude::*;

use row_data::RowData;

pub struct Window {
    pub widget: gtk::ApplicationWindow,
    text_buffer: gtk::TextBuffer,
}

impl Window {
    pub fn new(sender: glib::Sender<UiEvent>, notes: Vec<NoteItem>) -> Self {
        let builder =
            gtk::Builder::new_from_resource("/net/bloerg/Iridium/data/resources/ui/window.ui");
        let window: gtk::ApplicationWindow = builder.get_object("window").unwrap();

        let style_provider = gtk::CssProvider::new();
        style_provider.load_from_resource("/net/bloerg/Iridium/data/resources/css/base.css");
        gtk::StyleContext::add_provider_for_screen(
            &window.get_screen().unwrap(),
            &style_provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        let row_model = gio::ListStore::new(RowData::static_type());
        let note_list_box: gtk::ListBox = builder.get_object("iridium-note-list").unwrap();

        note_list_box.bind_model(
            Some(&row_model),
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

        for item in notes {
            row_model.append(&RowData::new(
                item.note.title.unwrap_or("foo".to_owned()).as_str(),
                item.item.uuid.as_str(),
            ));
        }

        let sender_ = sender.clone();

        note_list_box.connect_row_selected(move |_, row| {
            match row {
                Some(row) => {
                    let item = row_model.get_object(row.get_index() as u32).unwrap();
                    let item = item.downcast_ref::<RowData>().unwrap();
                    let uuid = item.get_property("uuid").unwrap().get::<String>();
                    sender_.send(UiEvent::NoteSelected(uuid.unwrap().unwrap())).unwrap();
                }
                None => {}
            }
        });

        let bold_tag = gtk::TextTag::new(Some("semibold"));

        // I'd like to use Pango::Weight::Bold but it's too much of a hassle.
        bold_tag.set_property_weight(600);

        let text_view: gtk::TextView = builder.get_object("iridium-text-view").unwrap();
        let text_buffer = text_view.get_buffer().unwrap();
        let tag_table = text_buffer.get_tag_table().unwrap();
        tag_table.add(&bold_tag);

        text_buffer.set_text("# this is a heading\nthis is regular text");
        let start = text_buffer.get_start_iter();
        let mut end = start.clone();
        end.forward_line();

        text_buffer.apply_tag(&bold_tag, &start, &end);

        Window {
            widget: window,
            text_buffer: text_buffer,
        }
    }

    pub fn load_note(&self, uuid: &str) {
        self.text_buffer.set_text(uuid);
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
