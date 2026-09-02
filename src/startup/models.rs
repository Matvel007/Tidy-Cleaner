use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupSource {
    User,
    System,
}

impl StartupSource {
    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "User",
            Self::System => "System",
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::System => "system",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupItem {
    pub id: String,
    pub file_name: String,
    pub name: String,
    pub comment: String,
    pub exec: String,
    pub icon: String,
    pub icon_path: Option<PathBuf>,
    pub source: StartupSource,
    pub enabled: bool,
    pub file_path: PathBuf,
    pub is_terminal: bool,
}

#[derive(Debug, Clone, Default)]
pub struct CreateStartupRequest {
    pub name: String,
    pub exec: String,
    pub comment: String,
    pub icon: String,
    pub terminal: bool,
}
