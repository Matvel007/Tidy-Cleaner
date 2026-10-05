use crate::applications::manager::ApplicationManager;
use crate::applications::models::{ApplicationItem, PackageSource, UninstallProgress};
use anyhow::Result;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex as ViewMutex};
use tokio::sync::{broadcast, Mutex};

struct ViewState {
    query: String,
    source: Option<PackageSource>,
    offset: usize,
    page_size: usize,
}

pub type UninstallTask = (
    broadcast::Receiver<UninstallProgress>,
    tokio::task::JoinHandle<Result<()>>,
);

pub struct ApplicationService {
    manager: ApplicationManager,
    cached_apps: Arc<Mutex<Vec<ApplicationItem>>>,
    view: ViewMutex<ViewState>,
    revision: Arc<AtomicU64>,
    operation: Arc<Mutex<()>>,
}

#[allow(dead_code)]
impl ApplicationService {
    pub fn new() -> Self {
        Self::with_manager(ApplicationManager::new())
    }

    pub fn with_manager(manager: ApplicationManager) -> Self {
        Self {
            manager,
            cached_apps: Arc::new(Mutex::new(Vec::new())),
            view: ViewMutex::new(ViewState {
                query: String::new(),
                source: None,
                offset: 0,
                page_size: 10,
            }),
            revision: Arc::new(AtomicU64::new(0)),
            operation: Arc::new(Mutex::new(())),
        }
    }

    pub fn view_revision(&self) -> Arc<AtomicU64> {
        self.revision.clone()
    }

    pub async fn refresh_installed_apps(&self) -> Vec<ApplicationItem> {
        let Ok(guard) = self.operation.clone().try_lock_owned() else {
            return self.get_cached_apps().await;
        };
        let manager = self.manager.clone();
        let previous = self.get_cached_apps().await;
        // The guard lives in the blocking task, even if an async caller is aborted.
        let result = tokio::task::spawn_blocking(move || {
            let apps = manager.list_all_preserving(&previous);
            (guard, apps)
        })
        .await;
        match result {
            Ok((_guard, apps)) => {
                *self.cached_apps.lock().await = apps.clone();
                self.revision.fetch_add(1, Ordering::SeqCst);
                apps
            }
            Err(error) => {
                tracing::error!(%error, "Package discovery worker failed; inventory retained");
                self.get_cached_apps().await
            }
        }
    }

    pub async fn get_cached_apps(&self) -> Vec<ApplicationItem> {
        self.cached_apps.lock().await.clone()
    }

    pub fn set_search_query(&self, query: String) {
        let mut view = self.view.lock().unwrap();
        view.query = query;
        view.offset = 0;
        self.revision.fetch_add(1, Ordering::SeqCst);
    }

    pub fn set_source_filter(&self, source: Option<PackageSource>) {
        let mut view = self.view.lock().unwrap();
        view.source = source;
        view.offset = 0;
        self.revision.fetch_add(1, Ordering::SeqCst);
    }

    pub fn set_page(&self, page: usize) {
        let mut view = self.view.lock().unwrap();
        view.offset = page.max(1).saturating_sub(1).saturating_mul(view.page_size);
        self.revision.fetch_add(1, Ordering::SeqCst);
    }

    pub fn set_page_size(&self, size: usize) {
        let mut view = self.view.lock().unwrap();
        view.page_size = size.clamp(1, 200);
        // Retain the exact first visible offset, not just the page that contains it.
        self.revision.fetch_add(1, Ordering::SeqCst);
    }

    pub async fn toggle_app_selection(&self, app_id: &str) {
        if let Some(app) = self
            .cached_apps
            .lock()
            .await
            .iter_mut()
            .find(|a| a.id == app_id)
        {
            app.selected = !app.selected;
        }
        self.revision.fetch_add(1, Ordering::SeqCst);
    }
    pub async fn select_all(&self) {
        for app in self.cached_apps.lock().await.iter_mut() {
            app.selected = true;
        }
        self.revision.fetch_add(1, Ordering::SeqCst);
    }
    pub async fn deselect_all(&self) {
        for app in self.cached_apps.lock().await.iter_mut() {
            app.selected = false;
        }
        self.revision.fetch_add(1, Ordering::SeqCst);
    }
    pub async fn get_current_view(&self) -> (Vec<ApplicationItem>, usize, usize, usize) {
        let cached = self.cached_apps.lock().await;
        let mut view = self.view.lock().unwrap();
        let filtered = ApplicationManager::filter_apps(&cached, &view.query, view.source);
        let total = filtered.len();
        let pages = total.div_ceil(view.page_size).max(1);
        if view.offset >= total {
            view.offset = pages.saturating_sub(1) * view.page_size;
        }
        let end = view.offset.saturating_add(view.page_size).min(total);
        (
            filtered[view.offset..end].to_vec(),
            view.offset / view.page_size + 1,
            pages,
            total,
        )
    }
    pub async fn get_selected_apps(&self) -> Vec<ApplicationItem> {
        self.cached_apps
            .lock()
            .await
            .iter()
            .filter(|a| a.selected)
            .cloned()
            .collect()
    }

    async fn find_app(&self, id: &str) -> Result<ApplicationItem> {
        self.cached_apps
            .lock()
            .await
            .iter()
            .find(|a| a.id == id)
            .cloned()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, id.to_string()).into())
    }

    pub async fn launch_app_by_id(&self, id: &str) -> Result<()> {
        let guard = self
            .operation
            .clone()
            .try_lock_owned()
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::WouldBlock))?;
        let app = self.find_app(id).await?;
        tokio::task::spawn_blocking(move || {
            let _guard = guard;
            ApplicationManager::launch_app(&app)
        })
        .await?
    }
    pub async fn create_shortcut_by_id(&self, id: &str) -> Result<std::path::PathBuf> {
        let guard = self
            .operation
            .clone()
            .try_lock_owned()
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::WouldBlock))?;
        let app = self.find_app(id).await?;
        tokio::task::spawn_blocking(move || {
            let _guard = guard;
            ApplicationManager::create_shortcut(&app)
        })
        .await?
    }
    pub async fn get_details_by_id(&self, id: &str) -> Result<Option<String>> {
        let guard = self
            .operation
            .clone()
            .try_lock_owned()
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::WouldBlock))?;
        let app = self.find_app(id).await?;
        let manager = self.manager.clone();
        tokio::task::spawn_blocking(move || {
            let _guard = guard;
            manager.get_details(&app)
        })
        .await?
    }

    pub async fn uninstall_selected(&self) -> Result<UninstallTask> {
        let guard = self
            .operation
            .clone()
            .try_lock_owned()
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::WouldBlock))?;
        let selected = self.get_selected_apps().await;
        if selected.is_empty() {
            return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput).into());
        }
        Ok(self.start_uninstall(selected, guard))
    }
    pub async fn uninstall_single_app(&self, id: &str) -> Result<UninstallTask> {
        let guard = self
            .operation
            .clone()
            .try_lock_owned()
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::WouldBlock))?;
        let app = self.find_app(id).await?;
        Ok(self.start_uninstall(vec![app], guard))
    }

    fn start_uninstall(
        &self,
        apps: Vec<ApplicationItem>,
        guard: tokio::sync::OwnedMutexGuard<()>,
    ) -> UninstallTask {
        let (tx, rx) = broadcast::channel(100);
        let manager = self.manager.clone();
        let cached = self.cached_apps.clone();
        let revision = self.revision.clone();
        let total = apps.len();
        let handle = tokio::spawn(async move {
            let progress = tx.clone();
            let worker = tokio::task::spawn_blocking(move || {
                // Keep the lock through cache reconciliation/completion, returning
                // it only after the blocking mutation has actually finished.
                let mut removed = Vec::new();
                let mut errors = Vec::new();
                for (index, app) in apps.iter().enumerate() {
                    let _ = progress.send(UninstallProgress {
                        current_app: app.name.clone(),
                        current_index: index + 1,
                        total_apps: total,
                        percent: index as f32 / total as f32 * 100.0,
                        is_completed: false,
                        error_message: None,
                    });
                    match manager.uninstall_app(app) {
                        Ok(()) => removed.push(app.id.clone()),
                        Err(error) => errors.push(format!("{}: {error:#}", app.name)),
                    }
                }
                (guard, removed, errors)
            })
            .await;
            let (guard, removed, errors) = match worker {
                Ok(values) => values,
                Err(error) => {
                    let detail = error.to_string();
                    let _ = tx.send(UninstallProgress {
                        current_app: String::new(),
                        current_index: total,
                        total_apps: total,
                        percent: 100.0,
                        is_completed: true,
                        error_message: Some(detail.clone()),
                    });
                    return Err(error.into());
                }
            };
            cached.lock().await.retain(|app| !removed.contains(&app.id));
            revision.fetch_add(1, Ordering::SeqCst);
            let detail = if errors.is_empty() {
                None
            } else {
                Some(errors.join("\n"))
            };
            let _ = tx.send(UninstallProgress {
                current_app: String::new(),
                current_index: total,
                total_apps: total,
                percent: 100.0,
                is_completed: true,
                error_message: detail.clone(),
            });
            drop(guard);
            match detail {
                Some(detail) => Err(anyhow::anyhow!(detail)),
                None => Ok(()),
            }
        });
        (rx, handle)
    }
}

impl Default for ApplicationService {
    fn default() -> Self {
        Self::new()
    }
}
