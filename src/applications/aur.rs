use crate::applications::desktop_entries::{DesktopEntryInfo, DesktopEntryRegistry};
use crate::applications::models::{ApplicationItem, PackageSource};
use crate::applications::polkit::PolkitExecutor;
use crate::applications::traits::PackageManagerProvider;
use crate::process::ReadOnlyCommand;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

pub struct AurProvider;

impl AurProvider {
    pub fn new() -> Self {
        Self
    }

    fn is_excluded(pkg_name: &str) -> bool {
        let lower = pkg_name.to_lowercase();
        lower.starts_with("ttf-")
            || lower.starts_with("otf-")
            || lower.starts_with("noto-")
            || lower.starts_with("font-")
            || lower.ends_with("-theme")
            || lower.ends_with("-icon-theme")
            || lower.ends_with("-icons")
            || lower.ends_with("-cursor")
            || lower.ends_with("-cursors")
            || lower.ends_with("-wallpapers")
            || lower.starts_with("caestelia-")
            || lower.starts_with("kvantum-")
            || lower.starts_with("plasma5-themes-")
            || lower.starts_with("plasma6-themes-")
            || lower.starts_with("gtk-theme-")
            || lower.starts_with("lib32-")
            || lower.starts_with("lib")
            || lower.starts_with("glibc")
            || lower.starts_with("linux-")
            || lower.contains("darkly")
    }

    fn parse_aur_packages(
        desktop_entries: &HashMap<String, DesktopEntryInfo>,
    ) -> Result<Vec<ApplicationItem>> {
        let output = Command::new("pacman")
            .args(["-Qm"])
            .scan_output()
            .context("Failed to execute pacman -Qm")?;

        if !output.status.success() {
            if output.status.code() == Some(1)
                && output.stdout.is_empty()
                && output.stderr.is_empty()
            {
                return Ok(Vec::new());
            }
            anyhow::bail!("pacman -Qm: {}", String::from_utf8_lossy(&output.stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut apps = Vec::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 2 {
                continue;
            }
            let pkg_name = parts[0].to_string();
            let version = parts[1].to_string();
            let pkg_lower = pkg_name.to_lowercase();
            let desktop_info = desktop_entries.get(&pkg_lower);

            // Filter out non-target programs
            if desktop_info.is_none() {
                if Self::is_excluded(&pkg_name) {
                    continue;
                }
                let bin_path = format!("/usr/bin/{}", pkg_name);
                let bin_path_nobin = format!("/usr/bin/{}", pkg_name.trim_end_matches("-bin"));
                if !Path::new(&bin_path).exists() && !Path::new(&bin_path_nobin).exists() {
                    continue;
                }
            }

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
                    (
                        pkg_name.clone(),
                        String::new(),
                        Some(pkg_name.clone()),
                        None,
                        false,
                        String::new(),
                    )
                };

            let icon_path = DesktopEntryRegistry::resolve_icon_path(&icon);

            apps.push(ApplicationItem {
                id: format!("aur:{}", pkg_name),
                package_id: pkg_name,
                name,
                version,
                description: desc,
                source: PackageSource::Aur,
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

impl Default for AurProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl PackageManagerProvider for AurProvider {
    fn name(&self) -> &'static str {
        "AUR"
    }

    fn source(&self) -> PackageSource {
        PackageSource::Aur
    }

    fn is_available(&self) -> bool {
        Command::new("pacman")
            .arg("--version")
            .scan_output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn list_installed(&self) -> Result<Vec<ApplicationItem>> {
        let desktop_entries = DesktopEntryRegistry::native_entries(PackageSource::Aur)?;
        Self::parse_aur_packages(&desktop_entries)
    }

    fn uninstall(&self, package_id: &str) -> Result<()> {
        crate::applications::polkit::validate_package_id(package_id)?;
        PolkitExecutor::run_with_pkexec("pacman", &["-R", "--noconfirm", "--", package_id])
    }

    fn get_details(&self, package_id: &str) -> Result<Option<String>> {
        let output = Command::new("pacman")
            .args(["-Qi", package_id])
            .scan_output()
            .context("Failed to get AUR package details")?;

        if output.status.success() {
            Ok(Some(String::from_utf8_lossy(&output.stdout).to_string()))
        } else {
            anyhow::bail!("pacman -Qi: {}", String::from_utf8_lossy(&output.stderr))
        }
    }
}
