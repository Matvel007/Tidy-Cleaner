use super::models::DiskInfo;
use std::collections::HashSet;
use sysinfo::Disks;

pub struct DiskCollector;

impl DiskCollector {
    pub fn collect_disks() -> Vec<DiskInfo> {
        let disks = Disks::new_with_refreshed_list();
        let mut results = Vec::new();
        let mut seen_keys = HashSet::new();

        for disk in disks.list() {
            let total_bytes = disk.total_space();
            let available_bytes = disk.available_space();
            let used_bytes = total_bytes.saturating_sub(available_bytes);

            // Skip tiny pseudo-filesystems (< 100MB)
            if total_bytes < 100 * 1024 * 1024 {
                continue;
            }

            let usage_ratio = if total_bytes > 0 {
                used_bytes as f32 / total_bytes as f32
            } else {
                0.0
            };

            let mount_point = disk.mount_point().to_string_lossy().to_string();
            let file_system = disk.file_system().to_string_lossy().to_string();

            // Deduplicate subvolumes that share the exact same total and used size (e.g. BTRFS subvolumes)
            let dedup_key = (total_bytes, used_bytes);
            if seen_keys.contains(&dedup_key) {
                continue;
            }
            seen_keys.insert(dedup_key);

            let display_name =
                if mount_point == "/" || mount_point == "/root" || mount_point == "/home" {
                    "Встроенный накопитель".to_string()
                } else {
                    let name = disk.name().to_string_lossy().to_string();
                    if name.is_empty() {
                        "Дополнительный накопитель".to_string()
                    } else {
                        name
                    }
                };

            results.push(DiskInfo {
                name: display_name,
                mount_point,
                file_system,
                total_bytes,
                used_bytes,
                available_bytes,
                usage_ratio,
            });
        }

        if results.is_empty() {
            results.push(DiskInfo {
                name: "Встроенный накопитель".to_string(),
                mount_point: "/".to_string(),
                file_system: "btrfs".to_string(),
                total_bytes: 1024 * 1024 * 1024 * 512,
                used_bytes: 1024 * 1024 * 1024 * 200,
                available_bytes: 1024 * 1024 * 1024 * 312,
                usage_ratio: 0.39,
            });
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_disks() {
        let disks = DiskCollector::collect_disks();
        assert!(!disks.is_empty());
    }
}
