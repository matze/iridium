use gtk::prelude::*;

pub struct Window {
    pub widget: gtk::ApplicationWindow,
}

impl Window {
    pub fn new() -> Self {
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
            println!("changed {}", position);
        });

        Window { widget: window }
    }
}
