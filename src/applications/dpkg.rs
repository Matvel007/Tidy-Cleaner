use crate::applications::desktop_entries::{DesktopEntryInfo, DesktopEntryRegistry};
use crate::applications::models::{ApplicationItem, PackageSource};
use crate::applications::polkit::PolkitExecutor;
use crate::applications::traits::PackageManagerProvider;
use anyhow::{bail, Context, Result};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

pub struct DpkgProvider;

impl DpkgProvider {
    pub fn new() -> Self {
        Self
    }

    fn is_excluded(pkg_name: &str) -> bool {
        let lower = pkg_name.to_lowercase();
        lower.starts_with("lib")
            || lower.starts_with("linux-")
            || lower.starts_with("gir1.2-")
            || lower.starts_with("python3-")
            || lower.starts_with("fonts-")
            || lower.ends_with("-theme")
            || lower.ends_with("-data")
            || lower.ends_with("-common")
    }

    fn parse_installed_packages(
        desktop_entries: &HashMap<String, DesktopEntryInfo>,
    ) -> Result<Vec<ApplicationItem>> {
        let output = Command::new("dpkg-query")
            .args(["-W", "-f=${Package}\t${Version}\t${Status}\n"])
            .output()
            .context("Failed to execute dpkg-query")?;

        if !output.status.success() {
            bail!("dpkg-query returned non-zero status");
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut apps = Vec::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() < 3 {
                continue;
            }
            let status = parts[2];
            if !status.contains("installed") {
                continue;
            }

            let pkg_name = parts[0].to_string();
            let version = parts[1].to_string();
            let pkg_lower = pkg_name.to_lowercase();
            let norm_pkg = pkg_lower.replace(['-', '_', '.'], "");

            let desktop_info = desktop_entries
                .get(&pkg_lower)
                .or_else(|| desktop_entries.get(&norm_pkg));

            if desktop_info.is_none() {
                if Self::is_excluded(&pkg_name) {
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
                id: format!("dpkg:{}", pkg_name),
                package_id: pkg_name,
                name,
                version,
                description: desc,
                source: PackageSource::Dpkg,
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

impl Default for DpkgProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl PackageManagerProvider for DpkgProvider {
    fn name(&self) -> &'static str {
        "APT / Dpkg"
    }

    fn source(&self) -> PackageSource {
        PackageSource::Dpkg
    }

    fn is_available(&self) -> bool {
        Command::new("dpkg-query")
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
        PolkitExecutor::run_with_pkexec("apt-get", &["remove", "-y", package_id])
    }

    fn get_details(&self, package_id: &str) -> Result<Option<String>> {
        let output = Command::new("apt-cache")
            .args(["show", package_id])
            .output()
            .context("Failed to get apt package details")?;

        if output.status.success() {
            Ok(Some(String::from_utf8_lossy(&output.stdout).to_string()))
        } else {
            Ok(None)
        }
    }
}
