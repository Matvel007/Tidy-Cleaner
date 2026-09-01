use super::models::CpuMetrics;
use std::collections::VecDeque;
use sysinfo::System;

pub const HISTORY_CAPACITY: usize = 24;

pub struct CpuCollector {
    history: VecDeque<f32>,
}

impl Default for CpuCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl CpuCollector {
    pub fn new() -> Self {
        let mut history = VecDeque::with_capacity(HISTORY_CAPACITY);
        for _ in 0..HISTORY_CAPACITY {
            history.push_back(0.0);
        }
        Self { history }
    }

    pub fn collect(&mut self, sys: &mut System) -> CpuMetrics {
        sys.refresh_cpu_usage();

        let cpus = sys.cpus();
        let core_count = cpus.len();

        let usage_percent = sys.global_cpu_usage();

        let frequency_mhz = if !cpus.is_empty() {
            cpus[0].frequency()
        } else {
            0
        };

        let brand_name = if !cpus.is_empty() {
            cpus[0].brand().trim().to_string()
        } else {
            "Unknown CPU".to_string()
        };

        if self.history.len() >= HISTORY_CAPACITY {
            self.history.pop_front();
        }
        self.history.push_back(usage_percent);

        CpuMetrics {
            usage_percent,
            core_count,
            frequency_mhz,
            brand_name,
            history: self.history.iter().copied().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_history_capacity() {
        let mut collector = CpuCollector::new();
        assert_eq!(collector.history.len(), HISTORY_CAPACITY);

        let mut sys = System::new();
        let metrics = collector.collect(&mut sys);
        assert_eq!(metrics.history.len(), HISTORY_CAPACITY);
    }
}
