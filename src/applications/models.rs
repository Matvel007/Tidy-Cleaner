use std::path::PathBuf;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageSource {
    Pacman,
    Aur,
    Flatpak,
    Snap,
    Dpkg,
    Rpm,
    AppImage,
}

impl PackageSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pacman => "Pacman",
            Self::Aur => "AUR",
            Self::Flatpak => "Flatpak",
            Self::Snap => "Snap",
            Self::Dpkg => "APT",
            Self::Rpm => "RPM",
            Self::AppImage => "AppImage",
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::Pacman => "pacman",
            Self::Aur => "aur",
            Self::Flatpak => "flatpak",
            Self::Snap => "snap",
            Self::Dpkg => "dpkg",
            Self::Rpm => "rpm",
            Self::AppImage => "appimage",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationItem {
    pub id: String,
    pub package_id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub source: PackageSource,
    pub icon: String,
    pub icon_path: Option<PathBuf>,
    pub exec_cmd: Option<String>,
    pub installed_size_bytes: Option<u64>,
    pub size_formatted: String,
    pub desktop_file_path: Option<PathBuf>,
    pub is_desktop_app: bool,
    pub selected: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct UninstallProgress {
    pub current_app: String,
    pub current_index: usize,
    pub total_apps: usize,
    pub percent: f32,
    pub is_completed: bool,
    pub error_message: Option<String>,
}
