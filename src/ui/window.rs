use crate::standardfile::NoteItem;
use crate::ui::state::UiEvent;
use gtk::prelude::*;

pub struct Window {
    pub widget: gtk::ApplicationWindow,
    text_buffer: gtk::TextBuffer,
}

struct NoteListRow {
    widget: gtk::ListBoxRow,
    item: NoteItem,
}

impl NoteListRow {
    pub fn new(item: NoteItem) -> Self {
        let title = Some(item.note.title.as_deref().unwrap_or("foo"));
        let label = gtk::Label::new(title);
        label.set_halign(gtk::Align::Start);
        label.set_margin_start(9);
        label.set_margin_end(9);
        label.set_margin_top(9);
        label.set_margin_bottom(9);
        label.set_widget_name("iridium-note-row-label");

        let widget = gtk::ListBoxRow::new();
        widget.add(&label);
        widget.set_widget_name("iridium-note-row");
        widget.show_all();

        Self { widget, item }
    }
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

        let note_list_box: gtk::ListBox = builder.get_object("iridium-note-list").unwrap();

        for item in notes {
            let row = NoteListRow::new(item);
            note_list_box.insert(&row.widget, -1);
        }

        let sender_ = sender.clone();

        note_list_box.connect_row_selected(move |_, _| {
            sender_.send(UiEvent::NoteSelected).unwrap();
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

        text_buffer.connect_changed(|text_buffer| {
            let position = text_buffer.get_property_cursor_position();
        });

        Window { widget: window, text_buffer: text_buffer }
    }

    pub fn load_note(&self, uuid: &str) {
        self.text_buffer.set_text(uuid);
    }
}
