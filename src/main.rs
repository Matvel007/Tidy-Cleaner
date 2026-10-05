mod app;
mod applications;
mod cleanup;
mod filesystem;
mod localization;
mod logging;
mod process;
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
    let (monitor_stop, monitor_stop_rx) = tokio::sync::watch::channel(false);
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
    let close_stop = monitor_stop.clone();
    window.on_window_close(move || {
        let _ = close_stop.send(true);
        if let Some(w) = win_close.upgrade() {
            let _ = w.window().hide();
        }
        let _ = slint::quit_event_loop();
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
                        // Only minimize and close remain (two 32px buttons plus padding).
                        let controls_zone = 88.0 * scale;
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

    // Start sampling only after the window is shown and the event loop is running.
    let (ready, ready_rx) = tokio::sync::watch::channel(false);
    slint::Timer::single_shot(Duration::ZERO, move || {
        let _ = ready.send(true);
    });
    let mut monitor_tasks = Vec::new();
    for gpu_only in [false, true] {
        let monitor = monitor.clone();
        let win_handle = window.as_weak();
        let localization = state.localization.clone();
        let mut stop = monitor_stop_rx.clone();
        let stop_sender = monitor_stop.clone();
        let mut ready = ready_rx.clone();
        monitor_tasks.push(tokio::spawn(async move {
            tokio::select! {
                _ = stop.wait_for(|stopped| *stopped) => return,
                started = ready.wait_for(|started| *started) => {
                    if started.is_err() { return; }
                }
            }
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    _ = stop.wait_for(|stopped| *stopped) => break,
                    _ = interval.tick() => {}
                }
                let sampler = monitor.clone();
                let result = tokio::task::spawn_blocking(move || {
                    if gpu_only {
                        sampler.sample_gpu();
                        None
                    } else {
                        Some(sampler.sample_snapshot())
                    }
                })
                .await;
                if *stop.borrow() {
                    break;
                }
                let snapshot = match result {
                    Ok(snapshot) => snapshot,
                    Err(error) => {
                        tracing::error!(%error, "Telemetry sampler failed");
                        break;
                    }
                };
                let handle = win_handle.clone();
                let stop_sender = stop_sender.clone();
                let localization = localization.clone();
                if slint::invoke_from_event_loop(move || {
                    if let Some(win) = handle.upgrade() {
                        if let Some(snapshot) = snapshot {
                            apply_snapshot_to_ui(&win, &snapshot, &localization);
                        }
                    } else {
                        let _ = stop_sender.send(true);
                    }
                })
                .is_err()
                {
                    break;
                }
            }
        }));
    }

    // Page navigation
    let state_clone = state.clone();
    window.on_page_selected(move |page| {
        state_clone.set_page(page);
    });

    // Honor the --minimized flag written into the autostart .desktop Exec line.
    // The window must be shown first, so minimize shortly after the event loop starts.
    if std::env::args().any(|a| a == "--minimized") {
        let win_min_start = window.as_weak();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(400)).await;
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(w) = win_min_start.upgrade() {
                    w.window().set_minimized(false);
                    w.window().set_minimized(true);
                }
            });
        });
    }

    let result = window.run();
    let _ = monitor_stop.send(true);
    for task in monitor_tasks {
        let _ = task.await;
    }
    result?;
    Ok(())
}

fn apply_snapshot_to_ui(
    window: &AppWindow,
    snapshot: &system::SystemSnapshot,
    localization: &localization::LocalizationService,
) {
    // 1. CPU
    window.set_cpu_usage_str(format!("{:.1}", snapshot.cpu.usage_percent).into());
    window.set_cpu_cores_str(format!("{}", snapshot.cpu.core_count).into());
    window.set_cpu_freq_str(format!("{} MHz", snapshot.cpu.frequency_mhz).into());
    window.set_cpu_brand(snapshot.cpu.brand_name.clone().into());
    let cpu_arc = system::generate_arc_svg_path(60.0, 60.0, 48.0, snapshot.cpu.usage_percent);
    window.set_cpu_arc_path(cpu_arc.into());

    // 2. GPU
    window.set_gpu_usage_str(
        if snapshot.gpu.usage_available {
            format!("{:.1}", snapshot.gpu.usage_percent)
        } else {
            "N/A".to_string()
        }
        .into(),
    );
    window.set_gpu_name(
        if snapshot.gpu.name.is_empty() {
            "N/A".to_string()
        } else {
            snapshot.gpu.name.clone()
        }
        .into(),
    );
    let gpu_vram_formatted = if snapshot.gpu.memory_available {
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
        window.set_disk1_name(display_disk_name(d1, 1, snapshot.disks.len()).into());
        window.set_disk1_fs(d1.file_system.clone().into());
        window.set_disk1_used_str(display_disk_bytes(d1.used_bytes, d1.total_bytes).into());
        window.set_disk1_total_str(display_disk_bytes(d1.total_bytes, d1.total_bytes).into());
        window.set_disk1_free_str(display_disk_bytes(d1.available_bytes, d1.total_bytes).into());
        window.set_disk1_usage_ratio(d1.usage_ratio);
        window.set_disk1_percent_str(
            if d1.total_bytes > 0 {
                format!("{:.0}%", d1.usage_ratio * 100.0)
            } else {
                "N/A".to_string()
            }
            .into(),
        );
    } else {
        window.set_disk1_name("N/A".into());
        window.set_disk1_fs("N/A".into());
        window.set_disk1_used_str("N/A".into());
        window.set_disk1_total_str("N/A".into());
        window.set_disk1_free_str("N/A".into());
        window.set_disk1_percent_str("N/A".into());
        window.set_disk1_usage_ratio(0.0);
    }

    if snapshot.disks.len() > 1 {
        let d2 = &snapshot.disks[1];
        window.set_has_disk2(true);
        window.set_disk2_name(display_disk_name(d2, 2, snapshot.disks.len()).into());
        window.set_disk2_fs(d2.file_system.clone().into());
        window.set_disk2_used_str(display_disk_bytes(d2.used_bytes, d2.total_bytes).into());
        window.set_disk2_total_str(display_disk_bytes(d2.total_bytes, d2.total_bytes).into());
        window.set_disk2_free_str(display_disk_bytes(d2.available_bytes, d2.total_bytes).into());
        window.set_disk2_usage_ratio(d2.usage_ratio);
        window.set_disk2_percent_str(
            if d2.total_bytes > 0 {
                format!("{:.0}%", d2.usage_ratio * 100.0)
            } else {
                "N/A".to_string()
            }
            .into(),
        );
    } else {
        window.set_has_disk2(false);
    }

    // 6. System Overview
    window.set_os_name(snapshot.overview.os_name.clone().into());
    window.set_kernel_version(snapshot.overview.kernel_version.clone().into());
    window.set_hostname(snapshot.overview.hostname.clone().into());
    window.set_uptime_str(snapshot.overview.uptime_formatted.clone().into());

    // 7. Swap
    let has_swap = snapshot.memory.total_swap_bytes > 0;
    window.set_has_swap(has_swap);
    window.set_is_zram(snapshot.memory.is_zram);
    if has_swap {
        let swap_used_gb = snapshot.memory.used_swap_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
        let swap_total_gb = snapshot.memory.total_swap_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
        window.set_swap_used_str(format!("{:.1} GB", swap_used_gb).into());
        window.set_swap_total_str(format!("{:.1} GB", swap_total_gb).into());
        window.set_swap_usage_str(format!("{:.1}", snapshot.memory.swap_usage_percent).into());
    }

    // 8. Network
    window.set_net_rx_str(snapshot.network.rx_speed_formatted.clone().into());
    window.set_net_tx_str(snapshot.network.tx_speed_formatted.clone().into());
    window.set_net_iface_str(snapshot.network.active_interface.clone().into());

    // 9. Battery
    window.set_has_battery(snapshot.battery.has_battery);
    if snapshot.battery.has_battery {
        let key = snapshot.battery.status.localization_key();
        let status = localization.t(key);
        let status = if status == key { "N/A" } else { &status };
        let bat_text = if snapshot.battery.energy_watts > 0.0 {
            format!(
                "{:.0}% ({}, {:.1}W)",
                snapshot.battery.charge_percent, status, snapshot.battery.energy_watts
            )
        } else {
            format!("{:.0}% ({})", snapshot.battery.charge_percent, status)
        };
        window.set_battery_str(bat_text.into());
    }
}

fn display_disk_bytes(bytes: u64, total: u64) -> String {
    if total > 0 {
        OsInfoCollector::format_bytes(bytes)
    } else {
        "N/A".to_string()
    }
}

fn display_disk_name(disk: &system::DiskInfo, position: usize, total: usize) -> String {
    let mount = disk.mount_point.trim();
    // Mount paths (and remote sources) identify storage without inventing friendly labels.
    let label = if system::disks::DiskCollector::is_remote(&disk.file_system) {
        format!("{} ({})", mount, disk.name)
    } else {
        mount.to_string()
    };
    if total > 2 {
        format!("{} [{}/{}]", label, position, total)
    } else {
        label
    }
}

#[cfg(test)]
mod telemetry_display_tests {
    use super::*;

    #[test]
    fn disk_labels_report_mounts_omitted_count_and_unavailable_capacity() {
        let mut disk = system::DiskInfo {
            name: "server:/storage".into(),
            mount_point: "/".into(),
            file_system: "ext4".into(),
            total_bytes: 0,
            used_bytes: 0,
            available_bytes: 0,
            usage_ratio: 0.0,
        };
        assert_eq!(display_disk_name(&disk, 1, 1), "/");
        assert_eq!(display_disk_name(&disk, 1, 3), "/ [1/3]");
        disk.mount_point = "/remote".into();
        disk.file_system = "nfs".into();
        assert_eq!(
            display_disk_name(&disk, 2, 3),
            "/remote (server:/storage) [2/3]"
        );
        assert_eq!(display_disk_bytes(0, 0), "N/A");
        assert_eq!(display_disk_bytes(0, 100), "0 B");
    }
}
