use std::path::PathBuf;
use gtk::ListBoxRow;
use uuid::Uuid;

pub enum AppEvent {
    AddNote,
    SelectNote(Uuid),
    Import(PathBuf, String),
    UpdateTitle(Uuid, String),
    UpdateText(Uuid, String),
    CreateStorage(String, String, Option<String>),
}

pub enum WindowEvent {
    AddNote(Uuid, String),
    SelectNote(ListBoxRow),
    ToggleSearchBar,
    UpdateTitle,
    UpdateText,
    UpdateFilter(Option<String>),
    ShowNotification(String),
    ShowMainContent,
}
