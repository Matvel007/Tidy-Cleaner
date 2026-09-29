use super::models::MemoryMetrics;
use std::collections::VecDeque;
use sysinfo::System;

pub const HISTORY_CAPACITY: usize = 24;

pub struct MemoryCollector {
    history: VecDeque<f32>,
}

impl Default for MemoryCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryCollector {
    pub fn new() -> Self {
        let mut history = VecDeque::with_capacity(HISTORY_CAPACITY);
        for _ in 0..HISTORY_CAPACITY {
            history.push_back(0.0);
        }
        Self { history }
    }

    pub fn collect(&mut self, sys: &mut System) -> MemoryMetrics {
        sys.refresh_memory();

        let total_bytes = sys.total_memory();
        let used_bytes = sys.used_memory();
        let available_bytes = sys.available_memory();

        let usage_percent = if total_bytes > 0 {
            (used_bytes as f32 / total_bytes as f32) * 100.0
        } else {
            0.0
        };

        let total_swap_bytes = sys.total_swap();
        let used_swap_bytes = sys.used_swap();
        let swap_usage_percent = if total_swap_bytes > 0 {
            (used_swap_bytes as f32 / total_swap_bytes as f32) * 100.0
        } else {
            0.0
        };

        let is_zram = if total_swap_bytes > 0 {
            std::fs::read_to_string("/proc/swaps")
                .map(|content| {
                    let active_swaps: Vec<&str> = content
                        .lines()
                        .skip(1)
                        .filter(|l| !l.trim().is_empty())
                        .collect();
                    !active_swaps.is_empty() && active_swaps.iter().all(|l| l.contains("zram"))
                })
                .unwrap_or(false)
        } else {
            false
        };

        if self.history.len() >= HISTORY_CAPACITY {
            self.history.pop_front();
        }
        self.history.push_back(usage_percent);

        MemoryMetrics {
            used_bytes,
            total_bytes,
            available_bytes,
            usage_percent,
            used_swap_bytes,
            total_swap_bytes,
            swap_usage_percent,
            is_zram,
            history: self.history.iter().copied().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_collector() {
        let mut collector = MemoryCollector::new();
        assert_eq!(collector.history.len(), HISTORY_CAPACITY);

        let mut sys = System::new();
        let metrics = collector.collect(&mut sys);
        assert_eq!(metrics.history.len(), HISTORY_CAPACITY);
    }
}
