//! Disk free-space monitor for the status bar.
//!
//! `DiskMonitor` runs a background thread that periodically reports free space
//! on a target disk via `sysinfo`. The target is either the disk containing
//! par-term's launch directory (default) or the disk containing the active
//! tab/pane's working directory (when follow-cwd is enabled).

use std::time::Instant;

/// Snapshot of free space on the monitored disk.
#[derive(Debug, Clone, Default)]
pub struct DiskMonitorData {
    /// Free bytes available to non-privileged users.
    pub free_bytes: u64,
    /// Total capacity of the disk (bytes).
    pub total_bytes: u64,
    /// Free percentage (0.0–100.0).
    pub free_percent: f32,
    /// When this data was last updated.
    pub last_update: Option<Instant>,
}

// ============================================================================
// Full implementation (feature enabled)
// ============================================================================

#[cfg(feature = "system-monitor")]
mod inner {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread::JoinHandle;
    use std::time::{Duration, Instant};

    use parking_lot::Mutex;
    use sysinfo::{Disk, DiskRefreshKind, Disks};

    use super::DiskMonitorData;

    /// Background disk free-space monitor.
    ///
    /// Spawns a polling thread that periodically refreshes disk free space via
    /// `sysinfo`. The monitored disk defaults to the one containing par-term's
    /// launch directory; with `set_follow_cwd(true)` it follows the active
    /// tab/pane's working directory instead.
    pub struct DiskMonitor {
        data: Arc<Mutex<DiskMonitorData>>,
        cwd: Arc<Mutex<Option<String>>>,
        follow_cwd: Arc<Mutex<bool>>,
        launch_dir: Arc<Mutex<PathBuf>>,
        running: Arc<AtomicBool>,
        thread: Mutex<Option<JoinHandle<()>>>,
    }

    impl DiskMonitor {
        /// Create a new (stopped) disk monitor, capturing the launch directory.
        ///
        /// par-term never chdir's its own process (the PTY child does), so
        /// `std::env::current_dir()` here is stable for the whole session.
        pub fn new() -> Self {
            let launch_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
            Self {
                data: Arc::new(Mutex::new(DiskMonitorData::default())),
                cwd: Arc::new(Mutex::new(None)),
                follow_cwd: Arc::new(Mutex::new(false)),
                launch_dir: Arc::new(Mutex::new(launch_dir)),
                running: Arc::new(AtomicBool::new(false)),
                thread: Mutex::new(None),
            }
        }

        /// Start the polling thread. No-op if already running.
        pub fn start(&self, poll_interval_secs: f32) {
            if self
                .running
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_err()
            {
                return;
            }

            let data = Arc::clone(&self.data);
            let cwd = Arc::clone(&self.cwd);
            let follow_cwd = Arc::clone(&self.follow_cwd);
            let launch_dir = Arc::clone(&self.launch_dir);
            let running = Arc::clone(&self.running);
            let interval = Duration::from_secs_f32(poll_interval_secs.max(5.0));

            let handle = std::thread::Builder::new()
                .name("status-bar-disk".to_string())
                .spawn(move || {
                    let mut disks = Disks::new_with_refreshed_list();
                    while running.load(Ordering::SeqCst) {
                        disks.refresh_specifics(true, DiskRefreshKind::nothing().with_storage());

                        let use_cwd = *follow_cwd.lock();
                        let target: PathBuf = if use_cwd {
                            cwd.lock()
                                .clone()
                                .filter(|c| !c.is_empty())
                                .map(PathBuf::from)
                                .unwrap_or_else(|| launch_dir.lock().clone())
                        } else {
                            launch_dir.lock().clone()
                        };

                        // Resolve target → disk; if following CWD but no disk
                        // matched (e.g. removable since removed), fall back to
                        // the launch dir's disk.
                        let mut disk = disk_for_path(disks.list(), &target);
                        if disk.is_none() && use_cwd {
                            disk = disk_for_path(disks.list(), &launch_dir.lock());
                        }

                        if let Some(d) = disk {
                            let total = d.total_space();
                            let free = d.available_space();
                            let pct = if total > 0 {
                                (free as f64 / total as f64 * 100.0) as f32
                            } else {
                                0.0
                            };
                            let mut dat = data.lock();
                            dat.free_bytes = free;
                            dat.total_bytes = total;
                            dat.free_percent = pct;
                            dat.last_update = Some(Instant::now());
                        }

                        let deadline = Instant::now() + interval;
                        while Instant::now() < deadline && running.load(Ordering::Relaxed) {
                            std::thread::sleep(Duration::from_millis(50));
                        }
                    }
                });

            match handle {
                Ok(h) => *self.thread.lock() = Some(h),
                Err(e) => {
                    // Reset running so start() can be retried; degrade
                    // gracefully rather than crashing the session.
                    self.running.store(false, Ordering::SeqCst);
                    crate::debug_error!(
                        "SESSION_LOGGER",
                        "failed to spawn disk monitor thread: {:?}",
                        e
                    );
                }
            }
        }

        /// Signal the polling thread to stop without waiting.
        pub fn signal_stop(&self) {
            self.running.store(false, Ordering::SeqCst);
        }

        /// Stop the polling thread and wait for it to finish.
        pub fn stop(&self) {
            self.signal_stop();
            if let Some(handle) = self.thread.lock().take() {
                let _ = handle.join();
            }
        }

        /// Whether the polling thread is currently running.
        pub fn is_running(&self) -> bool {
            self.running.load(Ordering::SeqCst)
        }

        /// Get a clone of the current data snapshot.
        pub fn data(&self) -> DiskMonitorData {
            self.data.lock().clone()
        }

        /// Update the active tab's working directory (called each frame).
        pub fn set_cwd(&self, new_cwd: Option<&str>) {
            *self.cwd.lock() = new_cwd.map(String::from);
        }

        /// Toggle between launch-disk (false) and follow-active-CWD (true).
        pub fn set_follow_cwd(&self, follow: bool) {
            *self.follow_cwd.lock() = follow;
        }
    }

    impl Default for DiskMonitor {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Drop for DiskMonitor {
        fn drop(&mut self) {
            self.stop();
        }
    }

    /// Return the disk whose mount point is the longest path-prefix of `target`.
    ///
    /// `Path::starts_with` compares by component, so `/` matches every absolute
    /// path on the root volume (the universal fallback). Pure over the slice.
    pub fn disk_for_path<'a>(disks: &'a [Disk], target: &Path) -> Option<&'a Disk> {
        disks
            .iter()
            .filter(|d| target.starts_with(d.mount_point()))
            .max_by_key(|d| d.mount_point().components().count())
    }
}

#[cfg(feature = "system-monitor")]
pub use inner::{DiskMonitor, disk_for_path};

// ============================================================================
// Stub implementation (feature disabled)
// ============================================================================

#[cfg(not(feature = "system-monitor"))]
mod inner {
    use super::DiskMonitorData;

    /// Stub disk monitor (sysinfo feature disabled). Same API, all no-ops.
    pub struct DiskMonitor;

    impl DiskMonitor {
        pub fn new() -> Self {
            Self
        }
        pub fn start(&self, _poll_interval_secs: f32) {}
        pub fn signal_stop(&self) {}
        pub fn stop(&self) {}
        pub fn is_running(&self) -> bool {
            false
        }
        pub fn data(&self) -> DiskMonitorData {
            DiskMonitorData::default()
        }
        pub fn set_cwd(&self, _new_cwd: Option<&str>) {}
        pub fn set_follow_cwd(&self, _follow: bool) {}
    }

    impl Default for DiskMonitor {
        fn default() -> Self {
            Self::new()
        }
    }
}

#[cfg(not(feature = "system-monitor"))]
pub use inner::DiskMonitor;

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disk_monitor_data_default() {
        let d = DiskMonitorData::default();
        assert_eq!(d.free_bytes, 0);
        assert_eq!(d.total_bytes, 0);
        assert_eq!(d.free_percent, 0.0);
        assert!(d.last_update.is_none());
    }

    #[cfg(feature = "system-monitor")]
    #[test]
    fn test_disk_for_path_resolves() {
        use sysinfo::Disks;
        let disks = Disks::new_with_refreshed_list();
        // The system temp dir must resolve to some disk.
        let tmp = std::env::temp_dir();
        let d = disk_for_path(disks.list(), &tmp);
        assert!(d.is_some(), "no disk matched temp dir {:?}", tmp);
        let d = d.unwrap();
        assert!(d.total_space() > 0);
        assert!(d.available_space() <= d.total_space());
    }

    #[cfg(feature = "system-monitor")]
    #[test]
    fn test_disk_for_path_returns_mount_prefix() {
        use sysinfo::Disks;
        let disks = Disks::new_with_refreshed_list();
        if disks.list().is_empty() {
            return; // no disks to assert against
        }
        // The temp dir lives on a real volume on every platform. The matched
        // disk's mount point must be a path-component prefix of the target —
        // the core invariant of disk_for_path. (Does not assume a Unix `/`
        // root, which matches no drive-letter volume on Windows.)
        let target = std::env::temp_dir();
        let matched = disk_for_path(disks.list(), &target);
        assert!(matched.is_some(), "no disk matched temp dir {target:?}");
        assert!(
            target.starts_with(matched.unwrap().mount_point()),
            "matched mount point must be a prefix of the target path"
        );
    }

    #[cfg(feature = "system-monitor")]
    #[test]
    fn test_disk_monitor_start_stop() {
        use std::time::Duration;
        let monitor = DiskMonitor::new();
        assert!(!monitor.is_running());
        monitor.start(5.0);
        assert!(monitor.is_running());
        // Wait for the first poll (mirrors system_monitor's flake-free approach).
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        let mut polled = false;
        while std::time::Instant::now() < deadline {
            if monitor.data().last_update.is_some() {
                polled = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(polled, "disk monitor recorded no initial poll within 10s");
        // free percent is sane
        let d = monitor.data();
        assert!(d.free_percent >= 0.0 && d.free_percent <= 100.0);
        monitor.stop();
        assert!(!monitor.is_running());
    }
}
