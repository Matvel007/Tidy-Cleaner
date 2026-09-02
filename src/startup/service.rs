use crate::startup::manager::StartupManager;
use crate::startup::models::{CreateStartupRequest, StartupItem};
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct StartupService {
    manager: StartupManager,
    cached_items: Arc<Mutex<Vec<StartupItem>>>,
    search_query: Arc<Mutex<String>>,
}

impl Default for StartupService {
    fn default() -> Self {
        Self::new()
    }
}

impl StartupService {
    pub fn new() -> Self {
        let manager = StartupManager::new();
        let initial_items = manager.list_items();

        Self {
            manager,
            cached_items: Arc::new(Mutex::new(initial_items)),
            search_query: Arc::new(Mutex::new(String::new())),
        }
    }

    pub async fn refresh_items(&self) {
        let mgr = self.manager.clone();
        let items = tokio::task::spawn_blocking(move || mgr.list_items())
            .await
            .unwrap_or_default();

        let mut cached = self.cached_items.lock().await;
        *cached = items;
    }

    pub async fn get_filtered_items(&self) -> Vec<StartupItem> {
        let cached = self.cached_items.lock().await;
        let query = self.search_query.lock().await;
        let query_lower = query.trim().to_lowercase();

        if query_lower.is_empty() {
            cached.clone()
        } else {
            cached
                .iter()
                .filter(|item| {
                    item.name.to_lowercase().contains(&query_lower)
                        || item.exec.to_lowercase().contains(&query_lower)
                        || item.comment.to_lowercase().contains(&query_lower)
                })
                .cloned()
                .collect()
        }
    }

    pub async fn set_search_query(&self, query: String) {
        let mut q = self.search_query.lock().await;
        *q = query;
    }

    pub async fn add_item(&self, req: CreateStartupRequest) -> Result<()> {
        let mgr = self.manager.clone();
        tokio::task::spawn_blocking(move || mgr.add_item(&req)).await??;
        self.refresh_items().await;
        Ok(())
    }

    pub async fn toggle_item(&self, id: &str, enable: bool) -> Result<()> {
        let mgr = self.manager.clone();
        let item_id = id.to_string();
        tokio::task::spawn_blocking(move || mgr.toggle_item(&item_id, enable)).await??;
        self.refresh_items().await;
        Ok(())
    }

    pub async fn remove_item(&self, id: &str) -> Result<()> {
        let mgr = self.manager.clone();
        let item_id = id.to_string();
        tokio::task::spawn_blocking(move || mgr.remove_item(&item_id)).await??;
        self.refresh_items().await;
        Ok(())
    }
}
