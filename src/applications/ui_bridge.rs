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
            tokio::spawn(async move {
                as_svc.set_search_query(q).await;
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
            let page = (page_num as usize).max(1);
            tokio::spawn(async move {
                as_svc.set_page(page).await;
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
        window.on_applications_open_app(move |app_id| {
            let as_svc = as_svc.clone();
            let id = app_id.to_string();
            tokio::spawn(async move {
                if let Err(err) = as_svc.launch_app_by_id(&id).await {
                    tracing::error!("Failed to launch application {}: {}", id, err);
                }
            });
        });
    }

    // 6b. Create desktop shortcut
    {
        let as_svc = app_service.clone();
        window.on_applications_create_shortcut(move |app_id| {
            let as_svc = as_svc.clone();
            let id = app_id.to_string();
            tokio::spawn(async move {
                match as_svc.create_shortcut_by_id(&id).await {
                    Ok(path) => {
                        tracing::info!("Created desktop shortcut at {:?}", path);
                    }
                    Err(err) => {
                        tracing::error!("Failed to create desktop shortcut for {}: {}", id, err);
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
                w.set_applications_is_uninstalling(true);
                w.set_applications_uninstall_progress(0.0);
                w.set_applications_uninstall_status_text(
                    st.localization.t("applications.status_preparing").into(),
                );
            }

            tokio::spawn(async move {
                let (mut rx, handle) = as_svc.uninstall_single_app(&id).await;
                let win_progress = win_handle.clone();

                tokio::spawn(async move {
                    while let Ok(progress) = rx.recv().await {
                        let win = win_progress.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(w) = win.upgrade() {
                                w.set_applications_uninstall_progress(progress.percent);
                                let status = match &progress.error_message {
                                    Some(err) => err.clone(),
                                    None => progress.current_app.to_string(),
                                };
                                w.set_applications_uninstall_status_text(status.into());
                                if progress.is_completed {
                                    w.set_applications_is_uninstalling(false);
                                }
                            }
                        });
                    }
                });

                let _ = handle.await;
                as_svc.refresh_installed_apps().await;
                let win_done = win_handle.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(w) = win_done.upgrade() {
                        w.set_applications_is_uninstalling(false);
                    }
                });
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
                w.set_applications_is_uninstalling(true);
                w.set_applications_uninstall_progress(0.0);
                w.set_applications_uninstall_status_text(
                    st.localization.t("applications.status_preparing").into(),
                );
            }

            tokio::spawn(async move {
                let (mut rx, handle) = as_svc.uninstall_selected().await;
                let win_progress = win_handle.clone();

                tokio::spawn(async move {
                    while let Ok(progress) = rx.recv().await {
                        let win = win_progress.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(w) = win.upgrade() {
                                w.set_applications_uninstall_progress(progress.percent);
                                let status = match &progress.error_message {
                                    Some(err) => err.clone(),
                                    None => progress.current_app.to_string(),
                                };
                                w.set_applications_uninstall_status_text(status.into());
                                if progress.is_completed {
                                    w.set_applications_is_uninstalling(false);
                                }
                            }
                        });
                    }
                });

                let _ = handle.await;
                as_svc.refresh_installed_apps().await;
                let win_done = win_handle.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(w) = win_done.upgrade() {
                        w.set_applications_is_uninstalling(false);
                    }
                });
                update_applications_view(&win_handle, &as_svc).await;
            });
        });
    }
}

async fn update_applications_view(win_weak: &slint::Weak<AppWindow>, service: &ApplicationService) {
    let (items, current_page, total_pages, total_items) = service.get_current_view().await;
    let selected = service.get_selected_apps().await;
    let selected_count = selected.len() as i32;

    let win_handle = win_weak.clone();
    let _ = slint::invoke_from_event_loop(move || {
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
            w.set_applications_list(ModelRc::new(VecModel::from(ui_apps)));
        }
    });
}
