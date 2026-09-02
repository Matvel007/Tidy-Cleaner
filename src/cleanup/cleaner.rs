use crate::cleanup::models::{CleanupItem, ScanPhase, ScanProgress};
use crate::filesystem::safety::{validate_path_safety, FSError};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Default, Clone)]
pub struct CleanupSummary {
    pub items_cleaned: usize,
    pub bytes_freed: u64,
    pub errors: Vec<String>,
}

pub struct Cleaner;

impl Cleaner {
    pub async fn run_clean(
        items: Vec<CleanupItem>,
        cancel_token: Arc<AtomicBool>,
        progress_tx: Option<UnboundedSender<ScanProgress>>,
    ) -> CleanupSummary {
        let mut summary = CleanupSummary::default();
        let total_items = items.len();

        for (idx, item) in items.into_iter().enumerate() {
            if cancel_token.load(Ordering::Relaxed) {
                if let Some(ref tx) = progress_tx {
                    let _ = tx.send(ScanProgress {
                        phase: ScanPhase::Cancelled,
                        current_item: String::new(),
                        items_found: summary.items_cleaned,
                        bytes_found: summary.bytes_freed,
                        percent: (idx as f32) / (total_items as f32) * 100.0,
                    });
                }
                break;
            }

            if !item.selected {
                continue;
            }

            // Dangerous items must never be deleted automatically, even if
            // something mis-selected them.
            if item.safety_level == crate::cleanup::models::RiskLevel::Dangerous {
                tracing::warn!("Skipping Dangerous item: {}", item.path.display());
                continue;
            }

            let path_str = item.path.display().to_string();

            if let Some(ref tx) = progress_tx {
                let _ = tx.send(ScanProgress {
                    phase: ScanPhase::Cleaning,
                    current_item: path_str.clone(),
                    items_found: summary.items_cleaned,
                    bytes_found: summary.bytes_freed,
                    percent: (idx as f32) / (total_items as f32) * 100.0,
                });
            }

            match Self::clean_target(&item.path).await {
                Ok(freed) => {
                    summary.items_cleaned += 1;
                    summary.bytes_freed += freed;
                }
                Err(e) => {
                    tracing::warn!("Failed to clean {}: {}", path_str, e);
                    summary.errors.push(format!("{}: {}", path_str, e));
                }
            }

            tokio::task::yield_now().await;
        }

        if let Some(ref tx) = progress_tx {
            let _ = tx.send(ScanProgress {
                phase: ScanPhase::Completed,
                current_item: String::new(),
                items_found: summary.items_cleaned,
                bytes_found: summary.bytes_freed,
                percent: 100.0,
            });
        }

        summary
    }

    async fn clean_target(path: &std::path::Path) -> Result<u64, FSError> {
        let target_path = path.to_path_buf();
        tokio::task::spawn_blocking(move || {
            let canonical = validate_path_safety(&target_path)?;
            let mut freed_bytes = 0u64;

            if canonical.is_file() || canonical.is_symlink() {
                if let Ok(meta) = canonical.symlink_metadata() {
                    freed_bytes = meta.len();
                }
                std::fs::remove_file(&canonical)?;
            } else if canonical.is_dir() {
                // Remove entries inside directory
                if let Ok(read_dir) = std::fs::read_dir(&canonical) {
                    for entry in read_dir.flatten() {
                        let entry_path = entry.path();
                        if let Ok(meta) = entry_path.symlink_metadata() {
                            freed_bytes += meta.len();
                        }
                        if entry_path.is_dir() {
                            let _ = std::fs::remove_dir_all(&entry_path);
                        } else {
                            let _ = std::fs::remove_file(&entry_path);
                        }
                    }
                }
            }

            Ok(freed_bytes)
        })
        .await
        .map_err(|e| FSError::Io(std::io::Error::other(e)))?
    }
}
