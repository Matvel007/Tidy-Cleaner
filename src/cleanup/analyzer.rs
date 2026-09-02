use crate::cleanup::models::{CleanupCategory, CleanupItem, RiskLevel};
use crate::cleanup::scanner::Scanner;

pub struct Analyzer;

impl Analyzer {
    pub fn calculate_selected_summary(items: &[CleanupItem]) -> (usize, u64, String) {
        let selected: Vec<&CleanupItem> = items.iter().filter(|i| i.selected).collect();
        let count = selected.len();
        let total_bytes: u64 = selected.iter().map(|i| i.size_bytes).sum();
        let formatted = Scanner::format_bytes(total_bytes);
        (count, total_bytes, formatted)
    }

    /// Selects only `Safe` items when `selected` is true (Warning/Dangerous
    /// require explicit per-item confirmation). Deselecting always clears all.
    pub fn set_all_selected(items: &mut [CleanupItem], selected: bool) {
        for item in items.iter_mut() {
            if !selected || item.safety_level == RiskLevel::Safe {
                item.selected = selected;
            }
        }
    }

    pub fn toggle_item_selected(items: &mut [CleanupItem], item_id: &str) {
        if let Some(item) = items.iter_mut().find(|i| i.id == item_id) {
            item.selected = !item.selected;
        }
    }

    #[allow(dead_code)]
    pub fn filter_items(
        items: &[CleanupItem],
        category: Option<CleanupCategory>,
        query: &str,
    ) -> Vec<CleanupItem> {
        items
            .iter()
            .filter(|item| {
                let match_category = match category {
                    Some(cat) => item.category == cat,
                    None => true,
                };

                let match_query = if query.is_empty() {
                    true
                } else {
                    let q = query.to_lowercase();
                    item.name.to_lowercase().contains(&q)
                        || item.path.to_string_lossy().to_lowercase().contains(&q)
                };

                match_category && match_query
            })
            .cloned()
            .collect()
    }
}
