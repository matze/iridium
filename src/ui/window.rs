use gtk::prelude::*;

pub struct Window {
    pub widget: gtk::ApplicationWindow,
}

impl Window {
    pub fn new() -> Self {
        let builder = gtk::Builder::new_from_resource("/net/bloerg/Iridium/data/resources/ui/window.ui");
        get_widget!(builder, gtk::ApplicationWindow, window);

        Window {
            widget: window,
        }
    }
}
