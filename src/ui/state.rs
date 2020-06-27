use std::path::PathBuf;
use gtk::ListBoxRow;
use uuid::Uuid;

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
    SelectNote(Uuid),
    Register(RemoteAuth),
    SignIn(RemoteAuth),
    Import(PathBuf, String, Option<String>),
    Update(Uuid, Option<String>, Option<String>),
    CreateStorage(User),
    Flush(Uuid),
    Quit,
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
