use crate::app::state::AppState;
use crate::localization::Language;
use crate::theme::ThemeMode;
use crate::AppWindow;
use slint::ComponentHandle;
use std::sync::Arc;

enum SettingsChange {
    Reconcile,
    Theme(ThemeMode),
    Language(Language),
    Autostart(bool),
    Minimized(bool),
}

pub fn setup_settings_handlers(window: &AppWindow, state: Arc<AppState>) {
    update_settings_ui(window, &state);
    // Enqueue on the UI thread, then consume in order, including initial reconciliation.
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let win_handle = window.as_weak();
    let worker_state = state.clone();
    tokio::spawn(async move {
        while let Some(change) = rx.recv().await {
            let st = worker_state.clone();
            let result = tokio::task::spawn_blocking(move || match change {
                SettingsChange::Reconcile => st.reconcile_autostart(),
                SettingsChange::Theme(mode) => st.set_theme(mode),
                SettingsChange::Language(lang) => st.set_language(lang),
                SettingsChange::Autostart(enabled) => st.set_autostart(enabled),
                SettingsChange::Minimized(minimized) => st.set_start_minimized(minimized),
            })
            .await;
            let error = match result {
                Ok(Ok(())) => String::new(),
                Ok(Err(e)) => format!("{:#}", e),
                Err(e) => e.to_string(),
            };
            if !error.is_empty() {
                tracing::error!("Settings operation failed: {}", error);
            }
            let st = worker_state.clone();
            let resolved_theme = match tokio::task::spawn_blocking(move || {
                if st.get_theme().is_dark() {
                    ThemeMode::Dark
                } else {
                    ThemeMode::Light
                }
            })
            .await
            {
                Ok(mode) => mode,
                Err(e) => {
                    tracing::error!("Theme worker failed: {}", e);
                    ThemeMode::Dark
                }
            };
            let win = win_handle.clone();
            let st = worker_state.clone();
            if let Err(e) = slint::invoke_from_event_loop(move || {
                if let Some(w) = win.upgrade() {
                    crate::app::apply_theme(&w, resolved_theme);
                    crate::app::update_ui_strings(&w, &st);
                    update_settings_ui(&w, &st);
                }
            }) {
                tracing::warn!("Failed to publish settings result: {}", e);
            }
        }
    });
    let send = move |change| {
        if let Err(e) = tx.send(change) {
            tracing::error!("Settings worker stopped: {}", e);
        }
    };
    send(SettingsChange::Reconcile);
    let send = Arc::new(send);
    let sender = send.clone();
    window.on_settings_theme_changed(move |index| {
        sender(SettingsChange::Theme(ThemeMode::from_i32(index)))
    });
    let sender = send.clone();
    window.on_settings_lang_changed(move |lang| {
        sender(SettingsChange::Language(Language::from_str_name(&lang)))
    });
    let sender = send.clone();
    window.on_settings_autostart_toggled(move |enabled| sender(SettingsChange::Autostart(enabled)));
    window.on_settings_start_minimized_toggled(move |minimized| {
        send(SettingsChange::Minimized(minimized))
    });
}

pub fn update_settings_ui(window: &AppWindow, state: &AppState) {
    if let Ok(s) = state.settings.read() {
        window.set_settings_autostart_enabled(s.autostart);
        window.set_settings_start_minimized_enabled(s.start_minimized);
    }
}
