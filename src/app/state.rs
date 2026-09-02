use crate::localization::{Language, LocalizationService};
use crate::settings::{AppSettings, SettingsStorage};
use crate::theme::ThemeMode;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone)]
pub struct AppState {
    pub settings: Arc<RwLock<AppSettings>>,
    pub localization: Arc<LocalizationService>,
    pub current_page: Arc<RwLock<i32>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        let settings = SettingsStorage::load();
        let loc = LocalizationService::new();
        loc.set_language(settings.get_language());

        Self {
            settings: Arc::new(RwLock::new(settings)),
            localization: Arc::new(loc),
            current_page: Arc::new(RwLock::new(0)),
        }
    }

    pub fn set_page(&self, page: i32) {
        if let Ok(mut p) = self.current_page.write() {
            *p = page;
        }
    }

    pub fn set_language(&self, lang: Language) {
        self.localization.set_language(lang);
        if let Ok(mut s) = self.settings.write() {
            s.language = lang.as_str().to_string();
            let _ = SettingsStorage::save(&s);
        }
    }

    pub fn set_theme(&self, theme: ThemeMode) {
        if let Ok(mut s) = self.settings.write() {
            s.theme = theme;
            let _ = SettingsStorage::save(&s);
        }
    }

    pub fn get_theme(&self) -> ThemeMode {
        self.settings
            .read()
            .map(|s| s.theme)
            .unwrap_or(ThemeMode::Dark)
    }

    pub fn get_language(&self) -> Language {
        self.localization.current_language()
    }
}
