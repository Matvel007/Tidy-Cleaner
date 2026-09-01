mod app;
mod localization;
mod logging;
mod settings;
mod system;
mod theme;

use app::AppState;
use localization::Language;
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use system::{OsInfoCollector, SystemMonitorService};
use theme::ThemeMode;

slint::include_modules!();

use slint::winit_030::{winit, EventResult, WinitWindowAccessor};

fn update_ui_strings(window: &AppWindow, state: &AppState) {
    let loc = &state.localization;
    let i18n = window.global::<I18n>();

    i18n.set_app_title(loc.t("app.title").into());
    i18n.set_nav_dashboard(loc.t("nav.dashboard").into());
    i18n.set_nav_cleanup(loc.t("nav.cleanup").into());
    i18n.set_nav_applications(loc.t("nav.applications").into());
    i18n.set_nav_startup(loc.t("nav.startup").into());
    i18n.set_nav_settings(loc.t("nav.settings").into());

    i18n.set_common_loading(loc.t("common.loading").into());
    i18n.set_common_save(loc.t("common.save").into());
    i18n.set_common_cancel(loc.t("common.cancel").into());
    i18n.set_common_close(loc.t("common.close").into());
    i18n.set_common_search(loc.t("common.search").into());
    i18n.set_common_refresh(loc.t("common.refresh").into());
    i18n.set_common_all(loc.t("common.all").into());
    i18n.set_common_enabled(loc.t("common.enabled").into());
    i18n.set_common_disabled(loc.t("common.disabled").into());
    i18n.set_common_status(loc.t("common.status").into());
    i18n.set_common_total(loc.t("common.total").into());
    i18n.set_common_used(loc.t("common.used").into());
    i18n.set_common_free(loc.t("common.free").into());
    i18n.set_common_open(loc.t("common.open").into());
    i18n.set_common_delete(loc.t("common.delete").into());
    i18n.set_common_version(loc.t("common.version").into());
    i18n.set_common_total_items(loc.t("common.total_items").into());

    i18n.set_dashboard_title(loc.t("dashboard.title").into());
    i18n.set_dashboard_subtitle(loc.t("dashboard.subtitle").into());
    i18n.set_dashboard_cpu_usage(loc.t("dashboard.cpu_usage").into());
    i18n.set_dashboard_cpu_cores(loc.t("dashboard.cpu_cores").into());
    i18n.set_dashboard_cpu_freq(loc.t("dashboard.cpu_freq").into());
    i18n.set_dashboard_ram_usage(loc.t("dashboard.ram_usage").into());
    i18n.set_dashboard_ram_used(loc.t("dashboard.ram_used").into());
    i18n.set_dashboard_ram_available(loc.t("dashboard.ram_available").into());
    i18n.set_dashboard_disks(loc.t("dashboard.disks").into());
    i18n.set_dashboard_system_info(loc.t("dashboard.system_info").into());
    i18n.set_dashboard_system_overview(loc.t("dashboard.system_overview").into());
    i18n.set_dashboard_os(loc.t("dashboard.os").into());
    i18n.set_dashboard_os_label(loc.t("dashboard.os_label").into());
    i18n.set_dashboard_kernel(loc.t("dashboard.kernel").into());
    i18n.set_dashboard_kernel_label(loc.t("dashboard.kernel_label").into());
    i18n.set_dashboard_arch(loc.t("dashboard.arch").into());
    i18n.set_dashboard_hostname(loc.t("dashboard.hostname").into());
    i18n.set_dashboard_host_label(loc.t("dashboard.host_label").into());
    i18n.set_dashboard_uptime(loc.t("dashboard.uptime").into());
    i18n.set_dashboard_uptime_label(loc.t("dashboard.uptime_label").into());
    i18n.set_dashboard_temperature(loc.t("dashboard.temperature").into());
    i18n.set_dashboard_cpu_temp(loc.t("dashboard.cpu_temp").into());
    i18n.set_dashboard_gpu_temp(loc.t("dashboard.gpu_temp").into());
    i18n.set_dashboard_temp_high(loc.t("dashboard.temp_high").into());
    i18n.set_dashboard_temp_optimal(loc.t("dashboard.temp_optimal").into());
    i18n.set_dashboard_storage_primary(loc.t("dashboard.storage_primary").into());
    i18n.set_dashboard_storage_secondary(loc.t("dashboard.storage_secondary").into());
    i18n.set_dashboard_storage_used(loc.t("dashboard.storage_used").into());
    i18n.set_dashboard_storage_free(loc.t("dashboard.storage_free").into());
    i18n.set_dashboard_storage_total(loc.t("dashboard.storage_total").into());
    i18n.set_dashboard_storage_filesystem(loc.t("dashboard.storage_filesystem").into());
    i18n.set_dashboard_cores_label(loc.t("dashboard.cores_label").into());

    i18n.set_cleanup_title(loc.t("cleanup.title").into());
    i18n.set_cleanup_subtitle(loc.t("cleanup.subtitle").into());
    i18n.set_cleanup_fast_scan(loc.t("cleanup.fast_scan").into());
    i18n.set_cleanup_full_scan(loc.t("cleanup.full_scan").into());
    i18n.set_cleanup_scan_start(loc.t("cleanup.scan_start").into());
    i18n.set_cleanup_clean_selected(loc.t("cleanup.clean_selected").into());
    i18n.set_cleanup_select_all(loc.t("cleanup.select_all").into());
    i18n.set_cleanup_deselect_all(loc.t("cleanup.deselect_all").into());
    i18n.set_cleanup_empty_title(loc.t("cleanup.empty_title").into());
    i18n.set_cleanup_empty_desc(loc.t("cleanup.empty_desc").into());

    i18n.set_applications_title(loc.t("applications.title").into());
    i18n.set_applications_subtitle(loc.t("applications.subtitle").into());
    i18n.set_applications_search_placeholder(loc.t("applications.search_placeholder").into());
    i18n.set_applications_uninstall_selected(loc.t("applications.uninstall_selected").into());
    i18n.set_applications_empty_title(loc.t("applications.empty_title").into());
    i18n.set_applications_empty_desc(loc.t("applications.empty_desc").into());

    i18n.set_startup_title(loc.t("startup.title").into());
    i18n.set_startup_subtitle(loc.t("startup.subtitle").into());
    i18n.set_startup_add_app(loc.t("startup.add_app").into());
    i18n.set_startup_empty_title(loc.t("startup.empty_title").into());
    i18n.set_startup_empty_desc(loc.t("startup.empty_desc").into());

    i18n.set_settings_title(loc.t("settings.title").into());
    i18n.set_settings_subtitle(loc.t("settings.subtitle").into());
    i18n.set_settings_appearance(loc.t("settings.appearance").into());
    i18n.set_settings_theme(loc.t("settings.theme").into());
    i18n.set_settings_theme_dark(loc.t("settings.theme_dark").into());
    i18n.set_settings_theme_light(loc.t("settings.theme_light").into());
    i18n.set_settings_theme_system(loc.t("settings.theme_system").into());
    i18n.set_settings_language(loc.t("settings.language").into());
    i18n.set_settings_startup_section(loc.t("settings.startup_section").into());
    i18n.set_settings_autostart(loc.t("settings.autostart").into());
    i18n.set_settings_start_minimized(loc.t("settings.start_minimized").into());
    i18n.set_settings_start_in_tray(loc.t("settings.start_in_tray").into());
    i18n.set_settings_cleanup_section(loc.t("settings.cleanup_section").into());
    i18n.set_settings_confirm_delete(loc.t("settings.confirm_delete").into());
    i18n.set_settings_show_hidden(loc.t("settings.show_hidden").into());
    i18n.set_settings_about(loc.t("settings.about").into());
    i18n.set_settings_version_label(loc.t("settings.version_label").into());
    i18n.set_settings_open_logs(loc.t("settings.open_logs").into());

    window.set_current_lang(state.get_language().as_str().into());
}

fn apply_theme(window: &AppWindow, theme_mode: ThemeMode) {
    let theme = window.global::<Theme>();
    theme.set_is_dark(theme_mode.is_dark());
    window.set_theme_mode(theme_mode.to_i32());
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = logging::init_logging();
    tracing::info!("Starting Tidy Cleaner");

    let state = Arc::new(AppState::new());
    let monitor = Arc::new(SystemMonitorService::new());
    let window = AppWindow::new()?;

    // Frameless window controls: minimize
    let win_min = window.as_weak();
    window.on_window_minimize(move || {
        if let Some(w) = win_min.upgrade() {
            w.window().set_minimized(false);
            w.window().set_minimized(true);
        }
    });

    // Frameless window controls: close
    let win_close = window.as_weak();
    window.on_window_close(move || {
        if let Some(w) = win_close.upgrade() {
            let _ = w.window().hide();
            std::process::exit(0);
        }
    });

    // Frameless window dragging.
    //
    // We don't move the window ourselves (Wayland ignores `set_position`); instead we
    // trigger the compositor's own interactive move via winit's `drag_window()`, which
    // maps to `xdg_toplevel.move` on Wayland and `_NET_WM_MOVERESIZE` on X11.
    //
    // A winit event filter is used (instead of a Slint `TouchArea`) so the move does not
    // leave a stuck mouse grab that would swallow clicks on the rest of the UI.
    let cursor_pos: Rc<Cell<Option<(f64, f64)>>> = Rc::new(Cell::new(None));
    window
        .window()
        .on_winit_window_event(move |slint_window, event| {
            match event {
                winit::event::WindowEvent::CursorMoved { position, .. } => {
                    cursor_pos.set(Some((position.x, position.y)));
                }
                winit::event::WindowEvent::MouseInput {
                    state: winit::event::ElementState::Pressed,
                    button: winit::event::MouseButton::Left,
                    ..
                } => {
                    if let Some((x, y)) = cursor_pos.get() {
                        let scale = slint_window.scale_factor() as f64;
                        let size = slint_window.size();
                        // Titlebar is 40 logical px tall; the right ~80 logical px are
                        // reserved for the minimize/close window controls.
                        let titlebar_height = 40.0 * scale;
                        let controls_zone = 80.0 * scale;
                        if y < titlebar_height && x < (size.width as f64 - controls_zone) {
                            slint_window.with_winit_window(|winit_window| {
                                let _ = winit_window.drag_window();
                            });
                            return EventResult::PreventDefault;
                        }
                    }
                }
                _ => {}
            }
            EventResult::Propagate
        });

    // Initial state sync
    let current_theme = state.get_theme();
    apply_theme(&window, current_theme);
    update_ui_strings(&window, &state);

    // Initial sampling of system metrics
    let initial_snapshot = monitor.sample_snapshot();
    apply_snapshot_to_ui(&window, &initial_snapshot);

    // Spawn async background monitoring loop on Tokio runtime
    let win_handle_monitor = window.as_weak();
    let monitor_clone = monitor.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(1000));
        loop {
            interval.tick().await;
            let snapshot = monitor_clone.sample_snapshot();
            let handle = win_handle_monitor.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(win) = handle.upgrade() {
                    apply_snapshot_to_ui(&win, &snapshot);
                }
            });
        }
    });

    // Event handlers
    let state_clone = state.clone();
    window.on_page_selected(move |page| {
        state_clone.set_page(page);
    });

    let state_clone = state.clone();
    let win_handle = window.as_weak();
    window.on_settings_theme_changed(move |mode_idx| {
        if let Some(win) = win_handle.upgrade() {
            let theme = ThemeMode::from_i32(mode_idx);
            state_clone.set_theme(theme);
            apply_theme(&win, theme);
        }
    });

    let state_clone = state.clone();
    let win_handle = window.as_weak();
    window.on_settings_lang_changed(move |lang_str| {
        if let Some(win) = win_handle.upgrade() {
            let lang = Language::from_str_name(lang_str.as_str());
            state_clone.set_language(lang);
            update_ui_strings(&win, &state_clone);
        }
    });

    let state_clone = state.clone();
    window.on_settings_autostart_toggled(move |val| {
        if let Ok(mut s) = state_clone.settings.write() {
            s.autostart = val;
            let _ = settings::SettingsStorage::save(&s);
        }
    });

    let state_clone = state.clone();
    window.on_settings_start_minimized_toggled(move |val| {
        if let Ok(mut s) = state_clone.settings.write() {
            s.start_minimized = val;
            let _ = settings::SettingsStorage::save(&s);
        }
    });

    let state_clone = state.clone();
    window.on_settings_start_in_tray_toggled(move |val| {
        if let Ok(mut s) = state_clone.settings.write() {
            s.start_in_tray = val;
            let _ = settings::SettingsStorage::save(&s);
        }
    });

    let state_clone = state.clone();
    window.on_settings_confirm_delete_toggled(move |val| {
        if let Ok(mut s) = state_clone.settings.write() {
            s.confirm_delete = val;
            let _ = settings::SettingsStorage::save(&s);
        }
    });

    let state_clone = state.clone();
    window.on_settings_show_hidden_toggled(move |val| {
        if let Ok(mut s) = state_clone.settings.write() {
            s.show_hidden = val;
            let _ = settings::SettingsStorage::save(&s);
        }
    });

    window.on_settings_open_logs_clicked(move || {
        let log_dir = logging::dirs_log_path();
        let _ = std::process::Command::new("xdg-open").arg(log_dir).spawn();
    });

    window.run()?;
    Ok(())
}

fn apply_snapshot_to_ui(window: &AppWindow, snapshot: &system::SystemSnapshot) {
    // 1. CPU
    window.set_cpu_usage_str(format!("{:.1}", snapshot.cpu.usage_percent).into());
    window.set_cpu_cores_str(format!("{}", snapshot.cpu.core_count).into());
    window.set_cpu_freq_str(format!("{} MHz", snapshot.cpu.frequency_mhz).into());
    window.set_cpu_brand(snapshot.cpu.brand_name.clone().into());
    let cpu_arc = system::generate_arc_svg_path(60.0, 60.0, 48.0, snapshot.cpu.usage_percent);
    window.set_cpu_arc_path(cpu_arc.into());

    // 2. GPU
    window.set_gpu_usage_str(format!("{:.1}", snapshot.gpu.usage_percent).into());
    window.set_gpu_name(snapshot.gpu.name.clone().into());
    let gpu_vram_formatted = if snapshot.gpu.total_memory_mb > 0 {
        format!(
            "{:.1} / {:.1} GB",
            snapshot.gpu.used_memory_mb as f64 / 1024.0,
            snapshot.gpu.total_memory_mb as f64 / 1024.0
        )
    } else {
        "Active".to_string()
    };
    window.set_gpu_vram_str(gpu_vram_formatted.into());
    let gpu_arc = system::generate_arc_svg_path(60.0, 60.0, 48.0, snapshot.gpu.usage_percent);
    window.set_gpu_arc_path(gpu_arc.into());

    // 3. RAM
    window.set_ram_usage_str(format!("{:.1}", snapshot.memory.usage_percent).into());
    let ram_used_gb = snapshot.memory.used_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
    let ram_total_gb = snapshot.memory.total_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
    window.set_ram_used_str(format!("{:.1}", ram_used_gb).into());
    window.set_ram_available_str(
        OsInfoCollector::format_bytes(snapshot.memory.available_bytes).into(),
    );
    window.set_ram_total_str(format!("{:.1} GB", ram_total_gb).into());
    let ram_arc = system::generate_arc_svg_path(60.0, 60.0, 48.0, snapshot.memory.usage_percent);
    window.set_ram_arc_path(ram_arc.into());

    // 4. Temperature
    window.set_cpu_temp_str(format!("{:.0}", snapshot.temperature.cpu_temp_c).into());
    window.set_cpu_temp_val(snapshot.temperature.cpu_temp_c);
    window.set_gpu_temp_str(format!("{:.0}", snapshot.temperature.gpu_temp_c).into());
    window.set_gpu_temp_val(snapshot.temperature.gpu_temp_c);

    // 5. Storage (Up to 2 disks)
    if let Some(d1) = snapshot.disks.first() {
        window.set_disk1_name(String::new().into());
        window.set_disk1_fs(d1.file_system.clone().into());
        window.set_disk1_used_str(OsInfoCollector::format_bytes(d1.used_bytes).into());
        window.set_disk1_total_str(OsInfoCollector::format_bytes(d1.total_bytes).into());
        window.set_disk1_free_str(OsInfoCollector::format_bytes(d1.available_bytes).into());
        window.set_disk1_usage_ratio(d1.usage_ratio);
        window.set_disk1_percent_str(format!("{:.0}%", d1.usage_ratio * 100.0).into());
    }

    if snapshot.disks.len() > 1 {
        let d2 = &snapshot.disks[1];
        window.set_has_disk2(true);
        let name2 = if d2.mount_point.contains("/media") || d2.mount_point.contains("/run") {
            "Virtual Disk (20 GB)".to_string()
        } else {
            String::new()
        };
        window.set_disk2_name(name2.into());
        window.set_disk2_fs(d2.file_system.clone().into());
        window.set_disk2_used_str(OsInfoCollector::format_bytes(d2.used_bytes).into());
        window.set_disk2_total_str(OsInfoCollector::format_bytes(d2.total_bytes).into());
        window.set_disk2_free_str(OsInfoCollector::format_bytes(d2.available_bytes).into());
        window.set_disk2_usage_ratio(d2.usage_ratio);
        window.set_disk2_percent_str(format!("{:.0}%", d2.usage_ratio * 100.0).into());
    } else {
        window.set_has_disk2(false);
    }

    // 6. System Overview
    window.set_os_name(snapshot.overview.os_name.clone().into());
    window.set_kernel_version(snapshot.overview.kernel_version.clone().into());
    window.set_hostname(snapshot.overview.hostname.clone().into());
    window.set_uptime_str(snapshot.overview.uptime_formatted.clone().into());
}
