# Status Bar Disk-Free Widget — Design

**Date:** 2026-08-01
**Status:** Approved
**Scope:** New status-bar widget showing free disk space (percent + bytes), cross-platform, defaulting to the disk par-term launched from with an option to follow the active tab/pane's working directory.

## Goal

Add a `DiskFree` built-in status-bar widget that reports free space on a disk:

- **Cross-platform** via the already-present `sysinfo` dependency (macOS/Linux/Windows). No new dependency, no new Cargo feature flag — `disk` is a default sysinfo feature and the existing `features = ["system"]` declaration keeps defaults on.
- **Default polling interval of 1 minute** (60s), independently configurable.
- **Default target: the disk par-term was launched from** (the filesystem containing the process's launch CWD).
- **Optional: follow the active tab/pane's CWD** — when enabled, report free space on the disk containing the active tab's working directory.
- **Render format:** percent first, free bytes in parens, fixed-width — e.g. `DISK  62% (250 GB free)`.

## Non-goals

- Per-disk multi-widget display (one widget, one disk at a time).
- Disk I/O throughput or inode metrics.
- Alerts/thresholds on low disk (future work).

## Approach: a third background poller

Add a `DiskMonitor` poller that mirrors the existing `GitBranchPoller` (path-scoped, own interval, own background thread, start/stop gated by widget presence, `set_cwd`/`status` API). This is the documented extension pattern: `src/status_bar/mod.rs` says *"add a poller in this file following the `GitBranchPoller` pattern."*

Rejected alternatives:
- **Extend `SystemMonitor`** (CPU/mem/net, 2s cadence) — either refreshes disk every 2s (wasteful) or needs a sub-tick counter; also forces path-scoped state into a struct that is currently system-global.
- **Sub-tick hybrid** — same thread, disk every N ticks. Couples disk cadence into the system monitor for negligible thread savings.

A third lightweight thread issuing one `statfs`-class syscall per minute is negligible overhead, and keeps `SystemMonitor` focused on global metrics.

## Component design

### 1. Config — `par-term-config/src/config/config_struct/status_bar_config.rs`

Two new fields on `StatusBarConfig`, following the existing interval/toggle pattern:

| Field | Type | Default | Notes |
|-------|------|---------|-------|
| `status_bar_disk_poll_interval` | `f32` | `60.0` | seconds; UI range 5.0–600.0 |
| `status_bar_disk_follow_cwd` | `bool` | `false` | `false` = launch disk; `true` = active tab's CWD disk |

Each gets a `default_*` function and a `Default`-impl entry with the **same expression** (the `config_yaml_compat` test fails if they diverge — see CLAUDE.md "Adding a New Configuration Option").

### 2. Widget id — `par-term-config/src/status_bar.rs`

- New variant `WidgetId::DiskFree`.
- `as_key` / `from_key` → `"disk_free"`.
- `label()` → `"Disk Free"`.
- `icon()` → `"\u{1f4bf}"` (💽 optical disk).
- New method `needs_disk_monitor() -> bool` (true only for `DiskFree`). **Not** added to `needs_system_monitor()` so the 2s CPU/mem thread is unaffected.
- Added to `default_widgets()` **disabled by default** (matches CPU/Memory/Network being off by default), Right section, order 3; bump Bell→4, Clock→5, UpdateAvailable→6.

### 3. Disk monitor — `src/status_bar/disk_monitor.rs` (new file)

Same structure as `src/status_bar/system_monitor.rs`: always-compiled `DiskMonitorData`, real impl behind `#[cfg(feature = "system-monitor")]`, no-op stub when the feature is off.

**`DiskMonitorData`** (always compiled):
```rust
pub struct DiskMonitorData {
    pub free_bytes: u64,
    pub total_bytes: u64,
    pub free_percent: f32,   // 0.0–100.0
    pub last_update: Option<Instant>,
}
```

**`DiskMonitor`** (feature-gated) — background thread, polls at `disk_poll_interval`:
- Captures the **launch dir** once via `std::env::current_dir()` at construction. par-term never chdir's its own process (the PTY child does), so this is stable for the session. No `set_launch_dir` API needed.
- `set_cwd(Option<&str>)` and `set_follow_cwd(bool)` — read by the poll thread each tick (under `parking_lot::Mutex`).
- `start(interval)`, `signal_stop()`, `stop()`, `is_running()`, `data() -> DiskMonitorData` — same shape as `SystemMonitor`.
- Sleeps in short increments so `stop()` returns promptly (same pattern as `SystemMonitor`).

**Target path resolution (per poll):**
1. target = active CWD if `follow_cwd == true && cwd.is_some()`, else launch dir.
2. Resolve target → disk via `disk_for_path(&disks, target)` (mount-point prefix match; a non-existent path still resolves to its closest parent mount, e.g. a deleted subdir of `/home` → the `/home` or `/` mount).
3. Fallback: if `follow_cwd` was set but the CWD disk resolves to `None` (e.g. CWD on a since-removed removable with no matching mount), re-resolve against the **launch dir**; if that also resolves to `None` (empty disk list), leave data at last-known-good (or zeros on first poll) — the widget renders empty (hidden).

**Pure helper (unit-testable, no `Disk` construction needed in tests):**
```rust
/// Return the disk whose mount point is the longest path-prefix of `target`.
pub fn disk_for_path<'a>(disks: &'a [Disk], target: &Path) -> Option<&'a Disk>
```
Backed by `sysinfo::Disk::mount_point()`. Compares by path components so `/` is the catch-all fallback for any absolute path on the root volume.

**Refresh efficiency:** keep a `sysinfo::Disks`, refresh each tick with `DiskRefreshKind::nothing().with_space()` (space only — cheapest refresh), then match. `mount_point()` is read after refresh.

### 4. Status bar UI — `src/status_bar/mod.rs`

- New field `disk_monitor: DiskMonitor` on `StatusBarUI`; init in `new()`.
- `signal_shutdown()` and `Drop`: stop the disk monitor too.
- `sync_monitor_state()`: compute `needs_disk = widgets.iter().any(|w| w.enabled && w.id.needs_disk_monitor())`; start/stop `disk_monitor` at `config.status_bar.status_bar_disk_poll_interval`; push `disk_monitor.set_follow_cwd(config.status_bar.status_bar_disk_follow_cwd)` each sync.
- `render()`: call `self.disk_monitor.set_cwd(cwd)` (cwd already computed for the git poller); add `disk_free_percent`, `disk_free_bytes`, `disk_total_bytes` to `WidgetContext` from `disk_monitor.data()`.

### 5. Widget text — `src/status_bar/widgets.rs`

- Extend `WidgetContext` with `disk_free_percent: f32`, `disk_free_bytes: u64`, `disk_total_bytes: u64`.
- `widget_text()` arm:
  ```rust
  WidgetId::DiskFree => format!("DISK {:>4.0}% ({}) free",
       ctx.disk_free_percent, format_bytes(ctx.disk_free_bytes))
  ```
  → e.g. `DISK  62% (250 GB free)`. Fixed-width percent and fixed-width bytes so the bar does not jump as values change.
- New public formatter `pub fn format_bytes(u64) -> String` alongside `format_bytes_per_sec` / `format_memory` in `system_monitor.rs`; adds a **TB tier** (disks are large): `B / KB / MB / GB / TB`, fixed-width.
- Interpolation variables in `resolve_variable()`:
  - `system.disk_free_percent` → `format!("{:.0}%", pct)`
  - `system.disk_free` → `format_bytes(free)`
  - `system.disk_total` → `format_bytes(total)`

### 6. Settings UI — `par-term-settings-ui/src/status_bar_tab/`

- `poll_intervals.rs`: add a disk-poll-interval slider (5.0–600.0, step 5.0) and a "Follow active tab's directory" checkbox (`follow_cwd`). Set `settings.has_changes = true` and `*changes_this_frame = true` on change.
- `widgets.rs`: ensure `Disk Free` appears in the widget picker (the new variant flows through automatically if the picker enumerates `WidgetId`; otherwise add it explicitly).
- Keywords (`keywords()` for the status bar tab): add `disk`, `free`, `space`, `storage`.

### 7. Docs — `docs/features/STATUS_BAR.md`

- Add `Disk Free` row to the built-in widgets table (Right section, Disabled default).
- Add the two config keys to the Configuration reference.
- Add `system.disk_free_percent` / `system.disk_free` / `system.disk_total` to the variable-interpolation table.
- Note the new widget under System Monitoring / Settings UI.

## Testing

- **`format_bytes`** — covers B/KB/MB/GB/TB tiers and fixed-width equality across magnitudes (mirrors the existing `test_format_bytes_per_sec`).
- **`disk_for_path`** — pure longest-mount-point-prefix logic; test against real `Disks::new_with_refreshed_list()` asserting the system temp dir resolves to `Some` with `free <= total`. Extract prefix-matching so a unit test can run on synthetic `Vec<(PathBuf, …)>` stand-ins without constructing `Disk`.
- **`widget_text(DiskFree)`** — asserts `DISK  62% (250 GB free)` shape and equal width across value magnitudes.
- **Config default round-trip** — `config_yaml_compat` already enforces serde-default ↔ `Default` agreement; the new fields must satisfy it.

## Cross-cutting notes

- **Feature gate:** the real `DiskMonitor` is `#[cfg(feature = "system-monitor")]` (same as `SystemMonitor`); the stub compiles when the feature is off. `par-term-config` is unaffected (no sysinfo dependency).
- **Construction sites:** `StatusBarUI::new()` is the single source (`src/app/window_state/impl_init.rs:79`); the launch-dir capture happens inside `new()`, so the `traits_impl.rs` call sites are unaffected.
- **CLAUDE.md config gotcha:** never swap a hand-written `Default` for `#[derive(Default)]`; the new fields' `Default`-impl expressions must equal their `#[serde(default = "…")]` functions.
