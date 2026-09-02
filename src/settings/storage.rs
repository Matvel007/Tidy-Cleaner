use super::models::AppSettings;
use crate::filesystem::xdg::get_config_dir;
use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

pub struct SettingsStorage;

impl SettingsStorage {
    pub fn config_path() -> PathBuf {
        get_config_dir().join("tidy-cleaner").join("config.json")
    }

    pub fn load() -> AppSettings {
        let path = Self::config_path();
        if let Ok(content) = fs::read_to_string(&path) {
            match serde_json::from_str::<AppSettings>(&content) {
                Ok(settings) => {
                    tracing::info!("Loaded settings from {:?}", path);
                    return settings;
                }
                Err(e) => {
                    tracing::warn!("Failed to parse settings at {:?}: {}", path, e);
                }
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

        // Atomic write: temp file + rename so a crash never truncates config.json.
        let tmp_path = path.with_extension("json.tmp");
        fs::write(&tmp_path, json)
            .with_context(|| format!("Failed to write settings to {:?}", tmp_path))?;
        fs::rename(&tmp_path, &path)
            .with_context(|| format!("Failed to move settings to {:?}", path))?;

        tracing::debug!("Settings saved to {:?}", path);
        Ok(())
    }
}
