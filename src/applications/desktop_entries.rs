use crate::applications::models::{ApplicationItem, PackageSource};
use crate::filesystem::xdg::get_user_desktop_dir;
use crate::process::ReadOnlyCommand;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct DesktopEntryInfo {
    pub name: String,
    pub exec: String,
    pub icon: String,
    pub comment: String,
    pub file_path: PathBuf,
    pub no_display: bool,
    pub flatpak_id: Option<String>,
    pub snap_name: Option<String>,
    pub terminal: bool,
    pub dbus_activatable: bool,
    pub working_directory: Option<PathBuf>,
}

pub struct DesktopEntryRegistry;

fn executable_available(exec: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;
    let executable = |p: &Path| {
        fs::metadata(p)
            .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    };
    if exec.contains('/') {
        return Path::new(exec).is_absolute() && executable(Path::new(exec));
    }
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|p| executable(&p.join(exec))))
        .unwrap_or(false)
}

/// Desktop Entry Exec parsing, not shell parsing. Unsupported field-code forms
/// are rejected instead of inventing shell expansion or discarding arguments.
pub fn parse_exec(exec: &str, name: &str, icon: &str, desktop: &Path) -> Result<Vec<String>> {
    let invalid = || {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "unsupported/malformed desktop Exec",
        )
    };
    if exec.contains(['\n', '\r', '\0']) {
        return Err(invalid().into());
    }
    let mut decoded = String::new();
    let mut chars = exec.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next().ok_or_else(invalid)? {
                's' => decoded.push(' '),
                'n' => decoded.push('\n'),
                'r' => decoded.push('\r'),
                't' => decoded.push('\t'),
                '\\' => decoded.push('\\'),
                next => {
                    decoded.push('\\');
                    decoded.push(next);
                }
            }
        } else {
            decoded.push(c);
        }
    }
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quoted = false;
    let mut started = false;
    let mut chars = decoded.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            '\\' => {
                word.push(chars.next().ok_or_else(invalid)?);
                started = true;
            }
            c if c.is_whitespace() && !quoted => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            c => {
                word.push(c);
                started = true;
            }
        }
    }
    if quoted {
        return Err(invalid().into());
    }
    if started {
        words.push(word);
    }
    let mut args = Vec::new();
    for word in words {
        match word.as_str() {
            "%f" | "%F" | "%u" | "%U" => {}
            "%c" => args.push(name.to_string()),
            "%k" => args.push(desktop.to_string_lossy().into_owned()),
            "%i" => {
                if !icon.is_empty() {
                    args.extend(["--icon".into(), icon.into()]);
                }
            }
            _ => {
                let mut expanded = String::new();
                let mut chars = word.chars();
                while let Some(c) = chars.next() {
                    if c == '%' && chars.next() != Some('%') {
                        return Err(invalid().into());
                    }
                    expanded.push(c);
                }
                args.push(expanded);
            }
        }
    }
    if args
        .first()
        .map(|s| s.is_empty() || s.starts_with('-'))
        .unwrap_or(true)
    {
        return Err(invalid().into());
    }
    Ok(args)
}

impl DesktopEntryRegistry {
    pub fn scan_system_entries() -> Result<HashMap<String, DesktopEntryInfo>> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default();
        let data_home = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| home.join(".local/share"));
        let mut dirs = vec![data_home.join("applications")];
        let data_dirs = std::env::var_os("XDG_DATA_DIRS")
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
        dirs.extend(
            std::env::split_paths(&data_dirs)
                .filter(|p| p.is_absolute())
                .map(|p| p.join("applications")),
        );
        dirs.extend([
            data_home.join("flatpak/exports/share/applications"),
            PathBuf::from("/var/lib/flatpak/exports/share/applications"),
            PathBuf::from("/var/lib/snapd/desktop/applications"),
        ]);
        Self::scan_directories(&dirs)
    }

    pub fn scan_directories(dirs: &[PathBuf]) -> Result<HashMap<String, DesktopEntryInfo>> {
        let mut map = HashMap::new();
        let mut seen = std::collections::HashSet::new();
        for dir in dirs {
            let mut paths = Vec::new();
            for entry in walkdir::WalkDir::new(dir).follow_links(false) {
                match entry {
                    Ok(entry) => {
                        let path = entry.into_path();
                        if path.extension().and_then(|s| s.to_str()) == Some("desktop") {
                            paths.push(path);
                        }
                    }
                    Err(error)
                        if error.io_error().map(|e| e.kind())
                            == Some(std::io::ErrorKind::NotFound) => {}
                    Err(error) => return Err(error.into()),
                }
            }
            paths.sort();
            for path in paths {
                let id = path
                    .strip_prefix(dir)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('/', "-");
                // A hidden or malformed high-priority entry still masks lower entries.
                if !seen.insert(id.clone()) {
                    continue;
                }
                match Self::parse_desktop_file(&path) {
                    Ok(info) => {
                        if !info.no_display {
                            map.insert(id, info);
                        }
                    }
                    Err(error)
                        if error.downcast_ref::<std::io::Error>().map(|e| e.kind())
                            == Some(std::io::ErrorKind::PermissionDenied) =>
                    {
                        return Err(error)
                    }
                    Err(_) => {}
                }
            }
        }
        Ok(map)
    }

    pub fn native_entries(source: PackageSource) -> Result<HashMap<String, DesktopEntryInfo>> {
        let mut entries: Vec<_> = Self::scan_system_entries()?.into_values().collect();
        entries.sort_by(|a, b| a.file_path.cmp(&b.file_path));
        let mut owned = HashMap::new();
        for info in entries {
            if info.flatpak_id.is_some() || info.snap_name.is_some() {
                continue;
            }
            let Ok(args) = parse_exec(&info.exec, &info.name, &info.icon, &info.file_path) else {
                continue;
            };
            let binary = args
                .first()
                .and_then(|s| Path::new(s).file_name())
                .and_then(|s| s.to_str())
                .unwrap_or("");
            // A generic interpreter/wrapper never establishes application identity.
            if [
                "flatpak", "snap", "env", "wine", "wine64", "sh", "bash", "python", "python3",
                "java",
            ]
            .contains(&binary)
            {
                continue;
            }
            let path = info.file_path.to_string_lossy();
            let mut command = match source {
                PackageSource::Pacman | PackageSource::Aur => {
                    let mut c = Command::new("pacman");
                    c.args(["-Qqo", "--", &path]);
                    c
                }
                PackageSource::Dpkg => {
                    let mut c = Command::new("dpkg-query");
                    c.args(["-S", &path]);
                    c
                }
                PackageSource::Rpm => {
                    let mut c = Command::new("rpm");
                    c.args(["-qf", "--qf", "%{NAME}\n", "--", &path]);
                    c
                }
                _ => return Ok(owned),
            };
            let output = command.env("LC_ALL", "C").scan_output()?;
            if !output.status.success() {
                continue;
            }
            let text = String::from_utf8_lossy(&output.stdout);
            let owner = if source == PackageSource::Dpkg {
                text.lines().find_map(|line| {
                    line.rsplit_once(": ")
                        .filter(|(_, p)| *p == path)
                        .map(|(o, _)| o)
                })
            } else {
                text.lines().next()
            };
            if let Some(owner) = owner {
                if crate::applications::polkit::validate_package_id(owner).is_ok() {
                    owned.entry(owner.to_lowercase()).or_insert(info);
                }
            }
        }
        Ok(owned)
    }

    pub fn parse_desktop_file(path: &Path) -> Result<DesktopEntryInfo> {
        let content = fs::read_to_string(path)?;
        let mut fields = HashMap::new();
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
                let mut decoded = String::new();
                if matches!(key, "Name" | "Comment" | "Icon" | "Path" | "TryExec")
                    || key.starts_with("Name[")
                    || key.starts_with("Comment[")
                {
                    let mut chars = val.chars();
                    while let Some(c) = chars.next() {
                        decoded.push(if c == '\\' {
                            match chars.next() {
                                Some('s') => ' ',
                                Some('n') => '\n',
                                Some('r') => '\r',
                                Some('t') => '\t',
                                Some('\\') => '\\',
                                _ => {
                                    return Err(std::io::Error::new(
                                        std::io::ErrorKind::InvalidData,
                                        "desktop string escape",
                                    )
                                    .into())
                                }
                            }
                        } else {
                            c
                        });
                    }
                } else {
                    decoded.push_str(val);
                }
                fields.insert(key.to_string(), decoded);
            }
        }

        let value = |key: &str| fields.get(key).cloned().unwrap_or_default();
        let localized = |key: &str| {
            let locale = ["LC_ALL", "LC_MESSAGES", "LANG"]
                .into_iter()
                .find_map(|key| std::env::var(key).ok().filter(|s| !s.is_empty()))
                .unwrap_or_default();
            let locale = if let Some((base, encoding)) = locale.split_once('.') {
                if let Some((_, modifier)) = encoding.split_once('@') {
                    format!("{base}@{modifier}")
                } else {
                    base.to_string()
                }
            } else {
                locale
            };
            let mut candidates = vec![locale.to_string()];
            if let Some((base, modifier)) = locale.split_once('@') {
                candidates.push(base.to_string());
                candidates.push(format!(
                    "{}@{}",
                    base.split('_').next().unwrap_or(base),
                    modifier
                ));
            }
            candidates.push(
                locale
                    .split(['_', '@'])
                    .next()
                    .unwrap_or(&locale)
                    .to_string(),
            );
            candidates
                .into_iter()
                .find_map(|l| fields.get(&format!("{key}[{l}]")).cloned())
                .unwrap_or_else(|| value(key))
        };
        let desktops = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
        let matches = |key: &str| {
            value(key)
                .split(';')
                .filter(|s| !s.is_empty())
                .any(|s| desktops.split(':').any(|d| d == s))
        };
        let try_exec = value("TryExec");
        let no_display = value("Hidden") == "true"
            || value("NoDisplay") == "true"
            || value("Type") != "Application"
            || (!value("OnlyShowIn").is_empty() && !matches("OnlyShowIn"))
            || matches("NotShowIn")
            || (!try_exec.is_empty() && !executable_available(&try_exec));
        let name = localized("Name");
        if name.is_empty() || !in_desktop_entry {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "desktop entry Name/group",
            )
            .into());
        }
        Ok(DesktopEntryInfo {
            name,
            exec: value("Exec"),
            icon: value("Icon"),
            comment: localized("Comment"),
            file_path: path.to_path_buf(),
            no_display,
            flatpak_id: fields.get("X-Flatpak").cloned(),
            snap_name: fields.get("X-SnapInstanceName").cloned(),
            terminal: value("Terminal") == "true",
            dbus_activatable: value("DBusActivatable") == "true",
            working_directory: fields
                .get("Path")
                .filter(|s| !s.is_empty())
                .map(PathBuf::from),
        })
    }

    /// Creates a FreeDesktop compliant desktop shortcut on user's Desktop
    pub fn create_desktop_shortcut(app: &ApplicationItem) -> Result<PathBuf> {
        let desktop_dir = get_user_desktop_dir();
        Self::create_shortcut_in(app, &desktop_dir)
    }

    pub fn create_shortcut_in(app: &ApplicationItem, desktop_dir: &Path) -> Result<PathBuf> {
        fs::create_dir_all(desktop_dir)?;
        // Hex encoding is injective, unlike sanitized display names.
        // Never replace any existing inode, including dangling symlinks.
        let identity: String = app.id.bytes().map(|b| format!("{b:02x}")).collect();
        // Long refs can exceed NAME_MAX. Use a stable hash plus exclusive suffixes.
        let base = if identity.len() <= 180 {
            identity
        } else {
            let hash = app.id.bytes().fold(0xcbf29ce484222325u64, |h, b| {
                (h ^ b as u64).wrapping_mul(0x100000001b3)
            });
            format!("{hash:016x}")
        };
        let content = if app.source != PackageSource::Flatpak {
            app.desktop_file_path
                .as_ref()
                .map(fs::read_to_string)
                .transpose()?
        } else {
            None
        };
        let content = match content {
            Some(content) => content,
            None => {
                let exec = app.exec_cmd.as_deref().context("missing desktop Exec")?;
                parse_exec(exec, &app.name, &app.icon, Path::new(""))?;
                let escape = |s: &str| {
                    s.replace('\\', "\\\\")
                        .replace('\n', "\\n")
                        .replace('\r', "\\r")
                };
                format!("[Desktop Entry]\nType=Application\nName={}\nComment={}\nExec={}\nIcon={}\nTerminal=false\nX-Tidy-Identity={}\n", escape(&app.name), escape(&app.description), exec, escape(&app.icon), escape(&app.id))
            }
        };
        for suffix in 0..1000 {
            let path = desktop_dir.join(format!("tidy-{base}-{suffix}.desktop"));
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
            match options.open(&path) {
                Ok(mut file) => {
                    file.write_all(content.as_bytes())?;
                    file.sync_all()?;
                    use std::os::unix::fs::PermissionsExt;
                    file.set_permissions(fs::Permissions::from_mode(0o755))?;
                    return Ok(path);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.into()),
            }
        }
        Err(std::io::Error::from(std::io::ErrorKind::AlreadyExists).into())
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

        let mut icon_roots = vec![
            PathBuf::from("/usr/share/icons"),
            PathBuf::from("/usr/share/pixmaps"),
            PathBuf::from("/var/lib/flatpak/exports/share/icons"),
        ];

        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            icon_roots.push(home.join(".local/share/icons"));
            icon_roots.push(home.join(".local/share/flatpak/exports/share/icons"));
        }

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
