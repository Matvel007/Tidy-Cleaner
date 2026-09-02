use crate::applications::desktop_entries::{DesktopEntryInfo, DesktopEntryRegistry};
use crate::applications::models::{ApplicationItem, PackageSource};
use crate::applications::polkit::PolkitExecutor;
use crate::applications::traits::PackageManagerProvider;
use anyhow::{bail, Context, Result};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

pub struct SnapProvider;

impl SnapProvider {
    pub fn new() -> Self {
        Self
    }

    fn parse_installed_packages(
        desktop_entries: &HashMap<String, DesktopEntryInfo>,
    ) -> Result<Vec<ApplicationItem>> {
        let output = Command::new("snap")
            .args(["list"])
            .output()
            .context("Failed to execute snap list")?;

        if !output.status.success() {
            bail!("snap returned non-zero status");
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut apps = Vec::new();

        for line in stdout.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }
            let snap_name = parts[0].to_string();
            let version = parts[1].to_string();

            // Skip base snaps
            if snap_name == "core"
                || snap_name.starts_with("core")
                || snap_name.starts_with("bare")
                || snap_name.starts_with("gnome-")
                || snap_name.starts_with("gtk-common-")
            {
                continue;
            }

            let snap_lower = snap_name.to_lowercase();
            let norm_snap = snap_lower.replace(['-', '_', '.'], "");

            let desktop_info = desktop_entries
                .get(&snap_lower)
                .or_else(|| desktop_entries.get(&norm_snap));

            let (name, icon, exec_cmd, desktop_file_path, is_desktop, desc) =
                if let Some(info) = desktop_info {
                    (
                        info.name.clone(),
                        info.icon.clone(),
                        Some(info.exec.clone()),
                        Some(info.file_path.clone()),
                        true,
                        info.comment.clone(),
                    )
                } else {
                    let snap_bin = format!("/snap/bin/{}", snap_name);
                    let is_bin = Path::new(&snap_bin).exists();
                    (
                        snap_name.clone(),
                        String::new(),
                        if is_bin {
                            Some(snap_bin)
                        } else {
                            Some(snap_name.clone())
                        },
                        None,
                        is_bin,
                        String::new(),
                    )
                };

            let icon_path = DesktopEntryRegistry::resolve_icon_path(&icon);

            apps.push(ApplicationItem {
                id: format!("snap:{}", snap_name),
                package_id: snap_name,
                name,
                version,
                description: desc,
                source: PackageSource::Snap,
                icon,
                icon_path,
                exec_cmd,
                installed_size_bytes: None,
                size_formatted: String::new(),
                desktop_file_path,
                is_desktop_app: is_desktop,
                selected: false,
            });
        }

        Ok(apps)
    }
}

impl Default for SnapProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl PackageManagerProvider for SnapProvider {
    fn name(&self) -> &'static str {
        "Snap"
    }

    fn source(&self) -> PackageSource {
        PackageSource::Snap
    }

    fn is_available(&self) -> bool {
        Command::new("snap")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn list_installed(&self) -> Result<Vec<ApplicationItem>> {
        if !self.is_available() {
            return Ok(Vec::new());
        }
        let desktop_entries = DesktopEntryRegistry::scan_system_entries();
        Self::parse_installed_packages(&desktop_entries)
    }

    fn uninstall(&self, package_id: &str) -> Result<()> {
        PolkitExecutor::run_with_pkexec("snap", &["remove", package_id])
    }

    fn get_details(&self, package_id: &str) -> Result<Option<String>> {
        let output = Command::new("snap")
            .args(["info", package_id])
            .output()
            .context("Failed to get snap details")?;

        if output.status.success() {
            Ok(Some(String::from_utf8_lossy(&output.stdout).to_string()))
        } else {
            Ok(None)
        }
    }
}
