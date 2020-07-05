use std::path::PathBuf;
use standardfile::Credentials;

pub struct RemoteAuth {
    pub credentials: Credentials,
    pub server: String,
}

pub enum AppEvent {
    AddNote,
    DeleteNote,
    SelectNote,
    Register(RemoteAuth),
    SignIn(RemoteAuth),
    Import(PathBuf, String, Option<String>),
    Update(Option<String>, Option<String>),
    UpdateFilter(Option<String>),
    CreateStorage(Credentials),
    FlushDirty,
    Quit,
}
