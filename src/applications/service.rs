use crate::applications::manager::ApplicationManager;
use crate::applications::models::{ApplicationItem, PackageSource, UninstallProgress};
use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

#[allow(dead_code)]
pub struct ApplicationService {
    manager: ApplicationManager,
    cached_apps: Arc<Mutex<Vec<ApplicationItem>>>,
    search_query: Arc<Mutex<String>>,
    source_filter: Arc<Mutex<Option<PackageSource>>>,
    current_page: Arc<Mutex<usize>>,
    page_size: usize,
    cancel_token: Arc<AtomicBool>,
}

#[allow(dead_code)]
impl ApplicationService {
    pub fn new() -> Self {
        Self {
            manager: ApplicationManager::new(),
            cached_apps: Arc::new(Mutex::new(Vec::new())),
            search_query: Arc::new(Mutex::new(String::new())),
            source_filter: Arc::new(Mutex::new(None)),
            current_page: Arc::new(Mutex::new(1)),
            page_size: 10,
            cancel_token: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn refresh_installed_apps(&self) -> Vec<ApplicationItem> {
        let manager_clone = self.manager.clone();
        let apps = match tokio::time::timeout(
            std::time::Duration::from_secs(120),
            tokio::task::spawn_blocking(move || manager_clone.list_all()),
        )
        .await
        {
            Ok(Ok(apps)) => apps,
            Ok(Err(e)) => {
                tracing::error!("Package discovery task failed: {}", e);
                Vec::new()
            }
            Err(_) => {
                tracing::warn!("Package discovery timed out after 120s");
                Vec::new()
            }
        };

        let mut cached = self.cached_apps.lock().await;
        *cached = apps.clone();
        apps
    }

    pub async fn get_cached_apps(&self) -> Vec<ApplicationItem> {
        let cached = self.cached_apps.lock().await;
        cached.clone()
    }

    pub async fn set_search_query(&self, query: String) {
        let mut q = self.search_query.lock().await;
        *q = query;
        let mut page = self.current_page.lock().await;
        *page = 1;
    }

    pub async fn set_source_filter(&self, source: Option<PackageSource>) {
        let mut sf = self.source_filter.lock().await;
        *sf = source;
        let mut page = self.current_page.lock().await;
        *page = 1;
    }

    pub async fn set_page(&self, page: usize) {
        let mut p = self.current_page.lock().await;
        *p = page;
    }

    pub async fn toggle_app_selection(&self, app_id: &str) {
        let mut cached = self.cached_apps.lock().await;
        for app in cached.iter_mut() {
            if app.id == app_id {
                app.selected = !app.selected;
                break;
            }
        }
    }

    pub async fn select_all(&self) {
        let mut cached = self.cached_apps.lock().await;
        for app in cached.iter_mut() {
            app.selected = true;
        }
    }

    pub async fn deselect_all(&self) {
        let mut cached = self.cached_apps.lock().await;
        for app in cached.iter_mut() {
            app.selected = false;
        }
    }

    pub async fn get_current_view(&self) -> (Vec<ApplicationItem>, usize, usize, usize) {
        let cached = self.cached_apps.lock().await;
        let query = self.search_query.lock().await;
        let source = *self.source_filter.lock().await;
        let page = *self.current_page.lock().await;

        let filtered = ApplicationManager::filter_apps(&cached, &query, source);
        let total_items = filtered.len();
        let (paged, current_page, total_pages) =
            ApplicationManager::paginate_apps(&filtered, page, self.page_size);

        (paged, current_page, total_pages, total_items)
    }

    pub async fn get_selected_apps(&self) -> Vec<ApplicationItem> {
        let cached = self.cached_apps.lock().await;
        cached.iter().filter(|a| a.selected).cloned().collect()
    }

    pub async fn launch_app_by_id(&self, app_id: &str) -> Result<()> {
        let cached = self.cached_apps.lock().await;
        if let Some(app) = cached.iter().find(|a| a.id == app_id) {
            ApplicationManager::launch_app(app)?;
        }
        Ok(())
    }

    pub async fn create_shortcut_by_id(&self, app_id: &str) -> Result<std::path::PathBuf> {
        let cached = self.cached_apps.lock().await;
        if let Some(app) = cached.iter().find(|a| a.id == app_id) {
            return ApplicationManager::create_shortcut(app);
        }
        anyhow::bail!("Application not found")
    }

    pub async fn get_details_by_id(&self, app_id: &str) -> Result<Option<String>> {
        let cached = self.cached_apps.lock().await;
        if let Some(app) = cached.iter().find(|a| a.id == app_id) {
            return self.manager.get_details(app);
        }
        Ok(None)
    }

    pub async fn uninstall_selected(
        &self,
    ) -> (
        broadcast::Receiver<UninstallProgress>,
        tokio::task::JoinHandle<Result<()>>,
    ) {
        let (tx, rx) = broadcast::channel(100);
        let selected = self.get_selected_apps().await;
        let cancel_token = self.cancel_token.clone();
        cancel_token.store(false, Ordering::SeqCst);

        let handle = tokio::spawn(async move {
            let total = selected.len();
            let manager = ApplicationManager::new();

            for (idx, app) in selected.iter().enumerate() {
                if cancel_token.load(Ordering::SeqCst) {
                    let _ = tx.send(UninstallProgress {
                        current_app: "Cancelled".to_string(),
                        current_index: idx,
                        total_apps: total,
                        percent: (idx as f32) / (total as f32) * 100.0,
                        is_completed: true,
                        error_message: Some("Uninstall cancelled by user".to_string()),
                    });
                    break;
                }

                let _ = tx.send(UninstallProgress {
                    current_app: app.name.clone(),
                    current_index: idx + 1,
                    total_apps: total,
                    percent: (idx as f32) / (total as f32) * 100.0,
                    is_completed: false,
                    error_message: None,
                });

                let app_clone = app.clone();
                let mgr = manager.clone();
                let result =
                    tokio::task::spawn_blocking(move || mgr.uninstall_app(&app_clone)).await;

                if let Ok(Err(err)) = result {
                    tracing::error!("Failed to uninstall {}: {}", app.name, err);
                    let _ = tx.send(UninstallProgress {
                        current_app: app.name.clone(),
                        current_index: idx + 1,
                        total_apps: total,
                        percent: (idx as f32 + 1.0) / (total as f32) * 100.0,
                        is_completed: false,
                        error_message: Some(format!("{}: {}", app.name, err)),
                    });
                }
            }

            let _ = tx.send(UninstallProgress {
                current_app: String::new(),
                current_index: total,
                total_apps: total,
                percent: 100.0,
                is_completed: true,
                error_message: None,
            });

            Ok(())
        });

        (rx, handle)
    }

    pub async fn uninstall_single_app(
        &self,
        app_id: &str,
    ) -> (
        broadcast::Receiver<UninstallProgress>,
        tokio::task::JoinHandle<Result<()>>,
    ) {
        let (tx, rx) = broadcast::channel(100);
        let cached = self.cached_apps.lock().await;
        let target_app = cached.iter().find(|a| a.id == app_id).cloned();
        drop(cached);

        let handle = tokio::spawn(async move {
            if let Some(app) = target_app {
                let _ = tx.send(UninstallProgress {
                    current_app: app.name.clone(),
                    current_index: 1,
                    total_apps: 1,
                    percent: 50.0,
                    is_completed: false,
                    error_message: None,
                });

                let manager = ApplicationManager::new();
                let app_clone = app.clone();
                let result =
                    tokio::task::spawn_blocking(move || manager.uninstall_app(&app_clone)).await;

                if let Ok(Err(err)) = result {
                    tracing::error!("Failed to uninstall {}: {}", app.name, err);
                    let _ = tx.send(UninstallProgress {
                        current_app: app.name.clone(),
                        current_index: 1,
                        total_apps: 1,
                        percent: 100.0,
                        is_completed: false,
                        error_message: Some(format!("{}: {}", app.name, err)),
                    });
                }

                let _ = tx.send(UninstallProgress {
                    current_app: String::new(),
                    current_index: 1,
                    total_apps: 1,
                    percent: 100.0,
                    is_completed: true,
                    error_message: None,
                });
            }
            Ok(())
        });

        (rx, handle)
    }
}

impl Default for ApplicationService {
    fn default() -> Self {
        Self::new()
    }
}
