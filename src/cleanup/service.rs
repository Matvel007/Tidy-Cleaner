use crate::cleanup::analyzer::Analyzer;
use crate::cleanup::cleaner::{Cleaner, CleanupSummary};
use crate::cleanup::models::{CleanupItem, CleanupRule, ScanProgress};
use crate::cleanup::rules::RuleRegistry;
use crate::cleanup::scanner::Scanner;
use crate::filesystem::safety::open_in_file_manager;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};
use tokio::sync::Mutex;

pub struct CleanupService {
    rules: Vec<CleanupRule>,
    active_operation: Arc<std::sync::Mutex<Option<CleanupOperation>>>,
    generation: Arc<AtomicU64>,
    cached_items: Arc<Mutex<Vec<CleanupItem>>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cleanup::cleaner::CleanupOutcome;
    use crate::cleanup::models::{CleanupCategory, RiskLevel};

    fn item(path: std::path::PathBuf) -> CleanupItem {
        CleanupItem {
            id: "unresolved".into(),
            rule_id: "regression".into(),
            name: String::new(),
            description: String::new(),
            path,
            size_bytes: 999,
            size_formatted: String::new(),
            safety_level: RiskLevel::Safe,
            category: CleanupCategory::ApplicationCache,
            selected: true,
        }
    }

    #[tokio::test]
    async fn auth_cancelled_operation_cannot_reset_or_finish_new_operation() {
        let service = CleanupService::new();
        let old = service.begin_operation().unwrap();
        assert!(service.begin_operation().is_none());
        service.cancel_current_operation();
        service.finish_operation(old.id);
        let new = service.begin_operation().unwrap();
        assert!(old.cancel.load(Ordering::Acquire));
        assert!(!new.cancel.load(Ordering::Acquire));
        service.finish_operation(old.id);
        assert!(service.begin_operation().is_none());
        let (_, handle) = service.run_clean_operation(Vec::new(), None, old).await;
        assert_eq!(handle.await.unwrap().outcome, CleanupOutcome::Partial);
        assert!(!new.cancel.load(Ordering::Acquire));
        service.finish_operation(new.id);
    }

    #[tokio::test]
    async fn repeated_submission_is_rejected_and_running_gate_survives_cancel() {
        let service = CleanupService::new();
        let operation = service.begin_operation().unwrap();
        assert!(service.claim_operation(&operation));
        assert!(!service.claim_operation(&operation));
        service.cancel_current_operation();
        service.finish_operation(operation.id);
        assert!(service.begin_operation().is_none());
        let (_, handle) = service
            .run_clean_operation(Vec::new(), None, operation.clone())
            .await;
        assert_eq!(handle.await.unwrap().outcome, CleanupOutcome::Partial);
        drop(OperationGuard {
            active: service.active_operation.clone(),
            id: operation.id,
        });
        assert!(service.begin_operation().is_some());
    }

    #[tokio::test]
    async fn failed_cleanup_and_cancelled_scan_preserve_cached_items() {
        let service = CleanupService::new();
        let target =
            item(std::env::temp_dir().join(format!("tidy_missing_{}", std::process::id())));
        *service.cached_items.lock().await = vec![target.clone()];
        let (_, handle) = service.run_clean_async(vec![target], None).await;
        assert_eq!(handle.await.unwrap().outcome, CleanupOutcome::Partial);
        assert_eq!(service.get_cached_items().await.len(), 1);
        let operation = service.begin_operation().unwrap();
        service.cancel_current_operation();
        let (_, handle) = service.run_scan_operation(true, operation).await;
        assert_eq!(handle.await.unwrap().len(), 1);
        assert_eq!(service.get_cached_items().await[0].id, "unresolved");
    }

    #[tokio::test]
    async fn aborting_waiter_does_not_release_the_worker_gate() {
        let service = CleanupService::new();
        let cache_lock = service.cached_items.lock().await;
        let (_, waiter) = service.run_clean_async(Vec::new(), None).await;
        waiter.abort();
        tokio::task::yield_now().await;
        assert!(service.begin_operation().is_none());
        service.cancel_current_operation();
        assert!(service.begin_operation().is_none());
        drop(cache_lock);
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            loop {
                if let Some(next) = service.begin_operation() {
                    assert!(!next.cancel.load(Ordering::Acquire));
                    service.finish_operation(next.id);
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }
}

#[derive(Clone)]
pub struct CleanupOperation {
    pub id: u64,
    pub cancel: Arc<AtomicBool>,
    started: Arc<AtomicBool>,
}

struct OperationGuard {
    active: Arc<std::sync::Mutex<Option<CleanupOperation>>>,
    id: u64,
}
impl Drop for OperationGuard {
    fn drop(&mut self) {
        let mut active = self.active.lock().unwrap_or_else(|e| e.into_inner());
        if active.as_ref().is_some_and(|op| op.id == self.id) {
            *active = None;
        }
    }
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
            active_operation: Arc::new(std::sync::Mutex::new(None)),
            generation: Arc::new(AtomicU64::new(0)),
            cached_items: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn cancel_current_operation(&self) {
        if let Some(op) = self.active_operation.lock().unwrap().as_ref() {
            op.cancel.store(true, Ordering::Release);
        }
    }

    pub fn begin_operation(&self) -> Option<CleanupOperation> {
        let mut active = self.active_operation.lock().unwrap();
        if active.is_some() {
            return None;
        }
        let op = CleanupOperation {
            id: self.generation.fetch_add(1, Ordering::AcqRel) + 1,
            cancel: Arc::new(AtomicBool::new(false)),
            started: Arc::new(AtomicBool::new(false)),
        };
        *active = Some(op.clone());
        Some(op)
    }

    pub fn is_latest_operation(&self, id: u64) -> bool {
        self.generation.load(Ordering::Acquire) == id
    }

    pub fn finish_operation(&self, id: u64) {
        let mut active = self.active_operation.lock().unwrap();
        // Auth cancellation may release a reservation, never a running transaction.
        if active
            .as_ref()
            .is_some_and(|op| op.id == id && !op.started.load(Ordering::Acquire))
        {
            *active = None;
        }
    }

    fn claim_operation(&self, operation: &CleanupOperation) -> bool {
        let active = self.active_operation.lock().unwrap();
        active
            .as_ref()
            .is_some_and(|op| op.id == operation.id && Arc::ptr_eq(&op.cancel, &operation.cancel))
            && !operation.started.swap(true, Ordering::AcqRel)
    }

    #[allow(dead_code)]
    pub async fn run_scan_async(
        &self,
        is_full_scan: bool,
    ) -> (
        UnboundedReceiver<ScanProgress>,
        tokio::task::JoinHandle<Vec<CleanupItem>>,
    ) {
        let Some(operation) = self.begin_operation() else {
            let (_, rx) = unbounded_channel();
            let items = self.get_cached_items().await;
            return (rx, tokio::spawn(async move { items }));
        };
        self.run_scan_operation(is_full_scan, operation).await
    }

    pub async fn run_scan_operation(
        &self,
        is_full_scan: bool,
        operation: CleanupOperation,
    ) -> (
        UnboundedReceiver<ScanProgress>,
        tokio::task::JoinHandle<Vec<CleanupItem>>,
    ) {
        let (tx, rx) = unbounded_channel();
        if !self.claim_operation(&operation) {
            let items = self.get_cached_items().await;
            return (rx, tokio::spawn(async move { items }));
        }
        let cancel_token = operation.cancel.clone();
        let rules = self.rules.clone();
        let cached_items = self.cached_items.clone();
        let active = self.active_operation.clone();
        let guard = OperationGuard {
            active,
            id: operation.id,
        };

        let worker = tokio::spawn(async move {
            let _guard = guard;
            let items =
                Scanner::run_scan(rules, is_full_scan, cancel_token.clone(), Some(tx)).await;
            let mut lock = cached_items.lock().await;
            if !cancel_token.load(Ordering::Acquire) {
                *lock = items;
            }
            let result = lock.clone();
            drop(lock);
            result
        });
        // The caller owns a waiter, not the worker. Aborting it cannot unlock
        // an in-flight filesystem traversal or package transaction.
        let handle = tokio::spawn(async move { worker.await.expect("cleanup scan worker failed") });

        (rx, handle)
    }

    #[allow(dead_code)]
    pub async fn run_clean_async(
        &self,
        items_to_clean: Vec<CleanupItem>,
        sudo_password: Option<String>,
    ) -> (
        UnboundedReceiver<ScanProgress>,
        tokio::task::JoinHandle<CleanupSummary>,
    ) {
        let Some(operation) = self.begin_operation() else {
            let (_, rx) = unbounded_channel();
            return (
                rx,
                tokio::spawn(async {
                    CleanupSummary {
                        outcome: crate::cleanup::cleaner::CleanupOutcome::Partial,
                        errors: vec!["cleanup operation already active".into()],
                        ..Default::default()
                    }
                }),
            );
        };
        self.run_clean_operation(items_to_clean, sudo_password, operation)
            .await
    }

    pub async fn run_clean_operation(
        &self,
        items_to_clean: Vec<CleanupItem>,
        sudo_password: Option<String>,
        operation: CleanupOperation,
    ) -> (
        UnboundedReceiver<ScanProgress>,
        tokio::task::JoinHandle<CleanupSummary>,
    ) {
        let (tx, rx) = unbounded_channel();
        if !self.claim_operation(&operation) {
            return (
                rx,
                tokio::spawn(async {
                    CleanupSummary {
                        outcome: crate::cleanup::cleaner::CleanupOutcome::Partial,
                        errors: vec!["stale or repeated cleanup operation refused".into()],
                        ..Default::default()
                    }
                }),
            );
        }
        let cancel_token = operation.cancel.clone();
        let cached_items = self.cached_items.clone();
        let active = self.active_operation.clone();
        let guard = OperationGuard {
            active,
            id: operation.id,
        };

        let worker = tokio::spawn(async move {
            let _guard = guard;
            let summary =
                Cleaner::run_clean(items_to_clean, cancel_token, Some(tx), sudo_password).await;
            // Clear only successfully cleaned items from cache
            let mut lock = cached_items.lock().await;
            lock.retain(|i| !summary.cleaned_ids.contains(&i.id));
            drop(lock);
            summary
        });
        let handle = tokio::spawn(async move { worker.await.expect("cleanup worker failed") });

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
