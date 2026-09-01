pub mod cpu;
pub mod disks;
pub mod gpu;
pub mod memory;
pub mod models;
pub mod os_info;
pub mod service;

#[allow(unused_imports)]
pub use cpu::CpuCollector;
#[allow(unused_imports)]
pub use disks::DiskCollector;
#[allow(unused_imports)]
pub use gpu::GpuCollector;
#[allow(unused_imports)]
pub use memory::MemoryCollector;
#[allow(unused_imports)]
pub use models::{
    generate_arc_svg_path, CpuMetrics, DiskInfo, GpuMetrics, MemoryMetrics, SystemOverview,
    SystemSnapshot,
};
pub use os_info::OsInfoCollector;
#[allow(unused_imports)]
pub use service::{SharedSystemMonitor, SystemMonitorService};
