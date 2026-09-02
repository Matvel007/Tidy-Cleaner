use crate::startup::desktop::DesktopAutostart;
use crate::startup::models::{CreateStartupRequest, StartupItem};
use crate::startup::scanner::StartupScanner;
use anyhow::{bail, Result};
use std::path::PathBuf;

#[derive(Clone, Default)]
pub struct StartupManager;

impl StartupManager {
    pub fn new() -> Self {
        Self
    }

    pub fn list_items(&self) -> Vec<StartupItem> {
        StartupScanner::scan_all()
    }

    pub fn add_item(&self, req: &CreateStartupRequest) -> Result<PathBuf> {
        DesktopAutostart::write_user_autostart_entry(req)
    }

    pub fn toggle_item(&self, item_id: &str, enable: bool) -> Result<()> {
        let items = self.list_items();
        if let Some(item) = items.iter().find(|i| i.id == item_id) {
            DesktopAutostart::toggle_entry(item, enable)?;
            Ok(())
        } else {
            bail!("Startup item {} not found", item_id);
        }
    }

    pub fn remove_item(&self, item_id: &str) -> Result<()> {
        let items = self.list_items();
        if let Some(item) = items.iter().find(|i| i.id == item_id) {
            DesktopAutostart::remove_entry(item)?;
            Ok(())
        } else {
            bail!("Startup item {} not found", item_id);
        }
    }
}
