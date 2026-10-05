use crate::startup::desktop::{with_directory_lock, DesktopAutostart, APP_AUTOSTART_FILE_NAME};
use crate::startup::models::{CreateStartupRequest, StartupItem, StartupSource};
use anyhow::Result;
use std::fs;

/// Filename used by older versions (before the underscore/hyphen mismatch was
/// fixed). Removed so a stale entry can't launch a second instance.
const LEGACY_AUTOSTART_FILE_NAME: &str = "tidy_cleaner.desktop";

pub struct AppAutostartManager;

impl AppAutostartManager {
    #[allow(dead_code)]
    pub fn set_app_autostart(enabled: bool, start_minimized: bool) -> Result<()> {
        with_directory_lock(&DesktopAutostart::get_user_autostart_dir(), || {
            Self::set_app_autostart_locked(enabled, start_minimized)
        })
    }

    #[allow(dead_code)]
    pub fn is_enabled() -> Result<bool> {
        with_directory_lock(&DesktopAutostart::get_user_autostart_dir(), || {
            Self::is_enabled_locked()
        })
    }

    pub(crate) fn effective_entry_locked() -> Result<Option<StartupItem>> {
        let user = DesktopAutostart::get_user_autostart_dir().join(APP_AUTOSTART_FILE_NAME);
        let paths = std::iter::once((user, StartupSource::User)).chain(
            DesktopAutostart::get_system_autostart_dirs()
                .into_iter()
                .map(|dir| (dir.join(APP_AUTOSTART_FILE_NAME), StartupSource::System)),
        );
        for (path, source) in paths {
            match fs::symlink_metadata(&path) {
                Ok(_) => return DesktopAutostart::parse_file(&path, source).map(Some),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
        Ok(None)
    }

    pub(crate) fn is_enabled_locked() -> Result<bool> {
        Ok(Self::effective_entry_locked()?
            .map(|item| item.enabled)
            .unwrap_or(false))
    }

    /// Updating launch options must never resurrect an entry disabled elsewhere.
    #[allow(dead_code)]
    pub fn set_start_minimized(start_minimized: bool) -> Result<()> {
        with_directory_lock(&DesktopAutostart::get_user_autostart_dir(), || {
            Self::set_start_minimized_locked(start_minimized)
        })
    }

    pub(crate) fn set_start_minimized_locked(start_minimized: bool) -> Result<()> {
        if Self::is_enabled_locked()? {
            Self::set_app_autostart_locked(true, start_minimized)?;
        }
        Ok(())
    }

    pub(crate) fn set_app_autostart_locked(enabled: bool, start_minimized: bool) -> Result<()> {
        let user_dir = DesktopAutostart::get_user_autostart_dir();
        let target_file = user_dir.join(APP_AUTOSTART_FILE_NAME);

        // Always clear any legacy entry from older versions.
        let legacy_file = user_dir.join(LEGACY_AUTOSTART_FILE_NAME);
        match fs::remove_file(&legacy_file) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }

        if enabled {
            let exe_path = std::env::current_exe()?.to_string_lossy().to_string();

            let exec_cmd = if start_minimized {
                format!("\"{}\" --minimized", exe_path)
            } else {
                format!("\"{}\"", exe_path)
            };

            let req = CreateStartupRequest {
                name: "Tidy Cleaner".to_string(),
                exec: exec_cmd,
                comment: "Linux System Cleaner & Optimizer".to_string(),
                icon: "tidy-cleaner".to_string(),
                terminal: false,
            };

            if !user_dir.exists() {
                fs::create_dir_all(&user_dir)?;
            }
            let content = DesktopAutostart::generate_desktop_file_content(&req);
            DesktopAutostart::atomic_write_locked(&target_file, &content, false)?;
        } else if let Some(item) = Self::effective_entry_locked()? {
            // A disabled override also masks a system-wide entry, if one exists.
            DesktopAutostart::toggle_entry_locked(&item, false)?;
        }

        Ok(())
    }
}
