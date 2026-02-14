//! System resource monitor for the status bar.
//!
//! Polls CPU, memory, and network usage on a background thread using `sysinfo`.
//! Data is shared via `Arc<parking_lot::Mutex<...>>` for lock-free reads from
//! the render thread.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

/// Snapshot of system resource usage.
#[derive(Debug, Clone, Default)]
pub struct SystemMonitorData {
    /// Global CPU usage percentage (0.0 - 100.0)
    pub cpu_usage: f32,
    /// Memory currently in use (bytes)
    pub memory_used: u64,
    /// Total physical memory (bytes)
    pub memory_total: u64,
    /// Network receive rate (bytes/sec)
    pub network_rx_rate: u64,
    /// Network transmit rate (bytes/sec)
    pub network_tx_rate: u64,
    /// When this data was last updated
    pub last_update: Option<Instant>,
}

/// Background system resource monitor.
///
/// Spawns a polling thread that periodically refreshes CPU, memory, and
/// network statistics via `sysinfo`.
pub struct SystemMonitor {
    data: Arc<Mutex<SystemMonitorData>>,
    running: Arc<AtomicBool>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl SystemMonitor {
    /// Create a new (stopped) system monitor.
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(SystemMonitorData::default())),
            running: Arc::new(AtomicBool::new(false)),
            thread: Mutex::new(None),
        }
    }

    /// Start the polling thread.
    ///
    /// If the monitor is already running, this is a no-op.
    pub fn start(&self, poll_interval_secs: f32) {
        if self
            .running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }

        let data = Arc::clone(&self.data);
        let running = Arc::clone(&self.running);
        let interval = Duration::from_secs_f32(poll_interval_secs.max(0.5));

        let handle = std::thread::Builder::new()
            .name("status-bar-sysmon".to_string())
            .spawn(move || {
                use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};

                let mut sys = System::new_with_specifics(
                    RefreshKind::nothing()
                        .with_cpu(CpuRefreshKind::everything())
                        .with_memory(MemoryRefreshKind::everything()),
                );
                let mut networks = sysinfo::Networks::new_with_refreshed_list();

                // First CPU poll is always 0% — need two samples.
                sys.refresh_cpu_all();
                std::thread::sleep(Duration::from_millis(200));

                let mut prev_rx: u64 = 0;
                let mut prev_tx: u64 = 0;
                let mut first_net = true;

                while running.load(Ordering::SeqCst) {
                    sys.refresh_cpu_all();
                    sys.refresh_memory();
                    networks.refresh(true);

                    // Network totals
                    let (mut total_rx, mut total_tx) = (0u64, 0u64);
                    for (_name, net) in networks.iter() {
                        total_rx = total_rx.saturating_add(net.total_received());
                        total_tx = total_tx.saturating_add(net.total_transmitted());
                    }

                    let (rx_rate, tx_rate) = if first_net {
                        first_net = false;
                        (0, 0)
                    } else {
                        let secs = interval.as_secs_f64();
                        let rx_delta = total_rx.saturating_sub(prev_rx);
                        let tx_delta = total_tx.saturating_sub(prev_tx);
                        (
                            (rx_delta as f64 / secs) as u64,
                            (tx_delta as f64 / secs) as u64,
                        )
                    };
                    prev_rx = total_rx;
                    prev_tx = total_tx;

                    {
                        let mut d = data.lock();
                        d.cpu_usage = sys.global_cpu_usage();
                        d.memory_used = sys.used_memory();
                        d.memory_total = sys.total_memory();
                        d.network_rx_rate = rx_rate;
                        d.network_tx_rate = tx_rate;
                        d.last_update = Some(Instant::now());
                    }

                    std::thread::sleep(interval);
                }
            })
            .expect("failed to spawn sysmon thread");

        *self.thread.lock() = Some(handle);
    }

    /// Stop the polling thread.
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.thread.lock().take() {
            let _ = handle.join();
        }
    }

    /// Whether the polling thread is currently running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Get a clone of the current data snapshot.
    pub fn data(&self) -> SystemMonitorData {
        self.data.lock().clone()
    }
}

impl Default for SystemMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for SystemMonitor {
    fn drop(&mut self) {
        self.stop();
    }
}

// ============================================================================
// Formatting helpers
// ============================================================================

/// Format bytes-per-second into a fixed-width human-readable string.
///
/// Output is always 10 characters wide (e.g. `"  1.0 KB/s"`) so the
/// status bar doesn't jump around when values change.
pub fn format_bytes_per_sec(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * 1024 * 1024;

    if bytes >= GB {
        format!("{:>5.1} GB/s", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:>5.1} MB/s", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:>5.1} KB/s", bytes as f64 / KB as f64)
    } else {
        // Extra space before "B" so width matches "KB", "MB", "GB"
        format!("{:>5}  B/s", bytes)
    }
}

/// Format memory usage (used / total) into a human-readable string.
///
/// Each side is fixed-width (7 chars, e.g. `"  4.0 GB"`) so the status
/// bar doesn't jump when values change.
pub fn format_memory(used: u64, total: u64) -> String {
    fn human(bytes: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = 1024 * 1024;
        const GB: u64 = 1024 * 1024 * 1024;

        if bytes >= GB {
            format!("{:>5.1} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:>5.1} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{:>5.1} KB", bytes as f64 / KB as f64)
        } else {
            format!("{:>5}  B", bytes)
        }
    }

    format!("{} / {}", human(used), human(total))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_monitor_data_default() {
        let d = SystemMonitorData::default();
        assert_eq!(d.cpu_usage, 0.0);
        assert_eq!(d.memory_used, 0);
        assert_eq!(d.memory_total, 0);
        assert_eq!(d.network_rx_rate, 0);
        assert_eq!(d.network_tx_rate, 0);
        assert!(d.last_update.is_none());
    }

    #[test]
    fn test_format_bytes_per_sec() {
        assert_eq!(format_bytes_per_sec(0), "    0  B/s");
        assert_eq!(format_bytes_per_sec(512), "  512  B/s");
        assert_eq!(format_bytes_per_sec(1024), "  1.0 KB/s");
        assert_eq!(format_bytes_per_sec(1536), "  1.5 KB/s");
        assert_eq!(format_bytes_per_sec(1_048_576), "  1.0 MB/s");
        assert_eq!(format_bytes_per_sec(1_073_741_824), "  1.0 GB/s");
        // All outputs have same width
        assert_eq!(
            format_bytes_per_sec(0).len(),
            format_bytes_per_sec(1024).len()
        );
        assert_eq!(
            format_bytes_per_sec(1024).len(),
            format_bytes_per_sec(1_048_576).len()
        );
    }

    #[test]
    fn test_format_memory() {
        assert_eq!(format_memory(0, 0), "    0  B /     0  B");
        // 1 GB used / 8 GB total
        assert_eq!(
            format_memory(1_073_741_824, 8_589_934_592),
            "  1.0 GB /   8.0 GB"
        );
        // 512 MB / 1 GB
        assert_eq!(
            format_memory(536_870_912, 1_073_741_824),
            "512.0 MB /   1.0 GB"
        );
    }

    #[test]
    fn test_system_monitor_start_stop() {
        let monitor = SystemMonitor::new();
        assert!(!monitor.is_running());

        monitor.start(1.0);
        assert!(monitor.is_running());

        // Give the thread a moment to do an initial poll
        std::thread::sleep(Duration::from_millis(500));

        let data = monitor.data();
        // After starting, last_update should be set (thread had 200ms init + sleep)
        assert!(data.last_update.is_some());

        monitor.stop();
        assert!(!monitor.is_running());
    }
}
