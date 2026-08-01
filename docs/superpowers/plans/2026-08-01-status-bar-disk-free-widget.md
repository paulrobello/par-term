# Status Bar Disk-Free Widget Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `DiskFree` status-bar widget that reports free disk space as `DISK  62% (250 GB free)`, cross-platform via `sysinfo`, defaulting to the launch disk at a 60s poll interval with an optional "follow active tab's CWD" toggle.

**Architecture:** A third background poller (`DiskMonitor`) that mirrors the existing `GitBranchPoller` (path-scoped, own interval, own thread, start/stop gated by widget presence). Disk data comes from the already-enabled `sysinfo` disk feature — no new dependency or Cargo feature. The launch disk is captured once via `std::env::current_dir()` inside `DiskMonitor::new()`; when follow-CWD is on, the active tab's path resolves to its containing mount via longest-prefix match, with launch-disk fallback.

**Tech Stack:** Rust (Edition 2024), `sysinfo` 0.39 (already a workspace dep, `disk` feature on by default), egui (settings UI), serde (config).

## Global Constraints

- **Verification gate:** `make checkall` (fmt-check, lint, typecheck, test). Every task runs at least `cargo check -p <crate>` for the crate it touched; the final task runs full `make checkall`.
- **Config default rule (CLAUDE.md gotcha):** every new `StatusBarConfig` field's `#[serde(default = "default_x")]` function MUST return the same value as the field's `Default`-impl entry. `par-term-config/tests/config_yaml_compat.rs` enforces this — never use `#[derive(Default)]` to add a field.
- **Logging in sub-crates:** sub-crates (par-term-config, par-term-settings-ui) must use `log::`, not `crate::debug_*!`. The root crate (src/status_bar/) may use `crate::debug_error!`.
- **Feature gate:** the real `DiskMonitor` is `#[cfg(feature = "system-monitor")]`; a no-op stub compiles when the feature is off, mirroring `src/status_bar/system_monitor.rs` exactly.
- **Render format (fixed, approved):** `DISK {:>4.0}% ({}) free` → e.g. `DISK  62% (250 GB free)`. Percent and bytes both fixed-width so the bar does not jump.
- **sysinfo 0.39 API (verified):** space-only refresh is `Disks::refresh_specifics(true, DiskRefreshKind::nothing().with_storage())`; `Disk::available_space()` / `total_space()` / `mount_point()`; path→disk match uses `Path::starts_with(mount_point)`.

## File Structure

| File | Responsibility | Action |
|------|----------------|--------|
| `par-term-config/src/status_bar.rs` | `WidgetId` enum + serialization + `default_widgets()` | Modify |
| `par-term-config/src/config/config_struct/status_bar_config.rs` | `StatusBarConfig` fields + defaults | Modify |
| `src/status_bar/system_monitor.rs` | `format_bytes` formatter (shared) | Modify |
| `src/status_bar/disk_monitor.rs` | `DiskMonitorData`, `DiskMonitor`, `disk_for_path` | Create |
| `src/status_bar/mod.rs` | `StatusBarUI` wiring of the poller | Modify |
| `src/status_bar/widgets.rs` | `WidgetContext` + `widget_text` DiskFree arm + interpolation | Modify |
| `par-term-settings-ui/src/status_bar_tab/poll_intervals.rs` | disk interval slider + follow-cwd checkbox | Modify |
| `par-term-settings-ui/src/status_bar_tab/mod.rs` | search keywords | Modify |
| `docs/features/STATUS_BAR.md` | user docs | Modify |

---

### Task 1: Config crate — `WidgetId::DiskFree` + two config fields

**Files:**
- Modify: `par-term-config/src/status_bar.rs`
- Modify: `par-term-config/src/config/config_struct/status_bar_config.rs`

**Interfaces:**
- Produces: `WidgetId::DiskFree` variant; `WidgetId::needs_disk_monitor() -> bool`; new entries in `default_widgets()`; `StatusBarConfig::status_bar_disk_poll_interval: f32` (default `60.0`); `StatusBarConfig::status_bar_disk_follow_cwd: bool` (default `false`). YAML key for the widget is `disk_free`.

- [ ] **Step 1: Add the `DiskFree` variant and metadata to `par-term-config/src/status_bar.rs`**

In the `WidgetId` enum, add a variant after `NetworkStatus`:
```rust
    /// Network throughput (rx/tx rates)
    NetworkStatus,
    /// Free disk space (percent + bytes)
    DiskFree,
```

In `impl WidgetId`, extend `label()`:
```rust
            WidgetId::NetworkStatus => "Network Status",
            WidgetId::DiskFree => "Disk Free",
```

Extend `icon()`:
```rust
            WidgetId::NetworkStatus => "\u{1f310}",    // globe with meridians
            WidgetId::DiskFree => "\u{1f4bf}",          // optical disk
```

Add a new method next to `needs_system_monitor`:
```rust
    /// Whether this widget requires the disk monitor to be running.
    pub fn needs_disk_monitor(&self) -> bool {
        matches!(self, WidgetId::DiskFree)
    }
```
(Do **not** add `DiskFree` to `needs_system_monitor()` — disk has its own poller.)

Extend `as_key()`:
```rust
            WidgetId::NetworkStatus => "network_status".to_string(),
            WidgetId::DiskFree => "disk_free".to_string(),
```

Extend `from_key()`:
```rust
            "network_status" => WidgetId::NetworkStatus,
            "disk_free" => WidgetId::DiskFree,
```

- [ ] **Step 2: Add `DiskFree` to `default_widgets()` (disabled, Right section)**

In `default_widgets()` in the same file, insert a new disabled entry after the `NetworkStatus` entry, and bump Bell/Clock/UpdateAvailable orders from 3/4/5 to 4/5/6:
```rust
        StatusBarWidgetConfig {
            id: WidgetId::NetworkStatus,
            enabled: false,
            section: StatusBarSection::Right,
            order: 2,
            format: None,
        },
        StatusBarWidgetConfig {
            id: WidgetId::DiskFree,
            enabled: false,
            section: StatusBarSection::Right,
            order: 3,
            format: None,
        },
        StatusBarWidgetConfig {
            id: WidgetId::BellIndicator,
            enabled: true,
            section: StatusBarSection::Right,
            order: 4,
            format: None,
        },
        StatusBarWidgetConfig {
            id: WidgetId::Clock,
            enabled: true,
            section: StatusBarSection::Right,
            order: 5,
            format: None,
        },
        StatusBarWidgetConfig {
            id: WidgetId::UpdateAvailable,
            enabled: true,
            section: StatusBarSection::Right,
            order: 6,
            format: None,
        },
```

- [ ] **Step 3: Add the two config fields to `StatusBarConfig`**

In `par-term-config/src/config/config_struct/status_bar_config.rs`, add fields after `status_bar_git_poll_interval`:
```rust
    /// Polling interval in seconds for disk free-space data
    #[serde(default = "default_status_bar_disk_poll_interval")]
    pub status_bar_disk_poll_interval: f32,

    /// Whether the disk widget follows the active tab's CWD (true) or uses the
    /// disk par-term was launched from (false, default).
    #[serde(default = "default_status_bar_disk_follow_cwd")]
    pub status_bar_disk_follow_cwd: bool,
```

Add the default functions next to `default_status_bar_git_poll_interval`:
```rust
fn default_status_bar_disk_poll_interval() -> f32 {
    60.0
}

fn default_status_bar_disk_follow_cwd() -> bool {
    false
}
```

In the `impl Default for StatusBarConfig` block, add the two fields (same expressions as the serde defaults):
```rust
            status_bar_disk_poll_interval: default_status_bar_disk_poll_interval(),
            status_bar_disk_follow_cwd: default_status_bar_disk_follow_cwd(),
```

- [ ] **Step 4: Verify the config crate compiles and tests pass**

Run: `cargo check -p par-term-config && cargo test -p par-term-config`
Expected: PASS — `config_yaml_compat` and widget-deserialize tests still green (the new default fns match the `Default` impl; the new `disk_free` key round-trips).

- [ ] **Step 5: Commit**

```bash
git add par-term-config/src/status_bar.rs par-term-config/src/config/config_struct/status_bar_config.rs
git commit -m "feat(config): add DiskFree widget id and disk status-bar config fields"
```

---

### Task 2: `format_bytes` formatter with TB tier

**Files:**
- Modify: `src/status_bar/system_monitor.rs`

**Interfaces:**
- Produces: `pub fn format_bytes(bytes: u64) -> String` — fixed-width human-readable, tiers B/KB/MB/GB/TB (e.g. `" 250.0 GB"`, `"  1.0 TB"`). Used by the DiskFree widget text (Task 4) and the `system.disk_free`/`system.disk_total` interpolation variables.

- [ ] **Step 1: Write the failing tests**

Append to the `#[cfg(test)] mod tests` block in `src/status_bar/system_monitor.rs`:
```rust
    #[test]
    fn test_format_bytes() {
        // B / KB / MB / GB / TB tiers
        assert_eq!(format_bytes(0), "    0  B");
        assert_eq!(format_bytes(512), "  512  B");
        assert_eq!(format_bytes(1024), "  1.0 KB");
        assert_eq!(format_bytes(1_048_576), "  1.0 MB");
        assert_eq!(format_bytes(1_073_741_824), "  1.0 GB");
        assert_eq!(format_bytes(1_099_511_627_776), "  1.0 TB");
        // 250 GB free (the approved format example)
        assert_eq!(format_bytes(250 * 1_073_741_824), "250.0 GB");
        // Fixed width across magnitudes (so the bar doesn't jump)
        assert_eq!(format_bytes(0).len(), format_bytes(1024).len());
        assert_eq!(format_bytes(1024).len(), format_bytes(1_099_511_627_776).len());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p par-term --lib format_bytes`
Expected: FAIL — `cannot find function format_bytes`.

- [ ] **Step 3: Add `format_bytes` next to `format_memory`**

In the "Formatting helpers" section of `src/status_bar/system_monitor.rs`, add:
```rust
/// Format a byte count into a fixed-width human-readable string.
///
/// Adds a TB tier on top of [`format_memory`]'s helper because disks are
/// large. Output is always 8 characters wide (e.g. `" 250.0 GB"`,
/// `"  1.0 TB"`) so the status bar doesn't jump around when values change.
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * 1024 * 1024;
    const TB: u64 = 1024 * 1024 * 1024 * 1024;

    if bytes >= TB {
        format!("{:>5.1} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:>5.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:>5.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:>5.1} KB", bytes as f64 / KB as f64)
    } else {
        // Extra space before "B" so width matches "KB", "MB", "GB", "TB"
        format!("{:>5}  B", bytes)
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p par-term --lib format_bytes`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/status_bar/system_monitor.rs
git commit -m "feat(status-bar): add format_bytes helper with TB tier"
```

---

### Task 3: `DiskMonitor` poller + `disk_for_path`

**Files:**
- Create: `src/status_bar/disk_monitor.rs`
- Modify: `src/status_bar/mod.rs` (module declaration only — add `pub mod disk_monitor;`)

**Interfaces:**
- Consumes: `sysinfo` (`Disk`, `Disks`, `DiskRefreshKind`) — already a dep; the `system-monitor` feature is already enabled.
- Produces:
  - `pub struct DiskMonitorData { free_bytes: u64, total_bytes: u64, free_percent: f32, last_update: Option<Instant> }`
  - `pub struct DiskMonitor` with `new()`, `start(poll_interval_secs: f32)`, `signal_stop()`, `stop()`, `is_running() -> bool`, `data() -> DiskMonitorData`, `set_cwd(Option<&str>)`, `set_follow_cwd(bool)`.
  - `pub fn disk_for_path<'a>(disks: &'a [Disk], target: &Path) -> Option<&'a Disk>` (feature-gated).

- [ ] **Step 1: Write the failing test for `disk_for_path`**

This is a pure longest-mount-point-prefix matcher; test it against the real disk list (the test machine always has at least a root mount):
```rust
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
    fn test_disk_for_path_longest_prefix() {
        use sysinfo::Disks;
        let disks = Disks::new_with_refreshed_list();
        if disks.list().is_empty() {
            return; // no disks to assert against
        }
        // A deeper path and its parent must resolve to disks whose mount points
        // are prefixes of the path. Deeper should never pick a *shorter* mount
        // than a path that only matches the shorter one.
        let root_disk = disk_for_path(disks.list(), std::path::Path::new("/"));
        assert!(root_disk.is_some(), "'/' must match the root mount");
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p par-term --lib disk_monitor`
Expected: FAIL — module/file not found.

- [ ] **Step 3: Create `src/status_bar/disk_monitor.rs`**

```rust
//! Disk free-space monitor for the status bar.
//!
//! `DiskMonitor` runs a background thread that periodically reports free space
//! on a target disk via `sysinfo`. The target is either the disk containing
//! par-term's launch directory (default) or the disk containing the active
//! tab/pane's working directory (when follow-cwd is enabled).

use std::path::PathBuf;
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
            let launch_dir =
                std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
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

    // NOTE: paste the three feature-gated tests from Step 1 here, inside:
    //   #[cfg(feature = "system-monitor")]
    //   #[test]
    //   fn test_disk_for_path_resolves() { ... }
    //   (and test_disk_for_path_longest_prefix, test_disk_monitor_start_stop)
}
```

Replace the trailing test-module placeholder with the three tests from Step 1 (they must live inside this `mod tests`).

- [ ] **Step 4: Register the module in `src/status_bar/mod.rs`**

In `src/status_bar/mod.rs`, add to the module declarations near the top:
```rust
pub mod git_poller;
pub mod disk_monitor;
pub mod system_monitor;
pub mod widgets;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p par-term --lib disk_monitor && cargo check -p par-term`
Expected: PASS — all three disk tests green; workspace compiles.

- [ ] **Step 6: Commit**

```bash
git add src/status_bar/disk_monitor.rs src/status_bar/mod.rs
git commit -m "feat(status-bar): add DiskMonitor background poller"
```

---

### Task 4: Widget text + `WidgetContext` disk fields + interpolation

**Files:**
- Modify: `src/status_bar/widgets.rs`

**Interfaces:**
- Consumes: `format_bytes` (Task 2), `DiskMonitorData` fields (Task 3).
- Produces: `WidgetContext` gains `disk_free_percent: f32`, `disk_free_bytes: u64`, `disk_total_bytes: u64`; `widget_text` handles `WidgetId::DiskFree`; `resolve_variable` handles `system.disk_free_percent`, `system.disk_free`, `system.disk_total`.

- [ ] **Step 1: Write the failing test for the DiskFree widget**

Add to `src/status_bar/widgets.rs` test module:
```rust
    #[test]
    fn test_widget_text_disk_free() {
        let ctx = make_ctx();
        // make_ctx (Task 4 Step 4) sets disk_free_percent=38.0, free=250GB, total=500GB.
        let text = widget_text(&WidgetId::DiskFree, &ctx, None);
        assert_eq!(text, "DISK  38% (250.0 GB free)");
        // Fixed width across magnitudes
        let mut ctx2 = make_ctx();
        ctx2.disk_free_bytes = 1_099_511_627_776; // 1.0 TB
        ctx2.disk_free_percent = 90.0;
        let text2 = widget_text(&WidgetId::DiskFree, &ctx2, None);
        assert_eq!(text2, "DISK  90% ( 1.0 TB free)");
        assert_eq!(text.len(), text2.len());
    }

    #[test]
    fn test_interpolate_disk_vars() {
        let ctx = make_ctx();
        let result = interpolate_format("free=\\(system.disk_free) pct=\\(system.disk_free_percent)", &ctx);
        assert_eq!(result, "free=250.0 GB pct=38%");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p par-term --lib widget_text`
Expected: FAIL — `WidgetContext` has no `disk_free_*` fields (compile error).

- [ ] **Step 3: Add the fields to `WidgetContext`**

In `src/status_bar/widgets.rs`, extend the `WidgetContext` struct:
```rust
    /// Available update version string (e.g., "0.20.0"), None if up-to-date
    pub update_available_version: Option<String>,
    /// Free-space percentage on the monitored disk (0.0–100.0)
    pub disk_free_percent: f32,
    /// Free bytes on the monitored disk
    pub disk_free_bytes: u64,
    /// Total bytes on the monitored disk
    pub disk_total_bytes: u64,
```

Add the `DiskFree` arm to `widget_text`'s `match id` (after `NetworkStatus`):
```rust
        WidgetId::DiskFree => format!(
            "DISK {:>4.0}% ({}) free",
            ctx.disk_free_percent,
            crate::status_bar::system_monitor::format_bytes(ctx.disk_free_bytes)
        ),
```

Add interpolation variables to `resolve_variable`:
```rust
        "system.memory" => format_memory(ctx.system_data.memory_used, ctx.system_data.memory_total),
        "system.disk_free_percent" => format!("{:.0}%", ctx.disk_free_percent),
        "system.disk_free" => crate::status_bar::system_monitor::format_bytes(ctx.disk_free_bytes),
        "system.disk_total" => crate::status_bar::system_monitor::format_bytes(ctx.disk_total_bytes),
        _ => String::new(),
```

- [ ] **Step 4: Update the test helper `make_ctx` to set disk fields**

In `widgets.rs`'s test module `make_ctx()`, add the three fields to the `WidgetContext { .. }` literal so the new struct is fully populated (it has no `..Default::default()`):
```rust
            time_format: "%H:%M:%S".to_string(),
            update_available_version: None,
            disk_free_percent: 38.0,
            disk_free_bytes: 250 * 1_073_741_824, // 250 GB
            disk_total_bytes: 500 * 1_073_741_824, // 500 GB
        }
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p par-term --lib widgets::`
Expected: PASS — new disk tests green; all existing widget tests still green.

- [ ] **Step 6: Commit**

```bash
git add src/status_bar/widgets.rs
git commit -m "feat(status-bar): render DiskFree widget + interpolation vars"
```

---

### Task 5: Wire `DiskMonitor` into `StatusBarUI`

**Files:**
- Modify: `src/status_bar/mod.rs`

**Interfaces:**
- Consumes: `DiskMonitor` (Task 3), the three new `WidgetContext` fields (Task 4), config fields (Task 1).
- Produces: a running disk poller started/stopped by `sync_monitor_state`, fed the active CWD each frame.

- [ ] **Step 1: Add the field, lifecycle hooks, sync, and render wiring**

In `src/status_bar/mod.rs`:

(a) Add to imports near the top:
```rust
use disk_monitor::DiskMonitor;
```

(b) Add the field to `pub struct StatusBarUI`:
```rust
    /// Background system resource monitor.
    system_monitor: SystemMonitor,
    /// Background disk free-space monitor.
    disk_monitor: DiskMonitor,
    /// Git branch poller.
    git_poller: GitBranchPoller,
```

(c) In `new()`, initialize it:
```rust
            system_monitor: SystemMonitor::new(),
            disk_monitor: DiskMonitor::new(),
            git_poller: GitBranchPoller::new(),
```

(d) In `signal_shutdown()`, add:
```rust
        self.system_monitor.signal_stop();
        self.disk_monitor.signal_stop();
        self.git_poller.signal_stop();
```

(e) In `sync_monitor_state()`, after the system-monitor block and before the git block, add the disk block:
```rust
        // Disk free-space monitor
        let needs_disk = config
            .status_bar
            .status_bar_widgets
            .iter()
            .any(|w| w.enabled && w.id.needs_disk_monitor());

        self.disk_monitor
            .set_follow_cwd(config.status_bar.status_bar_disk_follow_cwd);

        if needs_disk && !self.disk_monitor.is_running() {
            self.disk_monitor
                .start(config.status_bar.status_bar_disk_poll_interval);
        } else if !needs_disk && self.disk_monitor.is_running() {
            self.disk_monitor.stop();
        }
```

(f) In `render()`, right after the existing `self.git_poller.set_cwd(cwd);` line, add:
```rust
        self.disk_monitor.set_cwd(cwd);
```

(g) In the `WidgetContext { .. }` literal in `render()`, add the three disk fields from the monitor snapshot:
```rust
            update_available_version: self.update_available_version.clone(),
            disk_free_percent: self.disk_monitor.data().free_percent,
            disk_free_bytes: self.disk_monitor.data().free_bytes,
            disk_total_bytes: self.disk_monitor.data().total_bytes,
```
(Capture `let disk_data = self.disk_monitor.data();` once before the struct and reuse it to avoid three lock acquisitions — place it next to `let git_status = self.git_poller.status();`.)

(h) `impl Drop for StatusBarUI` already stops `system_monitor`; leave `disk_monitor`'s own `Drop` to stop it (it has one), so no edit needed there. But for symmetry and prompt shutdown, add `self.disk_monitor.stop();` alongside the existing `self.system_monitor.stop();`.

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p par-term`
Expected: PASS.

- [ ] **Step 3: Manual smoke test (optional but recommended)**

Run: `make run` (or `make build`), enable the status bar + the Disk Free widget in Settings, confirm `DISK NN% (… free)` appears and updates.
Expected: widget shows current free space for the launch disk.

- [ ] **Step 4: Commit**

```bash
git add src/status_bar/mod.rs
git commit -m "feat(status-bar): wire DiskMonitor into StatusBarUI"
```

---

### Task 6: Settings UI — disk interval slider + follow-cwd checkbox

**Files:**
- Modify: `par-term-settings-ui/src/status_bar_tab/poll_intervals.rs`
- Modify: `par-term-settings-ui/src/status_bar_tab/mod.rs` (keywords)

**Interfaces:**
- Consumes: `status_bar_disk_poll_interval`, `status_bar_disk_follow_cwd` (Task 1).

- [ ] **Step 1: Add the disk interval slider and follow-cwd checkbox**

In `par-term-settings-ui/src/status_bar_tab/poll_intervals.rs`, inside `collapsing_section(... |ui| { ... })`, append after the git-branch `ui.horizontal` block:
```rust
            ui.horizontal(|ui| {
                ui.label("Disk free space:");
                if ui
                    .add_sized(
                        [SLIDER_WIDTH, SLIDER_HEIGHT],
                        egui::Slider::new(
                            &mut settings.config.status_bar.status_bar_disk_poll_interval,
                            5.0..=600.0,
                        )
                        .suffix(" sec")
                        .show_value(true),
                    )
                    .on_hover_text("How often to poll free disk space (default 60 sec)")
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                if ui
                    .checkbox(
                        &mut settings.config.status_bar.status_bar_disk_follow_cwd,
                        "Follow active tab's directory",
                    )
                    .on_hover_text(
                        "Off (default): show the disk par-term launched from. \
                         On: show the disk containing the active tab/pane's working directory.",
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
```

- [ ] **Step 2: Add search keywords**

In `par-term-settings-ui/src/status_bar_tab/mod.rs`, add `"disk"`, `"free"`, `"space"`, `"storage"` to the Widgets `section_matches` array (the one containing `"cpu"`, `"memory"`, …). Also add `"disk"` to the Poll Intervals `section_matches` array (containing `"poll"`, `"interval"`, …). Then add these to the `keywords()` return array:
```rust
        "disk free",
        "disk space",
        "storage",
```

- [ ] **Step 3: Verify the settings crate compiles**

Run: `cargo check -p par-term-settings-ui`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add par-term-settings-ui/src/status_bar_tab/poll_intervals.rs par-term-settings-ui/src/status_bar_tab/mod.rs
git commit -m "feat(settings-ui): disk poll interval + follow-cwd controls"
```

---

### Task 7: Docs + full verification

**Files:**
- Modify: `docs/features/STATUS_BAR.md`

- [ ] **Step 1: Document the widget, config keys, and variables**

In `docs/features/STATUS_BAR.md`:

(a) Built-in widgets table — add a row after Network Status:
```
| **Disk Free** | `disk_free` | Free disk space as percent + bytes (e.g., "DISK 62% (250 GB free)") | Right | Disabled |
```
Update the intro line "par-term includes 10 built-in widgets" to "11 built-in widgets".

(b) "System Monitoring" section — add Disk Free alongside CPU/Memory/Network as a system widget that runs a background thread, and document:
```yaml
status_bar_disk_poll_interval: 60.0   # 5.0-600.0 sec (default 1 minute)
status_bar_disk_follow_cwd: false     # false = disk par-term launched from; true = active tab's disk
```

(c) Variable Interpolation table — add three rows:
```
| `\(system.disk_free_percent)` | Free disk percentage | `62%` |
| `\(system.disk_free)` | Free disk bytes (human-readable) | `250.0 GB` |
| `\(system.disk_total)` | Total disk bytes (human-readable) | `500.0 GB` |
```

(d) Default Widget Placement Right section — add "Disk (off)" after "Network (off)".

(e) Configuration reference YAML block — add the two keys.

- [ ] **Step 2: Run the full verification gate**

Run: `make checkall`
Expected: PASS — fmt-check, clippy, typecheck, and all tests green. If clippy warns on the new code, fix and re-run.

- [ ] **Step 3: Commit**

```bash
git add docs/features/STATUS_BAR.md
git commit -m "docs(status-bar): document DiskFree widget"
```

- [ ] **Step 4: Final integration smoke**

Run: `make run`, enable Status Bar + Disk Free widget, toggle "Follow active tab's directory" on, `cd` a remote/local tab onto a different mount, and confirm the displayed disk changes on the next poll.

---

## Self-Review

**Spec coverage:** Config fields (Task 1), `WidgetId::DiskFree` + `needs_disk_monitor` + default_widgets (Task 1), `DiskMonitorData`/`DiskMonitor`/`disk_for_path`/launch-dir capture/follow-cwd/fallback (Task 3), `format_bytes` TB tier (Task 2), widget text `DISK NN% (… free)` (Task 4), interpolation vars (Task 4), `StatusBarUI` wiring incl. sync_monitor_state gating + per-frame set_cwd (Task 5), settings UI + keywords (Task 6), docs (Task 7). ✓ All spec sections map to a task.

**Placeholder scan:** No TBD/TODO. The `disk_monitor.rs` test-module comment in Step 3 explicitly tells the implementer to paste the three Step-1 tests in (not leave a placeholder) — verified.

**Type consistency:** `needs_disk_monitor()` (Task 1) consumed in Task 5. `format_bytes(u64) -> String` (Task 2) consumed in Tasks 4. `DiskMonitor::{new,start,signal_stop,stop,is_running,data,set_cwd,set_follow_cwd}` (Task 3) consumed in Task 5; `DiskMonitorData::{free_bytes,total_bytes,free_percent}` consumed in Tasks 4 & 5. `WidgetContext` disk fields (Task 4) match render-site construction (Task 5). All names match across tasks. ✓
