use crate::app::state::AppState;
use crate::startup::models::{CreateStartupRequest, StartupSource};
use crate::startup::service::StartupService;
use crate::{AppWindow, StartupCardData};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::sync::Arc;

enum StartupChange {
    Refresh,
    Search(String),
    Toggle(String, bool),
    Add(CreateStartupRequest),
    Remove(String),
}

pub fn setup_startup_handlers(
    window: &AppWindow,
    service: Arc<StartupService>,
    state: Arc<AppState>,
) {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let win_handle = window.as_weak();
    let mut autostart_updates = state.autostart_updates.subscribe();
    tokio::spawn(async move {
        loop {
            let change = tokio::select! {
                biased;
                change = rx.recv() => match change { Some(change) => change, None => break },
                update = autostart_updates.changed() => {
                    if update.is_err() { break; }
                    StartupChange::Refresh
                }
            };
            let reconcile = !matches!(&change, StartupChange::Search(_));
            let result = match change {
                StartupChange::Refresh => {
                    service.refresh_items().await;
                    Ok(())
                }
                StartupChange::Search(query) => {
                    service.set_search_query(query).await;
                    Ok(())
                }
                StartupChange::Toggle(id, enable) => service.toggle_item(&id, enable).await,
                StartupChange::Add(req) => service.add_item(req).await,
                StartupChange::Remove(id) => service.remove_item(&id).await,
            };
            if let Err(e) = result {
                tracing::error!("Startup operation failed: {:#}", e);
            }
            update_startup_ui(
                win_handle.clone(),
                service.clone(),
                state.clone(),
                reconcile,
            )
            .await;
        }
    });
    let send = Arc::new(move |change| {
        if let Err(e) = tx.send(change) {
            tracing::error!("Startup worker stopped: {}", e);
        }
    });
    send(StartupChange::Refresh);

    // Search query changed
    let sender = send.clone();
    window.on_startup_search(move |query| {
        sender(StartupChange::Search(query.to_string()));
    });

    // Toggle enabled state
    let sender = send.clone();
    window.on_startup_toggle_item(move |id, enable| {
        sender(StartupChange::Toggle(id.to_string(), enable));
    });

    // Add new startup entry
    let sender = send.clone();
    window.on_startup_add_entry(move |name, exec, comment, terminal| {
        let req = CreateStartupRequest {
            name: name.to_string(),
            exec: exec.to_string(),
            comment: comment.to_string(),
            icon: String::new(),
            terminal,
        };
        sender(StartupChange::Add(req));
    });

    // Remove startup entry
    window.on_startup_remove_entry(move |id| {
        send(StartupChange::Remove(id.to_string()));
    });

    // Browse executable / script file
    let win_handle = window.as_weak();
    window.on_startup_browse_file(move || {
        let win = win_handle.clone();
        tokio::spawn(async move {
            let picked =
                tokio::task::spawn_blocking(crate::filesystem::dialog::FileDialog::pick_file)
                    .await
                    .ok()
                    .flatten();

            if let Some(path) = picked {
                let (path, is_executable) = match tokio::task::spawn_blocking(move || {
                    #[cfg(unix)]
                    let is_executable = {
                        use std::os::unix::fs::PermissionsExt;
                        std::fs::metadata(&path)
                            .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
                            .unwrap_or(false)
                    };
                    #[cfg(not(unix))]
                    let is_executable = false;
                    (path, is_executable)
                })
                .await
                {
                    Ok(value) => value,
                    Err(e) => {
                        tracing::error!("File inspection failed: {}", e);
                        return;
                    }
                };
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(w) = win.upgrade() {
                        let path_str = path.to_string_lossy().to_string();
                        let exec_command = if is_executable {
                            path_str
                        } else {
                            format!("xdg-open \"{}\"", path_str)
                        };
                        w.set_startup_add_exec_input(exec_command.as_str().into());

                        if w.get_startup_add_name_input().trim().is_empty() {
                            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                                let mut chars = stem.chars();
                                let capitalized = match chars.next() {
                                    None => String::new(),
                                    Some(first) => {
                                        first.to_uppercase().collect::<String>() + chars.as_str()
                                    }
                                };
                                w.set_startup_add_name_input(capitalized.as_str().into());
                            }
                        }
                    }
                });
            }
        });
    });
}

async fn update_startup_ui(
    win_handle: slint::Weak<AppWindow>,
    service: Arc<StartupService>,
    state: Arc<AppState>,
    reconcile: bool,
) {
    let items = service.get_filtered_items().await;
    let st = state.clone();
    let (items, icons, autostart_error) = match tokio::task::spawn_blocking(move || {
        let error = if reconcile {
            st.reconcile_autostart().err().map(|e| format!("{:#}", e))
        } else {
            None
        };
        let icons: Vec<_> = items
            .iter()
            .map(|item| {
                item.icon_path
                    .as_ref()
                    .and_then(|path| slint::Image::load_from_path(path).ok()?.to_rgba8())
            })
            .collect();
        (items, icons, error)
    })
    .await
    {
        Ok(value) => value,
        Err(e) => {
            tracing::error!("Startup UI preparation failed: {}", e);
            return;
        }
    };

    let _ = slint::invoke_from_event_loop(move || {
        if let Some(win) = win_handle.upgrade() {
            crate::settings::ui_bridge::update_settings_ui(&win, &state);
            if let Some(error) = autostart_error {
                tracing::error!("Autostart reconciliation failed: {}", error);
            }
            let loc = &state.localization;
            let mut ui_items = Vec::new();
            for (item, icon) in items.iter().zip(icons) {
                let (has_icon, img) = if let Some(buffer) = icon {
                    (true, slint::Image::from_rgba8(buffer))
                } else {
                    (false, slint::Image::default())
                };

                let source_name = match item.source {
                    StartupSource::User => loc.t("startup.source_user"),
                    StartupSource::System => loc.t("startup.source_system"),
                };

                ui_items.push(StartupCardData {
                    id: item.id.as_str().into(),
                    name: item.name.as_str().into(),
                    comment: item.comment.as_str().into(),
                    exec: item.exec.as_str().into(),
                    source_name: source_name.into(),
                    source_code: item.source.code().into(),
                    has_icon_image: has_icon,
                    icon_image: img,
                    enabled: item.enabled,
                    is_terminal: item.is_terminal,
                });
            }

            win.set_startup_total_items(ui_items.len() as i32);
            win.set_startup_items(ModelRc::new(VecModel::from(ui_items)));
        }
    });
}
