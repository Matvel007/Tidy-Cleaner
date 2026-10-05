use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    #[default]
    Dark = 0,
    Light = 1,
    System = 2,
}

impl ThemeMode {
    pub fn to_i32(self) -> i32 {
        match self {
            ThemeMode::Dark => 0,
            ThemeMode::Light => 1,
            ThemeMode::System => 2,
        }
    }

    pub fn from_i32(val: i32) -> Self {
        match val {
            0 => ThemeMode::Dark,
            1 => ThemeMode::Light,
            2 => ThemeMode::System,
            _ => ThemeMode::Dark,
        }
    }

    pub fn is_dark(&self) -> bool {
        match self {
            ThemeMode::Dark => true,
            ThemeMode::Light => false,
            ThemeMode::System => detect_system_dark(),
        }
    }
}

/// Best-effort detection of the current desktop color scheme without external
/// dependencies. Checks GTK theme hints, a legacy terminal hint, the KDE Plasma
/// color scheme, and finally the GNOME/GTK portal via `gsettings` when
/// available. Defaults to dark so an unknown desktop still has contrast.
fn detect_system_dark() -> bool {
    if let Ok(theme) = std::env::var("GTK_THEME") {
        if theme.to_lowercase().contains("dark") {
            return true;
        }
        if theme.to_lowercase().contains("light") {
            return false;
        }
    }

    if let Ok(fgbg) = std::env::var("COLORFGBG") {
        if let Some(last) = fgbg.split(';').next_back() {
            if last == "0" {
                return true;
            }
        }
    }

    // KDE Plasma does not expose GTK settings; its color scheme name (for
    // example "BreezeDark") is the stable light/dark indicator.
    if let Some(scheme) = kde_color_scheme() {
        let scheme = scheme.to_lowercase();
        if scheme.contains("dark") {
            return true;
        }
        if scheme.contains("light") {
            return false;
        }
    }

    if let Ok(out) = std::process::Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "color-scheme"])
        .output()
    {
        if out.status.success() {
            let scheme = String::from_utf8_lossy(&out.stdout).to_lowercase();
            if scheme.contains("dark") {
                return true;
            }
            if scheme.contains("light") {
                return false;
            }
        }
    }

    true
}

fn kde_color_scheme() -> Option<String> {
    for tool in ["kreadconfig6", "kreadconfig5"] {
        if let Ok(out) = std::process::Command::new(tool)
            .args([
                "--file",
                "kdeglobals",
                "--group",
                "General",
                "--key",
                "ColorScheme",
            ])
            .output()
        {
            if out.status.success() {
                let value = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !value.is_empty() {
                    return Some(value);
                }
            }
        }
    }

    let config_dir = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".config"))
        })?;
    let content = std::fs::read_to_string(config_dir.join("kdeglobals")).ok()?;
    let mut in_general = false;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_general = line == "[General]";
            continue;
        }
        if in_general {
            if let Some(value) = line.strip_prefix("ColorScheme=") {
                let value = value.trim().to_string();
                if !value.is_empty() {
                    return Some(value);
                }
            }
        }
    }
    None
}
