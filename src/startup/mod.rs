pub mod desktop;
pub mod manager;
pub mod models;
pub mod scanner;
pub mod service;
pub mod ui_bridge;

#[allow(unused_imports)]
pub use models::{CreateStartupRequest, StartupItem, StartupSource};
pub use service::StartupService;
pub use ui_bridge::setup_startup_handlers;
