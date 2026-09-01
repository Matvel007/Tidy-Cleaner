mod app;
mod localization;
mod logging;
mod settings;
mod theme;

use app::AppState;
use localization::Language;
use std::sync::Arc;
use theme::ThemeMode;

slint::include_modules!();

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
    i18n.set_dashboard_os(loc.t("dashboard.os").into());
    i18n.set_dashboard_kernel(loc.t("dashboard.kernel").into());
    i18n.set_dashboard_arch(loc.t("dashboard.arch").into());
    i18n.set_dashboard_hostname(loc.t("dashboard.hostname").into());
    i18n.set_dashboard_uptime(loc.t("dashboard.uptime").into());

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
    tracing::info!("Starting Linux System Manager (Cleaner)");

    let state = Arc::new(AppState::new());
    let window = AppWindow::new()?;

    // Sync initial state to UI
    let current_theme = state.get_theme();
    apply_theme(&window, current_theme);
    update_ui_strings(&window, &state);

    let state_clone = state.clone();
    let win_handle = window.as_weak();
    window.on_theme_toggle_requested(move || {
        if let Some(win) = win_handle.upgrade() {
            let current = state_clone.get_theme();
            let new_theme = if current.is_dark() {
                ThemeMode::Light
            } else {
                ThemeMode::Dark
            };
            state_clone.set_theme(new_theme);
            apply_theme(&win, new_theme);
        }
    });

    let state_clone = state.clone();
    let win_handle = window.as_weak();
    window.on_language_toggle_requested(move || {
        if let Some(win) = win_handle.upgrade() {
            let next_lang = match state_clone.get_language() {
                Language::En => Language::Ru,
                Language::Ru => Language::En,
            };
            state_clone.set_language(next_lang);
            update_ui_strings(&win, &state_clone);
        }
    });

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

    window.on_refresh_requested(|| {
        tracing::debug!("Refresh triggered");
    });

    window.run()?;
    Ok(())
}
