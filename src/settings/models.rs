use crate::localization::Language;
use crate::theme::ThemeMode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub theme: ThemeMode,
    pub language: String,
    pub autostart: bool,
    pub start_minimized: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme: ThemeMode::Dark,
            language: "en".to_string(),
            autostart: false,
            start_minimized: false,
        }
    }
}

impl AppSettings {
    pub fn get_language(&self) -> Language {
        Language::from_str_name(&self.language)
    }
}
