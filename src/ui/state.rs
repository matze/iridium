use std::path::PathBuf;
use gtk::ListBoxRow;
use uuid::Uuid;

pub enum AppEvent {
    AddNote,
    SelectNote(Uuid),
    Import(PathBuf),
    UpdateTitle(Uuid, String),
    UpdateText(Uuid, String),
}

pub enum WindowEvent {
    AddNote(Uuid, String),
    SelectNote(ListBoxRow),
    ToggleSearchBar,
    UpdateTitle,
    UpdateText,
}
