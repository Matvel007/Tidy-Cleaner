use crate::localization::{Language, LocalizationService};
use crate::settings::autostart::AppAutostartManager;
use crate::settings::{AppSettings, SettingsStorage};
use crate::startup::desktop::{with_directory_lock, DesktopAutostart};
use crate::theme::ThemeMode;
use anyhow::{Context, Result};
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone)]
pub struct AppState {
    pub settings: Arc<RwLock<AppSettings>>,
    pub localization: Arc<LocalizationService>,
    pub current_page: Arc<RwLock<i32>>,
    pub autostart_updates: tokio::sync::watch::Sender<()>,
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
            autostart_updates: tokio::sync::watch::channel(()).0,
        }
    }

    pub fn set_page(&self, page: i32) {
        if let Ok(mut p) = self.current_page.write() {
            *p = page;
        }
    }

    /// Call on a worker thread. No state lock is retained across filesystem I/O.
    pub fn update_settings(
        &self,
        change: impl FnOnce(&mut AppSettings) -> Result<()>,
    ) -> Result<()> {
        with_directory_lock(&DesktopAutostart::get_user_autostart_dir(), || {
            let mut settings = self
                .settings
                .read()
                .map_err(|_| anyhow::anyhow!("Settings lock poisoned"))?
                .clone();
            settings.autostart = AppAutostartManager::is_enabled_locked()?;
            let result = change(&mut settings).and_then(|_| {
                settings.autostart = AppAutostartManager::is_enabled_locked()?;
                SettingsStorage::save(&settings)
            });
            // Even if saving failed, the file (not the old JSON boolean) is authoritative.
            let actual = AppAutostartManager::is_enabled_locked()
                .context("Failed to reconcile autostart")?;
            let mut current = self
                .settings
                .write()
                .map_err(|_| anyhow::anyhow!("Settings lock poisoned"))?;
            if result.is_ok() {
                *current = settings;
            }
            current.autostart = actual;
            result
        })
    }

    pub fn set_language(&self, lang: Language) -> Result<()> {
        self.update_settings(|s| {
            s.language = lang.as_str().to_string();
            Ok(())
        })?;
        self.localization.set_language(lang);
        Ok(())
    }

    pub fn set_theme(&self, theme: ThemeMode) -> Result<()> {
        self.update_settings(|s| {
            s.theme = theme;
            Ok(())
        })
    }

    pub fn reconcile_autostart(&self) -> Result<()> {
        self.update_settings(|_| Ok(()))
    }

    pub fn set_autostart(&self, enabled: bool) -> Result<()> {
        let result = self.update_settings(|s| {
            AppAutostartManager::set_app_autostart_locked(enabled, s.start_minimized)
        });
        // The desktop file may have changed even if saving the JSON failed.
        self.autostart_updates.send_replace(());
        result
    }

    pub fn set_start_minimized(&self, minimized: bool) -> Result<()> {
        let result = self.update_settings(|s| {
            AppAutostartManager::set_start_minimized_locked(minimized)?;
            s.start_minimized = minimized;
            Ok(())
        });
        self.autostart_updates.send_replace(());
        result
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
