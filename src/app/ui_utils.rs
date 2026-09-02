use crate::app::state::AppState;
use crate::theme::ThemeMode;
use crate::{AppWindow, I18n, Theme};
use slint::ComponentHandle;

pub fn update_ui_strings(window: &AppWindow, state: &AppState) {
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
    i18n.set_cleanup_scanning(loc.t("cleanup.scanning").into());
    i18n.set_cleanup_cleaning(loc.t("cleanup.cleaning").into());
    i18n.set_cleanup_status_scanning(loc.t("cleanup.status_scanning").into());
    i18n.set_cleanup_status_preparing(loc.t("cleanup.status_preparing").into());
    i18n.set_cleanup_cancel(loc.t("cleanup.cancel").into());
    i18n.set_cleanup_clean_done(loc.t("cleanup.clean_done").into());
    i18n.set_cleanup_clean_done_desc(loc.t("cleanup.clean_done_desc").into());
    i18n.set_cleanup_clean_selected_btn(loc.t("cleanup.clean_selected_btn").into());
    i18n.set_cleanup_items_found(loc.t("cleanup.items_found").into());
    i18n.set_cleanup_reclaimable(loc.t("cleanup.reclaimable").into());
    i18n.set_cleanup_selected(loc.t("cleanup.selected").into());
    i18n.set_cleanup_nothing_found_title(loc.t("cleanup.nothing_found_title").into());
    i18n.set_cleanup_nothing_found_desc(loc.t("cleanup.nothing_found_desc").into());
    i18n.set_cleanup_risk_safe(loc.t("cleanup.risk.safe").into());
    i18n.set_cleanup_risk_warning(loc.t("cleanup.risk.warning").into());
    i18n.set_cleanup_risk_dangerous(loc.t("cleanup.risk.dangerous").into());
    i18n.set_cleanup_confirm_title(loc.t("cleanup.confirm_title").into());
    i18n.set_cleanup_confirm_desc(loc.t("cleanup.confirm_desc").into());
    i18n.set_cleanup_confirm_count_label(loc.t("cleanup.confirm_count_label").into());
    i18n.set_cleanup_confirm_size_label(loc.t("cleanup.confirm_size_label").into());

    i18n.set_applications_title(loc.t("applications.title").into());
    i18n.set_applications_subtitle(loc.t("applications.subtitle").into());
    i18n.set_applications_search_placeholder(loc.t("applications.search_placeholder").into());
    i18n.set_applications_filter_all(loc.t("applications.filter_all").into());
    i18n.set_applications_uninstall_selected(loc.t("applications.uninstall_selected").into());
    i18n.set_applications_uninstalling(loc.t("applications.uninstalling").into());
    i18n.set_applications_status_preparing(loc.t("applications.status_preparing").into());
    i18n.set_applications_confirm_uninstall_title(
        loc.t("applications.confirm_uninstall_title").into(),
    );
    i18n.set_applications_confirm_uninstall_desc(
        loc.t("applications.confirm_uninstall_desc").into(),
    );
    i18n.set_applications_selected_for_uninstall(
        loc.t("applications.selected_for_uninstall").into(),
    );
    i18n.set_applications_uninstall_btn(loc.t("applications.uninstall_btn").into());
    i18n.set_applications_shortcut_created(loc.t("applications.shortcut_created").into());
    i18n.set_applications_empty_title(loc.t("applications.empty_title").into());
    i18n.set_applications_empty_desc(loc.t("applications.empty_desc").into());

    i18n.set_startup_title(loc.t("startup.title").into());
    i18n.set_startup_subtitle(loc.t("startup.subtitle").into());
    i18n.set_startup_add_app(loc.t("startup.add_app").into());
    i18n.set_startup_empty_title(loc.t("startup.empty_title").into());
    i18n.set_startup_empty_desc(loc.t("startup.empty_desc").into());
    i18n.set_startup_source_user(loc.t("startup.source_user").into());
    i18n.set_startup_source_system(loc.t("startup.source_system").into());
    i18n.set_startup_enabled(loc.t("startup.enabled").into());
    i18n.set_startup_disabled(loc.t("startup.disabled").into());
    i18n.set_startup_add_modal_title(loc.t("startup.add_modal_title").into());
    i18n.set_startup_field_name(loc.t("startup.field_name").into());
    i18n.set_startup_field_name_placeholder(loc.t("startup.field_name_placeholder").into());
    i18n.set_startup_field_exec(loc.t("startup.field_exec").into());
    i18n.set_startup_field_exec_placeholder(loc.t("startup.field_exec_placeholder").into());
    i18n.set_startup_field_comment(loc.t("startup.field_comment").into());
    i18n.set_startup_field_comment_placeholder(loc.t("startup.field_comment_placeholder").into());
    i18n.set_startup_field_terminal(loc.t("startup.field_terminal").into());
    i18n.set_startup_save_btn(loc.t("startup.save_btn").into());
    i18n.set_startup_confirm_remove_title(loc.t("startup.confirm_remove_title").into());
    i18n.set_startup_confirm_remove_desc(loc.t("startup.confirm_remove_desc").into());
    i18n.set_startup_remove_btn(loc.t("startup.remove_btn").into());
    i18n.set_tooltip_remove_startup(loc.t("tooltip.remove_startup").into());
    i18n.set_tooltip_toggle_startup(loc.t("tooltip.toggle_startup").into());

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
    i18n.set_settings_include_downloads(loc.t("settings.include_downloads").into());
    i18n.set_settings_disks_section(loc.t("settings.disks_section").into());
    i18n.set_settings_disks_desc(loc.t("settings.disks_desc").into());
    i18n.set_settings_add_custom_mount(loc.t("settings.add_custom_mount").into());
    i18n.set_settings_custom_mount_placeholder(loc.t("settings.custom_mount_placeholder").into());
    i18n.set_settings_notifications_section(loc.t("settings.notifications_section").into());
    i18n.set_settings_notify_scan(loc.t("settings.notify_scan").into());
    i18n.set_settings_notify_cleanup(loc.t("settings.notify_cleanup").into());
    i18n.set_settings_updates_section(loc.t("settings.updates_section").into());
    i18n.set_settings_check_updates(loc.t("settings.check_updates").into());
    i18n.set_settings_updates_latest(loc.t("settings.updates_latest").into());
    i18n.set_settings_updates_checking(loc.t("settings.updates_checking").into());
    i18n.set_settings_about(loc.t("settings.about").into());
    i18n.set_settings_version_label(loc.t("settings.version_label").into());
    i18n.set_settings_open_logs(loc.t("settings.open_logs").into());

    i18n.set_tooltip_open_app(loc.t("tooltip.open_app").into());
    i18n.set_tooltip_create_shortcut(loc.t("tooltip.create_shortcut").into());
    i18n.set_tooltip_uninstall_app(loc.t("tooltip.uninstall_app").into());
    i18n.set_tooltip_open_folder(loc.t("tooltip.open_folder").into());

    window.set_current_lang(state.get_language().as_str().into());
}

pub fn apply_theme(window: &AppWindow, theme_mode: ThemeMode) {
    let theme = window.global::<Theme>();
    theme.set_is_dark(theme_mode.is_dark());
    window.set_theme_mode(theme_mode.to_i32());
}
