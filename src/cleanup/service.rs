use crate::cleanup::analyzer::Analyzer;
use crate::cleanup::cleaner::{Cleaner, CleanupSummary};
use crate::cleanup::models::{CleanupItem, CleanupRule, ScanProgress};
use crate::cleanup::rules::RuleRegistry;
use crate::cleanup::scanner::Scanner;
use crate::filesystem::safety::open_in_file_manager;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};
use tokio::sync::Mutex;

pub struct CleanupService {
    rules: Vec<CleanupRule>,
    active_cancel_token: Arc<AtomicBool>,
    cached_items: Arc<Mutex<Vec<CleanupItem>>>,
}

impl Default for CleanupService {
    fn default() -> Self {
        Self::new()
    }
}

impl CleanupService {
    pub fn new() -> Self {
        Self {
            rules: RuleRegistry::get_default_rules(),
            active_cancel_token: Arc::new(AtomicBool::new(false)),
            cached_items: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn cancel_current_operation(&self) {
        self.active_cancel_token.store(true, Ordering::Relaxed);
    }

    pub async fn run_scan_async(
        &self,
        is_full_scan: bool,
    ) -> (
        UnboundedReceiver<ScanProgress>,
        tokio::task::JoinHandle<Vec<CleanupItem>>,
    ) {
        let (tx, rx) = unbounded_channel();
        self.active_cancel_token.store(false, Ordering::Relaxed);

        let cancel_token = self.active_cancel_token.clone();
        let rules = self.rules.clone();
        let cached_items = self.cached_items.clone();

        let handle = tokio::spawn(async move {
            let items = Scanner::run_scan(rules, is_full_scan, cancel_token, Some(tx)).await;
            let mut lock = cached_items.lock().await;
            *lock = items.clone();
            items
        });

        (rx, handle)
    }

    pub async fn run_clean_async(
        &self,
        items_to_clean: Vec<CleanupItem>,
    ) -> (
        UnboundedReceiver<ScanProgress>,
        tokio::task::JoinHandle<CleanupSummary>,
    ) {
        let (tx, rx) = unbounded_channel();
        self.active_cancel_token.store(false, Ordering::Relaxed);

        let cancel_token = self.active_cancel_token.clone();
        let cached_items = self.cached_items.clone();

        let handle = tokio::spawn(async move {
            let summary = Cleaner::run_clean(items_to_clean, cancel_token, Some(tx)).await;
            // Clear or update cached items after cleaning
            let mut lock = cached_items.lock().await;
            lock.retain(|i| !i.selected);
            summary
        });

        (rx, handle)
    }

    pub async fn get_cached_items(&self) -> Vec<CleanupItem> {
        let lock = self.cached_items.lock().await;
        lock.clone()
    }

    pub async fn toggle_item(&self, item_id: &str) {
        let mut lock = self.cached_items.lock().await;
        Analyzer::toggle_item_selected(&mut lock, item_id);
    }

    pub async fn select_all(&self, selected: bool) {
        let mut lock = self.cached_items.lock().await;
        Analyzer::set_all_selected(&mut lock, selected);
    }

    pub fn open_path(&self, path: &Path) {
        if let Err(e) = open_in_file_manager(path) {
            tracing::warn!("Failed to open {}: {}", path.display(), e);
        }
    }
}
