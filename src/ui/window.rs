use gtk::prelude::*;

pub struct Window {
    pub widget: gtk::ApplicationWindow,
}

impl Window {
    pub fn new() -> Self {
        let builder =
            gtk::Builder::new_from_resource("/net/bloerg/Iridium/data/resources/ui/window.ui");
        get_widget!(builder, gtk::ApplicationWindow, window);

        let style_provider = gtk::CssProvider::new();
        style_provider.load_from_resource("/net/bloerg/Iridium/data/resources/css/base.css");
        gtk::StyleContext::add_provider_for_screen(
            &window.get_screen().unwrap(),
            &style_provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        Window { widget: window }
    }
}
