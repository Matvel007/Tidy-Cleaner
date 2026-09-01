use super::cpu::CpuCollector;
use super::disks::DiskCollector;
use super::gpu::GpuCollector;
use super::memory::MemoryCollector;
use super::models::SystemSnapshot;
use super::os_info::OsInfoCollector;
use super::temperature::TemperatureCollector;
use std::sync::{Arc, Mutex};
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};

pub struct SystemMonitorService {
    system: Mutex<System>,
    cpu_collector: Mutex<CpuCollector>,
    mem_collector: Mutex<MemoryCollector>,
}

impl Default for SystemMonitorService {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemMonitorService {
    pub fn new() -> Self {
        let refresh_kind = RefreshKind::nothing()
            .with_cpu(CpuRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything());
        let system = System::new_with_specifics(refresh_kind);

        Self {
            system: Mutex::new(system),
            cpu_collector: Mutex::new(CpuCollector::new()),
            mem_collector: Mutex::new(MemoryCollector::new()),
        }
    }

    pub fn sample_snapshot(&self) -> SystemSnapshot {
        let mut sys = self.system.lock().unwrap();
        let mut cpu_col = self.cpu_collector.lock().unwrap();
        let mut mem_col = self.mem_collector.lock().unwrap();

        let cpu = cpu_col.collect(&mut sys);
        let gpu = GpuCollector::collect();
        let memory = mem_col.collect(&mut sys);
        let disks = DiskCollector::collect_disks();
        let temperature = TemperatureCollector::collect();
        let overview = OsInfoCollector::collect_overview(&sys);

        SystemSnapshot {
            cpu,
            gpu,
            memory,
            disks,
            temperature,
            overview,
        }
    }
}

#[allow(dead_code)]
pub type SharedSystemMonitor = Arc<SystemMonitorService>;
