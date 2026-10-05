use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GpuMetrics {
    pub usage_percent: f32,
    pub usage_available: bool,
    pub name: String,
    pub used_memory_mb: u64,
    pub total_memory_mb: u64,
    pub memory_available: bool,
    pub temperature_c: Option<f32>,
}

pub struct GpuCollector;

impl GpuCollector {
    pub fn collect() -> GpuMetrics {
        // Explicit single-GPU policy: first NVIDIA device, otherwise first DRM card.
        if let Ok(output) = crate::process::output(
            Command::new("nvidia-smi").args([
                "--query-gpu=utilization.gpu,name,memory.used,memory.total,temperature.gpu",
                "--format=csv,noheader,nounits",
            ]),
            Duration::from_millis(750),
        ) {
            if output.status.success() {
                if let Some(metrics) = Self::parse_nvidia(&String::from_utf8_lossy(&output.stdout))
                {
                    return metrics;
                }
            }
        }
        Self::collect_from(Path::new("/sys/class/drm"))
    }

    pub fn parse_nvidia(text: &str) -> Option<GpuMetrics> {
        let parts: Vec<_> = text.lines().next()?.split(',').map(str::trim).collect();
        if parts.len() != 5 || parts[1].is_empty() {
            return None;
        }
        let usage = parts[0]
            .parse::<f32>()
            .ok()
            .filter(|v| (0.0..=100.0).contains(v));
        let used = parts[2].parse::<u64>().ok();
        let total = parts[3].parse::<u64>().ok();
        Some(GpuMetrics {
            usage_percent: usage.unwrap_or_default(),
            usage_available: usage.is_some(),
            name: parts[1].to_string(),
            used_memory_mb: used.unwrap_or_default(),
            total_memory_mb: total.unwrap_or_default(),
            memory_available: matches!((used, total), (Some(u), Some(t)) if t > 0 && u <= t),
            temperature_c: parts[4]
                .parse::<f32>()
                .ok()
                .filter(|v| (-40.0..=150.0).contains(v)),
        })
    }

    pub fn collect_from(root: &Path) -> GpuMetrics {
        let Ok(entries) = std::fs::read_dir(root) else {
            return GpuMetrics::default();
        };
        let mut cards: Vec<_> = entries
            .flatten()
            .filter(|entry| {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                name.strip_prefix("card").is_some_and(|suffix| {
                    !suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_digit())
                })
            })
            .collect();
        cards.sort_by_key(|entry| {
            entry.file_name().to_string_lossy()[4..]
                .parse::<u32>()
                .unwrap_or(u32::MAX)
        });
        for card in cards {
            let device = card.path().join("device");
            let vendor = std::fs::read_to_string(device.join("vendor")).unwrap_or_default();
            let name = match vendor.trim() {
                "0x1002" => "AMD GPU",
                "0x8086" => "Intel GPU",
                "0x10de" => "NVIDIA GPU",
                _ => continue,
            };
            let mut metrics = GpuMetrics {
                name: name.into(),
                ..GpuMetrics::default()
            };
            // AMD exposes a documented instantaneous utilization counter. Intel does not:
            // fdinfo is per-client engine time, not a global instantaneous percentage.
            if vendor.trim() == "0x1002" {
                let usage = std::fs::read_to_string(device.join("gpu_busy_percent"))
                    .ok()
                    .and_then(|s| s.trim().parse::<f32>().ok())
                    .filter(|v| (0.0..=100.0).contains(v));
                metrics.usage_percent = usage.unwrap_or_default();
                metrics.usage_available = usage.is_some();
                let bytes = |file: &str| {
                    std::fs::read_to_string(device.join(file))
                        .ok()
                        .and_then(|s| s.trim().parse::<u64>().ok())
                };
                let used = bytes("mem_info_vram_used");
                let total = bytes("mem_info_vram_total");
                metrics.used_memory_mb = used.unwrap_or_default() / (1024 * 1024);
                metrics.total_memory_mb = total.unwrap_or_default() / (1024 * 1024);
                metrics.memory_available =
                    matches!((used, total), (Some(u), Some(t)) if t > 0 && u <= t);
            }
            metrics.temperature_c =
                super::temperature::TemperatureCollector::read_gpu_hwmon(&device);
            return metrics;
        }
        GpuMetrics::default()
    }
}
