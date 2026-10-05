use crate::applications::desktop_entries::DesktopEntryRegistry;
use crate::applications::models::{ApplicationItem, PackageSource};
use crate::applications::traits::PackageManagerProvider;
use crate::process::ReadOnlyCommand;
use anyhow::{Context, Result};
use std::process::Command;

pub struct FlatpakProvider;

/// Installation plus full app ref is the mutation/launch identity, not an app ID.
pub fn target_args(target: &str) -> Result<(String, String)> {
    let invalid = || {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "invalid Flatpak installation/ref",
        )
    };
    let (installation, reference) = target.split_once('|').ok_or_else(invalid)?;
    let parts: Vec<_> = reference.split('/').collect();
    if parts.len() != 4
        || parts[0] != "app"
        || parts[1..].iter().any(|s| {
            s.is_empty()
                || s.starts_with('-')
                || !s
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        })
    {
        return Err(invalid().into());
    }
    let scope = match installation {
        "user" => "--user".to_string(),
        "system" => "--system".to_string(),
        _ if !installation.is_empty()
            && installation
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b)) =>
        {
            format!("--installation={installation}")
        }
        _ => return Err(invalid().into()),
    };
    Ok((scope, reference.to_string()))
}

impl FlatpakProvider {
    pub fn new() -> Self {
        Self
    }

    pub fn parse_listing(text: &str) -> Result<Vec<ApplicationItem>> {
        let mut apps = Vec::new();
        for line in text.lines().filter(|s| !s.trim().is_empty()) {
            let fields: Vec<_> = line.split('\t').collect();
            if fields.len() < 6 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "flatpak list columns",
                )
                .into());
            }
            let reference = if fields[0].starts_with("app/") {
                fields[0].to_string()
            } else {
                format!("app/{}", fields[0])
            };
            let target = format!("{}|{reference}", fields[1]);
            let (scope, reference) = target_args(&target)?;
            let app_id = reference.split('/').nth(1).unwrap();
            let icon = app_id.to_string();
            // Generic exported launchers may point to a different branch/scope.
            // Always use the exact ref for launch and shortcut, even when the
            // global desktop registry has an app with the same display name.
            apps.push(ApplicationItem {
                id: format!("flatpak:{target}"),
                package_id: target,
                name: fields[2].to_string(),
                version: fields[3].to_string(),
                description: fields[4].to_string(),
                source: PackageSource::Flatpak,
                icon_path: DesktopEntryRegistry::resolve_icon_path(&icon),
                icon,
                exec_cmd: Some(format!("flatpak run {scope} {reference}")),
                desktop_file_path: None,
                installed_size_bytes: None,
                size_formatted: fields[5].to_string(),
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
            .scan_output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    fn list_installed(&self) -> Result<Vec<ApplicationItem>> {
        let output = Command::new("flatpak")
            .args([
                "list",
                "--app",
                "--columns=ref:f,installation:f,name:f,version:f,description:f,size:f",
            ])
            .scan_output()
            .context("flatpak list")?;
        if !output.status.success() {
            anyhow::bail!("flatpak list: {}", String::from_utf8_lossy(&output.stderr));
        }
        Self::parse_listing(&String::from_utf8_lossy(&output.stdout))
    }
    fn uninstall(&self, package_id: &str) -> Result<()> {
        let (scope, reference) = target_args(package_id)?;
        // No --unused or --delete-data. A mutation is allowed to finish, not
        // arbitrarily killed during a deployment/database update.
        let output = Command::new("flatpak")
            .args([
                "uninstall",
                "--noninteractive",
                "--assumeyes",
                &scope,
                &reference,
            ])
            .output()
            .context("flatpak uninstall")?;
        if !output.status.success() {
            anyhow::bail!(
                "flatpak uninstall: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Ok(())
    }
    fn get_details(&self, package_id: &str) -> Result<Option<String>> {
        let (scope, reference) = target_args(package_id)?;
        let output = Command::new("flatpak")
            .args(["info", &scope, &reference])
            .scan_output()?;
        if !output.status.success() {
            anyhow::bail!("flatpak info: {}", String::from_utf8_lossy(&output.stderr));
        }
        Ok(Some(String::from_utf8_lossy(&output.stdout).into_owned()))
    }
}
