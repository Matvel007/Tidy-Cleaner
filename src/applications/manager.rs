use crate::applications::aur::AurProvider;
use crate::applications::desktop_entries::DesktopEntryRegistry;
use crate::applications::dpkg::DpkgProvider;
use crate::applications::flatpak::FlatpakProvider;
use crate::applications::models::{ApplicationItem, PackageSource};
use crate::applications::pacman::PacmanProvider;
use crate::applications::rpm::RpmProvider;
use crate::applications::snap::SnapProvider;
use crate::applications::traits::PackageManagerProvider;
use anyhow::{bail, Result};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

#[allow(dead_code)]
#[derive(Clone)]
pub struct ApplicationManager {
    providers: Vec<Arc<dyn PackageManagerProvider>>,
}

#[allow(dead_code)]
impl ApplicationManager {
    pub fn new() -> Self {
        let providers: Vec<Arc<dyn PackageManagerProvider>> = vec![
            Arc::new(PacmanProvider::new()),
            Arc::new(AurProvider::new()),
            Arc::new(FlatpakProvider::new()),
            Arc::new(DpkgProvider::new()),
            Arc::new(RpmProvider::new()),
            Arc::new(SnapProvider::new()),
        ];

        Self { providers }
    }

    pub fn list_all(&self) -> Vec<ApplicationItem> {
        let mut all_apps = Vec::new();

        for provider in &self.providers {
            if provider.is_available() {
                if let Ok(apps) = provider.list_installed() {
                    all_apps.extend(apps);
                }
            }
        }

        let mut seen_ids = std::collections::HashSet::new();
        all_apps.retain(|app| seen_ids.insert(app.id.clone()));

        // Sort: desktop apps first, then alphabetically by name
        all_apps.sort_by(|a, b| {
            if a.is_desktop_app != b.is_desktop_app {
                b.is_desktop_app.cmp(&a.is_desktop_app)
            } else {
                a.name.to_lowercase().cmp(&b.name.to_lowercase())
            }
        });

        all_apps
    }

    pub fn filter_apps(
        apps: &[ApplicationItem],
        query: &str,
        source_filter: Option<PackageSource>,
    ) -> Vec<ApplicationItem> {
        let query_lower = query.trim().to_lowercase();

        apps.iter()
            .filter(|app| {
                // Source filter
                if let Some(src) = source_filter {
                    if app.source != src {
                        return false;
                    }
                }

                // Query filter
                if !query_lower.is_empty() {
                    let matches_name = app.name.to_lowercase().contains(&query_lower);
                    let matches_pkg = app.package_id.to_lowercase().contains(&query_lower);
                    let matches_desc = app.description.to_lowercase().contains(&query_lower);
                    if !matches_name && !matches_pkg && !matches_desc {
                        return false;
                    }
                }

                true
            })
            .cloned()
            .collect()
    }

    pub fn paginate_apps(
        apps: &[ApplicationItem],
        page: usize,
        page_size: usize,
    ) -> (Vec<ApplicationItem>, usize, usize) {
        let total_items = apps.len();
        let total_pages = if total_items == 0 {
            1
        } else {
            total_items.div_ceil(page_size)
        };

        let current_page = page.clamp(1, total_pages);
        let start_idx = (current_page - 1) * page_size;
        let end_idx = (start_idx + page_size).min(total_items);

        let slice = if start_idx < total_items {
            apps[start_idx..end_idx].to_vec()
        } else {
            Vec::new()
        };

        (slice, current_page, total_pages)
    }

    pub fn launch_app(app: &ApplicationItem) -> Result<()> {
        if let Some(ref desktop_path) = app.desktop_file_path {
            if Command::new("gtk-launch")
                .arg(desktop_path.file_name().unwrap_or_default())
                .spawn()
                .is_ok()
            {
                return Ok(());
            }
        }

        if let Some(ref exec) = app.exec_cmd {
            let parts: Vec<&str> = exec.split_whitespace().collect();
            if let Some((bin, args)) = parts.split_first() {
                Command::new(bin).args(args).spawn()?;
                return Ok(());
            }
        }

        bail!("No executable command found for application")
    }

    pub fn create_shortcut(app: &ApplicationItem) -> Result<PathBuf> {
        DesktopEntryRegistry::create_desktop_shortcut(app)
    }

    pub fn uninstall_app(&self, app: &ApplicationItem) -> Result<()> {
        for provider in &self.providers {
            if provider.source() == app.source {
                return provider.uninstall(&app.package_id);
            }
        }
        bail!(
            "No package manager provider found for source {:?}",
            app.source
        )
    }

    pub fn get_details(&self, app: &ApplicationItem) -> Result<Option<String>> {
        for provider in &self.providers {
            if provider.source() == app.source {
                return provider.get_details(&app.package_id);
            }
        }
        Ok(None)
    }
}

impl Default for ApplicationManager {
    fn default() -> Self {
        Self::new()
    }
}
