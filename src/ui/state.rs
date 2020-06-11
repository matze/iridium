use std::path::PathBuf;
use uuid::Uuid;

pub enum AppEvent {
    AddNote,
    NoteSelected(String),
    Import(PathBuf),
    TitleUpdated(Uuid, String),
}

pub enum WindowEvent {
    AddNote(Uuid, String),
    SelectNote(i32),
    ToggleSearchBar,
    TitleUpdated,
}
