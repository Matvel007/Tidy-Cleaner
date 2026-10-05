use serde::{Deserialize, Serialize};
use std::path::Path;
use sysinfo::Components;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TemperatureMetrics {
    pub cpu_temp_c: Option<f32>,
    pub gpu_temp_c: Option<f32>,
}

pub struct TemperatureCollector;

impl TemperatureCollector {
    pub fn collect_cpu_temp() -> Option<f32> {
        let components = Components::new_with_refreshed_list();
        let mut max_temp: Option<f32> = None;

        for c in components.list() {
            let label = c.label().to_lowercase();
            if label.contains("cpu")
                || label.contains("core")
                || label.contains("package")
                || label.contains("k10temp")
                || label.contains("tctl")
            {
                if let Some(t) = c.temperature() {
                    if (-40.0..=150.0).contains(&t) {
                        max_temp = Some(max_temp.map_or(t, |previous| previous.max(t)));
                    }
                }
            }
        }

        if max_temp.is_some() {
            return max_temp;
        }

        Self::read_cpu_thermal(Path::new("/sys/class/thermal"))
    }

    pub fn read_cpu_thermal(root: &Path) -> Option<f32> {
        // Only CPU-type zones qualify; an ACPI/board sensor is not CPU temperature.
        if let Ok(entries) = std::fs::read_dir(root) {
            let mut max_temp: Option<f32> = None;
            for entry in entries.flatten() {
                let zone_dir = entry.path();
                let zone_type = std::fs::read_to_string(zone_dir.join("type"))
                    .unwrap_or_default()
                    .to_lowercase();
                let is_cpu_zone = zone_type.contains("x86_pkg_temp")
                    || zone_type.contains("cpu")
                    || zone_type.contains("k10temp")
                    || zone_type.contains("core");

                if !is_cpu_zone {
                    continue;
                }
                let temp_path = zone_dir.join("temp");
                if let Ok(content) = std::fs::read_to_string(temp_path) {
                    if let Ok(raw) = content.trim().parse::<f32>() {
                        let val = raw / 1000.0;
                        if (-40.0..=150.0).contains(&val) {
                            max_temp = Some(max_temp.map_or(val, |previous| previous.max(val)));
                        }
                    }
                }
            }
            return max_temp;
        }

        None
    }

    pub fn read_gpu_hwmon(device: &Path) -> Option<f32> {
        let mut entries: Vec<_> = std::fs::read_dir(device.join("hwmon"))
            .ok()?
            .flatten()
            .collect();
        entries.sort_by_key(|entry| entry.file_name());
        entries.into_iter().find_map(|entry| {
            std::fs::read_to_string(entry.path().join("temp1_input"))
                .ok()
                .and_then(|s| s.trim().parse::<f32>().ok())
                .map(|raw| raw / 1000.0)
                .filter(|v| (-40.0..=150.0).contains(v))
        })
    }
}
