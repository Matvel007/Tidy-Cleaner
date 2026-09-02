use crate::app::state::AppState;
use crate::startup::models::{CreateStartupRequest, StartupSource};
use crate::startup::service::StartupService;
use crate::{AppWindow, StartupCardData};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::sync::Arc;

pub fn setup_startup_handlers(
    window: &AppWindow,
    service: Arc<StartupService>,
    state: Arc<AppState>,
) {
    let win_handle = window.as_weak();
    let s_clone = service.clone();
    let st_clone = state.clone();

    // Initial load
    slint::spawn_local(async move {
        update_startup_ui(win_handle, s_clone, st_clone).await;
    })
    .unwrap();

    // Search query changed
    let win_handle = window.as_weak();
    let s_clone = service.clone();
    let st_clone = state.clone();
    window.on_startup_search(move |query| {
        let win = win_handle.clone();
        let s = s_clone.clone();
        let st = st_clone.clone();
        let q = query.to_string();
        slint::spawn_local(async move {
            s.set_search_query(q).await;
            update_startup_ui(win, s, st).await;
        })
        .unwrap();
    });

    // Toggle enabled state
    let win_handle = window.as_weak();
    let s_clone = service.clone();
    let st_clone = state.clone();
    window.on_startup_toggle_item(move |id, enable| {
        let win = win_handle.clone();
        let s = s_clone.clone();
        let st = st_clone.clone();
        let item_id = id.to_string();
        slint::spawn_local(async move {
            if let Err(e) = s.toggle_item(&item_id, enable).await {
                tracing::error!("Failed to toggle startup item {}: {:?}", item_id, e);
            }
            update_startup_ui(win, s, st).await;
        })
        .unwrap();
    });

    // Add new startup entry
    let win_handle = window.as_weak();
    let s_clone = service.clone();
    let st_clone = state.clone();
    window.on_startup_add_entry(move |name, exec, comment, terminal| {
        let win = win_handle.clone();
        let s = s_clone.clone();
        let st = st_clone.clone();
        let req = CreateStartupRequest {
            name: name.to_string(),
            exec: exec.to_string(),
            comment: comment.to_string(),
            icon: String::new(),
            terminal,
        };
        slint::spawn_local(async move {
            if let Err(e) = s.add_item(req).await {
                tracing::error!("Failed to add startup item: {:?}", e);
            }
            update_startup_ui(win, s, st).await;
        })
        .unwrap();
    });

    // Remove startup entry
    let win_handle = window.as_weak();
    let s_clone = service.clone();
    let st_clone = state.clone();
    window.on_startup_remove_entry(move |id| {
        let win = win_handle.clone();
        let s = s_clone.clone();
        let st = st_clone.clone();
        let item_id = id.to_string();
        slint::spawn_local(async move {
            if let Err(e) = s.remove_item(&item_id).await {
                tracing::error!("Failed to remove startup item {}: {:?}", item_id, e);
            }
            update_startup_ui(win, s, st).await;
        })
        .unwrap();
    });

    // Browse executable / script file
    let win_handle = window.as_weak();
    window.on_startup_browse_file(move || {
        let win = win_handle.clone();
        slint::spawn_local(async move {
            let picked =
                tokio::task::spawn_blocking(crate::filesystem::dialog::FileDialog::pick_file)
                    .await
                    .ok()
                    .flatten();

            if let Some(path) = picked {
                if let Some(w) = win.upgrade() {
                    let is_executable = path.is_file() && {
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            if let Ok(meta) = std::fs::metadata(&path) {
                                meta.permissions().mode() & 0o111 != 0
                            } else {
                                false
                            }
                        }
                        #[cfg(not(unix))]
                        {
                            false
                        }
                    };

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
            }
        })
        .unwrap();
    });
}

async fn update_startup_ui(
    win_handle: slint::Weak<AppWindow>,
    service: Arc<StartupService>,
    state: Arc<AppState>,
) {
    let items = service.get_filtered_items().await;
    let loc = &state.localization;

    let mut ui_items = Vec::new();
    for item in &items {
        let (has_icon, img) = if let Some(ref path) = item.icon_path {
            if let Ok(slint_img) = slint::Image::load_from_path(path) {
                (true, slint_img)
            } else {
                (false, slint::Image::default())
            }
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

    if let Some(win) = win_handle.upgrade() {
        win.set_startup_total_items(ui_items.len() as i32);
        win.set_startup_items(ModelRc::new(VecModel::from(ui_items)));
    }
}
