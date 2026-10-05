use crate::app::state::AppState;
use crate::applications::service::ApplicationService;
use crate::AppCardData;
use crate::AppWindow;
use slint::{ComponentHandle, ModelRc, VecModel};
use std::sync::Arc;

pub fn setup_applications_handlers(
    window: &AppWindow,
    app_service: Arc<ApplicationService>,
    state: Arc<AppState>,
) {
    // Initial async load
    {
        let as_svc = app_service.clone();
        let win_handle = window.as_weak();
        tokio::spawn(async move {
            as_svc.refresh_installed_apps().await;
            update_applications_view(&win_handle, &as_svc).await;
        });
    }

    // 1. Search changed
    {
        let as_svc = app_service.clone();
        let win_handle = window.as_weak();
        window.on_applications_search(move |query| {
            let as_svc = as_svc.clone();
            let win_handle = win_handle.clone();
            let q = query.to_string();
            as_svc.set_search_query(q);
            tokio::spawn(async move {
                update_applications_view(&win_handle, &as_svc).await;
            });
        });
    }

    // 2. Page changed
    {
        let as_svc = app_service.clone();
        let win_handle = window.as_weak();
        window.on_applications_page_change(move |page_num| {
            let as_svc = as_svc.clone();
            let win_handle = win_handle.clone();
            let page = page_num.max(1) as usize;
            as_svc.set_page(page);
            tokio::spawn(async move {
                update_applications_view(&win_handle, &as_svc).await;
            });
        });
    }

    // 3. Select All
    {
        let as_svc = app_service.clone();
        let win_handle = window.as_weak();
        window.on_applications_select_all(move || {
            let as_svc = as_svc.clone();
            let win_handle = win_handle.clone();
            tokio::spawn(async move {
                as_svc.select_all().await;
                update_applications_view(&win_handle, &as_svc).await;
            });
        });
    }

    // 4. Deselect All
    {
        let as_svc = app_service.clone();
        let win_handle = window.as_weak();
        window.on_applications_deselect_all(move || {
            let as_svc = as_svc.clone();
            let win_handle = win_handle.clone();
            tokio::spawn(async move {
                as_svc.deselect_all().await;
                update_applications_view(&win_handle, &as_svc).await;
            });
        });
    }

    // 5. Toggle single app selection
    {
        let as_svc = app_service.clone();
        let win_handle = window.as_weak();
        window.on_applications_toggle_app(move |app_id| {
            let as_svc = as_svc.clone();
            let win_handle = win_handle.clone();
            let id = app_id.to_string();
            tokio::spawn(async move {
                as_svc.toggle_app_selection(&id).await;
                update_applications_view(&win_handle, &as_svc).await;
            });
        });
    }

    // 6. Open application
    {
        let as_svc = app_service.clone();
        let win_handle = window.as_weak();
        window.on_applications_open_app(move |app_id| {
            let as_svc = as_svc.clone();
            let win = win_handle.clone();
            let id = app_id.to_string();
            tokio::spawn(async move {
                if let Err(err) = as_svc.launch_app_by_id(&id).await {
                    tracing::error!("Failed to launch application {}: {}", id, err);
                    show_status(&win, format!("{err:#}"));
                }
            });
        });
    }

    // 6b. Create desktop shortcut
    {
        let as_svc = app_service.clone();
        let win_handle = window.as_weak();
        let st = state.clone();
        window.on_applications_create_shortcut(move |app_id| {
            let as_svc = as_svc.clone();
            let win = win_handle.clone();
            let success = st.localization.t("applications.shortcut_created");
            let id = app_id.to_string();
            tokio::spawn(async move {
                match as_svc.create_shortcut_by_id(&id).await {
                    Ok(path) => {
                        tracing::info!("Created desktop shortcut at {:?}", path);
                        show_status(&win, success);
                    }
                    Err(err) => {
                        tracing::error!("Failed to create desktop shortcut for {}: {}", id, err);
                        show_status(&win, format!("{err:#}"));
                    }
                }
            });
        });
    }

    // 7. Uninstall single application
    {
        let as_svc = app_service.clone();
        let win_handle = window.as_weak();
        let st = state.clone();
        window.on_applications_uninstall_single(move |app_id| {
            let as_svc = as_svc.clone();
            let win_handle = win_handle.clone();
            let id = app_id.to_string();

            if let Some(w) = win_handle.upgrade() {
                if w.get_applications_is_uninstalling() {
                    w.set_applications_uninstall_status_text(
                        std::io::Error::from(std::io::ErrorKind::WouldBlock)
                            .to_string()
                            .into(),
                    );
                    return;
                }
                w.set_applications_is_uninstalling(true);
                w.set_applications_uninstall_progress(0.0);
                w.set_applications_uninstall_status_text(
                    st.localization.t("applications.status_preparing").into(),
                );
            }

            let completion = format!("{}: 100%", st.localization.t("applications.uninstalling"));
            tokio::spawn(async move {
                finish_uninstall(
                    &win_handle,
                    as_svc.uninstall_single_app(&id).await,
                    completion,
                )
                .await;
                update_applications_view(&win_handle, &as_svc).await;
            });
        });
    }

    // 8. Batch uninstall
    {
        let as_svc = app_service.clone();
        let win_handle = window.as_weak();
        let st = state.clone();
        window.on_applications_confirm_uninstall_batch(move || {
            let as_svc = as_svc.clone();
            let win_handle = win_handle.clone();

            if let Some(w) = win_handle.upgrade() {
                if w.get_applications_is_uninstalling() {
                    w.set_applications_uninstall_status_text(
                        std::io::Error::from(std::io::ErrorKind::WouldBlock)
                            .to_string()
                            .into(),
                    );
                    return;
                }
                w.set_applications_is_uninstalling(true);
                w.set_applications_uninstall_progress(0.0);
                w.set_applications_uninstall_status_text(
                    st.localization.t("applications.status_preparing").into(),
                );
            }

            let completion = format!("{}: 100%", st.localization.t("applications.uninstalling"));
            tokio::spawn(async move {
                finish_uninstall(&win_handle, as_svc.uninstall_selected().await, completion).await;
                update_applications_view(&win_handle, &as_svc).await;
            });
        });
    }
}

fn show_status(win: &slint::Weak<AppWindow>, text: String) {
    let win = win.clone();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(w) = win.upgrade() {
            w.set_applications_uninstall_status_text(text.into());
        }
    });
}

async fn finish_uninstall(
    win: &slint::Weak<AppWindow>,
    task: anyhow::Result<crate::applications::service::UninstallTask>,
    completion: String,
) {
    let result = match task {
        Ok((mut rx, handle)) => {
            loop {
                match rx.recv().await {
                    Ok(progress) if progress.is_completed => break,
                    Ok(progress) => {
                        let win = win.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(w) = win.upgrade() {
                                w.set_applications_uninstall_progress(progress.percent);
                                w.set_applications_uninstall_status_text(
                                    progress.current_app.into(),
                                );
                            }
                        });
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => break,
                }
            }
            match handle.await {
                Ok(result) => result,
                Err(error) => Err(error.into()),
            }
        }
        Err(error) => Err(error),
    };
    let status = match result {
        Ok(()) => completion,
        Err(error) => format!("{error:#}"),
    };
    let win = win.clone();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(w) = win.upgrade() {
            w.set_applications_uninstall_progress(100.0);
            w.set_applications_uninstall_status_text(status.into());
            w.set_applications_is_uninstalling(false);
        }
    });
}

async fn update_applications_view(win_weak: &slint::Weak<AppWindow>, service: &ApplicationService) {
    let revision = service.view_revision();
    let generation = revision.load(std::sync::atomic::Ordering::SeqCst);
    let (items, current_page, total_pages, total_items) = service.get_current_view().await;
    let selected = service.get_selected_apps().await;
    let selected_count = selected.len() as i32;

    let win_handle = win_weak.clone();
    let _ = slint::invoke_from_event_loop(move || {
        if revision.load(std::sync::atomic::Ordering::SeqCst) != generation {
            return;
        }
        if let Some(w) = win_handle.upgrade() {
            let mut ui_apps = Vec::new();
            for item in items {
                let (icon_image, has_icon_image) = if let Some(path) = &item.icon_path {
                    if let Ok(img) = slint::Image::load_from_path(path) {
                        (img, true)
                    } else {
                        (slint::Image::default(), false)
                    }
                } else {
                    (slint::Image::default(), false)
                };

                ui_apps.push(AppCardData {
                    id: item.id.into(),
                    name: item.name.into(),
                    version: item.version.into(),
                    source_name: item.source.as_str().into(),
                    source_code: item.source.code().into(),
                    description: item.description.into(),
                    has_icon_image,
                    icon_image,
                    is_desktop_app: item.is_desktop_app,
                    is_selected: item.selected,
                });
            }

            w.set_applications_current_page(current_page as i32);
            w.set_applications_total_pages(total_pages as i32);
            w.set_applications_total_items(total_items as i32);
            w.set_applications_selected_count(selected_count);
            w.set_applications_is_loading(false);
            w.set_applications_list(ModelRc::new(VecModel::from(ui_apps)));
        }
    });
}
