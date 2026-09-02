use std::path::PathBuf;

/// Returns the current user's home directory, or an empty path if `HOME` is
/// unset (callers must tolerate a non-existent base path).
pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
}

/// Resolves the XDG cache directory (`$XDG_CACHE_HOME`, default `~/.cache`).
pub fn get_cache_dir() -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| home_dir().join(".cache"))
}

/// Resolves the XDG config directory (`$XDG_CONFIG_HOME`, default `~/.config`).
pub fn get_config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| home_dir().join(".config"))
}

/// Resolves the XDG data directory (`$XDG_DATA_HOME`, default `~/.local/share`).
pub fn get_data_dir() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| home_dir().join(".local/share"))
}

/// Resolves user XDG directories dynamically according to the FreeDesktop XDG User Dirs specification.
/// Works reliably across all Linux distributions (Arch, Ubuntu, Debian, Fedora, openSUSE, etc.)
/// and all system locales (Russian: "Загрузки", English: "Downloads", etc.).
pub fn get_user_download_dir() -> PathBuf {
    let home = home_dir();

    // 1. Parse ~/.config/user-dirs.dirs
    let user_dirs_file = home.join(".config/user-dirs.dirs");
    if let Ok(content) = std::fs::read_to_string(&user_dirs_file) {
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("XDG_DOWNLOAD_DIR=") {
                let val = line
                    .trim_start_matches("XDG_DOWNLOAD_DIR=")
                    .trim_matches('"');
                if let Some(rel) = val.strip_prefix("$HOME/") {
                    let candidate = home.join(rel);
                    if candidate.exists() {
                        return candidate;
                    }
                } else if val.starts_with('/') {
                    let candidate = PathBuf::from(val);
                    if candidate.exists() {
                        return candidate;
                    }
                }
            }
        }
    }

    // 2. Try xdg-user-dir tool
    if let Ok(output) = std::process::Command::new("xdg-user-dir")
        .arg("DOWNLOAD")
        .output()
    {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !s.is_empty() {
                let candidate = PathBuf::from(s);
                if candidate.exists() {
                    return candidate;
                }
            }
        }
    }

    // 3. Fallback to common localized names
    let localized_names = [
        "Загрузки",
        "Downloads",
        "Téléchargements",
        "Descargas",
        "Pobrane",
    ];

    for name in &localized_names {
        let p = home.join(name);
        if p.exists() {
            return p;
        }
    }

    home.join("Downloads")
}

/// Resolves user XDG Desktop directory (e.g. ~/Desktop or ~/Рабочий стол)
pub fn get_user_desktop_dir() -> PathBuf {
    let home = home_dir();

    // 1. Parse ~/.config/user-dirs.dirs
    let user_dirs_file = home.join(".config/user-dirs.dirs");
    if let Ok(content) = std::fs::read_to_string(&user_dirs_file) {
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("XDG_DESKTOP_DIR=") {
                let val = line
                    .trim_start_matches("XDG_DESKTOP_DIR=")
                    .trim_matches('"');
                if let Some(rel) = val.strip_prefix("$HOME/") {
                    let candidate = home.join(rel);
                    if candidate.exists() {
                        return candidate;
                    }
                } else if val.starts_with('/') {
                    let candidate = PathBuf::from(val);
                    if candidate.exists() {
                        return candidate;
                    }
                }
            }
        }
    }

    // 2. Try xdg-user-dir tool
    if let Ok(output) = std::process::Command::new("xdg-user-dir")
        .arg("DESKTOP")
        .output()
    {
        if output.status.success() {
            let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !s.is_empty() {
                let candidate = PathBuf::from(s);
                if candidate.exists() {
                    return candidate;
                }
            }
        }
    }

    // 3. Fallback to common localized names
    let localized_names = [
        "Рабочий стол",
        "Desktop",
        "Bureau",
        "Escritorio",
        "Pulpit",
        "Schreibtisch",
    ];

    for name in &localized_names {
        let p = home.join(name);
        if p.exists() {
            return p;
        }
    }

    home.join("Desktop")
}
