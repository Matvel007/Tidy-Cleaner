mod app;
mod applications;
mod cleanup;
mod filesystem;
mod localization;
mod logging;
mod settings;
mod startup;
mod system;
mod theme;

use app::{apply_theme, update_ui_strings, AppState};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use system::{OsInfoCollector, SystemMonitorService};

slint::include_modules!();

use slint::winit_030::{winit, EventResult, WinitWindowAccessor};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = logging::init_logging();
    tracing::info!("Starting Tidy Cleaner");

    let state = Arc::new(AppState::new());
    let monitor = Arc::new(SystemMonitorService::new());
    let cleanup_service = Arc::new(cleanup::CleanupService::new());
    let window = AppWindow::new()?;
    window.set_is_kde(system::OsInfoCollector::is_kde());

    // Wire Subsystems
    cleanup::setup_cleanup_handlers(&window, cleanup_service.clone(), state.clone());

    let app_service = Arc::new(applications::service::ApplicationService::new());
    applications::ui_bridge::setup_applications_handlers(&window, app_service, state.clone());

    let startup_service = Arc::new(startup::StartupService::new());
    startup::setup_startup_handlers(&window, startup_service, state.clone());

    settings::setup_settings_handlers(&window, state.clone());

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

    // Frameless window dragging
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

    // Page navigation
    let state_clone = state.clone();
    window.on_page_selected(move |page| {
        state_clone.set_page(page);
    });

    // Honor the --minimized flag written into the autostart .desktop Exec line.
    if std::env::args().any(|a| a == "--minimized") {
        window.window().set_minimized(false);
        window.window().set_minimized(true);
    }

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
    window.set_gpu_name(
        if snapshot.gpu.name.is_empty() {
            "N/A".to_string()
        } else {
            snapshot.gpu.name.clone()
        }
        .into(),
    );
    let gpu_vram_formatted = if snapshot.gpu.total_memory_mb > 0 {
        format!(
            "{:.1} / {:.1} GB",
            snapshot.gpu.used_memory_mb as f64 / 1024.0,
            snapshot.gpu.total_memory_mb as f64 / 1024.0
        )
    } else {
        "N/A".to_string()
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
    match snapshot.temperature.cpu_temp_c {
        Some(v) => {
            window.set_cpu_temp_str(format!("{:.0}", v).into());
            window.set_cpu_temp_val(v);
        }
        None => {
            window.set_cpu_temp_str("N/A".into());
            window.set_cpu_temp_val(0.0);
        }
    }
    match snapshot.temperature.gpu_temp_c {
        Some(v) => {
            window.set_gpu_temp_str(format!("{:.0}", v).into());
            window.set_gpu_temp_val(v);
        }
        None => {
            window.set_gpu_temp_str("N/A".into());
            window.set_gpu_temp_val(0.0);
        }
    }

    // 5. Storage (Up to 2 disks)
    if let Some(d1) = snapshot.disks.first() {
        window.set_disk1_name(display_disk_name(d1).into());
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
        window.set_disk2_name(display_disk_name(d2).into());
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

fn display_disk_name(disk: &system::DiskInfo) -> String {
    let mount = disk.mount_point.trim();
    // Root mount falls back to the localized "Internal Storage" label in the UI.
    if mount.is_empty() || mount == "/" {
        return String::new();
    }
    // Use only the last path component for a short, friendly label
    // (e.g. "/mnt/storage" -> "storage", "/run/media/u/wd" -> "wd").
    mount
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(mount)
        .to_string()
}
