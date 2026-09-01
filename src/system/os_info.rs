use super::models::SystemOverview;
use sysinfo::System;

pub struct OsInfoCollector;

impl OsInfoCollector {
    pub fn collect_overview(sys: &System) -> SystemOverview {
        let os_name = System::name().unwrap_or_else(|| "Linux".to_string());
        let os_version = System::os_version().unwrap_or_default();
        let full_os = if os_version.is_empty() {
            os_name
        } else {
            format!("{} {}", os_name, os_version)
        };

        let kernel_version = System::kernel_version().unwrap_or_else(|| "Unknown".to_string());
        let arch = System::cpu_arch();
        let hostname = System::host_name().unwrap_or_else(|| "localhost".to_string());
        let uptime_seconds = System::uptime();
        let uptime_formatted = Self::format_uptime(uptime_seconds);

        let _ = sys; // Used for context

        SystemOverview {
            os_name: full_os,
            kernel_version,
            arch,
            hostname,
            uptime_seconds,
            uptime_formatted,
        }
    }

    pub fn format_uptime(seconds: u64) -> String {
        let days = seconds / 86400;
        let hours = (seconds % 86400) / 3600;
        let minutes = (seconds % 3600) / 60;
        let secs = seconds % 60;

        if days > 0 {
            format!("{}d {}h {}m {}s", days, hours, minutes, secs)
        } else if hours > 0 {
            format!("{}h {}m {}s", hours, minutes, secs)
        } else if minutes > 0 {
            format!("{}m {}s", minutes, secs)
        } else {
            format!("{}s", secs)
        }
    }

    pub fn format_bytes(bytes: u64) -> String {
        const KB: f64 = 1024.0;
        const MB: f64 = KB * 1024.0;
        const GB: f64 = MB * 1024.0;
        const TB: f64 = GB * 1024.0;

        let b = bytes as f64;
        if b >= TB {
            format!("{:.2} TB", b / TB)
        } else if b >= GB {
            format!("{:.2} GB", b / GB)
        } else if b >= MB {
            format!("{:.1} MB", b / MB)
        } else if b >= KB {
            format!("{:.0} KB", b / KB)
        } else {
            format!("{} B", bytes)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uptime_formatting() {
        assert_eq!(OsInfoCollector::format_uptime(45), "45s");
        assert_eq!(OsInfoCollector::format_uptime(125), "2m 5s");
        assert_eq!(OsInfoCollector::format_uptime(3665), "1h 1m 5s");
        assert_eq!(OsInfoCollector::format_uptime(90061), "1d 1h 1m 1s");
    }

    #[test]
    fn test_bytes_formatting() {
        assert_eq!(OsInfoCollector::format_bytes(500), "500 B");
        assert_eq!(OsInfoCollector::format_bytes(1024 * 500), "500 KB");
        assert_eq!(
            OsInfoCollector::format_bytes(1024 * 1024 * 1024 * 2),
            "2.00 GB"
        );
    }
}
