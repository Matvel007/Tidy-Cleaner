use crate::applications::desktop_entries::{DesktopEntryInfo, DesktopEntryRegistry};
use crate::applications::models::{ApplicationItem, PackageSource};
use crate::applications::polkit::PolkitExecutor;
use crate::applications::traits::PackageManagerProvider;
use crate::process::ReadOnlyCommand;
use anyhow::{bail, Context, Result};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Command;

pub struct PacmanProvider;

impl PacmanProvider {
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

    fn get_explicit_packages() -> Result<HashSet<String>> {
        let mut set = HashSet::new();
        let output = Command::new("pacman").args(["-Qen"]).scan_output()?;
        if !output.status.success()
            && !(output.status.code() == Some(1)
                && output.stdout.is_empty()
                && output.stderr.is_empty())
        {
            bail!("pacman -Qen: {}", String::from_utf8_lossy(&output.stderr));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if let Some(pkg) = line.split_whitespace().next() {
                set.insert(pkg.to_lowercase());
            }
        }
        Ok(set)
    }

    fn parse_installed_packages(
        desktop_entries: &HashMap<String, DesktopEntryInfo>,
    ) -> Result<Vec<ApplicationItem>> {
        let output = Command::new("pacman")
            .args(["-Qn"])
            .scan_output()
            .context("Failed to execute pacman -Qn")?;

        if !output.status.success() {
            if output.status.code() == Some(1)
                && output.stdout.is_empty()
                && output.stderr.is_empty()
            {
                return Ok(Vec::new());
            }
            bail!("pacman -Qn: {}", String::from_utf8_lossy(&output.stderr));
        }

        let explicit_pkgs = Self::get_explicit_packages()?;
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

            let is_explicit = explicit_pkgs.contains(&pkg_lower);

            // If not a desktop app, only keep explicitly installed standalone binaries
            if desktop_info.is_none() {
                if !is_explicit || Self::is_excluded(&pkg_name) {
                    continue;
                }
                let bin_path = format!("/usr/bin/{}", pkg_name);
                if !Path::new(&bin_path).exists() {
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
                id: format!("pacman:{}", pkg_name),
                package_id: pkg_name,
                name,
                version,
                description: desc,
                source: PackageSource::Pacman,
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

impl Default for PacmanProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl PackageManagerProvider for PacmanProvider {
    fn name(&self) -> &'static str {
        "Pacman"
    }

    fn source(&self) -> PackageSource {
        PackageSource::Pacman
    }

    fn is_available(&self) -> bool {
        Command::new("pacman")
            .arg("--version")
            .scan_output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn list_installed(&self) -> Result<Vec<ApplicationItem>> {
        let desktop_entries = DesktopEntryRegistry::native_entries(PackageSource::Pacman)?;
        Self::parse_installed_packages(&desktop_entries)
    }

    fn uninstall(&self, package_id: &str) -> Result<()> {
        crate::applications::polkit::validate_package_id(package_id)?;
        PolkitExecutor::run_with_pkexec("pacman", &["-R", "--noconfirm", "--", package_id])
    }

    fn get_details(&self, package_id: &str) -> Result<Option<String>> {
        let output = Command::new("pacman")
            .args(["-Qi", package_id])
            .scan_output()
            .context("Failed to get pacman package details")?;

        if output.status.success() {
            Ok(Some(String::from_utf8_lossy(&output.stdout).to_string()))
        } else {
            bail!("pacman -Qi: {}", String::from_utf8_lossy(&output.stderr))
        }
    }
}
