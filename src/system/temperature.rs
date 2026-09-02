use serde::{Deserialize, Serialize};
use std::process::Command;
use sysinfo::Components;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TemperatureMetrics {
    pub cpu_temp_c: Option<f32>,
    pub gpu_temp_c: Option<f32>,
}

pub struct TemperatureCollector;

impl TemperatureCollector {
    pub fn collect() -> TemperatureMetrics {
        TemperatureMetrics {
            cpu_temp_c: Self::collect_cpu_temp(),
            gpu_temp_c: Self::collect_gpu_temp(),
        }
    }

    fn collect_cpu_temp() -> Option<f32> {
        let components = Components::new_with_refreshed_list();
        let mut max_temp = 0.0f32;
        let mut found = false;

        for c in components.list() {
            let label = c.label().to_lowercase();
            if label.contains("cpu")
                || label.contains("core")
                || label.contains("package")
                || label.contains("k10temp")
                || label.contains("tctl")
            {
                if let Some(t) = c.temperature() {
                    if t > max_temp {
                        max_temp = t;
                        found = true;
                    }
                }
            }
        }

        if found && max_temp > 0.0 {
            return Some(max_temp);
        }

        // Fallback: /sys/class/thermal/thermal_zone*/temp, preferring CPU-type zones.
        if let Ok(entries) = std::fs::read_dir("/sys/class/thermal") {
            let mut fallback: Option<f32> = None;
            for entry in entries.flatten() {
                let zone_dir = entry.path();
                let zone_type = std::fs::read_to_string(zone_dir.join("type"))
                    .unwrap_or_default()
                    .to_lowercase();
                let is_cpu_zone = zone_type.contains("x86_pkg_temp")
                    || zone_type.contains("cpu")
                    || zone_type.contains("k10temp")
                    || zone_type.contains("core");

                let temp_path = zone_dir.join("temp");
                if let Ok(content) = std::fs::read_to_string(temp_path) {
                    if let Ok(raw) = content.trim().parse::<f32>() {
                        let val = if raw > 1000.0 { raw / 1000.0 } else { raw };
                        if (15.0..115.0).contains(&val) {
                            if is_cpu_zone {
                                return Some(val);
                            }
                            if fallback.is_none() {
                                fallback = Some(val);
                            }
                        }
                    }
                }
            }
            if fallback.is_some() {
                return fallback;
            }
        }

        None
    }

    fn collect_gpu_temp() -> Option<f32> {
        // 1. Try NVIDIA-SMI
        if let Ok(output) = Command::new("nvidia-smi")
            .args([
                "--query-gpu=temperature.gpu",
                "--format=csv,noheader,nounits",
            ])
            .output()
        {
            if output.status.success() {
                let text = String::from_utf8_lossy(&output.stdout);
                if let Some(line) = text.lines().next() {
                    if let Ok(temp) = line.trim().parse::<f32>() {
                        return Some(temp);
                    }
                }
            }
        }

        // 2. Try sysfs hwmon for AMD GPU
        if let Ok(entries) = std::fs::read_dir("/sys/class/drm") {
            for entry in entries.flatten() {
                let path = entry.path().join("device/hwmon");
                if let Ok(sub) = std::fs::read_dir(path) {
                    for h in sub.flatten() {
                        let t_path = h.path().join("temp1_input");
                        if let Ok(content) = std::fs::read_to_string(t_path) {
                            if let Ok(raw) = content.trim().parse::<f32>() {
                                return Some(if raw > 1000.0 { raw / 1000.0 } else { raw });
                            }
                        }
                    }
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_temperature_collector() {
        let t = TemperatureCollector::collect();
        if let Some(cpu) = t.cpu_temp_c {
            assert!((0.0..=120.0).contains(&cpu));
        }
        if let Some(gpu) = t.gpu_temp_c {
            assert!((0.0..=120.0).contains(&gpu));
        }
    }
}
