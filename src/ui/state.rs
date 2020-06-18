use std::path::PathBuf;
use gtk::ListBoxRow;
use uuid::Uuid;

pub enum AppEvent {
    AddNote,
    SelectNote(Uuid),
    Register(String, String, String),
    Import(PathBuf, String),
    Update(Uuid, Option<String>, Option<String>),
    CreateStorage(String, String, Option<String>),
    Flush(Uuid),
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
