use crate::applications::desktop_entries::DesktopEntryRegistry;
use crate::startup::desktop::DesktopAutostart;
use crate::startup::models::{StartupItem, StartupSource};
use std::collections::HashMap;
use std::fs;

pub struct StartupScanner;

impl StartupScanner {
    pub fn scan_all() -> Vec<StartupItem> {
        let mut items_map: HashMap<String, StartupItem> = HashMap::new();

        // 1. Scan System Autostart Dirs first
        for sys_dir in DesktopAutostart::get_system_autostart_dirs() {
            if sys_dir.exists() {
                if let Ok(entries) = fs::read_dir(&sys_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file()
                            && path.extension().and_then(|s| s.to_str()) == Some("desktop")
                        {
                            if let Ok(mut item) =
                                DesktopAutostart::parse_file(&path, StartupSource::System)
                            {
                                item.icon_path =
                                    DesktopEntryRegistry::resolve_icon_path(&item.icon);
                                items_map.insert(item.file_name.clone(), item);
                            }
                        }
                    }
                }
            }
        }

        // 2. Scan User Autostart Dir (Overrides System entries if matching file_name)
        let user_dir = DesktopAutostart::get_user_autostart_dir();
        if user_dir.exists() {
            if let Ok(entries) = fs::read_dir(&user_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file()
                        && path.extension().and_then(|s| s.to_str()) == Some("desktop")
                    {
                        if let Ok(mut item) =
                            DesktopAutostart::parse_file(&path, StartupSource::User)
                        {
                            item.icon_path = DesktopEntryRegistry::resolve_icon_path(&item.icon);
                            items_map.insert(item.file_name.clone(), item);
                        }
                    }
                }
            }
        }

        let mut items: Vec<StartupItem> = items_map.into_values().collect();
        items.sort_by_key(|a| a.name.to_lowercase());
        items
    }
}
