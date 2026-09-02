use crate::app::state::AppState;
use crate::localization::Language;
use crate::settings::autostart::AppAutostartManager;
use crate::settings::storage::SettingsStorage;
use crate::theme::ThemeMode;
use crate::AppWindow;
use slint::ComponentHandle;
use std::sync::Arc;

pub fn setup_settings_handlers(window: &AppWindow, state: Arc<AppState>) {
    let win_handle = window.as_weak();
    let st_clone = state.clone();

    // Initial load
    update_settings_ui(window, &state);

    // Theme Changed
    let win_handle_t = win_handle.clone();
    let st_t = st_clone.clone();
    window.on_settings_theme_changed(move |mode_idx| {
        let mode = ThemeMode::from_i32(mode_idx);
        st_t.set_theme(mode);
        if let Some(w) = win_handle_t.upgrade() {
            crate::app::apply_theme(&w, mode);
        }
    });

    // Language Changed
    let win_handle_l = win_handle.clone();
    let st_l = st_clone.clone();
    window.on_settings_lang_changed(move |lang_str| {
        let lang = Language::from_str_name(&lang_str);
        st_l.set_language(lang);
        if let Some(w) = win_handle_l.upgrade() {
            crate::app::update_ui_strings(&w, &st_l);
            update_settings_ui(&w, &st_l);
        }
    });

    // Autostart Toggled
    let win_handle_a = win_handle.clone();
    let st_a = st_clone.clone();
    window.on_settings_autostart_toggled(move |val| {
        if let Ok(mut s) = st_a.settings.write() {
            s.autostart = val;
            let _ = SettingsStorage::save(&s);
            let _ = AppAutostartManager::set_app_autostart(val, s.start_minimized);
        }
        if let Some(w) = win_handle_a.upgrade() {
            update_settings_ui(&w, &st_a);
        }
    });

    // Start Minimized Toggled
    let win_handle_m = win_handle.clone();
    let st_m = st_clone.clone();
    window.on_settings_start_minimized_toggled(move |val| {
        if let Ok(mut s) = st_m.settings.write() {
            s.start_minimized = val;
            let _ = SettingsStorage::save(&s);
            if s.autostart {
                let _ = AppAutostartManager::set_app_autostart(true, val);
            }
        }
        if let Some(w) = win_handle_m.upgrade() {
            update_settings_ui(&w, &st_m);
        }
    });
}

pub fn update_settings_ui(window: &AppWindow, state: &AppState) {
    if let Ok(s) = state.settings.read() {
        window.set_settings_autostart_enabled(s.autostart);
        window.set_settings_start_minimized_enabled(s.start_minimized);
    }
}
