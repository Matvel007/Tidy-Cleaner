use super::models::NetworkMetrics;
use std::time::Instant;
use sysinfo::Networks;

pub struct NetworkCollector {
    networks: Networks,
    last_sample: Instant,
    last_rx_total: u64,
    last_tx_total: u64,
}

impl Default for NetworkCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl NetworkCollector {
    pub fn new() -> Self {
        let networks = Networks::new_with_refreshed_list();
        let mut total_rx = 0u64;
        let mut total_tx = 0u64;
        for (_, data) in &networks {
            total_rx += data.total_received();
            total_tx += data.total_transmitted();
        }

        Self {
            networks,
            last_sample: Instant::now(),
            last_rx_total: total_rx,
            last_tx_total: total_tx,
        }
    }

    pub fn collect(&mut self) -> NetworkMetrics {
        self.networks.refresh(true);
        let now = Instant::now();
        let elapsed_secs = now.duration_since(self.last_sample).as_secs_f64().max(0.1);
        self.last_sample = now;

        let mut current_total_rx = 0u64;
        let mut current_total_tx = 0u64;
        let mut active_iface = String::new();
        let mut max_traffic = 0u64;

        for (name, data) in &self.networks {
            if name == "lo" {
                continue;
            }

            let rx = data.total_received();
            let tx = data.total_transmitted();
            current_total_rx += rx;
            current_total_tx += tx;

            let traffic = rx + tx;
            if traffic > max_traffic {
                max_traffic = traffic;
                active_iface = name.clone();
            }
        }

        let delta_rx = current_total_rx.saturating_sub(self.last_rx_total);
        let delta_tx = current_total_tx.saturating_sub(self.last_tx_total);
        self.last_rx_total = current_total_rx;
        self.last_tx_total = current_total_tx;

        let rx_per_sec = (delta_rx as f64 / elapsed_secs) as u64;
        let tx_per_sec = (delta_tx as f64 / elapsed_secs) as u64;

        NetworkMetrics {
            rx_bytes_per_sec: rx_per_sec,
            tx_bytes_per_sec: tx_per_sec,
            rx_speed_formatted: Self::format_speed(rx_per_sec),
            tx_speed_formatted: Self::format_speed(tx_per_sec),
            total_rx_bytes: current_total_rx,
            total_tx_bytes: current_total_tx,
            active_interface: active_iface,
        }
    }

    fn format_speed(bytes_per_sec: u64) -> String {
        const KB: f64 = 1024.0;
        const MB: f64 = 1024.0 * 1024.0;
        let b = bytes_per_sec as f64;
        if b >= MB {
            format!("{:.1} MB/s", b / MB)
        } else if b >= KB {
            format!("{:.0} KB/s", b / KB)
        } else {
            format!("{} B/s", bytes_per_sec)
        }
    }
}
