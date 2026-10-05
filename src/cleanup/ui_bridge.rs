use crate::app::AppState;
use crate::cleanup::analyzer::Analyzer;
use crate::cleanup::cleaner::CleanupOutcome;
use crate::cleanup::models::{CleanupItem, ScanPhase};
use crate::cleanup::scanner::Scanner;
use crate::cleanup::service::{CleanupOperation, CleanupService};
use crate::{AppWindow, CleanupItemData};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

struct PendingClean {
    items: Vec<CleanupItem>,
    operation: CleanupOperation,
    verifying: bool,
}

pub fn setup_cleanup_handlers(
    window: &AppWindow,
    cleanup_service: Arc<CleanupService>,
    state: Arc<AppState>,
) {
    let pending_clean = Arc::new(Mutex::new(None::<PendingClean>));
    // 1. Unified Start Scan (comprehensive scan)
    let cs = cleanup_service.clone();
    let st = state.clone();
    let win_handle = window.as_weak();
    window.on_cleanup_start_scan(move || {
        let Some(operation) = cs.begin_operation() else {
            return;
        };
        tracing::info!("Cleanup scan requested");
        if let Some(w) = win_handle.upgrade() {
            w.set_cleanup_is_scanning(true);
            w.set_cleanup_has_scanned(true);
            w.set_cleanup_progress_percent(0.0);
            w.set_cleanup_scan_status_text(st.localization.t("cleanup.status_scanning").into());
        }

        let cs = cs.clone();
        let st = st.clone();
        let win = win_handle.clone();
        tokio::spawn(async move {
            let id = operation.id;
            let cancel = operation.cancel.clone();
            let (mut rx, handle) = cs.run_scan_operation(true, operation).await;
            let win_progress = win.clone();
            let cs_progress = cs.clone();
            let progress_handle = tokio::spawn(async move {
                while let Some(progress) = rx.recv().await {
                    let win = win_progress.clone();
                    let cs = cs_progress.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if !cs.is_latest_operation(id) {
                            return;
                        }
                        if let Some(w) = win.upgrade() {
                            w.set_cleanup_progress_percent(progress.percent);
                            if progress.phase == ScanPhase::Scanning {
                                w.set_cleanup_scan_status_text(progress.current_item.into());
                            }
                            if progress.phase == ScanPhase::Completed
                                || progress.phase == ScanPhase::Cancelled
                            {
                                w.set_cleanup_is_scanning(false);
                            }
                        }
                    });
                }
            });

            let result = handle.await;
            let _ = progress_handle.await;
            let (items, text) = if let Ok(items) = result {
                let text = if cancel.load(Ordering::Acquire) {
                    st.localization.t("common.cancel")
                } else {
                    format!(
                        "{}: {}",
                        st.localization.t("cleanup.items_found"),
                        items.len()
                    )
                };
                (items, text)
            } else {
                (
                    cs.get_cached_items().await,
                    st.localization.t("cleanup.risk.warning"),
                )
            };
            let win_done = win.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if !cs.is_latest_operation(id) {
                    return;
                }
                if let Some(w) = win_done.upgrade() {
                    w.set_cleanup_is_scanning(false);
                    w.set_cleanup_scan_status_text(text.into());
                    update_cleanup_ui(&w, &items, &st);
                }
            });
        });
    });

    // 3. Cancel
    let cs = cleanup_service.clone();
    let pending_for_operation_cancel = pending_clean.clone();
    let win_cancel = window.as_weak();
    let st_cancel = state.clone();
    window.on_cleanup_cancel(move || {
        tracing::info!("Cleanup operation cancelled by user");
        cs.cancel_current_operation();
        if let Some(request) = pending_for_operation_cancel.lock().unwrap().take() {
            cs.finish_operation(request.operation.id);
            if let Some(w) = win_cancel.upgrade() {
                w.set_auth_modal_open(false);
                w.set_auth_password_input("".into());
                w.set_cleanup_scan_status_text(st_cancel.localization.t("common.cancel").into());
            }
        }
    });

    // 4. Select All
    let cs = cleanup_service.clone();
    let st = state.clone();
    let win_handle = window.as_weak();
    window.on_cleanup_select_all(move || {
        let cs = cs.clone();
        let st = st.clone();
        let win = win_handle.clone();
        tokio::spawn(async move {
            cs.select_all(true).await;
            let items = cs.get_cached_items().await;
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = win.upgrade() {
                    update_cleanup_ui(&w, &items, &st);
                }
            });
        });
    });

    // 5. Deselect All
    let cs = cleanup_service.clone();
    let st = state.clone();
    let win_handle = window.as_weak();
    window.on_cleanup_deselect_all(move || {
        let cs = cs.clone();
        let st = st.clone();
        let win = win_handle.clone();
        tokio::spawn(async move {
            cs.select_all(false).await;
            let items = cs.get_cached_items().await;
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = win.upgrade() {
                    update_cleanup_ui(&w, &items, &st);
                }
            });
        });
    });

    // 6. Toggle Item
    let cs = cleanup_service.clone();
    let st = state.clone();
    let win_handle = window.as_weak();
    window.on_cleanup_toggle_item(move |id_str| {
        let cs = cs.clone();
        let st = st.clone();
        let win = win_handle.clone();
        let id = id_str.to_string();
        tokio::spawn(async move {
            cs.toggle_item(&id).await;
            let items = cs.get_cached_items().await;
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = win.upgrade() {
                    update_cleanup_ui(&w, &items, &st);
                }
            });
        });
    });

    // 7. Open Item in File Manager
    let cs = cleanup_service.clone();
    window.on_cleanup_open_item(move |id_str| {
        let cs = cs.clone();
        let id = id_str.to_string();
        tokio::spawn(async move {
            let items = cs.get_cached_items().await;
            if let Some(item) = items.iter().find(|i| i.id == id) {
                if !item.path.to_string_lossy().starts_with("orphans:")
                    && !item.path.to_string_lossy().starts_with("journalctl:")
                {
                    cs.open_path(&item.path);
                }
            }
        });
    });

    // 8. Clean Selected
    let cs = cleanup_service.clone();
    let st = state.clone();
    let win_handle = window.as_weak();
    let pending_for_clean = pending_clean.clone();
    window.on_cleanup_clean_selected(move || {
        let Some(operation) = cs.begin_operation() else {
            return;
        };
        tracing::info!("Clean selected requested");
        let cs = cs.clone();
        let st = st.clone();
        let win = win_handle.clone();
        let pending = pending_for_clean.clone();

        tokio::spawn(async move {
            let all_items = cs.get_cached_items().await;
            let selected_items: Vec<CleanupItem> =
                all_items.into_iter().filter(|i| i.selected).collect();

            if selected_items.is_empty() || operation.cancel.load(Ordering::Acquire) {
                cs.finish_operation(operation.id);
                let _ = slint::invoke_from_event_loop(move || {
                    if !cs.is_latest_operation(operation.id) {
                        return;
                    }
                    if let Some(w) = win.upgrade() {
                        w.set_cleanup_is_cleaning(false);
                        if operation.cancel.load(Ordering::Acquire) {
                            w.set_cleanup_scan_status_text(
                                st.localization.t("common.cancel").into(),
                            );
                        }
                    }
                });
                return;
            }

            let needs_auth = selected_items
                .iter()
                .any(|i| i.path.to_string_lossy().starts_with("orphans:pacman:"));

            if needs_auth {
                let id = operation.id;
                {
                    let mut pending = pending.lock().unwrap();
                    if operation.cancel.load(Ordering::Acquire) {
                        cs.finish_operation(id);
                        return;
                    }
                    *pending = Some(PendingClean {
                        items: selected_items,
                        operation: operation.clone(),
                        verifying: false,
                    });
                }
                let _ = slint::invoke_from_event_loop(move || {
                    if !cs.is_latest_operation(id) || operation.cancel.load(Ordering::Acquire) {
                        return;
                    }
                    if let Some(w) = win.upgrade() {
                        w.set_auth_prompt_text(st.localization.t("auth.prompt.orphaned").into());
                        w.set_auth_error_text("".into());
                        w.set_auth_has_error(false);
                        w.set_auth_password_input("".into());
                        w.set_auth_modal_open(true);
                    }
                });
            } else {
                let win_prep = win.clone();
                let prep_text = st.localization.t("cleanup.status_preparing");
                let cs_prep = cs.clone();
                let op_prep = operation.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if !cs_prep.is_latest_operation(op_prep.id)
                        || op_prep.cancel.load(Ordering::Acquire)
                    {
                        return;
                    }
                    if let Some(w) = win_prep.upgrade() {
                        w.set_cleanup_is_cleaning(true);
                        w.set_cleanup_progress_percent(0.0);
                        w.set_cleanup_scan_status_text(prep_text.into());
                    }
                });
                execute_cleanup(selected_items, None, operation, cs, st, win).await;
            }
        });
    });

    // 9. Auth Cancelled
    let pending_for_cancel = pending_clean.clone();
    let cs = cleanup_service.clone();
    window.on_auth_cancelled(move || {
        if let Some(pending) = pending_for_cancel.lock().unwrap().take() {
            pending.operation.cancel.store(true, Ordering::Release);
            cs.finish_operation(pending.operation.id);
        }
    });

    // 10. Auth Submitted
    let cs = cleanup_service.clone();
    let st = state.clone();
    let win_handle = window.as_weak();
    let pending_for_submit = pending_clean.clone();
    window.on_auth_submitted(move |pwd_slint| {
        let operation = {
            let mut pending = pending_for_submit.lock().unwrap();
            let Some(request) = pending.as_mut() else {
                return;
            };
            if request.verifying || request.operation.cancel.load(Ordering::Acquire) {
                return;
            }
            request.verifying = true;
            request.operation.clone()
        };
        let pwd = pwd_slint.to_string();
        let cs = cs.clone();
        let st = st.clone();
        let win = win_handle.clone();
        let pending = pending_for_submit.clone();

        tokio::spawn(async move {
            let is_valid = verify_sudo_password(&pwd, operation.cancel.clone()).await;
            let items = {
                let mut lock = pending.lock().unwrap();
                if !lock
                    .as_ref()
                    .is_some_and(|request| request.operation.id == operation.id)
                {
                    return;
                }
                if operation.cancel.load(Ordering::Acquire) {
                    lock.take();
                    cs.finish_operation(operation.id);
                    return;
                }
                if is_valid {
                    lock.as_ref().map(|p| p.items.clone())
                } else {
                    lock.as_mut().unwrap().verifying = false;
                    None
                }
            };
            if is_valid {
                if let Some(selected_items) = items {
                    let st_prep = st.clone();
                    let win_prep = win.clone();
                    let cs_prep = cs.clone();
                    let op_prep = operation.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if !cs_prep.is_latest_operation(op_prep.id)
                            || op_prep.cancel.load(Ordering::Acquire)
                        {
                            return;
                        }
                        if let Some(w) = win_prep.upgrade() {
                            w.set_auth_modal_open(false);
                            w.set_auth_has_error(false);
                            w.set_auth_password_input("".into());
                            w.set_cleanup_is_cleaning(true);
                            w.set_cleanup_progress_percent(0.0);
                            w.set_cleanup_scan_status_text(
                                st_prep.localization.t("cleanup.status_preparing").into(),
                            );
                        }
                    });
                    let id = operation.id;
                    execute_cleanup(selected_items, Some(pwd), operation, cs, st, win).await;
                    let mut lock = pending.lock().unwrap();
                    if lock.as_ref().is_some_and(|p| p.operation.id == id) {
                        lock.take();
                    }
                }
            } else {
                let win_err = win.clone();
                let err_text = st.localization.t("auth.error.incorrect");
                let _ = slint::invoke_from_event_loop(move || {
                    if !cs.is_latest_operation(operation.id)
                        || operation.cancel.load(Ordering::Acquire)
                    {
                        return;
                    }
                    if let Some(w) = win_err.upgrade() {
                        w.set_auth_has_error(true);
                        w.set_auth_error_text(err_text.into());
                    }
                });
            }
        });
    });
}

async fn verify_sudo_password(password: &str, cancel: Arc<std::sync::atomic::AtomicBool>) -> bool {
    if cancel.load(Ordering::Acquire) {
        return false;
    }
    let mut cmd = tokio::process::Command::new("sudo");
    cmd.args(["-v", "-S", "-k", "-p", ""]);
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::piped());
    cmd.kill_on_drop(true);

    let verify = async {
        if let Ok(mut child) = cmd.spawn() {
            if let Some(mut stdin) = child.stdin.take() {
                use tokio::io::AsyncWriteExt;
                let _ = stdin.write_all(password.as_bytes()).await;
                let _ = stdin.write_all(b"\n").await;
            }
            if let Ok(output) = child.wait_with_output().await {
                return output.status.success();
            }
        }
        false
    };
    let cancellation = async {
        while !cancel.load(Ordering::Acquire) {
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    };
    tokio::select! {
        biased;
        _ = cancellation => false,
        result = tokio::time::timeout(std::time::Duration::from_secs(15), verify) => result.unwrap_or(false),
    }
}

async fn execute_cleanup(
    selected_items: Vec<CleanupItem>,
    sudo_password: Option<String>,
    operation: CleanupOperation,
    cs: Arc<CleanupService>,
    st: Arc<AppState>,
    win: slint::Weak<AppWindow>,
) {
    let id = operation.id;
    let (mut rx, handle) = cs
        .run_clean_operation(selected_items, sudo_password, operation)
        .await;
    let win_progress = win.clone();
    let cs_progress = cs.clone();
    let progress_handle = tokio::spawn(async move {
        while let Some(progress) = rx.recv().await {
            let win = win_progress.clone();
            let cs = cs_progress.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if !cs.is_latest_operation(id) {
                    return;
                }
                if let Some(w) = win.upgrade() {
                    w.set_cleanup_progress_percent(progress.percent);
                    if progress.phase == ScanPhase::Cleaning {
                        w.set_cleanup_scan_status_text(progress.current_item.into());
                    }
                    if progress.phase == ScanPhase::Completed
                        || progress.phase == ScanPhase::Cancelled
                    {
                        w.set_cleanup_is_cleaning(false);
                    }
                }
            });
        }
    });

    let result = handle.await;
    let _ = progress_handle.await;
    let outcome = result
        .as_ref()
        .map(|s| s.outcome)
        .unwrap_or(CleanupOutcome::Partial);
    let bytes = result.as_ref().map(|s| s.bytes_freed).unwrap_or(0);
    if let Ok(summary) = result {
        if !summary.errors.is_empty() {
            tracing::warn!("Cleanup finished with {} error(s)", summary.errors.len());
            for err in &summary.errors {
                tracing::warn!("  cleanup error: {}", err);
            }
        }
    }
    let remaining_items = cs.get_cached_items().await;
    let win_done = win.clone();
    let key = match outcome {
        CleanupOutcome::Complete => "cleanup.clean_done",
        CleanupOutcome::Partial => "cleanup.risk.warning",
        CleanupOutcome::Cancelled => "common.cancel",
    };
    let done_text = format!(
        "{}: {}",
        st.localization.t(key),
        Scanner::format_bytes(bytes)
    );
    let _ = slint::invoke_from_event_loop(move || {
        if !cs.is_latest_operation(id) {
            return;
        }
        if let Some(w) = win_done.upgrade() {
            w.set_cleanup_is_cleaning(false);
            w.set_cleanup_scan_status_text(done_text.into());
            update_cleanup_ui(&w, &remaining_items, &st);
        }
    });
}

pub fn update_cleanup_ui(window: &AppWindow, items: &[CleanupItem], state: &AppState) {
    let loc = &state.localization;

    let mut ui_items = Vec::new();
    let total_bytes: u64 = items.iter().map(|i| i.size_bytes).sum();
    let total_found_formatted = Scanner::format_bytes(total_bytes);

    let (selected_count, _, selected_formatted) = Analyzer::calculate_selected_summary(items);

    for item in items {
        let name_translated = loc.t(&item.name);
        let (risk_code, risk_text) = match item.safety_level {
            crate::cleanup::models::RiskLevel::Safe => ("safe", loc.t("cleanup.risk.safe")),
            crate::cleanup::models::RiskLevel::Warning => {
                ("warning", loc.t("cleanup.risk.warning"))
            }
            crate::cleanup::models::RiskLevel::Dangerous => {
                ("dangerous", loc.t("cleanup.risk.dangerous"))
            }
        };
        let desc_translated = if item.description.is_empty() {
            String::new()
        } else {
            loc.t(&item.description)
        };

        let path_display = if item.path.to_string_lossy().starts_with("orphans:") {
            let pkgs_part = item.path.to_string_lossy();
            let pkgs_list = &pkgs_part["orphans:".len()..];
            let pkgs_list = pkgs_list
                .strip_prefix("pacman:")
                .or_else(|| pkgs_list.strip_prefix("apt:"))
                .unwrap_or(pkgs_list);
            let count = pkgs_list.split(',').filter(|s| !s.is_empty()).count();
            format!(
                "{} ({}): {}",
                loc.t("cleanup.rule.orphaned"),
                count,
                pkgs_list.replace(',', ", ")
            )
        } else {
            item.path.display().to_string()
        };

        ui_items.push(CleanupItemData {
            id: item.id.clone().into(),
            name: name_translated.into(),
            description: desc_translated.into(),
            path_str: path_display.into(),
            size_str: item.size_formatted.clone().into(),
            risk_level_code: risk_code.into(),
            risk_level_text: risk_text.into(),
            is_selected: item.selected,
        });
    }

    let model: ModelRc<CleanupItemData> = ModelRc::new(VecModel::from(ui_items));
    window.set_cleanup_items(model);
    window.set_cleanup_total_found_size_str(total_found_formatted.into());
    window.set_cleanup_total_found_items_count(items.len() as i32);
    window.set_cleanup_total_selected_size_str(selected_formatted.into());
    window.set_cleanup_total_selected_items_count(selected_count as i32);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    #[tokio::test]
    async fn auth_cancel_kills_verification_and_does_not_validate() {
        if let Some(marker) = std::env::var_os("TIDY_AUTH_CANCEL_MARKER") {
            let marker = std::path::PathBuf::from(marker);
            let cancel = Arc::new(AtomicBool::new(false));
            let token = cancel.clone();
            let verification =
                tokio::spawn(async move { verify_sudo_password("test-only", token).await });
            tokio::time::timeout(std::time::Duration::from_secs(2), async {
                while !marker.exists() {
                    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                }
            })
            .await
            .unwrap();
            let pid: i32 = std::fs::read_to_string(&marker).unwrap().parse().unwrap();
            cancel.store(true, Ordering::Release);
            assert!(
                !tokio::time::timeout(std::time::Duration::from_secs(1), verification)
                    .await
                    .unwrap()
                    .unwrap()
            );
            tokio::time::timeout(std::time::Duration::from_secs(2), async {
                while unsafe { libc::kill(pid, 0) } == 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                }
            })
            .await
            .unwrap();
            return;
        }
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!("tidy_auth_cancel_{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("sudo"),
            b"#!/bin/sh\nprintf '%s' \"$$\" > \"$TIDY_AUTH_CANCEL_MARKER\"\nexec /bin/sleep 30\n",
        )
        .unwrap();
        std::fs::set_permissions(root.join("sudo"), std::fs::Permissions::from_mode(0o700))
            .unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "cleanup::ui_bridge::tests::auth_cancel_kills_verification_and_does_not_validate",
                "--nocapture",
            ])
            .env("PATH", &root)
            .env("TIDY_AUTH_CANCEL_MARKER", root.join("pid"))
            .status()
            .unwrap();
        assert!(status.success());
        std::fs::remove_dir_all(root).unwrap();
    }
}
