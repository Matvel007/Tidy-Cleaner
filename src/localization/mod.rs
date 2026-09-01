pub mod loader;

pub use loader::{Language, LocalizationBundle};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub const EN_XML: &str = include_str!("../../resources/i18n/en.xml");
pub const RU_XML: &str = include_str!("../../resources/i18n/ru.xml");

#[derive(Debug, Clone)]
pub struct LocalizationService {
    current_language: Arc<RwLock<Language>>,
    bundles: Arc<HashMap<Language, LocalizationBundle>>,
}

impl Default for LocalizationService {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalizationService {
    pub fn new() -> Self {
        let mut bundles = HashMap::new();
        let en_bundle =
            LocalizationBundle::from_xml(EN_XML).expect("Failed to parse embedded en.xml");
        let ru_bundle =
            LocalizationBundle::from_xml(RU_XML).expect("Failed to parse embedded ru.xml");

        bundles.insert(Language::En, en_bundle);
        bundles.insert(Language::Ru, ru_bundle);

        Self {
            current_language: Arc::new(RwLock::new(Language::En)),
            bundles: Arc::new(bundles),
        }
    }

    pub fn set_language(&self, lang: Language) {
        if let Ok(mut current) = self.current_language.write() {
            *current = lang;
        }
    }

    pub fn current_language(&self) -> Language {
        self.current_language
            .read()
            .map(|l| *l)
            .unwrap_or(Language::En)
    }

    pub fn t(&self, key: &str) -> String {
        let lang = self.current_language();
        if let Some(bundle) = self.bundles.get(&lang) {
            if let Some(val) = bundle.get(key) {
                return val.to_string();
            }
        }
        // Fallback to EN
        if let Some(bundle) = self.bundles.get(&Language::En) {
            if let Some(val) = bundle.get(key) {
                return val.to_string();
            }
        }
        key.to_string()
    }

    #[allow(dead_code)]
    pub fn bundle_for(&self, lang: Language) -> Option<&LocalizationBundle> {
        self.bundles.get(&lang)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_localization_switching() {
        let service = LocalizationService::new();
        service.set_language(Language::En);
        assert_eq!(service.t("nav.dashboard"), "Dashboard");

        service.set_language(Language::Ru);
        assert_eq!(service.t("nav.dashboard"), "Мониторинг");
    }

    #[test]
    fn test_fallback() {
        let service = LocalizationService::new();
        assert_eq!(
            service.t("non_existent_key_12345"),
            "non_existent_key_12345"
        );
    }
}
