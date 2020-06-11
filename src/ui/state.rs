use std::path::PathBuf;
use uuid::Uuid;

pub enum AppEvent {
    AddNote,
    SelectNote(String),
    Import(PathBuf),
    UpdateTitle(Uuid, String),
    UpdateText(Uuid, String),
}

pub enum WindowEvent {
    AddNote(Uuid, String),
    SelectNote(i32),
    ToggleSearchBar,
    UpdateTitle,
    UpdateText,
}
