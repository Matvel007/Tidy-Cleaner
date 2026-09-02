use std::path::PathBuf;
use std::process::Command;

pub struct FileDialog;

impl FileDialog {
    pub fn pick_file() -> Option<PathBuf> {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home".to_string());

        // 1. Try kdialog (KDE Plasma)
        if let Ok(output) = Command::new("kdialog")
            .arg("--getopenfilename")
            .arg(&home)
            .output()
        {
            if output.status.success() {
                let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path_str.is_empty() {
                    return Some(PathBuf::from(path_str));
                }
            }
        }

        // 2. Try zenity (GNOME / GTK)
        if let Ok(output) = Command::new("zenity")
            .arg("--file-selection")
            .arg(format!("--filename={}/", home))
            .output()
        {
            if output.status.success() {
                let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path_str.is_empty() {
                    return Some(PathBuf::from(path_str));
                }
            }
        }

        None
    }
}
