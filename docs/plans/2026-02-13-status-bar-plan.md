# Status Bar Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a configurable status bar with 10 built-in widgets, three-section layout, system monitoring, drag-and-drop settings UI, and auto-hide behavior.

**Architecture:** Render as an egui `TopBottomPanel` (mirroring the tmux status bar pattern in `src/tmux_status_bar_ui.rs`). Widgets read from the active tab's `SessionVariables` (badge infrastructure) and a background `SystemMonitor` thread (sysinfo crate). Viewport offsets stack with existing tab bar/tmux bar offsets.

**Tech Stack:** Rust, egui, sysinfo, chrono (already dep), par-term config system

---

### Task 1: Add sysinfo dependency

**Files:**
- Modify: `Cargo.toml:24-100`

**Step 1: Add sysinfo to Cargo.toml**

Add after the `base64` line (line 100):

```toml
sysinfo = "0.35"  # Cross-platform CPU/memory/network monitoring for status bar
```

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: Compiles without errors

**Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: add sysinfo dependency for status bar system monitoring"
```

---

### Task 2: Create status bar config types

**Files:**
- Create: `src/status_bar/config.rs`
- Modify: `src/config/types.rs`

**Step 1: Add StatusBarPosition enum to config/types.rs**

Add after the `TabBarMode` enum (around line 292):

```rust
/// Status bar position
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum StatusBarPosition {
    /// Status bar at the top of the window
    Top,
    /// Status bar at the bottom of the window (default)
    #[default]
    Bottom,
}
```

**Step 2: Create src/status_bar/config.rs**

```rust
//! Status bar configuration types.

use serde::{Deserialize, Serialize};

/// Which section a widget belongs to in the status bar
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum StatusBarSection {
    #[default]
    Left,
    Center,
    Right,
}

/// Built-in widget identifiers
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WidgetId {
    Clock,
    UsernameHostname,
    CurrentDirectory,
    GitBranch,
    CpuUsage,
    MemoryUsage,
    NetworkStatus,
    BellIndicator,
    CurrentCommand,
    /// Custom text widget with user-defined format
    Custom(String),
}

impl WidgetId {
    /// Display label for settings UI
    pub fn label(&self) -> &str {
        match self {
            Self::Clock => "Clock",
            Self::UsernameHostname => "Username@Hostname",
            Self::CurrentDirectory => "Current Directory",
            Self::GitBranch => "Git Branch",
            Self::CpuUsage => "CPU Usage",
            Self::MemoryUsage => "Memory Usage",
            Self::NetworkStatus => "Network Status",
            Self::BellIndicator => "Bell Indicator",
            Self::CurrentCommand => "Current Command",
            Self::Custom(_) => "Custom Text",
        }
    }

    /// Icon for settings UI
    pub fn icon(&self) -> &str {
        match self {
            Self::Clock => "\u{1f552}",
            Self::UsernameHostname => "\u{1f464}",
            Self::CurrentDirectory => "\u{1f4c1}",
            Self::GitBranch => "\u{1f33f}",
            Self::CpuUsage => "\u{1f4bb}",
            Self::MemoryUsage => "\u{1f4be}",
            Self::NetworkStatus => "\u{1f310}",
            Self::BellIndicator => "\u{1f514}",
            Self::CurrentCommand => "\u{25b6}\u{fe0f}",
            Self::Custom(_) => "\u{270f}\u{fe0f}",
        }
    }

    /// Whether this widget needs the system monitor
    pub fn needs_system_monitor(&self) -> bool {
        matches!(
            self,
            Self::CpuUsage | Self::MemoryUsage | Self::NetworkStatus
        )
    }
}

/// Configuration for a single status bar widget
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatusBarWidgetConfig {
    /// Widget identifier
    pub id: WidgetId,
    /// Whether the widget is enabled
    #[serde(default = "super::defaults::bool_true")]
    pub enabled: bool,
    /// Which section the widget appears in
    #[serde(default)]
    pub section: StatusBarSection,
    /// Order within the section (lower = further left/first)
    #[serde(default)]
    pub order: usize,
    /// Optional format string override (for custom text widgets)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
}

/// Default widget configuration
pub fn default_widgets() -> Vec<StatusBarWidgetConfig> {
    vec![
        StatusBarWidgetConfig {
            id: WidgetId::UsernameHostname,
            enabled: true,
            section: StatusBarSection::Left,
            order: 0,
            format: None,
        },
        StatusBarWidgetConfig {
            id: WidgetId::CurrentDirectory,
            enabled: true,
            section: StatusBarSection::Left,
            order: 1,
            format: None,
        },
        StatusBarWidgetConfig {
            id: WidgetId::GitBranch,
            enabled: true,
            section: StatusBarSection::Left,
            order: 2,
            format: None,
        },
        StatusBarWidgetConfig {
            id: WidgetId::CurrentCommand,
            enabled: true,
            section: StatusBarSection::Center,
            order: 0,
            format: None,
        },
        StatusBarWidgetConfig {
            id: WidgetId::CpuUsage,
            enabled: true,
            section: StatusBarSection::Right,
            order: 0,
            format: None,
        },
        StatusBarWidgetConfig {
            id: WidgetId::MemoryUsage,
            enabled: true,
            section: StatusBarSection::Right,
            order: 1,
            format: None,
        },
        StatusBarWidgetConfig {
            id: WidgetId::NetworkStatus,
            enabled: true,
            section: StatusBarSection::Right,
            order: 2,
            format: None,
        },
        StatusBarWidgetConfig {
            id: WidgetId::BellIndicator,
            enabled: false,
            section: StatusBarSection::Right,
            order: 3,
            format: None,
        },
        StatusBarWidgetConfig {
            id: WidgetId::Clock,
            enabled: true,
            section: StatusBarSection::Right,
            order: 4,
            format: None,
        },
    ]
}
```

**Step 3: Verify it compiles**

Run: `cargo check`
Expected: Compiles (module not yet wired in, will be wired in Task 3)

**Step 4: Commit**

```bash
git add src/status_bar/config.rs src/config/types.rs
git commit -m "feat(status-bar): add config types for widgets, sections, and positions"
```

---

### Task 3: Add status bar config fields to Config struct

**Files:**
- Modify: `src/config/mod.rs`
- Modify: `src/config/defaults.rs`
- Create: `src/status_bar/mod.rs` (minimal, just re-exports config)

**Step 1: Add default functions to src/config/defaults.rs**

Add at the end of the file, before the closing (look for the last function):

```rust
// ============================================================================
// Status Bar Defaults
// ============================================================================

pub fn status_bar_height() -> f32 {
    22.0
}

pub fn status_bar_bg_color() -> [u8; 3] {
    [30, 30, 30]
}

pub fn status_bar_bg_alpha() -> f32 {
    0.95
}

pub fn status_bar_fg_color() -> [u8; 3] {
    [200, 200, 200]
}

pub fn status_bar_font_size() -> f32 {
    12.0
}

pub fn status_bar_separator() -> String {
    " \u{2502} ".to_string()
}

pub fn status_bar_mouse_inactive_timeout() -> f32 {
    3.0
}

pub fn status_bar_system_poll_interval() -> f32 {
    2.0
}

pub fn status_bar_git_poll_interval() -> f32 {
    5.0
}
```

**Step 2: Add config fields to Config struct in src/config/mod.rs**

Add a new section after the badge config fields (after line ~1527). Follow the exact pattern of the badge fields:

```rust
    // ========================================================================
    // Status Bar Settings
    // ========================================================================
    /// Enable status bar display
    #[serde(default = "defaults::bool_false")]
    pub status_bar_enabled: bool,

    /// Status bar position (top or bottom)
    #[serde(default)]
    pub status_bar_position: crate::config::types::StatusBarPosition,

    /// Status bar height in logical pixels
    #[serde(default = "defaults::status_bar_height")]
    pub status_bar_height: f32,

    /// Status bar background color [R, G, B] (0-255)
    #[serde(default = "defaults::status_bar_bg_color")]
    pub status_bar_bg_color: [u8; 3],

    /// Status bar background opacity (0.0-1.0)
    #[serde(default = "defaults::status_bar_bg_alpha")]
    pub status_bar_bg_alpha: f32,

    /// Status bar foreground/text color [R, G, B] (0-255)
    #[serde(default = "defaults::status_bar_fg_color")]
    pub status_bar_fg_color: [u8; 3],

    /// Status bar font family (empty = use terminal font)
    #[serde(default)]
    pub status_bar_font: String,

    /// Status bar font size
    #[serde(default = "defaults::status_bar_font_size")]
    pub status_bar_font_size: f32,

    /// Separator string between widgets
    #[serde(default = "defaults::status_bar_separator")]
    pub status_bar_separator: String,

    /// Auto-hide status bar when window is fullscreen
    #[serde(default = "defaults::bool_true")]
    pub status_bar_auto_hide_fullscreen: bool,

    /// Auto-hide status bar after mouse inactivity
    #[serde(default = "defaults::bool_false")]
    pub status_bar_auto_hide_mouse_inactive: bool,

    /// Seconds of mouse inactivity before auto-hiding status bar
    #[serde(default = "defaults::status_bar_mouse_inactive_timeout")]
    pub status_bar_mouse_inactive_timeout: f32,

    /// System monitoring poll interval in seconds (CPU, memory, network)
    #[serde(default = "defaults::status_bar_system_poll_interval")]
    pub status_bar_system_poll_interval: f32,

    /// Git branch poll interval in seconds
    #[serde(default = "defaults::status_bar_git_poll_interval")]
    pub status_bar_git_poll_interval: f32,

    /// Widget configurations
    #[serde(default = "crate::status_bar::config::default_widgets")]
    pub status_bar_widgets: Vec<crate::status_bar::config::StatusBarWidgetConfig>,
```

**Step 3: Add fields to the Default impl for Config**

In the `impl Default for Config` block (around line 1604+), add:

```rust
            status_bar_enabled: defaults::bool_false(),
            status_bar_position: Default::default(),
            status_bar_height: defaults::status_bar_height(),
            status_bar_bg_color: defaults::status_bar_bg_color(),
            status_bar_bg_alpha: defaults::status_bar_bg_alpha(),
            status_bar_fg_color: defaults::status_bar_fg_color(),
            status_bar_font: String::new(),
            status_bar_font_size: defaults::status_bar_font_size(),
            status_bar_separator: defaults::status_bar_separator(),
            status_bar_auto_hide_fullscreen: defaults::bool_true(),
            status_bar_auto_hide_mouse_inactive: defaults::bool_false(),
            status_bar_mouse_inactive_timeout: defaults::status_bar_mouse_inactive_timeout(),
            status_bar_system_poll_interval: defaults::status_bar_system_poll_interval(),
            status_bar_git_poll_interval: defaults::status_bar_git_poll_interval(),
            status_bar_widgets: crate::status_bar::config::default_widgets(),
```

**Step 4: Create src/status_bar/mod.rs (minimal)**

```rust
//! Status bar system for par-term.
//!
//! Provides a configurable status bar with widgets for displaying
//! session info, system stats, and custom text.

pub mod config;
```

**Step 5: Register module in src/lib.rs**

Add `pub mod status_bar;` in alphabetical order (after `pub mod snippets;`, around line 59).

**Step 6: Verify it compiles**

Run: `cargo check`
Expected: Compiles without errors

**Step 7: Commit**

```bash
git add src/status_bar/mod.rs src/config/mod.rs src/config/defaults.rs src/config/types.rs src/lib.rs
git commit -m "feat(status-bar): add config fields and default values"
```

---

### Task 4: Create the SystemMonitor

**Files:**
- Create: `src/status_bar/system_monitor.rs`

**Step 1: Write tests for SystemMonitor**

Add at the bottom of the new file:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_monitor_data_default() {
        let data = SystemMonitorData::default();
        assert_eq!(data.cpu_usage, 0.0);
        assert_eq!(data.memory_used, 0);
        assert_eq!(data.memory_total, 0);
        assert_eq!(data.network_rx_rate, 0);
        assert_eq!(data.network_tx_rate, 0);
    }

    #[test]
    fn test_format_bytes_per_sec() {
        assert_eq!(format_bytes_per_sec(0), "0 B/s");
        assert_eq!(format_bytes_per_sec(512), "512 B/s");
        assert_eq!(format_bytes_per_sec(1024), "1.0 KB/s");
        assert_eq!(format_bytes_per_sec(1536), "1.5 KB/s");
        assert_eq!(format_bytes_per_sec(1048576), "1.0 MB/s");
        assert_eq!(format_bytes_per_sec(1073741824), "1.0 GB/s");
    }

    #[test]
    fn test_format_memory() {
        assert_eq!(format_memory(0, 0), "0 B / 0 B");
        assert_eq!(format_memory(1073741824, 8589934592), "1.0 GB / 8.0 GB");
        assert_eq!(format_memory(536870912, 1073741824), "512.0 MB / 1.0 GB");
    }

    #[test]
    fn test_system_monitor_start_stop() {
        let monitor = SystemMonitor::new();
        monitor.start(2.0);
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert!(monitor.is_running());
        let data = monitor.data();
        // After starting, data should have valid memory_total (> 0 on any real system)
        assert!(data.memory_total > 0);
        monitor.stop();
        assert!(!monitor.is_running());
    }
}
```

**Step 2: Implement SystemMonitor**

```rust
//! System monitoring for status bar widgets.
//!
//! Runs a background thread that polls CPU, memory, and network stats
//! using the `sysinfo` crate. Data is shared via `Arc<Mutex<>>`.

use parking_lot::Mutex;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, Networks, RefreshKind, System};

/// System monitoring data snapshot
#[derive(Debug, Clone, Default)]
pub struct SystemMonitorData {
    /// CPU usage as a percentage (0.0 - 100.0)
    pub cpu_usage: f32,
    /// Memory used in bytes
    pub memory_used: u64,
    /// Total memory in bytes
    pub memory_total: u64,
    /// Network receive rate in bytes/sec
    pub network_rx_rate: u64,
    /// Network transmit rate in bytes/sec
    pub network_tx_rate: u64,
    /// When this data was last updated
    pub last_update: Option<Instant>,
}

/// Background system monitor
pub struct SystemMonitor {
    data: Arc<Mutex<SystemMonitorData>>,
    running: Arc<AtomicBool>,
    thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl SystemMonitor {
    pub fn new() -> Self {
        Self {
            data: Arc::new(Mutex::new(SystemMonitorData::default())),
            running: Arc::new(AtomicBool::new(false)),
            thread: Mutex::new(None),
        }
    }

    /// Start the monitoring thread with the given poll interval in seconds
    pub fn start(&self, poll_interval_secs: f32) {
        if self.running.load(Ordering::Relaxed) {
            return;
        }
        self.running.store(true, Ordering::Relaxed);

        let data = Arc::clone(&self.data);
        let running = Arc::clone(&self.running);
        let interval = Duration::from_secs_f32(poll_interval_secs.max(0.5));

        let handle = std::thread::Builder::new()
            .name("status-bar-monitor".into())
            .spawn(move || {
                let mut sys = System::new_with_specifics(
                    RefreshKind::nothing()
                        .with_cpu(CpuRefreshKind::everything())
                        .with_memory(MemoryRefreshKind::everything()),
                );
                let mut networks = Networks::new_with_refreshed_list();
                let mut prev_rx: u64 = 0;
                let mut prev_tx: u64 = 0;
                let mut first_poll = true;

                // Initial CPU poll (sysinfo needs two polls for accurate CPU)
                sys.refresh_cpu_all();
                std::thread::sleep(Duration::from_millis(200));

                while running.load(Ordering::Relaxed) {
                    sys.refresh_cpu_all();
                    sys.refresh_memory();
                    networks.refresh(true);

                    let cpu_usage = sys.global_cpu_usage();
                    let memory_used = sys.used_memory();
                    let memory_total = sys.total_memory();

                    // Calculate network rates
                    let mut total_rx: u64 = 0;
                    let mut total_tx: u64 = 0;
                    for (_name, net) in networks.iter() {
                        total_rx += net.total_received();
                        total_tx += net.total_transmitted();
                    }

                    let (rx_rate, tx_rate) = if first_poll {
                        first_poll = false;
                        (0, 0)
                    } else {
                        let rx_delta = total_rx.saturating_sub(prev_rx);
                        let tx_delta = total_tx.saturating_sub(prev_tx);
                        let secs = interval.as_secs_f64();
                        (
                            (rx_delta as f64 / secs) as u64,
                            (tx_delta as f64 / secs) as u64,
                        )
                    };
                    prev_rx = total_rx;
                    prev_tx = total_tx;

                    {
                        let mut d = data.lock();
                        d.cpu_usage = cpu_usage;
                        d.memory_used = memory_used;
                        d.memory_total = memory_total;
                        d.network_rx_rate = rx_rate;
                        d.network_tx_rate = tx_rate;
                        d.last_update = Some(Instant::now());
                    }

                    std::thread::sleep(interval);
                }
            })
            .expect("Failed to spawn system monitor thread");

        *self.thread.lock() = Some(handle);
    }

    /// Stop the monitoring thread
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.thread.lock().take() {
            let _ = handle.join();
        }
    }

    /// Check if the monitor is currently running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Get a snapshot of the current data
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

/// Format bytes per second for display
pub fn format_bytes_per_sec(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B/s")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB/s", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB/s", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB/s", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

/// Format memory usage for display
pub fn format_memory(used: u64, total: u64) -> String {
    fn fmt(bytes: u64) -> String {
        if bytes == 0 {
            "0 B".to_string()
        } else if bytes < 1024 * 1024 {
            format!("{:.1} KB", bytes as f64 / 1024.0)
        } else if bytes < 1024 * 1024 * 1024 {
            format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
        }
    }
    format!("{} / {}", fmt(used), fmt(total))
}
```

**Step 3: Add module to status_bar/mod.rs**

Add `pub mod system_monitor;` to `src/status_bar/mod.rs`.

**Step 4: Run the tests**

Run: `cargo test system_monitor -- --include-ignored`
Expected: All 4 tests pass

**Step 5: Commit**

```bash
git add src/status_bar/system_monitor.rs src/status_bar/mod.rs
git commit -m "feat(status-bar): add system monitor with CPU/memory/network polling"
```

---

### Task 5: Create the widget rendering system

**Files:**
- Create: `src/status_bar/widgets.rs`

**Step 1: Write tests**

Add at the bottom of the new file:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::badge::SessionVariables;
    use crate::status_bar::system_monitor::SystemMonitorData;

    fn make_context() -> WidgetContext {
        let mut vars = SessionVariables::default();
        vars.hostname = "myhost".to_string();
        vars.username = "user".to_string();
        vars.path = "/home/user/project".to_string();
        vars.bell_count = 3;
        vars.current_command = Some("cargo build".to_string());

        WidgetContext {
            session_vars: vars,
            system_data: SystemMonitorData {
                cpu_usage: 42.5,
                memory_used: 4 * 1024 * 1024 * 1024,
                memory_total: 16 * 1024 * 1024 * 1024,
                network_rx_rate: 1024 * 100,
                network_tx_rate: 1024 * 50,
                last_update: Some(std::time::Instant::now()),
            },
            git_branch: Some("main".to_string()),
        }
    }

    #[test]
    fn test_widget_text_username_hostname() {
        let ctx = make_context();
        assert_eq!(widget_text(&WidgetId::UsernameHostname, &ctx, None), "user@myhost");
    }

    #[test]
    fn test_widget_text_current_directory() {
        let ctx = make_context();
        assert_eq!(widget_text(&WidgetId::CurrentDirectory, &ctx, None), "/home/user/project");
    }

    #[test]
    fn test_widget_text_git_branch() {
        let ctx = make_context();
        assert_eq!(widget_text(&WidgetId::GitBranch, &ctx, None), "\u{e0a0} main");
    }

    #[test]
    fn test_widget_text_git_branch_none() {
        let mut ctx = make_context();
        ctx.git_branch = None;
        assert_eq!(widget_text(&WidgetId::GitBranch, &ctx, None), "");
    }

    #[test]
    fn test_widget_text_cpu() {
        let ctx = make_context();
        assert_eq!(widget_text(&WidgetId::CpuUsage, &ctx, None), "CPU 42.5%");
    }

    #[test]
    fn test_widget_text_memory() {
        let ctx = make_context();
        let text = widget_text(&WidgetId::MemoryUsage, &ctx, None);
        assert_eq!(text, "MEM 4.0 GB / 16.0 GB");
    }

    #[test]
    fn test_widget_text_bell_indicator() {
        let ctx = make_context();
        assert_eq!(widget_text(&WidgetId::BellIndicator, &ctx, None), "\u{1f514} 3");
    }

    #[test]
    fn test_widget_text_bell_indicator_zero() {
        let mut ctx = make_context();
        ctx.session_vars.bell_count = 0;
        assert_eq!(widget_text(&WidgetId::BellIndicator, &ctx, None), "");
    }

    #[test]
    fn test_widget_text_current_command() {
        let ctx = make_context();
        assert_eq!(widget_text(&WidgetId::CurrentCommand, &ctx, None), "cargo build");
    }

    #[test]
    fn test_widget_text_current_command_none() {
        let mut ctx = make_context();
        ctx.session_vars.current_command = None;
        assert_eq!(widget_text(&WidgetId::CurrentCommand, &ctx, None), "");
    }

    #[test]
    fn test_widget_text_network() {
        let ctx = make_context();
        let text = widget_text(&WidgetId::NetworkStatus, &ctx, None);
        assert!(text.contains("\u{2191}")); // up arrow
        assert!(text.contains("\u{2193}")); // down arrow
    }

    #[test]
    fn test_sorted_widgets_for_section() {
        let widgets = crate::status_bar::config::default_widgets();
        let left = sorted_widgets_for_section(&widgets, StatusBarSection::Left);
        assert_eq!(left.len(), 3);
        assert_eq!(left[0].id, WidgetId::UsernameHostname);
        assert_eq!(left[1].id, WidgetId::CurrentDirectory);
        assert_eq!(left[2].id, WidgetId::GitBranch);

        let center = sorted_widgets_for_section(&widgets, StatusBarSection::Center);
        assert_eq!(center.len(), 1);
        assert_eq!(center[0].id, WidgetId::CurrentCommand);

        let right = sorted_widgets_for_section(&widgets, StatusBarSection::Right);
        assert_eq!(right.len(), 5);
        assert_eq!(right[4].id, WidgetId::Clock);
    }
}
```

**Step 2: Implement widgets.rs**

```rust
//! Status bar widget rendering.
//!
//! Each widget produces text output based on session variables,
//! system monitor data, or other sources.

use crate::badge::SessionVariables;
use crate::status_bar::config::{StatusBarSection, StatusBarWidgetConfig, WidgetId};
use crate::status_bar::system_monitor::{SystemMonitorData, format_bytes_per_sec, format_memory};

/// Context passed to widgets for rendering
#[derive(Debug, Clone)]
pub struct WidgetContext {
    /// Session variables for the active tab
    pub session_vars: SessionVariables,
    /// System monitoring data
    pub system_data: SystemMonitorData,
    /// Current git branch (polled separately)
    pub git_branch: Option<String>,
}

/// Get the display text for a widget
pub fn widget_text(
    id: &WidgetId,
    ctx: &WidgetContext,
    format_override: Option<&str>,
) -> String {
    if let Some(fmt) = format_override {
        return interpolate_format(fmt, ctx);
    }

    match id {
        WidgetId::Clock => chrono::Local::now().format("%H:%M:%S").to_string(),
        WidgetId::UsernameHostname => {
            if ctx.session_vars.username.is_empty() && ctx.session_vars.hostname.is_empty() {
                String::new()
            } else {
                format!("{}@{}", ctx.session_vars.username, ctx.session_vars.hostname)
            }
        }
        WidgetId::CurrentDirectory => ctx.session_vars.path.clone(),
        WidgetId::GitBranch => {
            if let Some(ref branch) = ctx.git_branch {
                format!("\u{e0a0} {branch}")
            } else {
                String::new()
            }
        }
        WidgetId::CpuUsage => format!("CPU {:.1}%", ctx.system_data.cpu_usage),
        WidgetId::MemoryUsage => {
            format!(
                "MEM {}",
                format_memory(ctx.system_data.memory_used, ctx.system_data.memory_total)
            )
        }
        WidgetId::NetworkStatus => {
            format!(
                "\u{2193} {} \u{2191} {}",
                format_bytes_per_sec(ctx.system_data.network_rx_rate),
                format_bytes_per_sec(ctx.system_data.network_tx_rate)
            )
        }
        WidgetId::BellIndicator => {
            if ctx.session_vars.bell_count > 0 {
                format!("\u{1f514} {}", ctx.session_vars.bell_count)
            } else {
                String::new()
            }
        }
        WidgetId::CurrentCommand => ctx
            .session_vars
            .current_command
            .clone()
            .unwrap_or_default(),
        WidgetId::Custom(_) => String::new(),
    }
}

/// Interpolate a format string with session variables
fn interpolate_format(fmt: &str, ctx: &WidgetContext) -> String {
    // Reuse badge variable interpolation pattern
    let mut result = fmt.to_string();
    result = result.replace("\\(session.hostname)", &ctx.session_vars.hostname);
    result = result.replace("\\(session.username)", &ctx.session_vars.username);
    result = result.replace("\\(session.path)", &ctx.session_vars.path);
    result = result.replace(
        "\\(session.current_command)",
        ctx.session_vars.current_command.as_deref().unwrap_or(""),
    );
    result = result.replace(
        "\\(session.bell_count)",
        &ctx.session_vars.bell_count.to_string(),
    );
    result = result.replace(
        "\\(session.exit_code)",
        &ctx.session_vars
            .exit_code
            .map(|c| c.to_string())
            .unwrap_or_default(),
    );
    if let Some(ref branch) = ctx.git_branch {
        result = result.replace("\\(git.branch)", branch);
    } else {
        result = result.replace("\\(git.branch)", "");
    }
    result = result.replace("\\(system.cpu)", &format!("{:.1}%", ctx.system_data.cpu_usage));
    result = result.replace(
        "\\(system.memory)",
        &format_memory(ctx.system_data.memory_used, ctx.system_data.memory_total),
    );
    result
}

/// Filter and sort widgets for a given section
pub fn sorted_widgets_for_section(
    widgets: &[StatusBarWidgetConfig],
    section: StatusBarSection,
) -> Vec<&StatusBarWidgetConfig> {
    let mut filtered: Vec<_> = widgets
        .iter()
        .filter(|w| w.enabled && w.section == section)
        .collect();
    filtered.sort_by_key(|w| w.order);
    filtered
}
```

**Step 3: Add module to status_bar/mod.rs**

Add `pub mod widgets;` to `src/status_bar/mod.rs`.

**Step 4: Run the tests**

Run: `cargo test status_bar::widgets -- -v`
Expected: All widget tests pass

**Step 5: Commit**

```bash
git add src/status_bar/widgets.rs src/status_bar/mod.rs
git commit -m "feat(status-bar): add widget text rendering with interpolation"
```

---

### Task 6: Create the StatusBarUI renderer

**Files:**
- Modify: `src/status_bar/mod.rs` (expand with StatusBarUI)

**Step 1: Implement StatusBarUI**

Replace the minimal `src/status_bar/mod.rs` with:

```rust
//! Status bar system for par-term.
//!
//! Provides a configurable status bar with widgets for displaying
//! session info, system stats, and custom text.

pub mod config;
pub mod system_monitor;
pub mod widgets;

use crate::badge::SessionVariables;
use crate::config::Config;
use config::{StatusBarSection, StatusBarWidgetConfig};
use system_monitor::{SystemMonitor, SystemMonitorData};
use widgets::{WidgetContext, sorted_widgets_for_section, widget_text};

use std::process::Command;
use std::time::Instant;

/// Git branch polling state
struct GitBranchPoller {
    /// Last polled branch name
    branch: Option<String>,
    /// Last poll time
    last_poll: Instant,
    /// Working directory to poll from
    cwd: Option<String>,
}

impl Default for GitBranchPoller {
    fn default() -> Self {
        Self {
            branch: None,
            last_poll: Instant::now(),
            cwd: None,
        }
    }
}

/// Status bar UI state and renderer
pub struct StatusBarUI {
    /// System monitor for CPU/memory/network
    system_monitor: SystemMonitor,
    /// Git branch polling state
    git_poller: GitBranchPoller,
    /// Last mouse activity time (for auto-hide)
    last_mouse_activity: Instant,
    /// Whether the bar is currently visible (for auto-hide animation)
    visible: bool,
}

impl StatusBarUI {
    pub fn new() -> Self {
        Self {
            system_monitor: SystemMonitor::new(),
            git_poller: GitBranchPoller::default(),
            last_mouse_activity: Instant::now(),
            visible: true,
        }
    }

    /// Get the height of the status bar (0 if not visible)
    pub fn height(&self, config: &Config, is_fullscreen: bool) -> f32 {
        if !config.status_bar_enabled {
            return 0.0;
        }
        if self.should_hide(config, is_fullscreen) {
            return 0.0;
        }
        config.status_bar_height
    }

    /// Check if the status bar should be hidden
    fn should_hide(&self, config: &Config, is_fullscreen: bool) -> bool {
        if config.status_bar_auto_hide_fullscreen && is_fullscreen {
            return true;
        }
        if config.status_bar_auto_hide_mouse_inactive {
            let elapsed = self.last_mouse_activity.elapsed().as_secs_f32();
            if elapsed > config.status_bar_mouse_inactive_timeout {
                return true;
            }
        }
        false
    }

    /// Notify that mouse activity occurred (for auto-hide)
    pub fn on_mouse_activity(&mut self) {
        self.last_mouse_activity = Instant::now();
    }

    /// Ensure system monitor is running/stopped based on config
    pub fn sync_monitor_state(&self, config: &Config) {
        let needs_monitor = config.status_bar_enabled
            && config
                .status_bar_widgets
                .iter()
                .any(|w| w.enabled && w.id.needs_system_monitor());

        if needs_monitor && !self.system_monitor.is_running() {
            self.system_monitor.start(config.status_bar_system_poll_interval);
        } else if !needs_monitor && self.system_monitor.is_running() {
            self.system_monitor.stop();
        }
    }

    /// Poll git branch if interval has elapsed
    fn poll_git_branch(&mut self, config: &Config, cwd: Option<&str>) {
        let git_enabled = config
            .status_bar_widgets
            .iter()
            .any(|w| w.enabled && w.id == config::WidgetId::GitBranch);

        if !git_enabled {
            self.git_poller.branch = None;
            return;
        }

        let elapsed = self.git_poller.last_poll.elapsed().as_secs_f32();
        let cwd_changed = self.git_poller.cwd.as_deref() != cwd;

        if elapsed < config.status_bar_git_poll_interval && !cwd_changed {
            return;
        }

        self.git_poller.last_poll = Instant::now();
        self.git_poller.cwd = cwd.map(|s| s.to_string());

        self.git_poller.branch = cwd
            .and_then(|dir| {
                Command::new("git")
                    .args(["rev-parse", "--abbrev-ref", "HEAD"])
                    .current_dir(dir)
                    .output()
                    .ok()
            })
            .and_then(|output| {
                if output.status.success() {
                    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
                } else {
                    None
                }
            });
    }

    /// Render the status bar. Returns the height consumed.
    pub fn render(
        &mut self,
        ctx: &egui::Context,
        config: &Config,
        session_vars: &SessionVariables,
        is_fullscreen: bool,
    ) -> f32 {
        if !config.status_bar_enabled {
            return 0.0;
        }

        if self.should_hide(config, is_fullscreen) {
            return 0.0;
        }

        // Poll git branch
        let cwd = if session_vars.path.is_empty() {
            None
        } else {
            Some(session_vars.path.as_str())
        };
        self.poll_git_branch(config, cwd);

        // Build widget context
        let widget_ctx = WidgetContext {
            session_vars: session_vars.clone(),
            system_data: if self.system_monitor.is_running() {
                self.system_monitor.data()
            } else {
                SystemMonitorData::default()
            },
            git_branch: self.git_poller.branch.clone(),
        };

        let bar_height = config.status_bar_height;
        let bg_color = egui::Color32::from_rgba_unmultiplied(
            config.status_bar_bg_color[0],
            config.status_bar_bg_color[1],
            config.status_bar_bg_color[2],
            (config.status_bar_bg_alpha * 255.0) as u8,
        );
        let fg_color = egui::Color32::from_rgb(
            config.status_bar_fg_color[0],
            config.status_bar_fg_color[1],
            config.status_bar_fg_color[2],
        );
        let font_size = config.status_bar_font_size;
        let separator = &config.status_bar_separator;

        // Create panel at top or bottom
        use crate::config::types::StatusBarPosition;
        let panel = match config.status_bar_position {
            StatusBarPosition::Top => {
                egui::TopBottomPanel::top("status_bar").exact_height(bar_height)
            }
            StatusBarPosition::Bottom => {
                egui::TopBottomPanel::bottom("status_bar").exact_height(bar_height)
            }
        };

        panel
            .frame(
                egui::Frame::NONE
                    .fill(bg_color)
                    .inner_margin(egui::Margin::symmetric(8, 2)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Left section
                    self.render_section(
                        ui,
                        &config.status_bar_widgets,
                        StatusBarSection::Left,
                        &widget_ctx,
                        fg_color,
                        font_size,
                        separator,
                    );

                    // Center section (use remaining space)
                    ui.with_layout(
                        egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                        |ui| {
                            self.render_section(
                                ui,
                                &config.status_bar_widgets,
                                StatusBarSection::Center,
                                &widget_ctx,
                                fg_color,
                                font_size,
                                separator,
                            );
                        },
                    );

                    // Right section
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            // Render right widgets in reverse order (right_to_left layout)
                            let widgets =
                                sorted_widgets_for_section(&config.status_bar_widgets, StatusBarSection::Right);
                            for (i, w) in widgets.iter().rev().enumerate() {
                                let text = widget_text(&w.id, &widget_ctx, w.format.as_deref());
                                if text.is_empty() {
                                    continue;
                                }
                                if i > 0 {
                                    ui.label(
                                        egui::RichText::new(separator)
                                            .color(fg_color.linear_multiply(0.4))
                                            .size(font_size)
                                            .monospace(),
                                    );
                                }
                                ui.label(
                                    egui::RichText::new(&text)
                                        .color(fg_color)
                                        .size(font_size)
                                        .monospace(),
                                );
                            }
                        },
                    );
                });
            });

        bar_height
    }

    /// Render widgets for a section
    fn render_section(
        &self,
        ui: &mut egui::Ui,
        widgets: &[StatusBarWidgetConfig],
        section: StatusBarSection,
        ctx: &WidgetContext,
        fg_color: egui::Color32,
        font_size: f32,
        separator: &str,
    ) {
        let section_widgets = sorted_widgets_for_section(widgets, section);
        let mut rendered_count = 0;
        for w in &section_widgets {
            let text = widget_text(&w.id, ctx, w.format.as_deref());
            if text.is_empty() {
                continue;
            }
            if rendered_count > 0 {
                ui.label(
                    egui::RichText::new(separator)
                        .color(fg_color.linear_multiply(0.4))
                        .size(font_size)
                        .monospace(),
                );
            }
            ui.label(
                egui::RichText::new(&text)
                    .color(fg_color)
                    .size(font_size)
                    .monospace(),
            );
            rendered_count += 1;
        }
    }
}

impl Default for StatusBarUI {
    fn default() -> Self {
        Self::new()
    }
}
```

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: Compiles without errors

**Step 3: Run all status bar tests**

Run: `cargo test status_bar -- -v`
Expected: All tests pass

**Step 4: Commit**

```bash
git add src/status_bar/mod.rs
git commit -m "feat(status-bar): add StatusBarUI renderer with three-section layout"
```

---

### Task 7: Integrate StatusBarUI into the window state

**Files:**
- Modify: `src/app/window_state.rs`

This is the critical integration task. Follow the existing tmux status bar pattern exactly.

**Step 1: Add import and field**

At the top of `window_state.rs`, add the import (near line 40, after the tmux imports):

```rust
use crate::status_bar::StatusBarUI;
```

Add field to `WindowState` struct (after `tmux_status_bar_ui` field, around line 87):

```rust
    /// Status bar UI
    pub(crate) status_bar_ui: StatusBarUI,
```

**Step 2: Initialize in constructor**

In the `WindowState::new()` or equivalent init function (around line 297, after `tmux_status_bar_ui: TmuxStatusBarUI::new()`):

```rust
            status_bar_ui: StatusBarUI::new(),
```

**Step 3: Calculate status bar height before mutable borrow**

Around line 1789, after the tmux status bar height calculation, add:

```rust
        // Calculate custom status bar height
        let custom_status_bar_height =
            self.status_bar_ui.height(&self.config, self.is_fullscreen);
```

**Step 4: Add to RendererSizing calculation**

The existing `status_bar_height` field in `RendererSizing` (line 58) currently only accounts for the tmux status bar. Update the calculation where `RendererSizing` is constructed (around line 2356-2365) to include both:

Find the line:
```rust
                status_bar_height: status_bar_height * renderer.scale_factor(),
```

Replace with:
```rust
                status_bar_height: (status_bar_height + custom_status_bar_height) * renderer.scale_factor(),
```

**Step 5: Capture session variables for status bar**

Around line 1973 (where badge state is captured), add:

```rust
            // Capture session variables for status bar
            let status_bar_session_vars = if self.config.status_bar_enabled {
                self.tab_manager.active_tab().and_then(|tab| {
                    tab.terminal
                        .try_lock()
                        .ok()
                        .map(|_term| tab.badge_state.session_vars().clone())
                })
            } else {
                None
            };
```

Note: Check where `badge_state` session variables are stored. If `SessionVariables` is on the `BadgeState`, access it from there. If it's on the tab directly, adjust accordingly. The exploration showed `BadgeState` has session variable data.

**Step 6: Render status bar in egui context**

After the tmux status bar render call (around line 2222), add:

```rust
                    // Render custom status bar
                    if let Some(ref session_vars) = status_bar_session_vars {
                        self.status_bar_ui.render(
                            ctx,
                            &self.config,
                            session_vars,
                            self.is_fullscreen,
                        );
                    }
```

**Step 7: Sync monitor state on config changes**

Find where config is applied/updated (settings save handler). Add:

```rust
self.status_bar_ui.sync_monitor_state(&self.config);
```

Also call it during initialization after config is loaded.

**Step 8: Add mouse activity forwarding**

In the mouse event handler (`src/app/mouse_events.rs`), when mouse motion is detected, add:

```rust
self.status_bar_ui.on_mouse_activity();
```

**Step 9: Apply viewport offset for status bar position**

The status bar needs to affect viewport like the tab bar does. In the section where `apply_tab_bar_offsets` is called, the status bar offset must also be applied.

Find where tab bar offsets are applied and add status bar offset handling:

```rust
        // Apply status bar offsets
        if custom_status_bar_height > 0.0 {
            use crate::config::types::StatusBarPosition;
            match self.config.status_bar_position {
                StatusBarPosition::Top => {
                    // Add to top offset (stack with tab bar if also at top)
                    let current_y = renderer.content_offset_y();
                    renderer.set_content_offset_y(current_y + custom_status_bar_height);
                }
                StatusBarPosition::Bottom => {
                    // Add to bottom inset (stack with tab bar if also at bottom)
                    let current_bottom = renderer.content_inset_bottom();
                    renderer.set_content_inset_bottom(current_bottom + custom_status_bar_height);
                }
            }
        }
```

**IMPORTANT:** This must happen AFTER `apply_tab_bar_offsets()` so offsets stack correctly, and AFTER the tmux status bar height is accounted for. The tmux status bar height is already included in `RendererSizing.status_bar_height` for pane calculations, but the renderer viewport offsets need to account for the custom status bar separately.

**Step 10: Verify it compiles and runs**

Run: `cargo build`
Expected: Compiles without errors

Run: `cargo test`
Expected: All tests pass

**Step 11: Commit**

```bash
git add src/app/window_state.rs src/app/mouse_events.rs
git commit -m "feat(status-bar): integrate StatusBarUI into window state and render pipeline"
```

---

### Task 8: Create the Settings UI tab (general settings)

**Files:**
- Create: `src/settings_ui/status_bar_tab.rs`
- Modify: `src/settings_ui/mod.rs`
- Modify: `src/settings_ui/sidebar.rs`

**Step 1: Add StatusBar variant to SettingsTab enum**

In `src/settings_ui/sidebar.rs`, add `StatusBar` variant after `ProgressBar` in the enum (around line 17):

```rust
    StatusBar,
```

Add to `display_name()`:
```rust
            Self::StatusBar => "Status Bar",
```

Add to `icon()`:
```rust
            Self::StatusBar => "\u{2501}",
```

Add to `all()` array.

Add search keywords:
```rust
        SettingsTab::StatusBar => &[
            "status", "status bar", "widget", "widgets",
            "cpu", "memory", "network", "git branch",
            "clock", "time", "hostname", "username",
            "auto hide", "poll interval", "separator",
            "bell indicator", "current command", "directory",
            "drag", "reorder", "section", "left", "center", "right",
        ],
```

**Step 2: Create status_bar_tab.rs (general settings section)**

```rust
//! Status bar settings tab.
//!
//! Contains:
//! - Status bar enable/disable, position, height
//! - Styling: colors, font, separator
//! - Auto-hide settings
//! - Poll intervals
//! - Drag-and-drop widget configurator

use super::SettingsUI;
use super::section::{SLIDER_WIDTH, collapsing_section};
use crate::config::types::StatusBarPosition;
use crate::status_bar::config::{StatusBarSection, StatusBarWidgetConfig, WidgetId};
use std::collections::HashSet;

const SLIDER_HEIGHT: f32 = 18.0;

/// Show the status bar tab content.
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    let query = settings.search_query.trim().to_lowercase();

    // General section
    if section_matches(
        &query,
        "General",
        &["enable", "status", "position", "height"],
    ) {
        show_general_section(ui, settings, changes_this_frame, collapsed);
    }

    // Styling section
    if section_matches(
        &query,
        "Styling",
        &["color", "font", "separator", "opacity", "background", "foreground"],
    ) {
        show_styling_section(ui, settings, changes_this_frame, collapsed);
    }

    // Auto-hide section
    if section_matches(
        &query,
        "Auto-Hide",
        &["auto", "hide", "fullscreen", "mouse", "timeout", "inactive"],
    ) {
        show_auto_hide_section(ui, settings, changes_this_frame, collapsed);
    }

    // Polling section
    if section_matches(
        &query,
        "Polling",
        &["poll", "interval", "refresh", "system", "git", "cpu", "memory"],
    ) {
        show_polling_section(ui, settings, changes_this_frame, collapsed);
    }

    // Widget configurator section
    if section_matches(
        &query,
        "Widgets",
        &["widget", "drag", "reorder", "section", "left", "center", "right",
          "clock", "cpu", "memory", "network", "git", "hostname", "command", "bell"],
    ) {
        show_widgets_section(ui, settings, changes_this_frame, collapsed);
    }
}

fn section_matches(query: &str, title: &str, keywords: &[&str]) -> bool {
    if query.is_empty() {
        return true;
    }
    if title.to_lowercase().contains(query) {
        return true;
    }
    keywords.iter().any(|k| k.to_lowercase().contains(query))
}

// ============================================================================
// General Section
// ============================================================================

fn show_general_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "General", "status_bar_general", true, collapsed, |ui| {
        if ui
            .checkbox(&mut settings.config.status_bar_enabled, "Enable status bar")
            .on_hover_text("Show a configurable status bar with widgets")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.add_space(8.0);

        // Position
        ui.horizontal(|ui| {
            ui.label("Position:");
            let mut position = settings.config.status_bar_position;
            egui::ComboBox::from_id_salt("status_bar_position")
                .selected_text(match position {
                    StatusBarPosition::Top => "Top",
                    StatusBarPosition::Bottom => "Bottom",
                })
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_value(&mut position, StatusBarPosition::Top, "Top")
                        .changed()
                    {
                        settings.config.status_bar_position = position;
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                    if ui
                        .selectable_value(&mut position, StatusBarPosition::Bottom, "Bottom")
                        .changed()
                    {
                        settings.config.status_bar_position = position;
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });
        });

        ui.add_space(4.0);

        // Height
        ui.horizontal(|ui| {
            ui.label("Height:");
            if ui
                .add_sized(
                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                    egui::Slider::new(&mut settings.config.status_bar_height, 16.0..=40.0)
                        .suffix(" px")
                        .show_value(true),
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });
    });
}

// ============================================================================
// Styling Section
// ============================================================================

fn show_styling_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "Styling", "status_bar_styling", true, collapsed, |ui| {
        // Background color
        ui.horizontal(|ui| {
            ui.label("Background:");
            let mut color = egui::Color32::from_rgb(
                settings.config.status_bar_bg_color[0],
                settings.config.status_bar_bg_color[1],
                settings.config.status_bar_bg_color[2],
            );
            if ui.color_edit_button_srgba(&mut color).changed() {
                settings.config.status_bar_bg_color = [color.r(), color.g(), color.b()];
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        // Background opacity
        ui.horizontal(|ui| {
            ui.label("BG Opacity:");
            if ui
                .add_sized(
                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                    egui::Slider::new(&mut settings.config.status_bar_bg_alpha, 0.0..=1.0)
                        .show_value(true),
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.add_space(4.0);

        // Foreground color
        ui.horizontal(|ui| {
            ui.label("Text color:");
            let mut color = egui::Color32::from_rgb(
                settings.config.status_bar_fg_color[0],
                settings.config.status_bar_fg_color[1],
                settings.config.status_bar_fg_color[2],
            );
            if ui.color_edit_button_srgba(&mut color).changed() {
                settings.config.status_bar_fg_color = [color.r(), color.g(), color.b()];
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.add_space(8.0);

        // Font
        ui.horizontal(|ui| {
            ui.label("Font:");
            if ui
                .add(
                    egui::TextEdit::singleline(&mut settings.config.status_bar_font)
                        .hint_text("(terminal font)")
                        .desired_width(150.0),
                )
                .on_hover_text("Font family for status bar (empty = use terminal font)")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        // Font size
        ui.horizontal(|ui| {
            ui.label("Font size:");
            if ui
                .add_sized(
                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                    egui::Slider::new(&mut settings.config.status_bar_font_size, 8.0..=24.0)
                        .suffix(" pt")
                        .show_value(true),
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.add_space(4.0);

        // Separator
        ui.horizontal(|ui| {
            ui.label("Separator:");
            if ui
                .add(
                    egui::TextEdit::singleline(&mut settings.config.status_bar_separator)
                        .desired_width(80.0),
                )
                .on_hover_text("Text separator between widgets")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });
    });
}

// ============================================================================
// Auto-Hide Section
// ============================================================================

fn show_auto_hide_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Auto-Hide",
        "status_bar_auto_hide",
        false,
        collapsed,
        |ui| {
            if ui
                .checkbox(
                    &mut settings.config.status_bar_auto_hide_fullscreen,
                    "Hide when fullscreen",
                )
                .on_hover_text("Automatically hide the status bar when the window enters fullscreen")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if ui
                .checkbox(
                    &mut settings.config.status_bar_auto_hide_mouse_inactive,
                    "Hide on mouse inactivity",
                )
                .on_hover_text("Hide the status bar after a period of mouse inactivity")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if settings.config.status_bar_auto_hide_mouse_inactive {
                ui.horizontal(|ui| {
                    ui.label("Timeout:");
                    if ui
                        .add_sized(
                            [SLIDER_WIDTH, SLIDER_HEIGHT],
                            egui::Slider::new(
                                &mut settings.config.status_bar_mouse_inactive_timeout,
                                1.0..=30.0,
                            )
                            .suffix(" sec")
                            .show_value(true),
                        )
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });
            }
        },
    );
}

// ============================================================================
// Polling Section
// ============================================================================

fn show_polling_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Poll Intervals",
        "status_bar_polling",
        false,
        collapsed,
        |ui| {
            ui.horizontal(|ui| {
                ui.label("System (CPU/mem/net):");
                if ui
                    .add_sized(
                        [SLIDER_WIDTH, SLIDER_HEIGHT],
                        egui::Slider::new(
                            &mut settings.config.status_bar_system_poll_interval,
                            0.5..=30.0,
                        )
                        .suffix(" sec")
                        .show_value(true),
                    )
                    .on_hover_text("How often to refresh CPU, memory, and network stats")
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Git branch:");
                if ui
                    .add_sized(
                        [SLIDER_WIDTH, SLIDER_HEIGHT],
                        egui::Slider::new(
                            &mut settings.config.status_bar_git_poll_interval,
                            1.0..=60.0,
                        )
                        .suffix(" sec")
                        .show_value(true),
                    )
                    .on_hover_text("How often to check the current git branch")
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        },
    );
}

// ============================================================================
// Widget Configurator Section (Drag-and-Drop)
// ============================================================================

fn show_widgets_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Widgets",
        "status_bar_widgets",
        true,
        collapsed,
        |ui| {
            ui.label("Drag widgets between sections to reorder. Click to toggle.");
            ui.add_space(8.0);

            // Show three columns for Left / Center / Right
            let available_width = ui.available_width();
            let column_width = (available_width - 20.0) / 3.0;

            ui.horizontal(|ui| {
                // Left column
                ui.vertical(|ui| {
                    ui.set_width(column_width);
                    ui.label(
                        egui::RichText::new("Left")
                            .strong()
                            .color(egui::Color32::from_rgb(100, 180, 255)),
                    );
                    ui.separator();
                    show_section_widgets(
                        ui,
                        &mut settings.config.status_bar_widgets,
                        StatusBarSection::Left,
                        &mut settings.has_changes,
                        changes_this_frame,
                    );
                });

                ui.separator();

                // Center column
                ui.vertical(|ui| {
                    ui.set_width(column_width);
                    ui.label(
                        egui::RichText::new("Center")
                            .strong()
                            .color(egui::Color32::from_rgb(100, 180, 255)),
                    );
                    ui.separator();
                    show_section_widgets(
                        ui,
                        &mut settings.config.status_bar_widgets,
                        StatusBarSection::Center,
                        &mut settings.has_changes,
                        changes_this_frame,
                    );
                });

                ui.separator();

                // Right column
                ui.vertical(|ui| {
                    ui.set_width(column_width);
                    ui.label(
                        egui::RichText::new("Right")
                            .strong()
                            .color(egui::Color32::from_rgb(100, 180, 255)),
                    );
                    ui.separator();
                    show_section_widgets(
                        ui,
                        &mut settings.config.status_bar_widgets,
                        StatusBarSection::Right,
                        &mut settings.has_changes,
                        changes_this_frame,
                    );
                });
            });

            ui.add_space(8.0);

            // Add custom widget button
            if ui.button("+ Add Custom Text Widget").clicked() {
                let custom_id = format!(
                    "custom_{}",
                    settings
                        .config
                        .status_bar_widgets
                        .iter()
                        .filter(|w| matches!(w.id, WidgetId::Custom(_)))
                        .count()
                        + 1
                );
                settings.config.status_bar_widgets.push(StatusBarWidgetConfig {
                    id: WidgetId::Custom(custom_id),
                    enabled: true,
                    section: StatusBarSection::Center,
                    order: 99,
                    format: Some(String::new()),
                });
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        },
    );
}

/// Show widgets in a section column with drag-and-drop support
fn show_section_widgets(
    ui: &mut egui::Ui,
    widgets: &mut Vec<StatusBarWidgetConfig>,
    section: StatusBarSection,
    has_changes: &mut bool,
    changes_this_frame: &mut bool,
) {
    // Collect indices of widgets in this section, sorted by order
    let mut section_indices: Vec<usize> = widgets
        .iter()
        .enumerate()
        .filter(|(_, w)| w.section == section)
        .map(|(i, _)| i)
        .collect();
    section_indices.sort_by_key(|&i| widgets[i].order);

    let mut swap: Option<(usize, usize)> = None;
    let mut toggle_idx: Option<usize> = None;
    let mut section_change: Option<(usize, StatusBarSection)> = None;

    for (pos, &widget_idx) in section_indices.iter().enumerate() {
        let widget = &widgets[widget_idx];
        let id = egui::Id::new(("status_widget", widget_idx));

        let text = format!(
            "{} {} {}",
            if widget.enabled { "\u{2705}" } else { "\u{274c}" },
            widget.id.icon(),
            widget.id.label()
        );

        let bg = if widget.enabled {
            egui::Color32::from_rgb(40, 45, 55)
        } else {
            egui::Color32::from_rgb(30, 30, 35)
        };

        let response = ui.add(
            egui::Button::new(
                egui::RichText::new(&text)
                    .size(11.0)
                    .color(if widget.enabled {
                        egui::Color32::from_rgb(220, 220, 220)
                    } else {
                        egui::Color32::from_rgb(100, 100, 100)
                    }),
            )
            .fill(bg)
            .min_size(egui::vec2(ui.available_width() - 4.0, 24.0)),
        );

        // Toggle enabled on click
        if response.clicked() {
            toggle_idx = Some(widget_idx);
        }

        // Drag-and-drop: move up/down within section
        if response.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        }

        // Context menu for moving between sections
        response.context_menu(|ui| {
            ui.label(egui::RichText::new("Move to:").strong());
            if section != StatusBarSection::Left && ui.button("Left").clicked() {
                section_change = Some((widget_idx, StatusBarSection::Left));
                ui.close_menu();
            }
            if section != StatusBarSection::Center && ui.button("Center").clicked() {
                section_change = Some((widget_idx, StatusBarSection::Center));
                ui.close_menu();
            }
            if section != StatusBarSection::Right && ui.button("Right").clicked() {
                section_change = Some((widget_idx, StatusBarSection::Right));
                ui.close_menu();
            }
            ui.separator();
            if pos > 0 && ui.button("\u{2191} Move Up").clicked() {
                swap = Some((widget_idx, section_indices[pos - 1]));
                ui.close_menu();
            }
            if pos < section_indices.len() - 1 && ui.button("\u{2193} Move Down").clicked() {
                swap = Some((widget_idx, section_indices[pos + 1]));
                ui.close_menu();
            }
            // Delete custom widgets
            if matches!(widget.id, WidgetId::Custom(_)) {
                ui.separator();
                if ui.button("\u{1f5d1} Delete").clicked() {
                    // Mark for deletion by disabling and setting order very high
                    // Actual removal handled below
                    ui.close_menu();
                }
            }
        });

        // Show format editor for custom widgets
        if matches!(widgets[widget_idx].id, WidgetId::Custom(_)) && widgets[widget_idx].enabled {
            let mut fmt = widgets[widget_idx].format.clone().unwrap_or_default();
            if ui
                .add(
                    egui::TextEdit::singleline(&mut fmt)
                        .hint_text("\\(session.username)@\\(session.hostname)")
                        .desired_width(ui.available_width() - 8.0),
                )
                .changed()
            {
                widgets[widget_idx].format = Some(fmt);
                *has_changes = true;
                *changes_this_frame = true;
            }
        }
    }

    // Apply deferred mutations
    if let Some(idx) = toggle_idx {
        widgets[idx].enabled = !widgets[idx].enabled;
        *has_changes = true;
        *changes_this_frame = true;
    }
    if let Some((a, b)) = swap {
        let order_a = widgets[a].order;
        let order_b = widgets[b].order;
        widgets[a].order = order_b;
        widgets[b].order = order_a;
        *has_changes = true;
        *changes_this_frame = true;
    }
    if let Some((idx, new_section)) = section_change {
        widgets[idx].section = new_section;
        // Set order to end of new section
        let max_order = widgets
            .iter()
            .filter(|w| w.section == new_section)
            .map(|w| w.order)
            .max()
            .unwrap_or(0);
        widgets[idx].order = max_order + 1;
        *has_changes = true;
        *changes_this_frame = true;
    }

    if section_indices.is_empty() {
        ui.label(
            egui::RichText::new("(empty)")
                .italics()
                .color(egui::Color32::from_rgb(80, 80, 80)),
        );
    }
}
```

**Step 3: Register tab in settings_ui/mod.rs**

Add the module declaration (near other tab modules):
```rust
pub(crate) mod status_bar_tab;
```

Add the dispatch arm in `show_tab_content()` (after the `ProgressBar` arm):
```rust
        SettingsTab::StatusBar => {
            status_bar_tab::show(ui, self, changes_this_frame, &mut collapsed);
        }
```

**Step 4: Verify it compiles**

Run: `cargo check`
Expected: Compiles without errors

**Step 5: Commit**

```bash
git add src/settings_ui/status_bar_tab.rs src/settings_ui/mod.rs src/settings_ui/sidebar.rs
git commit -m "feat(status-bar): add settings UI tab with widget configurator"
```

---

### Task 9: Add config serialization tests

**Files:**
- Create: `tests/status_bar_config_test.rs`

**Step 1: Write the tests**

```rust
//! Tests for status bar configuration serialization/deserialization.

use par_term::config::Config;
use par_term::status_bar::config::{StatusBarSection, StatusBarWidgetConfig, WidgetId, default_widgets};

#[test]
fn test_default_config_has_status_bar_fields() {
    let config = Config::default();
    assert!(!config.status_bar_enabled);
    assert_eq!(config.status_bar_height, 22.0);
    assert_eq!(config.status_bar_bg_color, [30, 30, 30]);
    assert!(!config.status_bar_widgets.is_empty());
}

#[test]
fn test_default_widgets_complete() {
    let widgets = default_widgets();
    assert_eq!(widgets.len(), 9);

    // Check all built-in widget IDs are present
    let ids: Vec<&WidgetId> = widgets.iter().map(|w| &w.id).collect();
    assert!(ids.contains(&&WidgetId::Clock));
    assert!(ids.contains(&&WidgetId::UsernameHostname));
    assert!(ids.contains(&&WidgetId::CurrentDirectory));
    assert!(ids.contains(&&WidgetId::GitBranch));
    assert!(ids.contains(&&WidgetId::CpuUsage));
    assert!(ids.contains(&&WidgetId::MemoryUsage));
    assert!(ids.contains(&&WidgetId::NetworkStatus));
    assert!(ids.contains(&&WidgetId::BellIndicator));
    assert!(ids.contains(&&WidgetId::CurrentCommand));
}

#[test]
fn test_widget_config_serialization_roundtrip() {
    let widget = StatusBarWidgetConfig {
        id: WidgetId::GitBranch,
        enabled: true,
        section: StatusBarSection::Left,
        order: 2,
        format: None,
    };

    let yaml = serde_yaml::to_string(&widget).expect("serialize");
    let deserialized: StatusBarWidgetConfig =
        serde_yaml::from_str(&yaml).expect("deserialize");

    assert_eq!(deserialized.id, widget.id);
    assert_eq!(deserialized.enabled, widget.enabled);
    assert_eq!(deserialized.section, widget.section);
    assert_eq!(deserialized.order, widget.order);
}

#[test]
fn test_custom_widget_config_serialization() {
    let widget = StatusBarWidgetConfig {
        id: WidgetId::Custom("my_widget".to_string()),
        enabled: true,
        section: StatusBarSection::Center,
        order: 0,
        format: Some("\\(session.username) on \\(session.hostname)".to_string()),
    };

    let yaml = serde_yaml::to_string(&widget).expect("serialize");
    let deserialized: StatusBarWidgetConfig =
        serde_yaml::from_str(&yaml).expect("deserialize");

    assert_eq!(deserialized.id, WidgetId::Custom("my_widget".to_string()));
    assert_eq!(
        deserialized.format,
        Some("\\(session.username) on \\(session.hostname)".to_string())
    );
}

#[test]
fn test_config_yaml_with_status_bar() {
    let yaml = r#"
status_bar_enabled: true
status_bar_position: top
status_bar_height: 28.0
status_bar_fg_color: [255, 255, 255]
status_bar_widgets:
  - id: clock
    enabled: true
    section: right
    order: 0
  - id: git_branch
    enabled: true
    section: left
    order: 0
"#;

    let config: Config = serde_yaml::from_str(yaml).expect("deserialize");
    assert!(config.status_bar_enabled);
    assert_eq!(config.status_bar_height, 28.0);
    assert_eq!(config.status_bar_widgets.len(), 2);
}
```

**Step 2: Run the tests**

Run: `cargo test status_bar_config_test -- -v`
Expected: All 5 tests pass

**Step 3: Commit**

```bash
git add tests/status_bar_config_test.rs
git commit -m "test(status-bar): add config serialization tests"
```

---

### Task 10: Update MATRIX.md and CHANGELOG.md

**Files:**
- Modify: `MATRIX.md` (section §23)
- Modify: `CHANGELOG.md`

**Step 1: Update MATRIX.md**

Find section §23 (Status Bar) in MATRIX.md. Change the status of all 10 features from `\u274c Not Implemented` to `\u2705 Implemented`. Keep the effort ratings and notes. Add implementation notes referencing `src/status_bar/`.

**Step 2: Update CHANGELOG.md**

Add to the Unreleased section (or create one):

```markdown
### Added
- **Status Bar**: Configurable status bar with widget system (#133)
  - 10 built-in widgets: clock, username@hostname, current directory, git branch, CPU usage, memory usage, network status, bell indicator, current command, custom text
  - Three-section layout (left/center/right) with configurable separator
  - Drag-and-drop widget configurator in Settings UI
  - System monitoring via background thread (CPU, memory, network)
  - Git branch polling with configurable interval
  - Auto-hide on fullscreen and/or mouse inactivity
  - Per-widget enable/disable and section assignment
  - Customizable colors, font, opacity, and height
  - Top or bottom positioning (stacks with tab bar)
```

**Step 3: Commit**

```bash
git add MATRIX.md CHANGELOG.md
git commit -m "docs: update MATRIX.md and CHANGELOG.md for status bar feature"
```

---

### Task 11: Update Makefile test target

**Files:**
- Modify: `Makefile`

**Step 1: Verify test target includes new tests**

Check if the existing `make test` target already runs `cargo test` which would pick up the new test file automatically. If it does, no changes needed. If there's a specific test target that lists test files, add:

```makefile
test-status-bar:
	cargo test status_bar -- -v
```

**Step 2: Run full test suite**

Run: `make test`
Expected: All tests pass, including new status bar tests

**Step 3: Run full checks**

Run: `make checkall` (or `make ci` / `make pre-commit`)
Expected: Format, lint, and tests all pass

**Step 4: Fix any lint/format issues**

Run: `make fmt && make lint`

**Step 5: Commit if changes needed**

```bash
git add Makefile
git commit -m "chore: add status bar test target to Makefile"
```

---

### Task 12: Create PR

**Step 1: Push branch**

```bash
git push -u origin feat/status-bar
```

**Step 2: Create PR**

```bash
gh pr create --title "feat(status-bar): add configurable status bar with widget components" --body "$(cat <<'EOF'
## Summary
- Adds a configurable status bar with 10 built-in widgets and three-section layout (left/center/right)
- Includes system monitoring (CPU, memory, network) via background thread using sysinfo
- Git branch polling with configurable interval
- Drag-and-drop widget configurator in Settings UI
- Auto-hide on fullscreen and/or mouse inactivity
- Custom text widgets with variable interpolation

Closes #133

## Test plan
- [ ] Enable status bar via Settings > Status Bar > Enable
- [ ] Verify widgets display correct data (clock, hostname, git branch)
- [ ] Test position toggle (top/bottom)
- [ ] Test auto-hide in fullscreen mode
- [ ] Test drag-and-drop widget reordering in Settings
- [ ] Verify status bar stacks correctly with tab bar at same position
- [ ] Run `cargo test status_bar` — all tests pass
- [ ] Run `make checkall` — format, lint, and tests pass
EOF
)"
```

**Step 3: Return PR URL**
