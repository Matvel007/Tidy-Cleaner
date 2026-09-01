use crate::localization::Language;
use crate::theme::ThemeMode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub theme: ThemeMode,
    pub language: String,
    pub autostart: bool,
    pub start_minimized: bool,
    pub start_in_tray: bool,
    pub confirm_delete: bool,
    pub show_hidden: bool,
    pub selected_disks: Vec<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: ThemeMode::Dark,
            language: "en".to_string(),
            autostart: false,
            start_minimized: false,
            start_in_tray: false,
            confirm_delete: true,
            show_hidden: false,
            selected_disks: vec!["/".to_string()],
        }
    }
}

impl AppSettings {
    pub fn get_language(&self) -> Language {
        Language::from_str_name(&self.language)
    }
}
