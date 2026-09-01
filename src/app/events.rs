use crate::localization::Language;
use crate::theme::ThemeMode;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum AppEvent {
    Navigate(i32),
    ChangeTheme(ThemeMode),
    ChangeLanguage(Language),
    RefreshData,
    OpenLogs,
}
