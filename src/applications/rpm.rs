use crate::applications::desktop_entries::{DesktopEntryInfo, DesktopEntryRegistry};
use crate::applications::models::{ApplicationItem, PackageSource};
use crate::applications::traits::PackageManagerProvider;
use crate::process::ReadOnlyCommand;
use anyhow::{bail, Context, Result};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

pub struct RpmProvider;

impl RpmProvider {
    pub fn new() -> Self {
        Self
    }

    fn is_excluded(pkg_name: &str) -> bool {
        let lower = pkg_name.to_lowercase();
        lower.starts_with("lib")
            || lower.starts_with("kernel-")
            || lower.starts_with("fonts-")
            || lower.ends_with("-theme")
            || lower.ends_with("-filesystem")
    }

    fn parse_installed_packages(
        desktop_entries: &HashMap<String, DesktopEntryInfo>,
    ) -> Result<Vec<ApplicationItem>> {
        let output = Command::new("rpm")
            .args(["-qa", "--qf", "%{NAME}\t%{VERSION}-%{RELEASE}\n"])
            .scan_output()
            .context("Failed to execute rpm -qa")?;

        if !output.status.success() {
            bail!("rpm returned non-zero status");
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut apps = Vec::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() < 2 {
                continue;
            }
            let pkg_name = parts[0].to_string();
            let version = parts[1].to_string();
            let pkg_lower = pkg_name.to_lowercase();
            let desktop_info = desktop_entries.get(&pkg_lower);

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
                id: format!("rpm:{}", pkg_name),
                package_id: pkg_name,
                name,
                version,
                description: desc,
                source: PackageSource::Rpm,
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

impl Default for RpmProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl PackageManagerProvider for RpmProvider {
    fn name(&self) -> &'static str {
        "RPM / DNF"
    }

    fn source(&self) -> PackageSource {
        PackageSource::Rpm
    }

    fn is_available(&self) -> bool {
        Command::new("rpm")
            .arg("--version")
            .scan_output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn list_installed(&self) -> Result<Vec<ApplicationItem>> {
        let desktop_entries = DesktopEntryRegistry::native_entries(PackageSource::Rpm)?;
        Self::parse_installed_packages(&desktop_entries)
    }

    fn uninstall(&self, package_id: &str) -> Result<()> {
        // Inventory is supported, mutation is not: neither deployment ownership
        // nor a complete approved transaction is represented by this interface.
        Err(std::io::Error::new(std::io::ErrorKind::Unsupported,
            format!("RPM inventory is read-only (mutable/OSTree/transactional); no approved transaction capability: {package_id}")).into())
    }

    fn get_details(&self, package_id: &str) -> Result<Option<String>> {
        let output = Command::new("rpm")
            .args(["-qi", package_id])
            .scan_output()
            .context("Failed to get rpm package details")?;

        if output.status.success() {
            Ok(Some(String::from_utf8_lossy(&output.stdout).to_string()))
        } else {
            bail!("rpm -qi: {}", String::from_utf8_lossy(&output.stderr))
        }
    }
}
