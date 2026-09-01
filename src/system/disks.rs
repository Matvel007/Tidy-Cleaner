use super::models::DiskInfo;
use sysinfo::Disks;

pub struct DiskCollector;

impl DiskCollector {
    pub fn collect_disks() -> Vec<DiskInfo> {
        let disks = Disks::new_with_refreshed_list();
        let mut results = Vec::new();

        for disk in disks.list() {
            let total_bytes = disk.total_space();
            let available_bytes = disk.available_space();
            let used_bytes = total_bytes.saturating_sub(available_bytes);

            let usage_ratio = if total_bytes > 0 {
                used_bytes as f32 / total_bytes as f32
            } else {
                0.0
            };

            let mount_point = disk.mount_point().to_string_lossy().to_string();
            let name = disk.name().to_string_lossy().to_string();
            let file_system = disk.file_system().to_string_lossy().to_string();

            // Ignore tiny virtual or read-only pseudo mounts if total_space is 0
            if total_bytes > 0 {
                results.push(DiskInfo {
                    name: if name.is_empty() {
                        "Disk".to_string()
                    } else {
                        name
                    },
                    mount_point,
                    file_system,
                    total_bytes,
                    used_bytes,
                    available_bytes,
                    usage_ratio,
                });
            }
        }

        // Sort by mount point length so root / comes first
        results.sort_by_key(|a| a.mount_point.len());
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_disks() {
        let disks = DiskCollector::collect_disks();
        // On any Linux system with mounted root, at least one disk should be discovered
        assert!(!disks.is_empty() || cfg!(not(target_os = "linux")));
    }
}
