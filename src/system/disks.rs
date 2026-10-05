use super::models::DiskInfo;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use sysinfo::Disks;

pub struct DiskCollector;

impl DiskCollector {
    pub fn collect_disks() -> Vec<DiskInfo> {
        let disks = Disks::new_with_refreshed_list();
        let results = disks
            .list()
            .iter()
            .map(|disk| {
                let total_bytes = disk.total_space();
                let available_bytes = disk.available_space().min(total_bytes);
                let used_bytes = total_bytes - available_bytes;
                DiskInfo {
                    name: disk.name().to_string_lossy().into(),
                    mount_point: disk.mount_point().to_string_lossy().into(),
                    file_system: disk.file_system().to_string_lossy().into(),
                    total_bytes,
                    used_bytes,
                    available_bytes,
                    usage_ratio: if total_bytes > 0 {
                        used_bytes as f32 / total_bytes as f32
                    } else {
                        0.0
                    },
                }
            })
            .collect();
        let mountinfo = std::fs::read_to_string("/proc/self/mountinfo").unwrap_or_default();
        let home = std::env::var_os("HOME");
        Self::select_disks(results, &mountinfo, home.as_deref().map(Path::new))
    }

    pub fn is_remote(fs: &str) -> bool {
        matches!(
            fs,
            "nfs"
                | "nfs4"
                | "cifs"
                | "smb3"
                | "ceph"
                | "9p"
                | "glusterfs"
                | "fuse.sshfs"
                | "fuse.rclone"
        )
    }

    pub fn select_disks(
        mut disks: Vec<DiskInfo>,
        mountinfo: &str,
        home: Option<&Path>,
    ) -> Vec<DiskInfo> {
        let mut identities = HashMap::new();
        for line in mountinfo.lines() {
            let Some((mount, fs)) = line.split_once(" - ") else {
                continue;
            };
            let fields: Vec<_> = mount.split_whitespace().collect();
            let fs: Vec<_> = fs.split_whitespace().collect();
            if fields.len() < 5 || fs.len() < 2 {
                continue;
            }
            let unescape = |value: &str| {
                value
                    .replace("\\040", " ")
                    .replace("\\011", "\t")
                    .replace("\\012", "\n")
                    .replace("\\134", "\\")
            };
            // Btrfs assigns different st_dev/major:minor IDs to subvolumes of one
            // filesystem. Its mount source identifies the shared storage instead.
            let identity = if fs[0] == "btrfs" {
                format!("btrfs:{}", unescape(fs[1]))
            } else {
                format!("{}:{}", fs[0], fields[2])
            };
            identities.insert(unescape(fields[4]), identity);
        }
        disks.retain(|disk| {
            !matches!(
                disk.file_system.as_str(),
                "proc"
                    | "sysfs"
                    | "tmpfs"
                    | "devtmpfs"
                    | "devpts"
                    | "cgroup"
                    | "cgroup2"
                    | "securityfs"
                    | "debugfs"
                    | "tracefs"
                    | "pstore"
                    | "mqueue"
                    | "hugetlbfs"
                    | "squashfs"
            )
        });
        let priority = |disk: &DiskInfo| {
            let mount = Path::new(&disk.mount_point);
            if Self::is_remote(&disk.file_system) {
                4
            } else if mount == Path::new("/") {
                0
            } else if home.is_some_and(|home| home.starts_with(mount)) {
                1
            } else if mount.starts_with("/boot") || mount.starts_with("/efi") {
                3
            } else {
                2
            }
        };
        disks.sort_by(|a, b| {
            priority(a)
                .cmp(&priority(b))
                .then_with(|| a.mount_point.cmp(&b.mount_point))
        });
        let mut seen = HashSet::new();
        disks.retain(|disk| {
            // Missing mount metadata is not evidence that two disks are identical.
            let key = identities
                .get(&disk.mount_point)
                .cloned()
                .unwrap_or_else(|| format!("mount:{}", disk.mount_point));
            seen.insert(key)
        });
        disks
    }
}
