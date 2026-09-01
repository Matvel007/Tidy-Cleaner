use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuMetrics {
    pub usage_percent: f32,
    pub core_count: usize,
    pub frequency_mhz: u64,
    pub brand_name: String,
    pub history: Vec<f32>,
}

impl Default for CpuMetrics {
    fn default() -> Self {
        Self {
            usage_percent: 0.0,
            core_count: 1,
            frequency_mhz: 0,
            brand_name: "Generic CPU".to_string(),
            history: vec![0.0; 24],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryMetrics {
    pub used_bytes: u64,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub usage_percent: f32,
    pub history: Vec<f32>,
}

impl Default for MemoryMetrics {
    fn default() -> Self {
        Self {
            used_bytes: 0,
            total_bytes: 0,
            available_bytes: 0,
            usage_percent: 0.0,
            history: vec![0.0; 24],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskInfo {
    pub name: String,
    pub mount_point: String,
    pub file_system: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
    pub usage_ratio: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SystemOverview {
    pub os_name: String,
    pub kernel_version: String,
    pub arch: String,
    pub hostname: String,
    pub uptime_seconds: u64,
    pub uptime_formatted: String,
}

#[derive(Debug, Clone, Default)]
pub struct SystemSnapshot {
    pub cpu: CpuMetrics,
    pub memory: MemoryMetrics,
    pub disks: Vec<DiskInfo>,
    pub overview: SystemOverview,
}
