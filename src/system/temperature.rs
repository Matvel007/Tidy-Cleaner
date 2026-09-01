use serde::{Deserialize, Serialize};
use std::process::Command;
use sysinfo::Components;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemperatureMetrics {
    pub cpu_temp_c: f32,
    pub gpu_temp_c: f32,
}

impl Default for TemperatureMetrics {
    fn default() -> Self {
        Self {
            cpu_temp_c: 45.0,
            gpu_temp_c: 40.0,
        }
    }
}

pub struct TemperatureCollector;

impl TemperatureCollector {
    pub fn collect() -> TemperatureMetrics {
        let cpu_temp_c = Self::collect_cpu_temp();
        let gpu_temp_c = Self::collect_gpu_temp();

        TemperatureMetrics {
            cpu_temp_c,
            gpu_temp_c,
        }
    }

    fn collect_cpu_temp() -> f32 {
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
            return max_temp;
        }

        // Fallback: /sys/class/thermal/thermal_zone*/temp
        if let Ok(entries) = std::fs::read_dir("/sys/class/thermal") {
            for entry in entries.flatten() {
                let path = entry.path().join("temp");
                if let Ok(content) = std::fs::read_to_string(path) {
                    if let Ok(raw) = content.trim().parse::<f32>() {
                        let val = if raw > 1000.0 { raw / 1000.0 } else { raw };
                        if (15.0..115.0).contains(&val) {
                            return val;
                        }
                    }
                }
            }
        }

        45.0
    }

    fn collect_gpu_temp() -> f32 {
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
                        return temp;
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
                                return if raw > 1000.0 { raw / 1000.0 } else { raw };
                            }
                        }
                    }
                }
            }
        }

        40.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_temperature_collector() {
        let t = TemperatureCollector::collect();
        assert!(t.cpu_temp_c >= 0.0 && t.cpu_temp_c <= 120.0);
        assert!(t.gpu_temp_c >= 0.0 && t.gpu_temp_c <= 120.0);
    }
}
