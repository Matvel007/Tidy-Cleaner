pub mod analyzer;
pub mod cleaner;
pub mod models;
pub mod rules;
pub mod scanner;
pub mod service;
pub mod ui_bridge;

#[allow(unused_imports)]
pub use models::{CleanupCategory, CleanupItem, CleanupRule, RiskLevel, ScanPhase, ScanProgress};
pub use service::CleanupService;
pub use ui_bridge::setup_cleanup_handlers;
