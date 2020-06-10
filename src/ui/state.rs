use std::path::PathBuf;
use uuid::Uuid;

pub enum AppEvent {
    NoteSelected(String),
    Import(PathBuf),
    ToggleSearchBar,
}

pub enum WindowEvent {
    AddNote(Uuid, String),
    SelectNote(i32),
}
