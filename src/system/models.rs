pub use super::gpu::GpuMetrics;
pub use super::temperature::TemperatureMetrics;
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
    pub gpu: GpuMetrics,
    pub memory: MemoryMetrics,
    pub disks: Vec<DiskInfo>,
    pub temperature: TemperatureMetrics,
    pub overview: SystemOverview,
}

pub fn generate_arc_svg_path(cx: f32, cy: f32, r: f32, percent: f32) -> String {
    if percent <= 0.5 {
        return String::new();
    }
    if percent >= 99.5 {
        return format!(
            "M {:.2} {:.2} A {:.2} {:.2} 0 1 1 {:.2} {:.2}",
            cx,
            cy - r,
            r,
            r,
            cx - 0.01,
            cy - r
        );
    }

    let p = percent.clamp(0.5, 99.5);
    let x0 = cx;
    let y0 = cy - r;

    let theta = (p / 100.0) * 360.0;
    let alpha = (theta - 90.0).to_radians();

    let x1 = cx + r * alpha.cos();
    let y1 = cy + r * alpha.sin();

    let large_arc = if theta > 180.0 { 1 } else { 0 };

    format!(
        "M {:.2} {:.2} A {:.2} {:.2} 0 {} 1 {:.2} {:.2}",
        x0, y0, r, r, large_arc, x1, y1
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arc_path() {
        let path_50 = generate_arc_svg_path(50.0, 50.0, 40.0, 50.0);
        assert!(path_50.starts_with("M 50.00 10.00"));
        assert!(path_50.contains("A 40.00 40.00 0 0 1"));

        let path_75 = generate_arc_svg_path(50.0, 50.0, 40.0, 75.0);
        assert!(path_75.contains("0 1 1"));
    }
}
