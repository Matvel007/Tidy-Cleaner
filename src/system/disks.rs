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
            let mount_point = disk.mount_point().to_string_lossy().to_string();
            let file_system = disk.file_system().to_string_lossy().to_string();

            // Skip tiny pseudo-filesystems (< 500MB) or boot/efi partitions
            if total_bytes < 500 * 1024 * 1024
                || mount_point == "/boot"
                || mount_point.starts_with("/boot/")
                || mount_point.starts_with("/efi")
            {
                continue;
            }

            let usage_ratio = if total_bytes > 0 {
                used_bytes as f32 / total_bytes as f32
            } else {
                0.0
            };

            // Deduplicate subvolumes that share the exact same total and used size (e.g. BTRFS subvolumes)
            let dedup_key = (total_bytes, used_bytes);
            if seen_keys.contains(&dedup_key) {
                continue;
            }
            seen_keys.insert(dedup_key);

            let name = disk.name().to_string_lossy().to_string();

            results.push(DiskInfo {
                name,
                mount_point,
                file_system,
                total_bytes,
                used_bytes,
                available_bytes,
                usage_ratio,
            });
        }

        // Sort by total bytes descending so main storage (950 GB) is first, followed by secondary storage (20 GB)
        results.sort_by_key(|a| std::cmp::Reverse(a.total_bytes));

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
