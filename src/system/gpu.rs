use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuMetrics {
    pub usage_percent: f32,
    pub name: String,
    pub used_memory_mb: u64,
    pub total_memory_mb: u64,
}

impl Default for GpuMetrics {
    fn default() -> Self {
        Self {
            usage_percent: 0.0,
            name: String::new(),
            used_memory_mb: 0,
            total_memory_mb: 0,
        }
    }
}

pub struct GpuCollector;

impl GpuCollector {
    pub fn collect() -> GpuMetrics {
        // 1. Try NVIDIA-SMI first
        if let Ok(output) = Command::new("nvidia-smi")
            .args([
                "--query-gpu=utilization.gpu,name,memory.used,memory.total",
                "--format=csv,noheader,nounits",
            ])
            .output()
        {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                if let Some(line) = text.lines().next() {
                    let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
                    if parts.len() >= 4 {
                        let usage_percent: f32 = parts[0].parse().unwrap_or(0.0);
                        let name = parts[1].to_string();
                        let used_memory_mb: u64 = parts[2].parse().unwrap_or(0);
                        let total_memory_mb: u64 = parts[3].parse().unwrap_or(0);

                        return GpuMetrics {
                            usage_percent,
                            name,
                            used_memory_mb,
                            total_memory_mb,
                        };
                    }
                }
            }
        }

        // 2. Try sysfs for AMD GPU (/sys/class/drm/card*/device/gpu_busy_percent)
        if let Ok(entries) = std::fs::read_dir("/sys/class/drm") {
            for entry in entries.flatten() {
                let path = entry.path().join("device/gpu_busy_percent");
                if path.exists() {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if let Ok(val) = content.trim().parse::<f32>() {
                            return GpuMetrics {
                                usage_percent: val,
                                name: "AMD GPU".to_string(),
                                used_memory_mb: 0,
                                total_memory_mb: 0,
                            };
                        }
                    }
                }
            }
        }

        // 3. Try sysfs for Intel iGPU (/sys/class/drm/card*/gt_busy_percent)
        if let Ok(entries) = std::fs::read_dir("/sys/class/drm") {
            for entry in entries.flatten() {
                let path = entry.path().join("gt_busy_percent");
                if path.exists() {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if let Ok(val) = content.trim().parse::<f32>() {
                            return GpuMetrics {
                                usage_percent: val,
                                name: "Intel GPU".to_string(),
                                used_memory_mb: 0,
                                total_memory_mb: 0,
                            };
                        }
                    }
                }
            }
        }

        // Fallback default (no GPU detected)
        GpuMetrics::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_collect() {
        let metrics = GpuCollector::collect();
        assert!(metrics.usage_percent >= 0.0 && metrics.usage_percent <= 100.0);
    }
}
