use crate::startup::models::{CreateStartupRequest, StartupItem, StartupSource};
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// Fixed filename used by the application's own autostart entry. Written and
/// removed under this exact name so enable/disable always line up.
pub const APP_AUTOSTART_FILE_NAME: &str = "tidy-cleaner.desktop";

pub struct DesktopAutostart;

impl DesktopAutostart {
    pub fn get_user_autostart_dir() -> PathBuf {
        let config_home = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| !p.as_os_str().is_empty())
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")));
        config_home
            .unwrap_or_else(|| PathBuf::from(".config"))
            .join("autostart")
    }

    pub fn get_system_autostart_dirs() -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        if let Ok(xdg_dirs) = std::env::var("XDG_CONFIG_DIRS") {
            for dir in xdg_dirs.split(':') {
                if !dir.is_empty() {
                    dirs.push(PathBuf::from(dir).join("autostart"));
                }
            }
        }
        if dirs.is_empty() {
            dirs.push(PathBuf::from("/etc/xdg/autostart"));
        }
        dirs
    }

    pub fn validate_request(req: &CreateStartupRequest) -> Result<()> {
        let name = req.name.trim();
        let exec = req.exec.trim();
        let comment = req.comment.trim();

        if name.is_empty() {
            bail!("Application name cannot be empty");
        }
        if exec.is_empty() {
            bail!("Command / Executable path cannot be empty");
        }
        if Self::contains_control_chars(name)
            || Self::contains_control_chars(exec)
            || Self::contains_control_chars(comment)
        {
            bail!("Autostart fields must not contain line breaks or control characters");
        }

        Ok(())
    }

    fn contains_control_chars(s: &str) -> bool {
        s.chars().any(|c| c.is_control())
    }

    /// Quotes the executable path only when it is a single existing path that
    /// contains spaces. Commands with arguments are left untouched.
    fn quote_exec_if_needed(exec: &str) -> String {
        let trimmed = exec.trim();
        if trimmed.is_empty() || trimmed.starts_with('"') {
            return trimmed.to_string();
        }
        if trimmed.contains(' ') && Path::new(trimmed).exists() {
            return format!("\"{}\"", trimmed);
        }
        trimmed.to_string()
    }

    /// Writes content atomically (temp file + rename) so a crash mid-write
    /// never leaves a truncated .desktop file behind.
    pub fn atomic_write_file(path: &Path, content: &str) -> Result<()> {
        let dir = path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("No parent directory for {:?}", path))?;
        let tmp = dir.join(format!(
            ".{}.tmp",
            path.file_name().unwrap_or_default().to_string_lossy()
        ));
        fs::write(&tmp, content)
            .with_context(|| format!("Failed to write temporary file {:?}", tmp))?;
        fs::rename(&tmp, path)
            .with_context(|| format!("Failed to move {:?} to {:?}", tmp, path))?;
        Ok(())
    }

    pub fn parse_file(path: &Path, source: StartupSource) -> Result<StartupItem> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read autostart file at {:?}", path))?;

        let file_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown.desktop")
            .to_string();

        let mut name = String::new();
        let mut comment = String::new();
        let mut exec = String::new();
        let mut icon = String::new();
        let mut is_terminal = false;
        let mut hidden = false;
        let mut autostart_enabled = true;
        let mut in_desktop_entry = false;

        for line in content.lines() {
            let line = line.trim();
            if line == "[Desktop Entry]" {
                in_desktop_entry = true;
                continue;
            } else if line.starts_with('[') && in_desktop_entry {
                break;
            }

            if !in_desktop_entry {
                continue;
            }

            if let Some((key, val)) = line.split_once('=') {
                let key = key.trim();
                let val = val.trim();
                match key {
                    "Name" if name.is_empty() => name = val.to_string(),
                    "Comment" if comment.is_empty() => comment = val.to_string(),
                    "Exec" if exec.is_empty() => {
                        let cleaned = val
                            .split_whitespace()
                            .filter(|w| !w.starts_with('%'))
                            .collect::<Vec<_>>()
                            .join(" ");
                        exec = cleaned;
                    }
                    "Icon" if icon.is_empty() => icon = val.to_string(),
                    "Terminal" => is_terminal = val.eq_ignore_ascii_case("true"),
                    "Hidden" => hidden = val.eq_ignore_ascii_case("true"),
                    "X-GNOME-Autostart-enabled" | "X-KDE-autostart-enabled"
                        if val.eq_ignore_ascii_case("false") =>
                    {
                        autostart_enabled = false;
                    }
                    _ => {}
                }
            }
        }

        if name.is_empty() {
            name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown Entry")
                .to_string();
        }

        let enabled = !hidden && autostart_enabled;
        let id = format!("{}:{}", source.code(), file_name);

        Ok(StartupItem {
            id,
            file_name,
            name,
            comment,
            exec,
            icon,
            icon_path: None,
            source,
            enabled,
            file_path: path.to_path_buf(),
            is_terminal,
        })
    }

    pub fn generate_desktop_file_content(req: &CreateStartupRequest) -> String {
        let name = req.name.trim();
        let exec = req.exec.trim();
        let comment = req.comment.trim();
        let icon = if req.icon.trim().is_empty() {
            "application-x-executable"
        } else {
            req.icon.trim()
        };

        let home_str = std::env::var("HOME").unwrap_or_default();
        let expanded_exec = if let Some(stripped) = exec.strip_prefix("~/") {
            if home_str.is_empty() {
                exec.to_string()
            } else {
                format!("{}/{}", home_str, stripped)
            }
        } else if exec.contains("~/") {
            if home_str.is_empty() {
                exec.to_string()
            } else {
                exec.replace("~/", &format!("{}/", home_str))
            }
        } else {
            exec.to_string()
        };

        // Quote only the executable's own path when it contains a space; leave
        // arguments untouched. Field codes (%f, %U, ...) are preserved as-is.
        let formatted_exec = Self::quote_exec_if_needed(&expanded_exec);

        format!(
            "[Desktop Entry]\n\
            Type=Application\n\
            Version=1.0\n\
            Name={}\n\
            Comment={}\n\
            Exec={}\n\
            Icon={}\n\
            Terminal={}\n\
            StartupNotify=false\n\
            X-GNOME-Autostart-enabled=true\n\
            X-KDE-autostart-enabled=true\n\
            Categories=Utility;\n",
            name,
            comment,
            formatted_exec,
            icon,
            if req.terminal { "true" } else { "false" }
        )
    }

    pub fn write_user_autostart_entry(req: &CreateStartupRequest) -> Result<PathBuf> {
        Self::validate_request(req)?;

        let dir = Self::get_user_autostart_dir();
        if !dir.exists() {
            fs::create_dir_all(&dir)
                .with_context(|| format!("Failed to create user autostart dir {:?}", dir))?;
        }

        let safe_name: String = req
            .name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let file_name = format!("{}.desktop", safe_name.to_lowercase());
        let target_file = dir.join(file_name);

        let content = Self::generate_desktop_file_content(req);
        Self::atomic_write_file(&target_file, &content)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(metadata) = fs::metadata(&target_file) {
                let mut perms = metadata.permissions();
                perms.set_mode(0o755);
                let _ = fs::set_permissions(&target_file, perms);
            }
        }

        Ok(target_file)
    }

    pub fn toggle_entry(item: &StartupItem, enable: bool) -> Result<()> {
        let user_dir = Self::get_user_autostart_dir();
        if !user_dir.exists() {
            fs::create_dir_all(&user_dir)?;
        }

        let target_file = user_dir.join(&item.file_name);

        let content = if target_file.exists() {
            fs::read_to_string(&target_file)?
        } else if item.file_path.exists() {
            fs::read_to_string(&item.file_path)?
        } else {
            bail!("Autostart file does not exist");
        };

        let mut lines: Vec<String> = Vec::new();
        let mut has_hidden = false;
        let mut has_gnome_enabled = false;
        let mut has_kde_enabled = false;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("Hidden=") {
                has_hidden = true;
                lines.push(format!("Hidden={}", if enable { "false" } else { "true" }));
            } else if trimmed.starts_with("X-GNOME-Autostart-enabled=") {
                has_gnome_enabled = true;
                lines.push(format!(
                    "X-GNOME-Autostart-enabled={}",
                    if enable { "true" } else { "false" }
                ));
            } else if trimmed.starts_with("X-KDE-autostart-enabled=") {
                has_kde_enabled = true;
                lines.push(format!(
                    "X-KDE-autostart-enabled={}",
                    if enable { "true" } else { "false" }
                ));
            } else {
                lines.push(line.to_string());
            }
        }

        if !has_hidden {
            lines.push(format!("Hidden={}", if enable { "false" } else { "true" }));
        }
        if !has_gnome_enabled {
            lines.push(format!(
                "X-GNOME-Autostart-enabled={}",
                if enable { "true" } else { "false" }
            ));
        }
        if !has_kde_enabled {
            lines.push(format!(
                "X-KDE-autostart-enabled={}",
                if enable { "true" } else { "false" }
            ));
        }

        let new_content = lines.join("\n") + "\n";
        Self::atomic_write_file(&target_file, &new_content)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(metadata) = fs::metadata(&target_file) {
                let mut perms = metadata.permissions();
                perms.set_mode(0o755);
                let _ = fs::set_permissions(&target_file, perms);
            }
        }

        Ok(())
    }

    pub fn remove_entry(item: &StartupItem) -> Result<()> {
        let user_dir = Self::get_user_autostart_dir();
        let user_file = user_dir.join(&item.file_name);

        if user_file.exists() {
            fs::remove_file(&user_file)
                .with_context(|| format!("Failed to delete autostart file {:?}", user_file))?;
        }

        // If it was a system entry, disable it by writing Hidden=true override
        if item.source == StartupSource::System {
            Self::toggle_entry(item, false)?;
        }

        Ok(())
    }
}
