use crate::cleanup::models::{CleanupItem, RiskLevel, ScanPhase, ScanProgress};
use crate::filesystem::safety::traverse_target;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum CleanupOutcome {
    #[default]
    Complete,
    Partial,
    Cancelled,
}

#[derive(Debug, Default, Clone)]
pub struct CleanupSummary {
    pub items_cleaned: usize,
    /// Observed regular-file bytes unlinked, not a claim about reclaimed disk blocks.
    pub bytes_freed: u64,
    pub errors: Vec<String>,
    pub cleaned_ids: Vec<String>,
    pub outcome: CleanupOutcome,
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
        let total_items = items.len().max(1);
        for (idx, item) in items.into_iter().enumerate() {
            if cancel_token.load(Ordering::Acquire) {
                summary.outcome = CleanupOutcome::Cancelled;
                break;
            }
            if !item.selected {
                continue;
            }
            if let Some(tx) = &progress_tx {
                let _ = tx.send(ScanProgress {
                    phase: ScanPhase::Cleaning,
                    current_item: item.path.display().to_string(),
                    items_found: summary.items_cleaned,
                    bytes_found: summary.bytes_freed,
                    percent: idx as f32 / total_items as f32 * 100.0,
                });
            }
            let result = if item.safety_level == RiskLevel::Dangerous {
                Err("dangerous cleanup target refused".to_string())
            } else if item.rule_id == "systemd_journal"
                || item.path.to_string_lossy().starts_with("journalctl:")
            {
                // Vacuum does not reclaim the scanned active journal. Without a reliable
                // before/after measurement, report zero observed bytes, not the scan size.
                Self::vacuum_journal(cancel_token.clone()).await
            } else if item.rule_id == "orphaned_packages"
                || item.path.to_string_lossy().starts_with("orphans:")
            {
                let path = item.path.to_string_lossy();
                if let Some(list) = path.strip_prefix("orphans:pacman:") {
                    let pkgs: Vec<&str> = list.split(',').filter(|s| !s.is_empty()).collect();
                    Self::clean_orphans(&pkgs, sudo_password.as_deref(), &cancel_token).await
                } else {
                    Err("orphan cleanup disabled pending validated transaction support".into())
                }
            } else if item.rule_id == "jetbrains_cache"
                && !crate::cleanup::scanner::jetbrains_disposable_target(&item.path)
            {
                Err("non-disposable JetBrains target refused".into())
            } else {
                let path = item.path.clone();
                let cancel = cancel_token.clone();
                match tokio::task::spawn_blocking(move || {
                    traverse_target(&path, &cancel, true, false)
                })
                .await
                {
                    Ok(Ok(report)) => {
                        summary.bytes_freed = summary.bytes_freed.saturating_add(report.bytes);
                        summary.errors.extend(
                            report
                                .errors
                                .iter()
                                .map(|e| format!("{}: {}", item.path.display(), e)),
                        );
                        if report.cancelled {
                            summary.outcome = CleanupOutcome::Cancelled;
                        }
                        if report.errors.is_empty() && !report.cancelled {
                            Ok(())
                        } else {
                            continue;
                        }
                    }
                    Ok(Err(e)) => Err(e.to_string()),
                    Err(e) => Err(e.to_string()),
                }
            };
            match result {
                Ok(()) => {
                    summary.items_cleaned += 1;
                    summary.cleaned_ids.push(item.id);
                }
                Err(e) => summary
                    .errors
                    .push(format!("{}: {}", item.path.display(), e)),
            }
        }
        if cancel_token.load(Ordering::Acquire) {
            summary.outcome = CleanupOutcome::Cancelled;
        }
        if summary.outcome != CleanupOutcome::Cancelled && !summary.errors.is_empty() {
            summary.outcome = CleanupOutcome::Partial;
        }
        if let Some(tx) = progress_tx {
            let phase = match summary.outcome {
                CleanupOutcome::Complete => ScanPhase::Completed,
                CleanupOutcome::Partial => ScanPhase::Partial,
                CleanupOutcome::Cancelled => ScanPhase::Cancelled,
            };
            let _ = tx.send(ScanProgress {
                phase,
                current_item: String::new(),
                items_found: summary.items_cleaned,
                bytes_found: summary.bytes_freed,
                percent: if phase == ScanPhase::Completed {
                    100.0
                } else {
                    0.0
                },
            });
        }
        summary
    }

    async fn vacuum_journal(cancel: Arc<AtomicBool>) -> Result<(), String> {
        let cancelled = async {
            while !cancel.load(Ordering::Acquire) {
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        };
        let output = tokio::select! {
          biased;
          _ = cancelled => return Err("journal vacuum cancelled".into()),
          result = tokio::time::timeout(
            Duration::from_secs(30),
            tokio::process::Command::new("journalctl")
                .args(["--user", "--vacuum-size=10M"])
                .kill_on_drop(true)
                .output(),
        ) => result,
        }
        .map_err(|_| "journal vacuum timed out".to_string())?
        .map_err(|e| e.to_string())?;
        if output.status.success() {
            Ok(())
        } else {
            Err("journal vacuum failed".into())
        }
    }

    async fn clean_orphans(
        pkgs: &[&str],
        sudo_password: Option<&str>,
        cancel: &AtomicBool,
    ) -> Result<(), String> {
        if pkgs.is_empty() {
            return Err("empty orphan transaction refused".into());
        }
        if pkgs.iter().any(|p| {
            p.starts_with('-')
                || !p
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"@._+-".contains(&b))
        }) {
            return Err("invalid package name".into());
        }
        if !std::path::Path::new("/usr/bin/pacman").exists()
            && !std::path::Path::new("/bin/pacman").exists()
        {
            // APT remove/autoremove may expand the transaction beyond the displayed set.
            return Err("APT orphan cleanup disabled pending validated transaction support".into());
        }
        let query = crate::cleanup::scanner::bounded_output(
            tokio::process::Command::new("pacman")
                .args(["-Qtdq"])
                .env("LC_ALL", "C"),
        )
        .await
        .map_err(|_| "orphan revalidation failed".to_string())?;
        let current = String::from_utf8_lossy(&query.stdout);
        if !query.status.success()
            || pkgs
                .iter()
                .any(|pkg| !current.lines().any(|line| line.trim() == *pkg))
        {
            return Err("orphan set changed; rescan required".into());
        }
        if cancel.load(Ordering::Acquire) {
            return Err("orphan cleanup cancelled".into());
        }
        let mut cmd = tokio::process::Command::new("sudo");
        // Native cleanup still holds a transient sudo password String. Never log
        // stdin or sudo stderr; moving elevation to polkit remains separate work.
        // Exact removals only: pacman -R checks dependencies; -Rs/-Rn can remove unseen packages.
        cmd.args(["-S", "-p", "", "--", "pacman", "-R", "--noconfirm", "--"])
            .args(pkgs)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        let mut child = cmd.spawn().map_err(|e| e.to_string())?;
        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            if let Some(pwd) = sudo_password {
                stdin
                    .write_all(pwd.as_bytes())
                    .await
                    .map_err(|_| "sudo input failed".to_string())?;
                stdin
                    .write_all(b"\n")
                    .await
                    .map_err(|_| "sudo input failed".to_string())?;
            }
        }
        // Once started, let the package transaction finish. Cancellation stops subsequent targets.
        let status = child.wait().await.map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err("package removal failed".into())
        }
    }
}
