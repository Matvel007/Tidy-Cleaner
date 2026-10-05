use super::models::{BatteryMetrics, BatteryStatus};
use std::fs;
use std::path::Path;

pub struct BatteryCollector;

impl BatteryCollector {
    pub fn collect() -> BatteryMetrics {
        Self::collect_from(Path::new("/sys/class/power_supply"))
    }

    pub fn collect_from(p: &Path) -> BatteryMetrics {
        if !p.exists() {
            return BatteryMetrics::default();
        }

        if let Ok(entries) = fs::read_dir(p) {
            let mut entries: Vec<_> = entries.flatten().collect();
            entries.sort_by_key(|entry| entry.file_name());
            for entry in entries {
                if fs::read_to_string(entry.path().join("type"))
                    .is_ok_and(|kind| kind.trim() == "Battery")
                    && !fs::read_to_string(entry.path().join("present"))
                        .is_ok_and(|present| present.trim() == "0")
                {
                    let bat_path = entry.path();

                    let capacity: f32 = fs::read_to_string(bat_path.join("capacity"))
                        .ok()
                        .and_then(|s| s.trim().parse().ok())
                        .unwrap_or(0.0);

                    let status = fs::read_to_string(bat_path.join("status"))
                        .ok()
                        .map(|s| BatteryStatus::from_sysfs(&s))
                        .unwrap_or_default();

                    // Power in Watts (micro-watts to watts)
                    let power_u_w: Option<f32> = fs::read_to_string(bat_path.join("power_now"))
                        .ok()
                        .and_then(|s| s.trim().parse().ok());

                    let watts = if let Some(uw) = power_u_w {
                        uw / 1_000_000.0
                    } else {
                        // Fallback: current_now * voltage_now
                        let current: Option<f32> = fs::read_to_string(bat_path.join("current_now"))
                            .ok()
                            .and_then(|s| s.trim().parse().ok());
                        let voltage: Option<f32> = fs::read_to_string(bat_path.join("voltage_now"))
                            .ok()
                            .and_then(|s| s.trim().parse().ok());
                        if let (Some(c), Some(v)) = (current, voltage) {
                            (c * v) / 1_000_000_000_000.0
                        } else {
                            0.0
                        }
                    };

                    // Health calculation
                    let full_energy = fs::read_to_string(bat_path.join("energy_full"))
                        .or_else(|_| fs::read_to_string(bat_path.join("charge_full")))
                        .ok()
                        .and_then(|s| s.trim().parse::<f32>().ok());
                    let design_energy = fs::read_to_string(bat_path.join("energy_full_design"))
                        .or_else(|_| fs::read_to_string(bat_path.join("charge_full_design")))
                        .ok()
                        .and_then(|s| s.trim().parse::<f32>().ok());

                    let health_percent = match (full_energy, design_energy) {
                        (Some(full), Some(design)) if design > 0.0 => {
                            (full / design * 100.0).clamp(0.0, 100.0)
                        }
                        _ => 100.0,
                    };

                    return BatteryMetrics {
                        has_battery: true,
                        charge_percent: capacity.clamp(0.0, 100.0),
                        status,
                        energy_watts: watts,
                        health_percent,
                    };
                }
            }
        }

        BatteryMetrics::default()
    }
}
