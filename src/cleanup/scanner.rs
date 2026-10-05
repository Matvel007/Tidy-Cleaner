use crate::cleanup::models::{CleanupItem, CleanupRule, ScanPhase, ScanProgress};
use crate::filesystem::safety::traverse_target;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;

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

            if rule.id == "jetbrains_cache" {
                if let Ok(mut products) = tokio::fs::read_dir(&rule.base_path).await {
                    while let Ok(Some(product)) = products.next_entry().await {
                        if cancel_token.load(Ordering::Acquire) {
                            break;
                        }
                        for disposable in ["caches", "index"] {
                            let path = product.path().join(disposable);
                            if !jetbrains_disposable_target(&path) {
                                continue;
                            }
                            let size = Self::calculate_size(&path, cancel_token.clone()).await;
                            if size > 0 && !cancel_token.load(Ordering::Acquire) {
                                items.push(CleanupItem {
                                    id: format!(
                                        "jetbrains_{}_{}",
                                        product.file_name().to_string_lossy(),
                                        disposable
                                    ),
                                    rule_id: rule.id.clone(),
                                    name: rule.name_key.clone(),
                                    description: rule.description_key.clone(),
                                    path,
                                    size_bytes: size,
                                    size_formatted: Self::format_bytes(size),
                                    safety_level: rule.safety_level,
                                    category: rule.category,
                                    selected: true,
                                });
                            }
                        }
                    }
                }
                continue;
            }

            if rule.id == "systemd_journal" {
                if let Some(item) = Self::scan_systemd_journal(rule, idx).await {
                    items.push(item);
                }
                continue;
            }

            if rule.id == "flatpak_apps" {
                if rule.base_path.exists() {
                    let flatpak_items = Self::scan_flatpak_apps(rule, cancel_token.clone()).await;
                    items.extend(flatpak_items);
                }
                continue;
            }

            if rule.id == "orphaned_packages" {
                if let Some(item) = Self::scan_orphaned_packages(rule, idx).await {
                    items.push(item);
                }
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

            if size_bytes > 0 && !cancel_token.load(Ordering::Acquire) {
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
                phase: if cancel_token.load(Ordering::Acquire) {
                    ScanPhase::Cancelled
                } else {
                    ScanPhase::Completed
                },
                current_item: String::new(),
                items_found: items.len(),
                bytes_found: total_bytes,
                percent: if cancel_token.load(Ordering::Acquire) {
                    0.0
                } else {
                    100.0
                },
            });
        }

        items
    }

    async fn calculate_size(path: &Path, cancel_token: Arc<AtomicBool>) -> u64 {
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || {
            traverse_target(&path, &cancel_token, false, false)
                .ok()
                .filter(|r| r.errors.is_empty() && !r.cancelled)
                .map(|r| r.bytes)
                .unwrap_or(0)
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

    async fn scan_systemd_journal(rule: &CleanupRule, idx: usize) -> Option<CleanupItem> {
        let output = bounded_output(
            tokio::process::Command::new("journalctl")
                .args(["--user", "--disk-usage"])
                .env("LC_ALL", "C"),
        )
        .await
        .ok()?;

        if !output.status.success() {
            return None;
        }

        let text = String::from_utf8_lossy(&output.stdout);
        let size_bytes = Self::parse_journal_size(&text);
        if size_bytes > 0 {
            Some(CleanupItem {
                id: format!("{}_{}", rule.id, idx),
                rule_id: rule.id.clone(),
                name: rule.name_key.clone(),
                description: rule.description_key.clone(),
                path: std::path::PathBuf::from("journalctl:--user"),
                size_bytes,
                size_formatted: Self::format_bytes(size_bytes),
                safety_level: rule.safety_level,
                category: rule.category,
                selected: true,
            })
        } else {
            None
        }
    }

    pub fn parse_journal_size(text: &str) -> u64 {
        let lower = text.to_lowercase();
        if let Some(pos) = lower.find("take up ") {
            let after = &lower[pos + 8..];
            if let Some(token) = after.split_whitespace().next() {
                let token = token.trim_end_matches('.');
                let (num_str, mult) = if let Some(rest) = token.strip_suffix('g') {
                    (rest, 1024.0 * 1024.0 * 1024.0)
                } else if let Some(rest) = token.strip_suffix('m') {
                    (rest, 1024.0 * 1024.0)
                } else if let Some(rest) = token.strip_suffix('k') {
                    (rest, 1024.0)
                } else if let Some(rest) = token.strip_suffix('b') {
                    (rest, 1.0)
                } else {
                    (token, 1.0)
                };

                let num_str = num_str.replace(',', ".");
                if let Ok(val) = num_str.parse::<f64>() {
                    return (val * mult) as u64;
                }
            }
        }
        0
    }

    async fn scan_flatpak_apps(
        rule: &CleanupRule,
        cancel_token: Arc<AtomicBool>,
    ) -> Vec<CleanupItem> {
        let mut result = Vec::new();
        if let Ok(mut entries) = tokio::fs::read_dir(&rule.base_path).await {
            let mut app_idx = 0;
            while let Ok(Some(entry)) = entries.next_entry().await {
                if cancel_token.load(Ordering::Relaxed) {
                    break;
                }
                let cache_dir = entry.path().join("cache");
                if cache_dir.exists() && cache_dir.is_dir() {
                    let app_name = entry.file_name().to_string_lossy().to_string();
                    let size = Self::calculate_size(&cache_dir, cancel_token.clone()).await;
                    if size > 0 && !cancel_token.load(Ordering::Acquire) {
                        result.push(CleanupItem {
                            id: format!("flatpak_{}_{}", app_name, app_idx),
                            rule_id: "flatpak_apps".to_string(),
                            name: format!("Flatpak: {}", app_name),
                            description: rule.description_key.clone(),
                            path: cache_dir,
                            size_bytes: size,
                            size_formatted: Self::format_bytes(size),
                            safety_level: rule.safety_level,
                            category: rule.category,
                            selected: true,
                        });
                        app_idx += 1;
                    }
                }
            }
        }
        result
    }

    async fn scan_orphaned_packages(rule: &CleanupRule, idx: usize) -> Option<CleanupItem> {
        // 1. Arch Linux (pacman)
        if let Ok(output) = bounded_output(
            tokio::process::Command::new("pacman")
                .args(["-Qtdq"])
                .env("LC_ALL", "C"),
        )
        .await
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let pkgs: Vec<String> = stdout
                    .lines()
                    .map(|l| l.trim().to_string())
                    .filter(|l| !l.is_empty())
                    .collect();

                if !pkgs.is_empty() {
                    let mut size = 0u64;
                    if let Ok(qi_out) = bounded_output(
                        tokio::process::Command::new("pacman")
                            .args(["-Qi"])
                            .args(&pkgs)
                            .env("LC_ALL", "C"),
                    )
                    .await
                    {
                        if qi_out.status.success() {
                            let qi_str = String::from_utf8_lossy(&qi_out.stdout);
                            size = Self::parse_pacman_installed_size(&qi_str);
                        }
                    }

                    let formatted = Self::format_bytes(size);
                    return Some(CleanupItem {
                        id: format!("{}_{}", rule.id, idx),
                        rule_id: rule.id.clone(),
                        name: rule.name_key.clone(),
                        description: rule.description_key.clone(),
                        path: std::path::PathBuf::from(format!(
                            "orphans:pacman:{}",
                            pkgs.join(",")
                        )),
                        size_bytes: size,
                        size_formatted: formatted,
                        safety_level: rule.safety_level,
                        category: rule.category,
                        selected: false,
                    });
                }
            }
        }

        // 2. Debian/Ubuntu (apt-get)
        if let Ok(output) = bounded_output(
            tokio::process::Command::new("apt-get")
                .args(["-s", "autoremove"])
                .env("LC_ALL", "C"),
        )
        .await
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let mut pkgs = Vec::new();
                for line in stdout.lines() {
                    if line.starts_with("Remv ") {
                        if let Some(pkg) = line.split_whitespace().nth(1) {
                            pkgs.push(pkg.to_string());
                        }
                    }
                }
                if !pkgs.is_empty() {
                    let size = Self::parse_apt_freed_size(&stdout);
                    let formatted = Self::format_bytes(size);
                    return Some(CleanupItem {
                        id: format!("{}_{}", rule.id, idx),
                        rule_id: rule.id.clone(),
                        name: rule.name_key.clone(),
                        description: rule.description_key.clone(),
                        path: std::path::PathBuf::from(format!("orphans:apt:{}", pkgs.join(","))),
                        size_bytes: size,
                        size_formatted: formatted,
                        safety_level: rule.safety_level,
                        category: rule.category,
                        selected: false,
                    });
                }
            }
        }

        None
    }

    pub fn parse_pacman_installed_size(text: &str) -> u64 {
        let mut total = 0u64;
        for line in text.lines() {
            if line.contains("Installed Size") {
                if let Some(pos) = line.find(':') {
                    let parts: Vec<&str> = line[pos + 1..].split_whitespace().collect();
                    if parts.len() >= 2 {
                        let num_str = parts[0].replace(',', ".");
                        let unit = parts[1].to_lowercase();
                        let mult: f64 = if unit.starts_with("gib") || unit.starts_with("g") {
                            1024.0 * 1024.0 * 1024.0
                        } else if unit.starts_with("mib") || unit.starts_with("m") {
                            1024.0 * 1024.0
                        } else if unit.starts_with("kib") || unit.starts_with("k") {
                            1024.0
                        } else {
                            1.0
                        };
                        if let Ok(val) = num_str.parse::<f64>() {
                            total += (val * mult) as u64;
                        }
                    }
                }
            }
        }
        total
    }

    pub fn parse_apt_freed_size(text: &str) -> u64 {
        let lower = text.to_lowercase();
        if let Some(pos) = lower.find("will be freed") {
            let before = &lower[..pos];
            if let Some(comma_pos) = before.rfind(',') {
                let segment = &before[comma_pos + 1..];
                let parts: Vec<&str> = segment.split_whitespace().collect();
                if parts.len() >= 2 {
                    let num_str = parts[0].replace(',', ".");
                    let unit = parts[1];
                    let mult: f64 = if unit.starts_with('g') {
                        1024.0 * 1024.0 * 1024.0
                    } else if unit.starts_with('m') {
                        1024.0 * 1024.0
                    } else if unit.starts_with('k') {
                        1024.0
                    } else {
                        1.0
                    };
                    if let Ok(val) = num_str.parse::<f64>() {
                        return (val * mult) as u64;
                    }
                }
            }
        }
        0
    }
}

pub(crate) fn jetbrains_disposable_target(path: &Path) -> bool {
    let Some(leaf) = path.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    if !["caches", "index"].contains(&leaf) {
        return false;
    }
    let Some(product_path) = path.parent() else {
        return false;
    };
    if product_path
        .parent()
        .and_then(Path::file_name)
        .and_then(|s| s.to_str())
        != Some("JetBrains")
    {
        return false;
    }
    let Some(product) = product_path.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    [
        "IntelliJIdea",
        "IdeaIC",
        "PyCharm",
        "PyCharmCE",
        "WebStorm",
        "CLion",
        "GoLand",
        "Rider",
        "RustRover",
        "DataGrip",
        "PhpStorm",
        "RubyMine",
    ]
    .iter()
    .any(|prefix| {
        product.strip_prefix(prefix).is_some_and(|version| {
            version.starts_with(|c: char| c.is_ascii_digit())
                && version.bytes().all(|b| b.is_ascii_digit() || b == b'.')
        })
    })
}

pub(crate) async fn bounded_output(
    cmd: &mut tokio::process::Command,
) -> std::io::Result<std::process::Output> {
    tokio::time::timeout(Duration::from_secs(15), cmd.kill_on_drop(true).output())
        .await
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "cleanup query timed out"))?
}
