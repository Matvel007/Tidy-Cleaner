use tidy_cleaner::settings::models::AppSettings;
use tidy_cleaner::theme::ThemeMode;

#[test]
fn test_default_settings() {
    let settings = AppSettings::default();
    assert_eq!(settings.language, "en");
    assert_eq!(settings.theme, ThemeMode::Dark);
    assert!(!settings.autostart);
    assert!(!settings.start_minimized);
}

#[test]
fn test_settings_serialization_roundtrip() {
    let settings = AppSettings {
        language: "ru".to_string(),
        theme: ThemeMode::Light,
        autostart: true,
        start_minimized: true,
    };

    let json = serde_json::to_string(&settings).expect("Serialization failed");
    let deserialized: AppSettings = serde_json::from_str(&json).expect("Deserialization failed");

    assert_eq!(deserialized.language, "ru");
    assert_eq!(deserialized.theme, ThemeMode::Light);
    assert!(deserialized.autostart);
    assert!(deserialized.start_minimized);
}
