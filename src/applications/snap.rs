use crate::applications::desktop_entries::{DesktopEntryInfo, DesktopEntryRegistry};
use crate::applications::models::{ApplicationItem, PackageSource};
use crate::applications::polkit::PolkitExecutor;
use crate::applications::traits::PackageManagerProvider;
use crate::process::ReadOnlyCommand;
use anyhow::{bail, Context, Result};
use std::collections::HashMap;
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
            .scan_output()
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

            let desktop_info = desktop_entries
                .values()
                .filter(|info| info.snap_name.as_deref() == Some(snap_name.as_str()))
                .min_by(|a, b| a.file_path.cmp(&b.file_path));
            // Bases/content runtimes do not export applications. In particular,
            // gnome-calculator is an app, not a gnome runtime by name prefix.
            let Some(desktop_info) = desktop_info else {
                continue;
            };

            let (name, icon, exec_cmd, desktop_file_path, is_desktop, desc) = {
                let info = desktop_info;
                (
                    info.name.clone(),
                    info.icon.clone(),
                    Some(info.exec.clone()),
                    Some(info.file_path.clone()),
                    true,
                    info.comment.clone(),
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
            .scan_output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn list_installed(&self) -> Result<Vec<ApplicationItem>> {
        let desktop_entries = DesktopEntryRegistry::scan_system_entries()?;
        Self::parse_installed_packages(&desktop_entries)
    }

    fn uninstall(&self, package_id: &str) -> Result<()> {
        crate::applications::polkit::validate_package_id(package_id)?;
        PolkitExecutor::run_with_pkexec("snap", &["remove", package_id])
    }

    fn get_details(&self, package_id: &str) -> Result<Option<String>> {
        let output = Command::new("snap")
            .args(["info", package_id])
            .scan_output()
            .context("Failed to get snap details")?;

        if output.status.success() {
            Ok(Some(String::from_utf8_lossy(&output.stdout).to_string()))
        } else {
            bail!("snap info: {}", String::from_utf8_lossy(&output.stderr))
        }
    }
}
