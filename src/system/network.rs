use super::models::NetworkMetrics;
use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};
use sysinfo::Networks;

#[derive(Clone)]
pub struct InterfaceCounters {
    pub name: String,
    pub identity: u32,
    pub physical: bool,
    pub rx: u64,
    pub tx: u64,
}

pub struct NetworkCollector {
    networks: Networks,
    last_sample: Instant,
    previous: HashMap<(String, u32), (u64, u64)>,
}

impl Default for NetworkCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl NetworkCollector {
    pub fn new() -> Self {
        Self {
            networks: Networks::new(),
            last_sample: Instant::now(),
            previous: HashMap::new(),
        }
    }

    pub fn collect(&mut self) -> NetworkMetrics {
        self.networks.refresh(true);
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_sample);
        self.last_sample = now;
        let counters: Vec<_> = self
            .networks
            .iter()
            .map(|(name, data)| {
                let path = Path::new("/sys/class/net").join(name);
                InterfaceCounters {
                    name: name.clone(),
                    identity: std::fs::read_to_string(path.join("ifindex"))
                        .ok()
                        .and_then(|s| s.trim().parse().ok())
                        .unwrap_or_default(),
                    physical: path.join("device").exists(),
                    rx: data.total_received(),
                    tx: data.total_transmitted(),
                }
            })
            .collect();
        self.sample_counters(&counters, elapsed)
    }

    pub fn sample_counters(
        &mut self,
        counters: &[InterfaceCounters],
        elapsed: Duration,
    ) -> NetworkMetrics {
        let mut candidates = Vec::new();
        let mut previous = HashMap::new();
        for counter in counters.iter().filter(|counter| counter.name != "lo") {
            let key = (counter.name.clone(), counter.identity);
            // Rebaseline new interfaces and reset counters individually, not the
            // aggregate (which can spike or suppress unrelated live traffic).
            let (rx, tx) = self
                .previous
                .get(&key)
                .map(|&(rx, tx)| (counter.rx.saturating_sub(rx), counter.tx.saturating_sub(tx)))
                .unwrap_or_default();
            previous.insert(key, (counter.rx, counter.tx));
            candidates.push((counter, rx, tx));
        }
        self.previous = previous;
        // Report wire traffic on physical links, not both a VPN/bridge and its
        // underlying device. Virtual-only environments report one busiest link.
        let physical = candidates.iter().any(|(counter, _, _)| counter.physical);
        candidates.retain(|(counter, _, _)| !physical || counter.physical);
        candidates.sort_by(|a, b| {
            b.1.saturating_add(b.2)
                .cmp(&a.1.saturating_add(a.2))
                .then_with(|| a.0.name.cmp(&b.0.name))
        });
        if !physical {
            candidates.truncate(1);
        }
        let active_interface = candidates
            .first()
            .filter(|(_, rx, tx)| *rx > 0 || *tx > 0)
            .map(|(counter, _, _)| counter.name.clone())
            .unwrap_or_default();
        let (mut rx, mut tx, mut total_rx, mut total_tx) = (0u64, 0u64, 0u64, 0u64);
        for (counter, drx, dtx) in candidates {
            rx = rx.saturating_add(drx);
            tx = tx.saturating_add(dtx);
            total_rx = total_rx.saturating_add(counter.rx);
            total_tx = total_tx.saturating_add(counter.tx);
        }
        let seconds = elapsed.as_secs_f64();
        let rate = |bytes: u64| {
            if seconds > 0.0 {
                (bytes as f64 / seconds) as u64
            } else {
                0
            }
        };
        NetworkMetrics {
            rx_bytes_per_sec: rate(rx),
            tx_bytes_per_sec: rate(tx),
            rx_speed_formatted: Self::format_speed(rate(rx)),
            tx_speed_formatted: Self::format_speed(rate(tx)),
            total_rx_bytes: total_rx,
            total_tx_bytes: total_tx,
            active_interface,
        }
    }

    fn format_speed(bytes_per_sec: u64) -> String {
        let b = bytes_per_sec as f64;
        if b >= 1024.0 * 1024.0 {
            format!("{:.1} MB/s", b / (1024.0 * 1024.0))
        } else if b >= 1024.0 {
            format!("{:.0} KB/s", b / 1024.0)
        } else {
            format!("{} B/s", bytes_per_sec)
        }
    }
}
