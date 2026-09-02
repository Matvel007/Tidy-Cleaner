use crate::applications::models::ApplicationItem;
use crate::filesystem::xdg::get_user_desktop_dir;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct DesktopEntryInfo {
    pub name: String,
    pub exec: String,
    pub icon: String,
    pub comment: String,
    pub file_path: PathBuf,
    pub no_display: bool,
}

pub struct DesktopEntryRegistry;

impl DesktopEntryRegistry {
    pub fn scan_system_entries() -> HashMap<String, DesktopEntryInfo> {
        let mut map = HashMap::new();
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/home/anonim"));

        let dirs = vec![
            PathBuf::from("/usr/share/applications"),
            home.join(".local/share/applications"),
            PathBuf::from("/var/lib/flatpak/exports/share/applications"),
            home.join(".local/share/flatpak/exports/share/applications"),
        ];

        for dir in dirs {
            if !dir.exists() {
                continue;
            }
            if let Ok(entries) = fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file()
                        && path.extension().and_then(|s| s.to_str()) == Some("desktop")
                    {
                        if let Ok(info) = Self::parse_desktop_file(&path) {
                            if !info.no_display {
                                let stem = path
                                    .file_stem()
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("")
                                    .to_lowercase();
                                map.insert(stem.clone(), info.clone());

                                // Also index by normalized stem without hyphens / dots
                                let norm_stem = stem.replace(['-', '_', '.'], "");
                                if !norm_stem.is_empty() && !map.contains_key(&norm_stem) {
                                    map.insert(norm_stem, info.clone());
                                }

                                // Also index by binary name in exec
                                let binary_name = info
                                    .exec
                                    .split_whitespace()
                                    .next()
                                    .and_then(|e| {
                                        let cleaned = e.trim_matches('"').trim_matches('\'');
                                        Path::new(cleaned).file_name()
                                    })
                                    .and_then(|s| s.to_str())
                                    .unwrap_or("")
                                    .to_lowercase();
                                if !binary_name.is_empty() && !map.contains_key(&binary_name) {
                                    map.insert(binary_name.clone(), info.clone());
                                    let norm_bin = binary_name.replace(['-', '_', '.'], "");
                                    if !norm_bin.is_empty() && !map.contains_key(&norm_bin) {
                                        map.insert(norm_bin, info.clone());
                                    }
                                }

                                // Index by name normalized
                                let norm_name =
                                    info.name.to_lowercase().replace([' ', '-', '_', '.'], "");
                                if !norm_name.is_empty() && !map.contains_key(&norm_name) {
                                    map.insert(norm_name, info);
                                }
                            }
                        }
                    }
                }
            }
        }

        map
    }

    pub fn parse_desktop_file(path: &Path) -> Result<DesktopEntryInfo> {
        let content = fs::read_to_string(path)?;
        let mut name = String::new();
        let mut exec = String::new();
        let mut icon = String::new();
        let mut comment = String::new();
        let mut no_display = false;
        let mut in_desktop_entry = false;

        for line in content.lines() {
            let line = line.trim();
            if line == "[Desktop Entry]" {
                in_desktop_entry = true;
                continue;
            } else if line.starts_with('[') && in_desktop_entry {
                // Another section starts
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
                    "Exec" if exec.is_empty() => {
                        let cleaned = val
                            .split_whitespace()
                            .filter(|w| !w.starts_with('%'))
                            .collect::<Vec<_>>()
                            .join(" ");
                        exec = cleaned;
                    }
                    "Icon" if icon.is_empty() => icon = val.to_string(),
                    "Comment" if comment.is_empty() => comment = val.to_string(),
                    "NoDisplay" => no_display = val.eq_ignore_ascii_case("true"),
                    _ => {}
                }
            }
        }

        if name.is_empty() {
            name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown App")
                .to_string();
        }

        Ok(DesktopEntryInfo {
            name,
            exec,
            icon,
            comment,
            file_path: path.to_path_buf(),
            no_display,
        })
    }

    /// Creates a FreeDesktop compliant desktop shortcut on user's Desktop
    pub fn create_desktop_shortcut(app: &ApplicationItem) -> Result<PathBuf> {
        let desktop_dir = get_user_desktop_dir();
        if !desktop_dir.exists() {
            fs::create_dir_all(&desktop_dir).with_context(|| {
                format!("Failed to create desktop directory: {:?}", desktop_dir)
            })?;
        }

        // Clean filename for desktop shortcut
        let safe_name: String = app
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
        let target_file = desktop_dir.join(format!("{}.desktop", safe_name));

        // If existing desktop file is available, copy and ensure executable
        if let Some(orig_path) = &app.desktop_file_path {
            if orig_path.is_file() {
                if let Ok(orig_content) = fs::read_to_string(orig_path) {
                    let mut updated_content = String::new();
                    for line in orig_content.lines() {
                        if line.starts_with("Icon=") && !app.icon.is_empty() {
                            updated_content.push_str(&format!("Icon={}\n", app.icon));
                        } else {
                            updated_content.push_str(line);
                            updated_content.push('\n');
                        }
                    }
                    fs::write(&target_file, updated_content)?;

                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        if let Ok(metadata) = fs::metadata(&target_file) {
                            let mut perms = metadata.permissions();
                            perms.set_mode(0o755);
                            let _ = fs::set_permissions(&target_file, perms);
                        }
                    }
                    return Ok(target_file);
                }
            }
        }

        let exec_str = app
            .exec_cmd
            .clone()
            .unwrap_or_else(|| app.package_id.clone());

        let icon_str = if !app.icon.is_empty() {
            app.icon.clone()
        } else if let Some(icon_path) = &app.icon_path {
            icon_path.to_string_lossy().to_string()
        } else {
            "application-x-executable".to_string()
        };

        let content = format!(
            "[Desktop Entry]\n\
            Version=1.0\n\
            Type=Application\n\
            Name={}\n\
            Comment={}\n\
            Exec={}\n\
            Icon={}\n\
            Terminal=false\n\
            StartupNotify=true\n\
            Categories=Utility;\n",
            app.name, app.description, exec_str, icon_str
        );

        fs::write(&target_file, content)
            .with_context(|| format!("Failed to write desktop shortcut at {:?}", target_file))?;

        // Set executable permissions on Linux
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

    /// Resolves an icon name to an actual on-disk file path according to FreeDesktop Icon Spec
    pub fn resolve_icon_path(icon_name: &str) -> Option<PathBuf> {
        let icon_name = icon_name.trim();
        if icon_name.is_empty() {
            return None;
        }

        let direct = PathBuf::from(icon_name);
        if direct.is_file() {
            return Some(direct);
        }

        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/home/anonim"));

        let icon_roots = [
            PathBuf::from("/usr/share/icons"),
            home.join(".local/share/icons"),
            PathBuf::from("/var/lib/flatpak/exports/share/icons"),
            home.join(".local/share/flatpak/exports/share/icons"),
        ];

        let sizes = [
            "scalable", "64x64", "48x48", "128x128", "256x256", "32x32", "512x512", "apps",
        ];
        let extensions = ["svg", "png", "xpm"];

        // 1. Search in hicolor
        for root in &icon_roots {
            let hicolor = root.join("hicolor");
            if hicolor.exists() {
                for size in &sizes {
                    for ext in &extensions {
                        let candidate = hicolor
                            .join(size)
                            .join("apps")
                            .join(format!("{}.{}", icon_name, ext));
                        if candidate.is_file() {
                            return Some(candidate);
                        }
                    }
                }
            }
        }

        // 2. Search in all installed themes (Papirus, Breeze, Adwaita, etc.)
        for root in &icon_roots {
            if let Ok(entries) = fs::read_dir(root) {
                for entry in entries.flatten() {
                    let theme_dir = entry.path();
                    if theme_dir.is_dir() {
                        for size in &sizes {
                            for ext in &extensions {
                                let candidate = theme_dir
                                    .join(size)
                                    .join("apps")
                                    .join(format!("{}.{}", icon_name, ext));
                                if candidate.is_file() {
                                    return Some(candidate);
                                }
                                let candidate_alt = theme_dir
                                    .join("apps")
                                    .join(size)
                                    .join(format!("{}.{}", icon_name, ext));
                                if candidate_alt.is_file() {
                                    return Some(candidate_alt);
                                }
                            }
                        }
                    }
                }
            }
        }

        // 3. Search in /usr/share/pixmaps
        let pixmaps = PathBuf::from("/usr/share/pixmaps");
        for ext in &["png", "svg", "xpm"] {
            let candidate = pixmaps.join(format!("{}.{}", icon_name, ext));
            if candidate.is_file() {
                return Some(candidate);
            }
        }

        None
    }
}
