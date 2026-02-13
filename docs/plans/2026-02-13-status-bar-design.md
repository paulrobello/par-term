# Status Bar Design

**Issue**: #133
**Date**: 2026-02-13

## Overview

Add a configurable status bar with widget components to par-term. The status bar displays per-tab context (git branch, cwd, hostname) and global system info (CPU, memory, network, clock) in a three-section layout (Left / Center / Right).

## Architecture

- **Rendering**: egui `TopBottomPanel` — same pattern as tab bar and tmux status bar
- **Viewport**: Height feeds into `set_content_offset_y()` (top) or `set_content_inset_bottom()` (bottom); stacks with tab bar when at same position
- **Data sources**: Active tab's `SessionVariables` for per-tab widgets, shared `SystemMonitor` for system widgets
- **Updates**: Dirty-flag mechanism triggers redraws on widget data changes

### Module Structure

```
src/status_bar/
├── mod.rs              — StatusBarUI struct, panel rendering, layout orchestration
├── widgets.rs          — Widget trait + all built-in widget implementations
├── system_monitor.rs   — Background polling thread (sysinfo crate)
└── config.rs           — StatusBarWidgetConfig, StatusBarSection, widget IDs
```

## Widget System

### Widget Trait

```rust
trait StatusBarWidget {
    fn id(&self) -> &str;
    fn label(&self) -> &str;
    fn render(&self, ui: &mut egui::Ui, ctx: &WidgetContext);
    fn update(&mut self);
    fn min_width(&self) -> f32;
}
```

`WidgetContext` provides per-tab `SessionVariables` + `SystemMonitorData` + config.

### Built-in Widgets

| Widget | Default Section | Data Source |
|--------|----------------|-------------|
| Clock | Right | `chrono::Local` |
| Username@Hostname | Left | `SessionVariables` |
| Current Directory | Left | `SessionVariables` |
| Git Branch | Left | Shell integration / git polling |
| CPU Usage | Right | `sysinfo` |
| Memory Usage | Right | `sysinfo` |
| Network Status | Right | `sysinfo` |
| Bell Indicator | Right | `SessionVariables.bell_count` |
| Current Command | Center | `SessionVariables.current_command` |
| Custom Text | Any | User-defined format string with `\(var)` interpolation |

### Per-Widget Config

- `enabled: bool`
- `section: Left | Center | Right`
- `order: usize` (within section)
- `format: Option<String>` (override display format)

## Layout & Styling

**Three-section layout**:
- Left: `left_to_right` layout
- Center: `centered_and_justified` layout
- Right: `right_to_left` layout
- Widgets separated by configurable separator (default `" │ "`)

**Styling config**:
- `status_bar_height` (default 22.0)
- `status_bar_position` (Top / Bottom)
- `status_bar_bg_color` + `status_bar_bg_alpha`
- `status_bar_fg_color`
- `status_bar_font` + `status_bar_font_size`
- `status_bar_separator`

**Auto-hide** (both independently configurable):
- `status_bar_auto_hide_fullscreen` (default true)
- `status_bar_auto_hide_mouse_inactive` (default false)
- `status_bar_mouse_inactive_timeout` (default 3.0 seconds)
- When hidden, height contribution goes to 0, terminal grid recalculates

## System Monitor

**Background thread** (not tokio — CPU-bound `sysinfo` work). Shared via `Arc<Mutex<SystemMonitorData>>`.

**`SystemMonitorData`**:
- `cpu_usage: f32` (percentage)
- `memory_used: u64` / `memory_total: u64`
- `network_rx_rate: u64` / `network_tx_rate: u64` (bytes/sec)
- `last_update: Instant`

**Configurable intervals**:
- `status_bar_system_poll_interval` (default 2.0 seconds) — CPU/memory/network
- `status_bar_git_poll_interval` (default 5.0 seconds) — git branch polling

**Lifecycle**: Monitor thread starts when status bar is enabled with any system widget active. Stops when disabled or no system widgets remain.

**Git branch**: Runs `git rev-parse --abbrev-ref HEAD` in the tab's working directory. Falls back to shell integration data if available.

## Settings UI

**New "Status Bar" tab** with two sub-sections:

**General settings**: Enable/disable, position, height, colors, font, separator, auto-hide options, poll intervals.

**Drag-and-drop widget configurator**:
- Three columns (Left / Center / Right)
- Widget cards with: name, enable/disable checkbox, drag handle
- Drag between sections to reassign, within section to reorder
- Click card to expand per-widget settings
- "Add Widget" button for custom text widgets
- Live preview — status bar updates as you configure

**Quick search keywords**: `"status"`, `"status bar"`, `"widget"`, `"cpu"`, `"memory"`, `"network"`, `"git branch"`, `"clock"`, `"hostname"`, `"auto hide"`, `"poll interval"`

## Configuration Schema

```yaml
status_bar_enabled: false
status_bar_position: bottom
status_bar_height: 22.0
status_bar_bg_color: [30, 30, 30]
status_bar_bg_alpha: 0.95
status_bar_fg_color: [200, 200, 200]
status_bar_font: ""
status_bar_font_size: 12.0
status_bar_separator: " │ "
status_bar_auto_hide_fullscreen: true
status_bar_auto_hide_mouse_inactive: false
status_bar_mouse_inactive_timeout: 3.0
status_bar_system_poll_interval: 2.0
status_bar_git_poll_interval: 5.0
status_bar_widgets:
  - id: "username_hostname"
    enabled: true
    section: left
    order: 0
  - id: "current_directory"
    enabled: true
    section: left
    order: 1
  - id: "git_branch"
    enabled: true
    section: left
    order: 2
  - id: "current_command"
    enabled: true
    section: center
    order: 0
  - id: "cpu_usage"
    enabled: true
    section: right
    order: 0
  - id: "memory_usage"
    enabled: true
    section: right
    order: 1
  - id: "network_status"
    enabled: true
    section: right
    order: 2
  - id: "bell_indicator"
    enabled: false
    section: right
    order: 3
  - id: "clock"
    enabled: true
    section: right
    order: 4
```

## Files to Create

- `src/status_bar/mod.rs`
- `src/status_bar/widgets.rs`
- `src/status_bar/system_monitor.rs`
- `src/status_bar/config.rs`
- `src/settings_ui/status_bar_tab.rs`

## Files to Modify

- `src/config/mod.rs` + `src/config/defaults.rs` — New config fields
- `src/app/window_state.rs` — Height calc, viewport offset, render call
- `src/settings_ui/mod.rs` + `sidebar.rs` — Register tab + search keywords
- `src/renderer/mod.rs` — Offset stacking adjustments if needed
- `MATRIX.md` — Mark §23 features as implemented
- `CHANGELOG.md` — Document the feature

## Dependencies

- `sysinfo` — CPU/memory/network monitoring (new)
- `chrono` — Clock widget (already present)

## Testing

- Unit tests for widget rendering logic (format strings, data display)
- Unit tests for layout calculations (section assignment, ordering)
- Unit tests for config serialization/deserialization
- Unit tests for system monitor data formatting
- Integration test for viewport offset stacking (tab bar + status bar)
