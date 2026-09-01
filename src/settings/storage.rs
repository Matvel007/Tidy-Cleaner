use super::models::AppSettings;
use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

pub struct SettingsStorage;

impl SettingsStorage {
    pub fn config_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home)
            .join(".config")
            .join("tidy-cleaner")
            .join("config.json")
    }

    pub fn load() -> AppSettings {
        let path = Self::config_path();
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(settings) = serde_json::from_str::<AppSettings>(&content) {
                tracing::info!("Loaded settings from {:?}", path);
                return settings;
            }
        }
        tracing::info!("Using default settings");
        AppSettings::default()
    }

    pub fn save(settings: &AppSettings) -> Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory {:?}", parent))?;
        }

        let json = serde_json::to_string_pretty(settings)
            .context("Failed to serialize settings to JSON")?;
        fs::write(&path, json)
            .with_context(|| format!("Failed to write settings to {:?}", path))?;
        tracing::debug!("Settings saved to {:?}", path);
        Ok(())
    }
}
