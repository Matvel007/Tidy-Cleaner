//! Deterministic telemetry fixtures. These integration tests run once; unit tests
//! inside src/system are compiled into both the library and the binary targets.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tidy_cleaner::system::battery::BatteryCollector;
use tidy_cleaner::system::disks::DiskCollector;
use tidy_cleaner::system::gpu::GpuCollector;
use tidy_cleaner::system::models::{BatteryStatus, DiskInfo};
use tidy_cleaner::system::network::{InterfaceCounters, NetworkCollector};
use tidy_cleaner::system::temperature::TemperatureCollector;

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "tidy-telemetry-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn write(&self, name: &str, value: &str) {
        let path = self.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, value).unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

fn disk(mount: &str, fs: &str) -> DiskInfo {
    DiskInfo {
        name: "/dev/test".into(),
        mount_point: mount.into(),
        file_system: fs.into(),
        total_bytes: 100,
        used_bytes: 40,
        available_bytes: 60,
        usage_ratio: 0.4,
    }
}

#[test]
fn equal_accounting_does_not_merge_distinct_filesystems() {
    let mounts = "1 0 8:1 / / rw - ext4 /dev/sda1 rw\n2 0 8:2 / /home rw - ext4 /dev/sda2 rw";
    let disks = DiskCollector::select_disks(
        vec![disk("/home", "ext4"), disk("/", "ext4")],
        mounts,
        Some(Path::new("/home/user")),
    );
    assert_eq!(disks.len(), 2);
    assert_eq!(disks[0].mount_point, "/");
    assert_eq!(disks[1].mount_point, "/home");
}

#[test]
fn btrfs_subvolumes_share_storage_despite_distinct_device_ids() {
    let mounts =
        "1 0 0:1 /@ / rw - btrfs /dev/sda1 rw\n2 0 0:2 /@home /home rw - btrfs /dev/sda1 rw";
    let disks = DiskCollector::select_disks(
        vec![disk("/home", "btrfs"), disk("/", "btrfs")],
        mounts,
        Some(Path::new("/home/user")),
    );
    assert_eq!(disks.len(), 1);
    assert_eq!(disks[0].mount_point, "/");
}

#[test]
fn bind_mounts_deduplicate_and_escaped_mount_paths_work() {
    let mounts =
        "1 0 8:1 / / rw - ext4 /dev/sda1 rw\n2 0 8:1 /data /mnt/my\\040disk rw - ext4 /dev/sda1 rw";
    assert_eq!(
        DiskCollector::select_disks(
            vec![disk("/mnt/my disk", "ext4"), disk("/", "ext4")],
            mounts,
            None
        )
        .len(),
        1
    );
}

#[test]
fn disks_keep_small_and_boot_storage_deprioritize_remote_exclude_pseudo() {
    let disks = DiskCollector::select_disks(
        vec![
            disk("/remote", "nfs"),
            disk("/boot", "vfat"),
            disk("/tmp", "tmpfs"),
            disk("/data", "ext4"),
            disk("/home", "ext4"),
            disk("/", "ext4"),
        ],
        "",
        Some(Path::new("/home/user")),
    );
    let mounts: Vec<_> = disks.iter().map(|disk| disk.mount_point.as_str()).collect();
    assert_eq!(mounts, ["/", "/home", "/data", "/boot", "/remote"]);
}

fn interface(name: &str, identity: u32, physical: bool, rx: u64, tx: u64) -> InterfaceCounters {
    InterfaceCounters {
        name: name.into(),
        identity,
        physical,
        rx,
        tx,
    }
}

#[test]
fn network_baseline_and_loopback_are_consistent() {
    let mut collector = NetworkCollector::new();
    let first = collector.sample_counters(
        &[
            interface("eth0", 1, true, 100, 200),
            interface("lo", 2, false, 10_000, 10_000),
        ],
        Duration::from_secs(1),
    );
    assert_eq!(first.rx_bytes_per_sec, 0);
    assert_eq!(first.total_rx_bytes, 100);
    let next = collector.sample_counters(
        &[
            interface("eth0", 1, true, 300, 300),
            interface("lo", 2, false, 90_000, 90_000),
        ],
        Duration::from_secs(2),
    );
    assert_eq!(next.rx_bytes_per_sec, 100);
    assert_eq!(next.tx_bytes_per_sec, 50);
    assert_eq!(next.active_interface, "eth0");
}

#[test]
fn network_hotplug_and_counter_resets_do_not_hide_other_traffic() {
    let mut collector = NetworkCollector::new();
    collector.sample_counters(
        &[
            interface("eth0", 1, true, 1_000, 1_000),
            interface("eth1", 2, true, 2_000, 2_000),
        ],
        Duration::from_secs(1),
    );
    let next = collector.sample_counters(
        &[
            interface("eth0", 1, true, 10, 20),
            interface("eth1", 2, true, 2_100, 2_200),
            interface("eth2", 3, true, 9_000, 9_000),
        ],
        Duration::from_secs(1),
    );
    assert_eq!(next.rx_bytes_per_sec, 100);
    assert_eq!(next.tx_bytes_per_sec, 200);
    assert_eq!(next.active_interface, "eth1");
    let next = collector.sample_counters(
        &[interface("eth1", 2, true, 2_200, 2_400)],
        Duration::from_secs(1),
    );
    assert_eq!(next.rx_bytes_per_sec, 100);
    assert_eq!(next.tx_bytes_per_sec, 200);
}

#[test]
fn network_recreated_interface_and_replug_get_new_baselines() {
    let mut collector = NetworkCollector::new();
    collector.sample_counters(
        &[interface("eth0", 1, true, 100, 100)],
        Duration::from_secs(1),
    );
    let replaced = collector.sample_counters(
        &[interface("eth0", 2, true, 90_000, 90_000)],
        Duration::from_secs(1),
    );
    assert_eq!(replaced.rx_bytes_per_sec, 0);
    collector.sample_counters(&[], Duration::from_secs(1));
    let replugged = collector.sample_counters(
        &[interface("eth0", 2, true, 95_000, 95_000)],
        Duration::from_secs(1),
    );
    assert_eq!(replugged.rx_bytes_per_sec, 0);
}

#[test]
fn network_active_is_current_traffic_not_lifetime_and_vpn_is_not_added_twice() {
    let mut collector = NetworkCollector::new();
    collector.sample_counters(
        &[
            interface("eth0", 1, true, 100_000, 100_000),
            interface("wlan0", 2, true, 100, 100),
            interface("tun0", 3, false, 500, 500),
        ],
        Duration::from_secs(1),
    );
    let next = collector.sample_counters(
        &[
            interface("eth0", 1, true, 100_000, 100_000),
            interface("wlan0", 2, true, 1_100, 600),
            interface("tun0", 3, false, 1_500, 1_000),
        ],
        Duration::from_secs(1),
    );
    assert_eq!(next.active_interface, "wlan0");
    assert_eq!(next.rx_bytes_per_sec, 1_000);
    assert_eq!(next.tx_bytes_per_sec, 500);
}

#[test]
fn virtual_only_network_uses_one_busy_link_and_idle_has_no_active_label() {
    let mut collector = NetworkCollector::new();
    let baseline = [
        interface("br0", 1, false, 0, 0),
        interface("veth0", 2, false, 0, 0),
    ];
    collector.sample_counters(&baseline, Duration::from_secs(1));
    let busy = [
        interface("br0", 1, false, 500, 200),
        interface("veth0", 2, false, 500, 200),
    ];
    let next = collector.sample_counters(&busy, Duration::from_secs(1));
    assert_eq!(next.rx_bytes_per_sec, 500);
    assert_eq!(next.active_interface, "br0");
    assert_eq!(
        collector
            .sample_counters(&busy, Duration::from_secs(1))
            .active_interface,
        ""
    );
    assert_eq!(
        collector
            .sample_counters(&busy, Duration::ZERO)
            .rx_bytes_per_sec,
        0
    );
}

#[test]
fn nvidia_first_device_has_independent_metric_availability() {
    let gpu =
        GpuCollector::parse_nvidia("0, NVIDIA One, 0, 8192, 35\n100, NVIDIA Two, 1024, 2048, 90")
            .unwrap();
    assert_eq!(gpu.name, "NVIDIA One");
    assert!(gpu.usage_available);
    assert_eq!(gpu.usage_percent, 0.0);
    assert!(gpu.memory_available);
    assert_eq!(gpu.temperature_c, Some(35.0));
    let missing = GpuCollector::parse_nvidia("[N/A], NVIDIA One, [N/A], [N/A], [N/A]").unwrap();
    assert!(!missing.usage_available);
    assert!(!missing.memory_available);
    assert_eq!(missing.temperature_c, None);
}

#[test]
fn malformed_and_nonfinite_gpu_values_do_not_become_available_zero() {
    assert!(GpuCollector::parse_nvidia("").is_none());
    assert!(GpuCollector::parse_nvidia("bad csv").is_none());
    let gpu = GpuCollector::parse_nvidia("NaN, NVIDIA, 100, 50, inf").unwrap();
    assert!(!gpu.usage_available);
    assert!(!gpu.memory_available);
    assert_eq!(gpu.temperature_c, None);
}

#[test]
fn intel_identification_does_not_claim_gt_busy_is_a_supported_counter() {
    let fixture = Fixture::new();
    fixture.write("card0/device/vendor", "0x8086\n");
    fixture.write("card0/gt_busy_percent", "85");
    fixture.write("card0/device/hwmon/hwmon0/temp1_input", "45000\n");
    let gpu = GpuCollector::collect_from(&fixture.0);
    assert_eq!(gpu.name, "Intel GPU");
    assert!(!gpu.usage_available);
    assert!(!gpu.memory_available);
    assert_eq!(gpu.temperature_c, Some(45.0));
}

#[test]
fn amd_counter_zero_is_available_and_connectors_are_not_cards() {
    let fixture = Fixture::new();
    fixture.write("card0-HDMI-A-1/device/vendor", "0x8086");
    fixture.write("card1/device/vendor", "0x1002");
    fixture.write("card1/device/gpu_busy_percent", "0");
    fixture.write("card1/device/mem_info_vram_used", "1048576");
    fixture.write("card1/device/mem_info_vram_total", "2097152");
    let gpu = GpuCollector::collect_from(&fixture.0);
    assert_eq!(gpu.name, "AMD GPU");
    assert!(gpu.usage_available);
    assert!(gpu.memory_available);
    assert_eq!(gpu.used_memory_mb, 1);
    assert_eq!(gpu.total_memory_mb, 2);
}

#[test]
fn missing_gpu_and_invalid_temperature_are_unavailable() {
    let fixture = Fixture::new();
    assert!(GpuCollector::collect_from(&fixture.0).name.is_empty());
    fixture.write("device/hwmon/hwmon0/temp1_input", "NaN");
    assert_eq!(
        TemperatureCollector::read_gpu_hwmon(&fixture.0.join("device")),
        None
    );
    fixture.write("device/hwmon/hwmon0/temp1_input", "500\n");
    assert_eq!(
        TemperatureCollector::read_gpu_hwmon(&fixture.0.join("device")),
        Some(0.5)
    );
}

#[test]
fn battery_type_not_filename_and_absent_battery_is_skipped() {
    let fixture = Fixture::new();
    fixture.write("BAT0/type", "Mains");
    fixture.write("BAT1/type", "Battery");
    fixture.write("BAT1/present", "0");
    fixture.write("custom/type", "Battery");
    fixture.write("custom/capacity", "72");
    fixture.write("custom/status", "Discharging\n");
    fixture.write("custom/power_now", "12500000");
    let battery = BatteryCollector::collect_from(&fixture.0);
    assert!(battery.has_battery);
    assert_eq!(battery.charge_percent, 72.0);
    assert_eq!(battery.status, BatteryStatus::Discharging);
    assert_eq!(battery.energy_watts, 12.5);
}

#[test]
fn cpu_thermal_fallback_does_not_mislabel_board_sensors() {
    let fixture = Fixture::new();
    fixture.write("thermal_zone0/type", "acpitz");
    fixture.write("thermal_zone0/temp", "90000");
    assert_eq!(TemperatureCollector::read_cpu_thermal(&fixture.0), None);
    fixture.write("thermal_zone1/type", "x86_pkg_temp");
    fixture.write("thermal_zone1/temp", "53000");
    fixture.write("thermal_zone2/type", "cpu-thermal");
    fixture.write("thermal_zone2/temp", "56000");
    fixture.write("thermal_zone3/type", "cpu-thermal");
    fixture.write("thermal_zone3/temp", "NaN");
    assert_eq!(
        TemperatureCollector::read_cpu_thermal(&fixture.0),
        Some(56.0)
    );
}

#[test]
fn battery_current_voltage_fallback_health_and_status_keys() {
    let fixture = Fixture::new();
    fixture.write("battery/type", "Battery");
    fixture.write("battery/capacity", "120");
    fixture.write("battery/current_now", "2000000");
    fixture.write("battery/voltage_now", "12000000");
    fixture.write("battery/charge_full", "80");
    fixture.write("battery/charge_full_design", "100");
    let battery = BatteryCollector::collect_from(&fixture.0);
    assert_eq!(battery.charge_percent, 100.0);
    assert_eq!(battery.energy_watts, 24.0);
    assert_eq!(battery.health_percent, 80.0);
    assert_eq!(battery.status, BatteryStatus::Unknown);
    for (value, expected) in [
        ("Charging", "charging"),
        ("Discharging", "discharging"),
        ("Full", "full"),
        ("Not charging", "not_charging"),
        ("bad", "unknown"),
    ] {
        assert_eq!(
            BatteryStatus::from_sysfs(value).localization_key(),
            format!("dashboard.battery_{expected}")
        );
    }
}
