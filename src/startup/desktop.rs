use crate::startup::models::{CreateStartupRequest, StartupItem, StartupSource};
use anyhow::{bail, Context, Result};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// All cooperating instances lock the directory before read/modify/write or delete.
/// The lock file is persistent: unlinking it would allow two different lock inodes.
pub fn with_directory_lock<T>(dir: &Path, operation: impl FnOnce() -> Result<T>) -> Result<T> {
    fs::create_dir_all(dir)?;
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(0x20000).mode(0o600); // O_NOFOLLOW
    }
    let lock = options.open(dir.join(".tidy-cleaner.lock"))?;
    if !lock.metadata()?.is_file() {
        bail!("Persistence lock is not a regular file");
    }
    lock.lock()?;
    operation()
}

/// Fixed filename used by the application's own autostart entry. Written and
/// removed under this exact name so enable/disable always line up.
pub const APP_AUTOSTART_FILE_NAME: &str = "tidy-cleaner.desktop";

pub struct DesktopAutostart;

impl DesktopAutostart {
    pub fn get_user_autostart_dir() -> PathBuf {
        let config_home = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")));
        config_home
            .unwrap_or_else(|| PathBuf::from(".config"))
            .join("autostart")
    }

    pub fn get_system_autostart_dirs() -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        if let Ok(xdg_dirs) = std::env::var("XDG_CONFIG_DIRS") {
            for dir in xdg_dirs.split(':') {
                if Path::new(dir).is_absolute() {
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
            || Self::contains_control_chars(&req.icon)
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
        let dir = path.parent().context("No parent directory")?;
        with_directory_lock(dir, || Self::atomic_write_locked(path, content, false))
    }

    pub(crate) fn atomic_write_locked(path: &Path, content: &str, create_only: bool) -> Result<()> {
        let dir = path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("No parent directory for {:?}", path))?;
        let (tmp, mut file) = loop {
            let tmp = dir.join(format!(
                ".{}.{}.{}.tmp",
                path.file_name().unwrap_or_default().to_string_lossy(),
                std::process::id(),
                TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            match options.open(&tmp) {
                Ok(file) => break (tmp, file),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e).context("Failed to create temporary file"),
            }
        };
        let result: Result<()> = (|| {
            file.write_all(content.as_bytes())?;
            file.sync_all()?;
            if create_only {
                // Unlike rename, hard_link atomically refuses any existing destination.
                fs::hard_link(&tmp, path)?;
                fs::remove_file(&tmp)?;
            } else {
                fs::rename(&tmp, path)?;
            }
            File::open(dir)?.sync_all()?;
            Ok(())
        })();
        if result.is_err() {
            if let Err(e) = fs::remove_file(&tmp) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    tracing::warn!("Failed to remove temporary file {:?}: {}", tmp, e);
                }
            }
        }
        result.with_context(|| format!("Failed to publish {:?}", path))
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
        with_directory_lock(&dir, || {
            // A new user entry must not silently replace an effective system entry either.
            for system in Self::get_system_autostart_dirs() {
                match fs::symlink_metadata(system.join(target_file.file_name().unwrap())) {
                    Ok(_) => bail!("An autostart entry with this filename already exists"),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e.into()),
                }
            }
            Self::atomic_write_locked(&target_file, &content, true)
        })?;

        Ok(target_file)
    }

    pub fn toggle_entry(item: &StartupItem, enable: bool) -> Result<()> {
        let user_dir = Self::get_user_autostart_dir();
        with_directory_lock(&user_dir, || Self::toggle_entry_locked(item, enable))
    }

    pub(crate) fn toggle_entry_locked(item: &StartupItem, enable: bool) -> Result<()> {
        let user_dir = Self::get_user_autostart_dir();

        let target_file = user_dir.join(&item.file_name);

        let content = if target_file.exists() {
            fs::read_to_string(&target_file)?
        } else if item.file_path.exists() {
            fs::read_to_string(&item.file_path)?
        } else {
            bail!("Autostart file does not exist");
        };

        let new_content = Self::with_enabled(&content, enable)?;
        Self::atomic_write_locked(&target_file, &new_content, false)
    }

    pub(crate) fn with_enabled(content: &str, enable: bool) -> Result<String> {
        let mut lines: Vec<String> = content.lines().map(str::to_owned).collect();
        let start = lines
            .iter()
            .position(|l| l.trim() == "[Desktop Entry]")
            .context("Missing Desktop Entry group")?
            + 1;
        let end = lines[start..]
            .iter()
            .position(|l| l.trim().starts_with('['))
            .map(|i| start + i)
            .unwrap_or(lines.len());
        let keys = [
            "Hidden",
            "X-GNOME-Autostart-enabled",
            "X-KDE-autostart-enabled",
        ];
        let mut group: Vec<String> = lines[start..end]
            .iter()
            .filter(|l| {
                !l.split_once('=')
                    .map(|(k, _)| keys.contains(&k.trim()))
                    .unwrap_or(false)
            })
            .cloned()
            .collect();
        for key in keys {
            let value = if key == "Hidden" { !enable } else { enable };
            group.push(format!("{}={}", key, value));
        }
        lines.splice(start..end, group);
        Ok(lines.join("\n") + "\n")
    }

    pub fn remove_entry(item: &StartupItem) -> Result<()> {
        let user_dir = Self::get_user_autostart_dir();
        with_directory_lock(&user_dir, || {
            // Removing an effective system entry means disable, never expose its fallback.
            if item.source == StartupSource::System {
                return Self::toggle_entry_locked(item, false);
            }
            let user_file = user_dir.join(&item.file_name);
            fs::remove_file(&user_file)
                .with_context(|| format!("Failed to delete autostart file {:?}", user_file))
        })
    }
}
