use crate::applications::desktop_entries::DesktopEntryRegistry;
use crate::applications::models::{ApplicationItem, PackageSource};
use crate::applications::traits::PackageManagerProvider;
use anyhow::{Context, Result};
use std::process::Command;

pub struct FlatpakProvider;

impl FlatpakProvider {
    pub fn new() -> Self {
        Self
    }

    fn parse_flatpak_list() -> Result<Vec<ApplicationItem>> {
        let output = Command::new("flatpak")
            .args([
                "list",
                "--app",
                "--columns=name,application,version,description,size,origin",
            ])
            .output()
            .context("Failed to execute flatpak list")?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let desktop_entries = DesktopEntryRegistry::scan_system_entries();
        let mut apps = Vec::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() < 2 {
                continue;
            }
            let name = parts[0].trim().to_string();
            let app_id = parts[1].trim().to_string();
            let version = parts
                .get(2)
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            let description = parts
                .get(3)
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            let size_str = parts
                .get(4)
                .map(|s| s.trim().to_string())
                .unwrap_or_default();

            let desktop_info = desktop_entries.get(&app_id.to_lowercase());
            let (icon, exec_cmd, desktop_file_path) = if let Some(info) = desktop_info {
                (
                    info.icon.clone(),
                    Some(format!("flatpak run {}", app_id)),
                    Some(info.file_path.clone()),
                )
            } else {
                (
                    app_id.clone(),
                    Some(format!("flatpak run {}", app_id)),
                    None,
                )
            };

            let icon_path = DesktopEntryRegistry::resolve_icon_path(&icon);

            apps.push(ApplicationItem {
                id: format!("flatpak:{}", app_id),
                package_id: app_id,
                name,
                version,
                description,
                source: PackageSource::Flatpak,
                icon,
                icon_path,
                exec_cmd,
                installed_size_bytes: None,
                size_formatted: size_str,
                desktop_file_path,
                is_desktop_app: true,
                selected: false,
            });
        }

        Ok(apps)
    }
}

impl Default for FlatpakProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl PackageManagerProvider for FlatpakProvider {
    fn name(&self) -> &'static str {
        "Flatpak"
    }

    fn source(&self) -> PackageSource {
        PackageSource::Flatpak
    }

    fn is_available(&self) -> bool {
        Command::new("flatpak")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn list_installed(&self) -> Result<Vec<ApplicationItem>> {
        if !self.is_available() {
            return Ok(Vec::new());
        }
        Self::parse_flatpak_list()
    }

    fn uninstall(&self, package_id: &str) -> Result<()> {
        let output = Command::new("flatpak")
            .args(["uninstall", "-y", package_id])
            .output()
            .context("Failed to execute flatpak uninstall")?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Flatpak uninstall failed: {}", err.trim());
        }
        Ok(())
    }

    fn get_details(&self, package_id: &str) -> Result<Option<String>> {
        let output = Command::new("flatpak")
            .args(["info", package_id])
            .output()
            .context("Failed to get flatpak info")?;

        if output.status.success() {
            Ok(Some(String::from_utf8_lossy(&output.stdout).to_string()))
        } else {
            Ok(None)
        }
    }
}
