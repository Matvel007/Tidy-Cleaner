use crate::cleanup::models::{CleanupItem, CleanupRule, ScanPhase, ScanProgress};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use walkdir::WalkDir;

pub struct Scanner;

impl Scanner {
    pub async fn run_scan(
        rules: Vec<CleanupRule>,
        is_full_scan: bool,
        cancel_token: Arc<AtomicBool>,
        progress_tx: Option<UnboundedSender<ScanProgress>>,
    ) -> Vec<CleanupItem> {
        let mut items = Vec::new();
        let total_rules = rules.len();

        for (idx, rule) in rules.iter().enumerate() {
            if cancel_token.load(Ordering::Relaxed) {
                if let Some(ref tx) = progress_tx {
                    let _ = tx.send(ScanProgress {
                        phase: ScanPhase::Cancelled,
                        current_item: String::new(),
                        items_found: items.len(),
                        bytes_found: items.iter().map(|i: &CleanupItem| i.size_bytes).sum(),
                        percent: (idx as f32) / (total_rules as f32) * 100.0,
                    });
                }
                break;
            }

            // Skip deep-scan-only rules during fast scan
            if rule.is_deep_scan && !is_full_scan {
                continue;
            }

            if !rule.base_path.exists() {
                continue;
            }

            let path_display = rule.base_path.display().to_string();

            if let Some(ref tx) = progress_tx {
                let _ = tx.send(ScanProgress {
                    phase: ScanPhase::Scanning,
                    current_item: path_display.clone(),
                    items_found: items.len(),
                    bytes_found: items.iter().map(|i: &CleanupItem| i.size_bytes).sum(),
                    percent: (idx as f32) / (total_rules as f32) * 100.0,
                });
            }

            // Calculate size of target path
            let size_bytes = Self::calculate_size(&rule.base_path, cancel_token.clone()).await;

            if size_bytes > 0 {
                let formatted = Self::format_bytes(size_bytes);
                items.push(CleanupItem {
                    id: format!("{}_{}", rule.id, idx),
                    rule_id: rule.id.clone(),
                    name: rule.name_key.clone(),
                    description: rule.description_key.clone(),
                    path: rule.base_path.clone(),
                    size_bytes,
                    size_formatted: formatted,
                    safety_level: rule.safety_level,
                    category: rule.category,
                    selected: rule.safety_level == crate::cleanup::models::RiskLevel::Safe,
                });
            }

            // Yield to Tokio runtime to keep UI responsive
            tokio::task::yield_now().await;
        }

        // Sort: Safe items first (by size descending), followed by Warning/Dangerous items at the bottom (by size descending)
        items.sort_by(|a, b| {
            let risk_rank = |r: crate::cleanup::models::RiskLevel| match r {
                crate::cleanup::models::RiskLevel::Safe => 0,
                crate::cleanup::models::RiskLevel::Warning => 1,
                crate::cleanup::models::RiskLevel::Dangerous => 2,
            };
            let rank_a = risk_rank(a.safety_level);
            let rank_b = risk_rank(b.safety_level);
            if rank_a != rank_b {
                rank_a.cmp(&rank_b)
            } else {
                b.size_bytes.cmp(&a.size_bytes)
            }
        });

        if let Some(ref tx) = progress_tx {
            let total_bytes: u64 = items.iter().map(|i| i.size_bytes).sum();
            let _ = tx.send(ScanProgress {
                phase: ScanPhase::Completed,
                current_item: String::new(),
                items_found: items.len(),
                bytes_found: total_bytes,
                percent: 100.0,
            });
        }

        items
    }

    async fn calculate_size(path: &Path, cancel_token: Arc<AtomicBool>) -> u64 {
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || {
            let mut total_size = 0u64;

            for entry in WalkDir::new(&path)
                .follow_links(false)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                if cancel_token.load(Ordering::Relaxed) {
                    break;
                }

                if let Ok(meta) = entry.metadata() {
                    if meta.is_file() {
                        total_size += meta.len();
                    }
                }
            }

            total_size
        })
        .await
        .unwrap_or(0)
    }

    pub fn format_bytes(bytes: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;

        if bytes >= GB {
            format!("{:.2} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.1} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:.0} KB", bytes as f64 / KB as f64)
        } else {
            format!("{} B", bytes)
        }
    }
}
