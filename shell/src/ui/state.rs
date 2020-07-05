use std::path::PathBuf;
use standardfile::Credentials;

pub enum AppEvent {
    AddNote,
    DeleteNote,
    SelectNote,
    Register(String, Credentials),
    SignIn(String, Credentials),
    Import(PathBuf, String, Option<String>),
    Update(Option<String>, Option<String>),
    UpdateFilter(Option<String>),
    CreateStorage(Credentials),
    FlushDirty,
    Quit,
}
