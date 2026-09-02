use crate::startup::desktop::{DesktopAutostart, APP_AUTOSTART_FILE_NAME};
use crate::startup::models::CreateStartupRequest;
use anyhow::Result;
use std::fs;

/// Filename used by older versions (before the underscore/hyphen mismatch was
/// fixed). Removed so a stale entry can't launch a second instance.
const LEGACY_AUTOSTART_FILE_NAME: &str = "tidy_cleaner.desktop";

pub struct AppAutostartManager;

impl AppAutostartManager {
    pub fn set_app_autostart(enabled: bool, start_minimized: bool) -> Result<()> {
        let user_dir = DesktopAutostart::get_user_autostart_dir();
        let target_file = user_dir.join(APP_AUTOSTART_FILE_NAME);

        // Always clear any legacy entry from older versions.
        let legacy_file = user_dir.join(LEGACY_AUTOSTART_FILE_NAME);
        if legacy_file.exists() {
            let _ = fs::remove_file(&legacy_file);
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
            DesktopAutostart::atomic_write_file(&target_file, &content)?;
        } else if target_file.exists() {
            let _ = fs::remove_file(&target_file);
        }

        Ok(())
    }
}
