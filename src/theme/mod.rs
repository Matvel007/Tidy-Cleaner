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
/// dependencies. Checks GTK theme hints, a legacy terminal hint, and finally
/// the GNOME/GTK portal via `gsettings` when available. Defaults to dark.
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
