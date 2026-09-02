use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    Safe,
    Warning,
    Dangerous,
}

#[allow(dead_code)]
impl RiskLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            RiskLevel::Safe => "Safe",
            RiskLevel::Warning => "Warning",
            RiskLevel::Dangerous => "Dangerous",
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CleanupCategory {
    ApplicationCache,
    Thumbnails,
    BrowserCache,
    PackageCache,
    BuildCache,
    CrashReports,
    Trash,
    Downloads,
    SystemLogs,
    Leftovers,
}

#[allow(dead_code)]
impl CleanupCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            CleanupCategory::ApplicationCache => "App Cache",
            CleanupCategory::Thumbnails => "Thumbnails",
            CleanupCategory::BrowserCache => "Browser Cache",
            CleanupCategory::PackageCache => "Package Cache",
            CleanupCategory::BuildCache => "Build Cache",
            CleanupCategory::CrashReports => "Crash Dumps",
            CleanupCategory::Trash => "Trash",
            CleanupCategory::Downloads => "Downloads",
            CleanupCategory::SystemLogs => "System Logs",
            CleanupCategory::Leftovers => "Leftovers",
        }
    }

    pub fn icon_name(&self) -> &'static str {
        match self {
            CleanupCategory::ApplicationCache => "cache",
            CleanupCategory::Thumbnails => "thumbnails",
            CleanupCategory::BrowserCache => "browser",
            CleanupCategory::PackageCache => "package",
            CleanupCategory::BuildCache => "build",
            CleanupCategory::CrashReports => "crash",
            CleanupCategory::Trash => "trash",
            CleanupCategory::Downloads => "downloads",
            CleanupCategory::SystemLogs => "logs",
            CleanupCategory::Leftovers => "leftovers",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CleanupRule {
    pub id: String,
    pub name_key: String,
    pub description_key: String,
    pub category: CleanupCategory,
    pub base_path: PathBuf,
    pub is_deep_scan: bool,
    pub safety_level: RiskLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanupItem {
    pub id: String,
    pub rule_id: String,
    pub name: String,
    pub description: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub size_formatted: String,
    pub safety_level: RiskLevel,
    pub category: CleanupCategory,
    pub selected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanPhase {
    Idle,
    Scanning,
    Completed,
    Cleaning,
    Cancelled,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ScanProgress {
    pub phase: ScanPhase,
    pub current_item: String,
    pub items_found: usize,
    pub bytes_found: u64,
    pub percent: f32,
}

impl Default for ScanProgress {
    fn default() -> Self {
        Self {
            phase: ScanPhase::Idle,
            current_item: String::new(),
            items_found: 0,
            bytes_found: 0,
            percent: 0.0,
        }
    }
}
