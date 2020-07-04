use std::path::PathBuf;

pub struct User {
    pub identifier: String,
    pub password: String,
}

pub struct RemoteAuth {
    pub user: User,
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
    CreateStorage(User),
    FlushDirty,
    Quit,
}
