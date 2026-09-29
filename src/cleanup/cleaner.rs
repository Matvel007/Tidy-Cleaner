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
    pub cleaned_ids: Vec<String>,
}

pub struct Cleaner;

impl Cleaner {
    pub async fn run_clean(
        items: Vec<CleanupItem>,
        cancel_token: Arc<AtomicBool>,
        progress_tx: Option<UnboundedSender<ScanProgress>>,
        sudo_password: Option<String>,
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

            if item.rule_id == "systemd_journal"
                || item.path.to_string_lossy().starts_with("journalctl:")
            {
                let _ = tokio::process::Command::new("journalctl")
                    .args(["--user", "--vacuum-size=10M"])
                    .output()
                    .await;
                summary.items_cleaned += 1;
                summary.bytes_freed += item.size_bytes;
                summary.cleaned_ids.push(item.id);
                continue;
            }

            if item.rule_id == "orphaned_packages"
                || item.path.to_string_lossy().starts_with("orphans:")
            {
                let path_str = item.path.to_string_lossy();
                let pkgs_part = path_str.trim_start_matches("orphans:");
                let pkgs: Vec<&str> = pkgs_part.split(',').filter(|s| !s.is_empty()).collect();

                let clean_result = Self::clean_orphans(&pkgs, sudo_password.as_deref()).await;
                match clean_result {
                    Ok(_) => {
                        summary.items_cleaned += 1;
                        summary.bytes_freed += item.size_bytes;
                        summary.cleaned_ids.push(item.id);
                    }
                    Err(e) => {
                        tracing::warn!("Failed to remove orphaned packages: {}", e);
                        summary.errors.push(e);
                    }
                }
                continue;
            }

            match Self::clean_target(&item.path).await {
                Ok((freed, item_errors)) => {
                    let reported = if freed > 0 { freed } else { item.size_bytes };
                    summary.items_cleaned += 1;
                    summary.bytes_freed += reported;
                    summary.cleaned_ids.push(item.id);
                    summary.errors.extend(item_errors);
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

    async fn clean_target(path: &std::path::Path) -> Result<(u64, Vec<String>), FSError> {
        let target_path = path.to_path_buf();
        tokio::task::spawn_blocking(move || {
            let canonical = validate_path_safety(&target_path)?;
            let mut freed_bytes = 0u64;
            let mut errors = Vec::new();

            if canonical.is_file() || canonical.is_symlink() {
                if let Ok(meta) = canonical.symlink_metadata() {
                    freed_bytes = meta.len();
                }
                std::fs::remove_file(&canonical)?;
            } else if canonical.is_dir() {
                // Safely remove entries inside directory without following symlinks
                if let Ok(read_dir) = std::fs::read_dir(&canonical) {
                    for entry in read_dir.flatten() {
                        let entry_path = entry.path();
                        let file_type = match entry.file_type() {
                            Ok(ft) => ft,
                            Err(e) => {
                                errors.push(format!("{}: {}", entry_path.display(), e));
                                continue;
                            }
                        };

                        if file_type.is_symlink() {
                            // Symlinks MUST NEVER be traversed; delete the link itself
                            let size = std::fs::symlink_metadata(&entry_path).map(|m| m.len()).unwrap_or(0);
                            if let Err(e) = std::fs::remove_file(&entry_path) {
                                errors.push(format!("{}: {}", entry_path.display(), e));
                            } else {
                                freed_bytes += size;
                            }
                        } else if file_type.is_dir() {
                            let size = compute_dir_size(&entry_path);
                            if let Err(e) = std::fs::remove_dir_all(&entry_path) {
                                errors.push(format!("{}: {}", entry_path.display(), e));
                            } else {
                                freed_bytes += size;
                            }
                        } else {
                            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                            if let Err(e) = std::fs::remove_file(&entry_path) {
                                errors.push(format!("{}: {}", entry_path.display(), e));
                            } else {
                                freed_bytes += size;
                            }
                        }
                    }
                }

                // If cleaning standard FreeDesktop Trash, ensure empty files and info dirs exist
                if canonical.ends_with("Trash") {
                    let _ = std::fs::create_dir_all(canonical.join("files"));
                    let _ = std::fs::create_dir_all(canonical.join("info"));
                }
            }

            Ok((freed_bytes, errors))
        })
        .await
        .map_err(|e| FSError::Io(std::io::Error::other(e)))?
    }

    async fn clean_orphans(pkgs: &[&str], sudo_password: Option<&str>) -> Result<(), String> {
        if pkgs.is_empty() {
            return Ok(());
        }

        let is_pacman = std::path::Path::new("/usr/bin/pacman").exists()
            || std::path::Path::new("/bin/pacman").exists();

        if is_pacman {
            let mut cmd = tokio::process::Command::new("sudo");
            cmd.args(["-S", "-p", "", "pacman", "-Rns", "--noconfirm"]);
            cmd.args(pkgs);
            cmd.stdin(std::process::Stdio::piped());
            cmd.stdout(std::process::Stdio::piped());
            cmd.stderr(std::process::Stdio::piped());

            let mut child = cmd
                .spawn()
                .map_err(|e| format!("Failed to spawn sudo pacman: {}", e))?;

            if let Some(mut stdin) = child.stdin.take() {
                use tokio::io::AsyncWriteExt;
                if let Some(pwd) = sudo_password {
                    let _ = stdin.write_all(pwd.as_bytes()).await;
                    let _ = stdin.write_all(b"\n").await;
                }
            }

            let output = child
                .wait_with_output()
                .await
                .map_err(|e| format!("Failed to wait for sudo pacman: {}", e))?;

            if output.status.success() {
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(format!("pacman -Rns failed: {}", stderr.trim()))
            }
        } else {
            let mut cmd = tokio::process::Command::new("sudo");
            cmd.args(["-S", "-p", "", "apt-get", "autoremove", "-y"]);
            cmd.stdin(std::process::Stdio::piped());
            cmd.stdout(std::process::Stdio::piped());
            cmd.stderr(std::process::Stdio::piped());

            let mut child = cmd
                .spawn()
                .map_err(|e| format!("Failed to spawn sudo apt-get: {}", e))?;

            if let Some(mut stdin) = child.stdin.take() {
                use tokio::io::AsyncWriteExt;
                if let Some(pwd) = sudo_password {
                    let _ = stdin.write_all(pwd.as_bytes()).await;
                    let _ = stdin.write_all(b"\n").await;
                }
            }

            let output = child
                .wait_with_output()
                .await
                .map_err(|e| format!("Failed to wait for sudo apt-get: {}", e))?;

            if output.status.success() {
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                Err(format!("apt-get autoremove failed: {}", stderr.trim()))
            }
        }
    }
}

fn compute_dir_size(path: &std::path::Path) -> u64 {
    walkdir::WalkDir::new(path)
        .same_file_system(true)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum()
}
