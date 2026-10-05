use super::battery::BatteryCollector;
use super::cpu::CpuCollector;
use super::disks::DiskCollector;
use super::gpu::{GpuCollector, GpuMetrics};
use super::memory::MemoryCollector;
use super::models::SystemSnapshot;
use super::network::NetworkCollector;
use super::os_info::OsInfoCollector;
use super::temperature::{TemperatureCollector, TemperatureMetrics};
use std::sync::{Arc, Mutex};
use sysinfo::System;

pub struct SystemMonitorService {
    system: Mutex<System>,
    cpu_collector: Mutex<CpuCollector>,
    mem_collector: Mutex<MemoryCollector>,
    net_collector: Mutex<NetworkCollector>,
    gpu: Mutex<GpuMetrics>,
}

impl Default for SystemMonitorService {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemMonitorService {
    pub fn new() -> Self {
        Self {
            system: Mutex::new(System::new()),
            cpu_collector: Mutex::new(CpuCollector::new()),
            mem_collector: Mutex::new(MemoryCollector::new()),
            net_collector: Mutex::new(NetworkCollector::new()),
            gpu: Mutex::new(GpuMetrics::default()),
        }
    }

    pub fn sample_snapshot(&self) -> SystemSnapshot {
        let mut sys = self.system.lock().unwrap();
        let mut cpu_col = self.cpu_collector.lock().unwrap();
        let mut mem_col = self.mem_collector.lock().unwrap();
        let mut net_col = self.net_collector.lock().unwrap();

        let cpu = cpu_col.collect(&mut sys);
        let gpu = self.gpu.lock().unwrap().clone();
        let memory = mem_col.collect(&mut sys);
        let disks = DiskCollector::collect_disks();
        let temperature = TemperatureMetrics {
            cpu_temp_c: TemperatureCollector::collect_cpu_temp(),
            gpu_temp_c: gpu.temperature_c,
        };
        let overview = OsInfoCollector::collect_overview(&sys);
        let network = net_col.collect();
        let battery = BatteryCollector::collect();

        SystemSnapshot {
            cpu,
            gpu,
            memory,
            disks,
            temperature,
            overview,
            network,
            battery,
        }
    }

    // Run independently of the core sampler: a slow driver cannot freeze CPU/RAM/network.
    pub fn sample_gpu(&self) {
        let gpu = GpuCollector::collect();
        *self.gpu.lock().unwrap() = gpu;
    }
}

#[allow(dead_code)]
pub type SharedSystemMonitor = Arc<SystemMonitorService>;
