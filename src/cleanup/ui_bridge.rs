use crate::app::AppState;
use crate::cleanup::analyzer::Analyzer;
use crate::cleanup::models::{CleanupItem, ScanPhase};
use crate::cleanup::scanner::Scanner;
use crate::cleanup::service::CleanupService;
use crate::{AppWindow, CleanupItemData};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::sync::Arc;

pub fn setup_cleanup_handlers(
    window: &AppWindow,
    cleanup_service: Arc<CleanupService>,
    state: Arc<AppState>,
) {
    // 1. Unified Start Scan (comprehensive scan)
    let cs = cleanup_service.clone();
    let st = state.clone();
    let win_handle = window.as_weak();
    window.on_cleanup_start_scan(move || {
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
            let (mut rx, handle) = cs.run_scan_async(true).await;
            let win_progress = win.clone();
            tokio::spawn(async move {
                while let Some(progress) = rx.recv().await {
                    let win = win_progress.clone();
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(w) = win.upgrade() {
                            w.set_cleanup_progress_percent(progress.percent);
                            w.set_cleanup_scan_status_text(progress.current_item.into());
                            if progress.phase == ScanPhase::Completed
                                || progress.phase == ScanPhase::Cancelled
                            {
                                w.set_cleanup_is_scanning(false);
                            }
                        }
                    });
                }
            });

            if let Ok(items) = handle.await {
                let win_done = win.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(w) = win_done.upgrade() {
                        w.set_cleanup_is_scanning(false);
                        update_cleanup_ui(&w, &items, &st);
                    }
                });
            }
        });
    });

    // 3. Cancel
    let cs = cleanup_service.clone();
    window.on_cleanup_cancel(move || {
        tracing::info!("Cleanup operation cancelled by user");
        cs.cancel_current_operation();
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

    let pending_clean = Arc::new(tokio::sync::Mutex::new(None::<Vec<CleanupItem>>));

    // 8. Clean Selected
    let cs = cleanup_service.clone();
    let st = state.clone();
    let win_handle = window.as_weak();
    let pending_for_clean = pending_clean.clone();
    window.on_cleanup_clean_selected(move || {
        tracing::info!("Clean selected requested");
        let cs = cs.clone();
        let st = st.clone();
        let win = win_handle.clone();
        let pending = pending_for_clean.clone();

        tokio::spawn(async move {
            let all_items = cs.get_cached_items().await;
            let selected_items: Vec<CleanupItem> =
                all_items.into_iter().filter(|i| i.selected).collect();

            if selected_items.is_empty() {
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(w) = win.upgrade() {
                        w.set_cleanup_is_cleaning(false);
                    }
                });
                return;
            }

            let needs_auth = selected_items.iter().any(|i| {
                i.rule_id == "orphaned_packages"
                    || i.path.to_string_lossy().starts_with("orphans:")
            });

            if needs_auth {
                *pending.lock().await = Some(selected_items);
                let _ = slint::invoke_from_event_loop(move || {
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
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(w) = win_prep.upgrade() {
                        w.set_cleanup_is_cleaning(true);
                        w.set_cleanup_progress_percent(0.0);
                        w.set_cleanup_scan_status_text(prep_text.into());
                    }
                });
                execute_cleanup(selected_items, None, cs, st, win).await;
            }
        });
    });

    // 9. Auth Cancelled
    let pending_for_cancel = pending_clean.clone();
    window.on_auth_cancelled(move || {
        let pending = pending_for_cancel.clone();
        tokio::spawn(async move {
            *pending.lock().await = None;
        });
    });

    // 10. Auth Submitted
    let cs = cleanup_service.clone();
    let st = state.clone();
    let win_handle = window.as_weak();
    let pending_for_submit = pending_clean.clone();
    window.on_auth_submitted(move |pwd_slint| {
        let pwd = pwd_slint.to_string();
        let cs = cs.clone();
        let st = st.clone();
        let win = win_handle.clone();
        let pending = pending_for_submit.clone();

        tokio::spawn(async move {
            let is_valid = verify_sudo_password(&pwd).await;
            if is_valid {
                let items = pending.lock().await.take();
                if let Some(selected_items) = items {
                    let st_prep = st.clone();
                    let win_prep = win.clone();
                    let _ = slint::invoke_from_event_loop(move || {
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
                    execute_cleanup(selected_items, Some(pwd), cs, st, win).await;
                }
            } else {
                let win_err = win.clone();
                let err_text = st.localization.t("auth.error.incorrect");
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(w) = win_err.upgrade() {
                        w.set_auth_has_error(true);
                        w.set_auth_error_text(err_text.into());
                    }
                });
            }
        });
    });
}

async fn verify_sudo_password(password: &str) -> bool {
    let mut cmd = tokio::process::Command::new("sudo");
    cmd.args(["-v", "-S", "-k", "-p", ""]);
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::piped());

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
}

async fn execute_cleanup(
    selected_items: Vec<CleanupItem>,
    sudo_password: Option<String>,
    cs: Arc<CleanupService>,
    st: Arc<AppState>,
    win: slint::Weak<AppWindow>,
) {
    let (mut rx, handle) = cs.run_clean_async(selected_items, sudo_password).await;
    let win_progress = win.clone();
    tokio::spawn(async move {
        while let Some(progress) = rx.recv().await {
            let win = win_progress.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = win.upgrade() {
                    w.set_cleanup_progress_percent(progress.percent);
                    w.set_cleanup_scan_status_text(progress.current_item.into());
                    if progress.phase == ScanPhase::Completed
                        || progress.phase == ScanPhase::Cancelled
                    {
                        w.set_cleanup_is_cleaning(false);
                    }
                }
            });
        }
    });

    if let Ok(summary) = handle.await {
        if !summary.errors.is_empty() {
            tracing::warn!("Cleanup finished with {} error(s)", summary.errors.len());
            for err in &summary.errors {
                tracing::warn!("  cleanup error: {}", err);
            }
        }
    }
    let remaining_items = cs.get_cached_items().await;
    let win_done = win.clone();
    let done_text = st.localization.t("cleanup.clean_done");
    let _ = slint::invoke_from_event_loop(move || {
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
