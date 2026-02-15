# AI Terminal Inspector + ACP Agent Integration — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a DevTools-style right-side panel with terminal state inspection, JSON export, and interactive ACP agent chat that can suggest commands to the terminal.

**Architecture:** New `src/ai_inspector/` module for the egui panel UI and `src/acp/` module for the ACP protocol client. The panel integrates into `WindowState` alongside existing overlays (SearchUI, SettingsUI). Terminal reflows columns when the panel opens/closes. ACP agents are spawned as subprocesses communicating via JSON-RPC 2.0 over stdio.

**Tech Stack:** Rust, egui (UI), serde_json (serialization), tokio (async subprocess I/O), toml (agent configs)

---

## Phase 1: Foundation

### Task 1: Add config fields

**Files:**
- Modify: `src/config/mod.rs`
- Modify: `src/config/defaults.rs`

**Step 1: Add default functions to `src/config/defaults.rs`**

Add these functions (follow existing patterns in the file):

```rust
pub fn ai_inspector_enabled() -> bool { true }
pub fn ai_inspector_width() -> f32 { 300.0 }
pub fn ai_inspector_default_scope() -> String { "visible".to_string() }
pub fn ai_inspector_view_mode() -> String { "cards".to_string() }
pub fn ai_inspector_live_update() -> bool { true }
pub fn ai_inspector_show_zones() -> bool { true }
pub fn ai_inspector_agent() -> String { "claude.com".to_string() }
pub fn ai_inspector_auto_launch() -> bool { true }
pub fn ai_inspector_auto_context() -> bool { true }
pub fn ai_inspector_context_max_lines() -> usize { 200 }
pub fn ai_inspector_auto_approve() -> bool { false }
```

**Step 2: Add fields to `Config` struct in `src/config/mod.rs`**

Add after the last existing field group (before the closing brace of the struct), with a comment section header:

```rust
// AI Inspector
#[serde(default = "defaults::ai_inspector_enabled")]
pub ai_inspector_enabled: bool,
#[serde(default = "defaults::ai_inspector_width")]
pub ai_inspector_width: f32,
#[serde(default = "defaults::ai_inspector_default_scope")]
pub ai_inspector_default_scope: String,
#[serde(default = "defaults::ai_inspector_view_mode")]
pub ai_inspector_view_mode: String,
#[serde(default = "defaults::ai_inspector_live_update")]
pub ai_inspector_live_update: bool,
#[serde(default = "defaults::ai_inspector_show_zones")]
pub ai_inspector_show_zones: bool,
#[serde(default = "defaults::ai_inspector_agent")]
pub ai_inspector_agent: String,
#[serde(default = "defaults::ai_inspector_auto_launch")]
pub ai_inspector_auto_launch: bool,
#[serde(default = "defaults::ai_inspector_auto_context")]
pub ai_inspector_auto_context: bool,
#[serde(default = "defaults::ai_inspector_context_max_lines")]
pub ai_inspector_context_max_lines: usize,
#[serde(default = "defaults::ai_inspector_auto_approve")]
pub ai_inspector_auto_approve: bool,
```

**Step 3: Update `Default` impl for `Config`**

Add matching fields to the `Default` impl (at ~line 1762):

```rust
ai_inspector_enabled: defaults::ai_inspector_enabled(),
ai_inspector_width: defaults::ai_inspector_width(),
ai_inspector_default_scope: defaults::ai_inspector_default_scope(),
ai_inspector_view_mode: defaults::ai_inspector_view_mode(),
ai_inspector_live_update: defaults::ai_inspector_live_update(),
ai_inspector_show_zones: defaults::ai_inspector_show_zones(),
ai_inspector_agent: defaults::ai_inspector_agent(),
ai_inspector_auto_launch: defaults::ai_inspector_auto_launch(),
ai_inspector_auto_context: defaults::ai_inspector_auto_context(),
ai_inspector_context_max_lines: defaults::ai_inspector_context_max_lines(),
ai_inspector_auto_approve: defaults::ai_inspector_auto_approve(),
```

**Step 4: Build and test**

Run: `cargo build && cargo test config`
Expected: All pass, no errors.

**Step 5: Commit**

```bash
git add src/config/
git commit -m "feat(config): add AI inspector configuration fields"
```

---

### Task 2: Add `toml` dependency

**Files:**
- Modify: `Cargo.toml`

**Step 1: Add toml crate**

Add to `[dependencies]` section:
```toml
toml = "0.8"
```

**Step 2: Build**

Run: `cargo build`
Expected: Clean build.

**Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: add toml dependency for ACP agent configs"
```

---

### Task 3: Create snapshot data model

**Files:**
- Create: `src/ai_inspector/mod.rs`
- Create: `src/ai_inspector/snapshot.rs`
- Modify: `src/lib.rs` (add `pub mod ai_inspector;`)

**Step 1: Create module structure**

Create `src/ai_inspector/mod.rs`:
```rust
pub mod snapshot;
```

Add `pub mod ai_inspector;` to `src/lib.rs`.

**Step 2: Create snapshot types in `src/ai_inspector/snapshot.rs`**

```rust
use serde::{Deserialize, Serialize};

/// Scope for terminal state snapshots.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotScope {
    Visible,
    Recent(usize),
    Full,
}

impl SnapshotScope {
    /// Parse from config string like "visible", "recent_10", "full".
    pub fn from_config_str(s: &str) -> Self {
        if s == "visible" {
            Self::Visible
        } else if s == "full" {
            Self::Full
        } else if let Some(n) = s.strip_prefix("recent_") {
            Self::Recent(n.parse().unwrap_or(10))
        } else {
            Self::Visible
        }
    }

    pub fn to_config_str(&self) -> String {
        match self {
            Self::Visible => "visible".to_string(),
            Self::Recent(n) => format!("recent_{n}"),
            Self::Full => "full".to_string(),
        }
    }
}

/// A single command entry from shell integration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandEntry {
    pub command: String,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub cwd: Option<String>,
    pub output: Option<String>,
    pub output_line_count: usize,
}

/// Environment metadata from shell integration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EnvironmentInfo {
    pub hostname: Option<String>,
    pub username: Option<String>,
    pub cwd: Option<String>,
    pub shell: Option<String>,
}

/// Terminal dimensions and cursor state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalInfo {
    pub cols: usize,
    pub rows: usize,
    pub cursor: (usize, usize),
}

/// Complete terminal state snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotData {
    pub timestamp: String,
    pub scope: String,
    pub environment: EnvironmentInfo,
    pub terminal: TerminalInfo,
    pub commands: Vec<CommandEntry>,
}

impl SnapshotData {
    /// Serialize snapshot to pretty-printed JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scope_from_config_str() {
        assert_eq!(SnapshotScope::from_config_str("visible"), SnapshotScope::Visible);
        assert_eq!(SnapshotScope::from_config_str("full"), SnapshotScope::Full);
        assert_eq!(SnapshotScope::from_config_str("recent_10"), SnapshotScope::Recent(10));
        assert_eq!(SnapshotScope::from_config_str("recent_25"), SnapshotScope::Recent(25));
        assert_eq!(SnapshotScope::from_config_str("unknown"), SnapshotScope::Visible);
    }

    #[test]
    fn test_scope_roundtrip() {
        let scopes = vec![
            SnapshotScope::Visible,
            SnapshotScope::Full,
            SnapshotScope::Recent(10),
        ];
        for scope in scopes {
            let s = scope.to_config_str();
            assert_eq!(SnapshotScope::from_config_str(&s), scope);
        }
    }

    #[test]
    fn test_snapshot_to_json() {
        let snapshot = SnapshotData {
            timestamp: "2026-02-15T10:00:00Z".to_string(),
            scope: "visible".to_string(),
            environment: EnvironmentInfo {
                hostname: Some("test-host".to_string()),
                username: Some("user".to_string()),
                cwd: Some("/home/user".to_string()),
                shell: Some("zsh".to_string()),
            },
            terminal: TerminalInfo {
                cols: 80,
                rows: 24,
                cursor: (0, 0),
            },
            commands: vec![CommandEntry {
                command: "echo hello".to_string(),
                exit_code: Some(0),
                duration_ms: 100,
                cwd: Some("/home/user".to_string()),
                output: Some("hello\n".to_string()),
                output_line_count: 1,
            }],
        };
        let json = snapshot.to_json().unwrap();
        assert!(json.contains("echo hello"));
        assert!(json.contains("test-host"));
    }
}
```

**Step 3: Build and test**

Run: `cargo build && cargo test snapshot`
Expected: All 3 tests pass.

**Step 4: Commit**

```bash
git add src/ai_inspector/ src/lib.rs
git commit -m "feat(ai-inspector): add snapshot data model with serialization"
```

---

### Task 4: Implement snapshot gathering from TerminalManager

**Files:**
- Modify: `src/ai_inspector/snapshot.rs`

**Step 1: Add gather function**

Add to `snapshot.rs` (this uses `TerminalManager` to build snapshots):

```rust
use crate::terminal::TerminalManager;

impl SnapshotData {
    /// Gather a snapshot from the terminal manager.
    pub fn gather(
        terminal: &TerminalManager,
        scope: &SnapshotScope,
        max_output_lines: usize,
    ) -> Self {
        let history = terminal.get_command_history();
        let commands_to_include = match scope {
            SnapshotScope::Visible => {
                // Only commands visible on current screen — take last few
                let visible_rows = terminal.visible_rows();
                history.iter().rev().take(visible_rows).rev().cloned().collect::<Vec<_>>()
            }
            SnapshotScope::Recent(n) => {
                history.iter().rev().take(*n).rev().cloned().collect()
            }
            SnapshotScope::Full => history,
        };

        let commands: Vec<CommandEntry> = commands_to_include
            .into_iter()
            .map(|(cmd, exit_code, duration_ms)| {
                CommandEntry {
                    command: cmd,
                    exit_code,
                    duration_ms,
                    cwd: terminal.shell_integration_cwd(),
                    output: None, // Output gathering is expensive, defer to explicit request
                    output_line_count: 0,
                }
            })
            .collect();

        let (cursor_col, cursor_row) = terminal.cursor_position();

        let environment = EnvironmentInfo {
            hostname: terminal.shell_integration_hostname(),
            username: terminal.shell_integration_username(),
            cwd: terminal.shell_integration_cwd(),
            shell: None, // Shell name not directly available, could be inferred
        };

        let terminal_info = TerminalInfo {
            cols: terminal.cols(),
            rows: terminal.rows(),
            cursor: (cursor_col, cursor_row),
        };

        Self {
            timestamp: chrono::Utc::now().to_rfc3339(),
            scope: scope.to_config_str(),
            environment,
            terminal: terminal_info,
            commands,
        }
    }
}
```

**Note:** The `visible_rows()`, `cols()`, `rows()` methods may need to be added to `TerminalManager` if they don't exist. Check and add thin wrappers if needed. The gather function should compile — if `TerminalManager` is missing any of these methods, add them as thin wrappers around the inner `PtySession`.

**Step 2: Build**

Run: `cargo build`
Expected: Clean build (fix any missing method issues).

**Step 3: Commit**

```bash
git add src/ai_inspector/snapshot.rs src/terminal/mod.rs
git commit -m "feat(ai-inspector): implement snapshot gathering from TerminalManager"
```

---

## Phase 2: Panel UI Shell

### Task 5: Create basic AI Inspector panel UI

**Files:**
- Create: `src/ai_inspector/panel.rs`
- Modify: `src/ai_inspector/mod.rs`

**Step 1: Create panel struct and basic rendering**

Create `src/ai_inspector/panel.rs`:

```rust
use egui::{Color32, Context, Frame, Margin, Sense, Stroke};

use crate::ai_inspector::snapshot::{SnapshotData, SnapshotScope};

/// View mode for zone content display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Cards,
    Timeline,
    Tree,
    ListDetail,
}

impl ViewMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Cards => "Cards",
            Self::Timeline => "Timeline",
            Self::Tree => "Tree",
            Self::ListDetail => "List + Detail",
        }
    }

    pub fn all() -> &'static [ViewMode] {
        &[Self::Cards, Self::Timeline, Self::Tree, Self::ListDetail]
    }
}

/// Actions returned by the panel to the main event loop.
#[derive(Debug, Clone)]
pub enum InspectorAction {
    None,
    Close,
    CopyJson(String),
    SaveToFile(String),
    WriteToTerminal(String),
}

/// The AI Inspector side panel.
pub struct AIInspectorPanel {
    /// Whether the panel is open.
    pub open: bool,
    /// Panel width in pixels.
    pub width: f32,
    /// Minimum panel width.
    min_width: f32,
    /// Maximum panel width ratio (fraction of window width).
    max_width_ratio: f32,
    /// Whether the user is dragging the resize handle.
    resizing: bool,
    /// Current snapshot scope.
    pub scope: SnapshotScope,
    /// Current view mode.
    pub view_mode: ViewMode,
    /// Whether live update is enabled.
    pub live_update: bool,
    /// Whether zone content is shown.
    pub show_zones: bool,
    /// Current snapshot data (updated periodically).
    pub snapshot: Option<SnapshotData>,
    /// Whether snapshot needs refresh.
    pub needs_refresh: bool,
}

impl AIInspectorPanel {
    pub fn new(config: &crate::config::Config) -> Self {
        Self {
            open: false,
            width: config.ai_inspector_width,
            min_width: 200.0,
            max_width_ratio: 0.5,
            resizing: false,
            scope: SnapshotScope::from_config_str(&config.ai_inspector_default_scope),
            view_mode: match config.ai_inspector_view_mode.as_str() {
                "timeline" => ViewMode::Timeline,
                "tree" => ViewMode::Tree,
                "list_detail" => ViewMode::ListDetail,
                _ => ViewMode::Cards,
            },
            live_update: config.ai_inspector_live_update,
            show_zones: config.ai_inspector_show_zones,
            snapshot: None,
            needs_refresh: true,
        }
    }

    /// Returns true if panel just opened (caller should trigger auto-launch).
    pub fn toggle(&mut self) -> bool {
        self.open = !self.open;
        if self.open {
            self.needs_refresh = true;
        }
        self.open // true = just opened
    }

    /// Returns the width the panel consumes (0 if closed).
    pub fn consumed_width(&self) -> f32 {
        if self.open { self.width } else { 0.0 }
    }

    /// Render the panel. Returns an action for the main event loop.
    pub fn show(&mut self, ctx: &Context) -> InspectorAction {
        if !self.open {
            return InspectorAction::None;
        }

        let mut action = InspectorAction::None;
        let viewport = ctx.input(|i| i.screen_rect());
        let max_width = viewport.width() * self.max_width_ratio;
        self.width = self.width.clamp(self.min_width, max_width);

        let panel_rect = egui::Rect::from_min_size(
            egui::pos2(viewport.max.x - self.width, viewport.min.y),
            egui::vec2(self.width, viewport.height()),
        );

        // Background
        let panel_frame = Frame {
            inner_margin: Margin::same(8),
            fill: Color32::from_rgba_unmultiplied(24, 24, 24, 255),
            stroke: Stroke::new(1.0, Color32::from_gray(50)),
            ..Default::default()
        };

        egui::Area::new(egui::Id::new("ai_inspector_panel"))
            .fixed_pos(panel_rect.min)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.set_min_size(panel_rect.size());
                ui.set_max_size(panel_rect.size());

                panel_frame.show(ui, |ui| {
                    // Title bar
                    ui.horizontal(|ui| {
                        ui.heading("AI Inspector");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("✕").clicked() {
                                action = InspectorAction::Close;
                            }
                        });
                    });
                    ui.separator();

                    // Controls bar
                    ui.horizontal(|ui| {
                        // Scope selector
                        ui.label("Scope:");
                        egui::ComboBox::from_id_salt("scope_selector")
                            .selected_text(self.scope.to_config_str())
                            .show_ui(ui, |ui| {
                                let scopes = [
                                    SnapshotScope::Visible,
                                    SnapshotScope::Recent(5),
                                    SnapshotScope::Recent(10),
                                    SnapshotScope::Recent(25),
                                    SnapshotScope::Recent(50),
                                    SnapshotScope::Full,
                                ];
                                for s in &scopes {
                                    if ui.selectable_label(self.scope == *s, s.to_config_str()).clicked() {
                                        self.scope = s.clone();
                                        self.needs_refresh = true;
                                    }
                                }
                            });
                    });

                    ui.horizontal(|ui| {
                        // View mode selector
                        ui.label("View:");
                        egui::ComboBox::from_id_salt("view_selector")
                            .selected_text(self.view_mode.label())
                            .show_ui(ui, |ui| {
                                for mode in ViewMode::all() {
                                    if ui.selectable_label(self.view_mode == *mode, mode.label()).clicked() {
                                        self.view_mode = *mode;
                                    }
                                }
                            });

                        // Live/Paused toggle
                        let live_label = if self.live_update { "⏸" } else { "▶" };
                        if ui.small_button(live_label).on_hover_text(
                            if self.live_update { "Pause updates" } else { "Resume live updates" }
                        ).clicked() {
                            self.live_update = !self.live_update;
                        }

                        // Refresh button
                        if ui.small_button("🔄").on_hover_text("Refresh").clicked() {
                            self.needs_refresh = true;
                        }
                    });
                    ui.separator();

                    // Environment strip
                    if let Some(snapshot) = &self.snapshot {
                        let env = &snapshot.environment;
                        ui.horizontal_wrapped(|ui| {
                            if let Some(user) = &env.username {
                                if let Some(host) = &env.hostname {
                                    ui.label(format!("{user}@{host}"));
                                } else {
                                    ui.label(user.as_str());
                                }
                            }
                            if let Some(cwd) = &env.cwd {
                                ui.label(format!("📁 {cwd}"));
                            }
                        });
                        ui.horizontal(|ui| {
                            if let Some(shell) = &env.shell {
                                ui.label(format!("🐚 {shell}"));
                            }
                            ui.label(format!("{} commands", snapshot.commands.len()));
                        });
                        ui.separator();
                    }

                    // Zone content area
                    if self.show_zones {
                        self.render_zones(ui);
                    }

                    // Spacer to push action bar to bottom
                    ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                        // Action bar
                        ui.horizontal(|ui| {
                            if ui.button("📋 Copy JSON").clicked() {
                                if let Some(snapshot) = &self.snapshot {
                                    if let Ok(json) = snapshot.to_json() {
                                        action = InspectorAction::CopyJson(json);
                                    }
                                }
                            }
                            if ui.button("💾 Save").clicked() {
                                if let Some(snapshot) = &self.snapshot {
                                    if let Ok(json) = snapshot.to_json() {
                                        action = InspectorAction::SaveToFile(json);
                                    }
                                }
                            }
                        });
                        ui.separator();
                    });
                });
            });

        // Handle resize drag on left edge
        let resize_rect = egui::Rect::from_min_size(
            egui::pos2(panel_rect.min.x - 4.0, panel_rect.min.y),
            egui::vec2(8.0, panel_rect.height()),
        );
        let resize_response = ctx.interact(resize_rect, egui::Id::new("inspector_resize"), Sense::drag());
        if resize_response.dragged() {
            self.width = (viewport.max.x - ctx.input(|i| i.pointer.hover_pos().unwrap_or_default().x))
                .clamp(self.min_width, max_width);
            self.resizing = true;
        }
        if resize_response.drag_stopped() {
            self.resizing = false;
        }
        // Change cursor on hover
        if resize_response.hovered() || self.resizing {
            ctx.set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }

        // Handle escape to close
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            action = InspectorAction::Close;
        }

        action
    }

    fn render_zones(&self, ui: &mut egui::Ui) {
        let Some(snapshot) = &self.snapshot else {
            ui.label("No data available");
            return;
        };

        if snapshot.commands.is_empty() {
            ui.label("No commands recorded");
            return;
        }

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                match self.view_mode {
                    ViewMode::Cards => self.render_cards(ui, snapshot),
                    ViewMode::Timeline => self.render_timeline(ui, snapshot),
                    ViewMode::Tree => self.render_tree(ui, snapshot),
                    ViewMode::ListDetail => self.render_list_detail(ui, snapshot),
                }
            });
    }

    fn render_cards(&self, ui: &mut egui::Ui, snapshot: &SnapshotData) {
        for cmd in snapshot.commands.iter().rev() {
            let frame = Frame {
                inner_margin: Margin::same(6),
                fill: Color32::from_gray(32),
                stroke: Stroke::new(1.0, Color32::from_gray(50)),
                corner_radius: egui::CornerRadius::same(4),
                ..Default::default()
            };
            frame.show(ui, |ui| {
                // Command header
                ui.horizontal(|ui| {
                    ui.monospace(format!("$ {}", cmd.command));
                });
                ui.horizontal(|ui| {
                    // Exit code badge
                    match cmd.exit_code {
                        Some(0) => {
                            ui.colored_label(Color32::from_rgb(76, 175, 80), "✅ 0");
                        }
                        Some(code) => {
                            ui.colored_label(Color32::from_rgb(244, 67, 54), format!("❌ {code}"));
                        }
                        None => {
                            ui.colored_label(Color32::from_gray(128), "⏳ running");
                        }
                    }
                    if cmd.duration_ms > 0 {
                        let duration = if cmd.duration_ms >= 1000 {
                            format!("⏱ {:.1}s", cmd.duration_ms as f64 / 1000.0)
                        } else {
                            format!("⏱ {}ms", cmd.duration_ms)
                        };
                        ui.weak(duration);
                    }
                });
                if let Some(cwd) = &cmd.cwd {
                    ui.weak(format!("📁 {cwd}"));
                }
                // Collapsible output (placeholder — output gathering comes later)
                if let Some(output) = &cmd.output {
                    let lines = output.lines().count();
                    let id = egui::Id::new(format!("output_{}", cmd.command));
                    egui::CollapsingHeader::new(format!("Output ({lines} lines)"))
                        .id_salt(id)
                        .show(ui, |ui| {
                            ui.monospace(output);
                        });
                }
            });
            ui.add_space(4.0);
        }
    }

    fn render_timeline(&self, ui: &mut egui::Ui, snapshot: &SnapshotData) {
        for cmd in snapshot.commands.iter().rev() {
            ui.horizontal(|ui| {
                match cmd.exit_code {
                    Some(0) => { ui.colored_label(Color32::from_rgb(76, 175, 80), "✅"); }
                    Some(_) => { ui.colored_label(Color32::from_rgb(244, 67, 54), "❌"); }
                    None => { ui.colored_label(Color32::from_gray(128), "⏳"); }
                }
                ui.monospace(&cmd.command);
                if cmd.duration_ms > 0 {
                    ui.weak(format!("{:.1}s", cmd.duration_ms as f64 / 1000.0));
                }
            });
            ui.separator();
        }
    }

    fn render_tree(&self, ui: &mut egui::Ui, snapshot: &SnapshotData) {
        for cmd in snapshot.commands.iter().rev() {
            let header = format!("$ {}", cmd.command);
            egui::CollapsingHeader::new(header)
                .id_salt(egui::Id::new(format!("tree_{}", cmd.command)))
                .show(ui, |ui| {
                    ui.label(format!("Exit: {:?}", cmd.exit_code));
                    ui.label(format!("Duration: {}ms", cmd.duration_ms));
                    if let Some(cwd) = &cmd.cwd {
                        ui.label(format!("CWD: {cwd}"));
                    }
                    if let Some(output) = &cmd.output {
                        egui::CollapsingHeader::new("Output").show(ui, |ui| {
                            ui.monospace(output);
                        });
                    }
                });
        }
    }

    fn render_list_detail(&self, ui: &mut egui::Ui, snapshot: &SnapshotData) {
        // Simple list view for now — full split pane comes in a refinement pass
        for cmd in snapshot.commands.iter().rev() {
            ui.horizontal(|ui| {
                match cmd.exit_code {
                    Some(0) => { ui.colored_label(Color32::from_rgb(76, 175, 80), "✅"); }
                    Some(_) => { ui.colored_label(Color32::from_rgb(244, 67, 54), "❌"); }
                    None => { ui.colored_label(Color32::from_gray(128), "⏳"); }
                }
                ui.monospace(&cmd.command);
            });
        }
    }
}
```

**Step 2: Update `src/ai_inspector/mod.rs`**

```rust
pub mod panel;
pub mod snapshot;
```

**Step 3: Build**

Run: `cargo build`
Expected: Clean build.

**Step 4: Commit**

```bash
git add src/ai_inspector/
git commit -m "feat(ai-inspector): create panel UI with view modes and controls"
```

---

### Task 6: Integrate panel into WindowState

**Files:**
- Modify: `src/app/window_state.rs`
- Modify: `src/app/input_events.rs`

**Step 1: Add AIInspectorPanel field to WindowState**

In `src/app/window_state.rs`, add the import:
```rust
use crate::ai_inspector::panel::{AIInspectorPanel, InspectorAction};
```

Add field to `WindowState` struct (near line 134, alongside other UI fields):
```rust
pub(crate) ai_inspector: AIInspectorPanel,
```

Initialize in `WindowState::new()` (near line 341):
```rust
ai_inspector: AIInspectorPanel::new(&config),
```

**Step 2: Add panel rendering to the egui rendering section**

Find where `self.search_ui.show()` is called (~line 2341) and add nearby:
```rust
let inspector_action = self.ai_inspector.show(ctx);
match inspector_action {
    InspectorAction::Close => {
        self.ai_inspector.open = false;
        self.needs_redraw = true;
    }
    InspectorAction::CopyJson(json) => {
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            let _ = clipboard.set_text(json);
        }
    }
    InspectorAction::SaveToFile(json) => {
        // File save dialog
        if let Some(path) = rfd::FileDialog::new()
            .set_file_name(&format!(
                "par-term-snapshot-{}.json",
                chrono::Local::now().format("%Y-%m-%d-%H%M%S")
            ))
            .add_filter("JSON", &["json"])
            .save_file()
        {
            let _ = std::fs::write(path, json);
        }
    }
    InspectorAction::WriteToTerminal(cmd) => {
        if let Some(tab) = self.tab_manager.active_tab() {
            if let Ok(term) = tab.terminal.try_lock() {
                let _ = term.write(cmd.as_bytes());
            }
        }
    }
    InspectorAction::None => {}
}
```

**Step 3: Add snapshot refresh logic**

In the main rendering loop (near where terminal content is read), add snapshot refresh:
```rust
if self.ai_inspector.open && self.ai_inspector.needs_refresh {
    if let Some(tab) = self.tab_manager.active_tab() {
        if let Ok(term) = tab.terminal.try_lock() {
            let snapshot = crate::ai_inspector::snapshot::SnapshotData::gather(
                &term,
                &self.ai_inspector.scope,
                self.config.ai_inspector_context_max_lines,
            );
            self.ai_inspector.snapshot = Some(snapshot);
            self.ai_inspector.needs_refresh = false;
        }
    }
}
```

**Step 4: Add keybinding in `src/app/input_events.rs`**

Find the search keybinding section (~line 782) and add nearby for Cmd+I / Ctrl+Shift+I:

```rust
// AI Inspector toggle: Cmd+I (macOS) / Ctrl+Shift+I (other)
#[cfg(target_os = "macos")]
let is_inspector = {
    let cmd = self.input_handler.modifiers.state().super_key();
    cmd && !shift
        && matches!(event.logical_key, Key::Character(ref c) if c.eq_ignore_ascii_case("i"))
};
#[cfg(not(target_os = "macos"))]
let is_inspector = {
    let ctrl = self.input_handler.modifiers.state().control_key();
    ctrl && shift
        && matches!(event.logical_key, Key::Character(ref c) if c.eq_ignore_ascii_case("i"))
};

if is_inspector && self.config.ai_inspector_enabled {
    let just_opened = self.ai_inspector.toggle();
    self.needs_redraw = true;
    log::debug!("AI Inspector toggled: {}", self.ai_inspector.open);

    // Auto-launch agent when panel opens
    if just_opened && self.config.ai_inspector_auto_launch {
        let agent_id = self.config.ai_inspector_agent.clone();
        if !agent_id.is_empty() {
            // Trigger agent connection (handled in WindowState event loop)
            // Set pending_connect_agent = Some(agent_id)
        }
    }
    return true;
}
```

**Step 5: Build and run**

Run: `cargo build`
Expected: Clean build.

Manual test: Run debug build, press Cmd+I, verify panel appears on right side.

**Step 6: Commit**

```bash
git add src/app/window_state.rs src/app/input_events.rs
git commit -m "feat(ai-inspector): integrate panel into window state with keybinding"
```

---

### Task 7: Terminal reflow when panel opens/closes

**Files:**
- Modify: `src/app/window_state.rs` (or `src/renderer/mod.rs` depending on where grid_size is calculated)

**Step 1: Investigate grid_size calculation**

Find where `renderer.grid_size()` is implemented and understand how terminal columns are calculated from window width. The panel width needs to be subtracted from the available width before calculating columns.

**Step 2: Adjust column calculation**

The exact approach depends on how the renderer calculates grid size. The panel width should reduce the effective rendering area. Look for where `surface_width` or window width is used to compute columns, and subtract `self.ai_inspector.consumed_width()`.

This likely involves:
1. Passing the inspector's consumed width to the renderer
2. The renderer subtracting it from available width when computing grid dimensions
3. Triggering a terminal resize when the panel opens/closes/resizes

**Step 3: Trigger resize on panel toggle**

After toggling the inspector panel, trigger a terminal resize:
```rust
// After ai_inspector.toggle() or resize:
if let Some(renderer) = &self.renderer {
    let (cols, rows) = renderer.grid_size();
    // Resize terminal to new dimensions
    if let Some(tab) = self.tab_manager.active_tab() {
        if let Ok(term) = tab.terminal.try_lock() {
            let cell_width = renderer.cell_width();
            let cell_height = renderer.cell_height();
            let width_px = (cols as f32 * cell_width) as usize;
            let height_px = (rows as f32 * cell_height) as usize;
            let _ = term.resize_with_pixels(cols, rows, width_px, height_px);
        }
    }
}
```

**Note:** This task requires careful investigation of the renderer's layout code. The implementer should trace through `renderer.grid_size()` to understand exactly where to inject the panel width offset. This may require adding a `set_panel_width(f32)` method to the renderer.

**Step 4: Build and test manually**

Run: `cargo build && DEBUG_LEVEL=3 cargo run`
Test: Toggle panel, verify terminal reflows, run `tput cols` to confirm column count changes.

**Step 5: Commit**

```bash
git add src/app/window_state.rs src/renderer/
git commit -m "feat(ai-inspector): reflow terminal columns when panel opens/closes"
```

---

## Phase 3: ACP Protocol Implementation

### Task 8: Create JSON-RPC 2.0 client

**Files:**
- Create: `src/acp/mod.rs`
- Create: `src/acp/jsonrpc.rs`
- Modify: `src/lib.rs` (add `pub mod acp;`)

**Step 1: Create the ACP module**

Create `src/acp/mod.rs`:
```rust
pub mod jsonrpc;
pub mod protocol;
pub mod agent;
pub mod agents;
```

Add `pub mod acp;` to `src/lib.rs`.

**Step 2: Implement JSON-RPC client in `src/acp/jsonrpc.rs`**

```rust
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::sync::{mpsc, oneshot, Mutex};

/// A JSON-RPC 2.0 request.
#[derive(Debug, Serialize)]
pub struct Request {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
}

/// A JSON-RPC 2.0 response.
#[derive(Debug, Deserialize)]
pub struct Response {
    pub jsonrpc: String,
    pub id: Option<u64>,
    pub result: Option<Value>,
    pub error: Option<RpcError>,
}

/// A JSON-RPC 2.0 error.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RPC error {}: {}", self.code, self.message)
    }
}

impl std::error::Error for RpcError {}

/// An incoming message from the agent (could be response, notification, or RPC call).
#[derive(Debug, Deserialize)]
pub struct IncomingMessage {
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Option<u64>,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub params: Option<Value>,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub error: Option<RpcError>,
}

impl IncomingMessage {
    /// Is this a response to a previous request?
    pub fn is_response(&self) -> bool {
        self.result.is_some() || self.error.is_some()
    }

    /// Is this a notification (no id, has method)?
    pub fn is_notification(&self) -> bool {
        self.id.is_none() && self.method.is_some()
    }

    /// Is this an RPC call from the agent (has id and method)?
    pub fn is_rpc_call(&self) -> bool {
        self.id.is_some() && self.method.is_some()
    }
}

/// Manages JSON-RPC communication over stdio with an agent subprocess.
pub struct JsonRpcClient {
    /// Next request ID.
    next_id: AtomicU64,
    /// Writer to agent stdin.
    stdin: Arc<Mutex<ChildStdin>>,
    /// Pending requests waiting for responses.
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, RpcError>>>>>,
    /// Channel for incoming notifications and RPC calls.
    incoming_tx: mpsc::UnboundedSender<IncomingMessage>,
    /// Receiver for incoming messages (handed to consumer).
    incoming_rx: Option<mpsc::UnboundedReceiver<IncomingMessage>>,
}

impl JsonRpcClient {
    /// Create a new client from a child process's stdin/stdout.
    pub fn new(stdin: ChildStdin, stdout: ChildStdout) -> Self {
        let (incoming_tx, incoming_rx) = mpsc::unbounded_channel();
        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, RpcError>>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let client = Self {
            next_id: AtomicU64::new(1),
            stdin: Arc::new(Mutex::new(stdin)),
            pending: pending.clone(),
            incoming_tx: incoming_tx.clone(),
            incoming_rx: Some(incoming_rx),
        };

        // Spawn reader task
        let pending_clone = pending;
        let tx_clone = incoming_tx;
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => break, // EOF
                    Ok(_) => {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        match serde_json::from_str::<IncomingMessage>(trimmed) {
                            Ok(msg) => {
                                if msg.is_response() {
                                    // Route to pending request
                                    if let Some(id) = msg.id {
                                        let mut pending = pending_clone.lock().await;
                                        if let Some(sender) = pending.remove(&id) {
                                            let result = if let Some(err) = msg.error {
                                                Err(err)
                                            } else {
                                                Ok(msg.result.unwrap_or(Value::Null))
                                            };
                                            let _ = sender.send(result);
                                        }
                                    }
                                } else {
                                    // Notification or RPC call — send to consumer
                                    let _ = tx_clone.send(msg);
                                }
                            }
                            Err(e) => {
                                log::error!("Failed to parse JSON-RPC message: {e}: {trimmed}");
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Error reading from agent stdout: {e}");
                        break;
                    }
                }
            }
        });

        client
    }

    /// Take the incoming message receiver (can only be called once).
    pub fn take_incoming(&mut self) -> Option<mpsc::UnboundedReceiver<IncomingMessage>> {
        self.incoming_rx.take()
    }

    /// Send a request and wait for a response.
    pub async fn request(&self, method: &str, params: Option<Value>) -> Result<Value, RpcError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = Request {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
            id: Some(id),
        };

        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            pending.insert(id, tx);
        }

        // Send request
        let json = serde_json::to_string(&request).map_err(|e| RpcError {
            code: -32603,
            message: format!("Serialization error: {e}"),
            data: None,
        })?;

        {
            let mut stdin = self.stdin.lock().await;
            stdin
                .write_all(format!("{json}\n").as_bytes())
                .await
                .map_err(|e| RpcError {
                    code: -32603,
                    message: format!("Write error: {e}"),
                    data: None,
                })?;
            stdin.flush().await.map_err(|e| RpcError {
                code: -32603,
                message: format!("Flush error: {e}"),
                data: None,
            })?;
        }

        // Wait for response
        rx.await.map_err(|_| RpcError {
            code: -32603,
            message: "Response channel closed".to_string(),
            data: None,
        })?
    }

    /// Send a notification (no response expected).
    pub async fn notify(&self, method: &str, params: Option<Value>) -> Result<(), RpcError> {
        let request = Request {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
            id: None,
        };

        let json = serde_json::to_string(&request).map_err(|e| RpcError {
            code: -32603,
            message: format!("Serialization error: {e}"),
            data: None,
        })?;

        let mut stdin = self.stdin.lock().await;
        stdin
            .write_all(format!("{json}\n").as_bytes())
            .await
            .map_err(|e| RpcError {
                code: -32603,
                message: format!("Write error: {e}"),
                data: None,
            })?;
        stdin.flush().await.map_err(|e| RpcError {
            code: -32603,
            message: format!("Flush error: {e}"),
            data: None,
        })?;

        Ok(())
    }

    /// Send a response to an RPC call from the agent.
    pub async fn respond(&self, id: u64, result: Value) -> Result<(), RpcError> {
        let response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": result,
        });

        let json = serde_json::to_string(&response).map_err(|e| RpcError {
            code: -32603,
            message: format!("Serialization error: {e}"),
            data: None,
        })?;

        let mut stdin = self.stdin.lock().await;
        stdin
            .write_all(format!("{json}\n").as_bytes())
            .await
            .map_err(|e| RpcError {
                code: -32603,
                message: format!("Write error: {e}"),
                data: None,
            })?;
        stdin.flush().await.map_err(|e| RpcError {
            code: -32603,
            message: format!("Flush error: {e}"),
            data: None,
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_incoming_message_classification() {
        // Response
        let msg: IncomingMessage = serde_json::from_str(
            r#"{"jsonrpc":"2.0","id":1,"result":{"ok":true}}"#,
        ).unwrap();
        assert!(msg.is_response());
        assert!(!msg.is_notification());
        assert!(!msg.is_rpc_call());

        // Notification
        let msg: IncomingMessage = serde_json::from_str(
            r#"{"jsonrpc":"2.0","method":"session/update","params":{}}"#,
        ).unwrap();
        assert!(!msg.is_response());
        assert!(msg.is_notification());
        assert!(!msg.is_rpc_call());

        // RPC call from agent
        let msg: IncomingMessage = serde_json::from_str(
            r#"{"jsonrpc":"2.0","id":5,"method":"session/request_permission","params":{}}"#,
        ).unwrap();
        assert!(!msg.is_response());
        assert!(!msg.is_notification());
        assert!(msg.is_rpc_call());
    }

    #[test]
    fn test_request_serialization() {
        let req = Request {
            jsonrpc: "2.0".to_string(),
            method: "initialize".to_string(),
            params: Some(serde_json::json!({"protocolVersion": 1})),
            id: Some(1),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("initialize"));
        assert!(json.contains("protocolVersion"));
    }
}
```

**Step 3: Build and test**

Run: `cargo build && cargo test jsonrpc`
Expected: Tests pass.

**Step 4: Commit**

```bash
git add src/acp/ src/lib.rs
git commit -m "feat(acp): implement JSON-RPC 2.0 client over stdio"
```

---

### Task 9: Create ACP protocol types

**Files:**
- Create: `src/acp/protocol.rs`

**Step 1: Define ACP message types**

```rust
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Client capabilities sent during initialize.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientCapabilities {
    pub fs: FsCapabilities,
    pub terminal: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FsCapabilities {
    pub read_text_file: bool,
    pub write_text_file: bool,
}

/// Client info sent during initialize.
#[derive(Debug, Clone, Serialize)]
pub struct ClientInfo {
    pub name: String,
    pub title: String,
    pub version: String,
}

/// Initialize request params.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub protocol_version: u32,
    pub client_capabilities: ClientCapabilities,
    pub client_info: ClientInfo,
}

/// Initialize response.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub protocol_version: u32,
    pub agent_capabilities: Option<AgentCapabilities>,
    pub auth_methods: Option<Vec<AuthMethod>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCapabilities {
    pub load_session: Option<bool>,
    pub prompt_capabilities: Option<PromptCapabilities>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptCapabilities {
    pub audio: Option<bool>,
    pub embedded_content: Option<bool>,
    pub image: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthMethod {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

/// Session new/load request params.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionNewParams {
    pub cwd: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<Vec<Value>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionLoadParams {
    pub cwd: String,
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<Vec<Value>>,
}

/// Session new/load response.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionResult {
    pub session_id: String,
    pub modes: Option<ModesInfo>,
    pub models: Option<ModelsInfo>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModesInfo {
    pub current_mode_id: Option<String>,
    pub available_modes: Option<Vec<ModeEntry>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModeEntry {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelsInfo {
    pub current_model_id: Option<String>,
    pub available_models: Option<Vec<ModelEntry>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelEntry {
    pub model_id: String,
    pub name: String,
    pub description: Option<String>,
}

/// Content block types for prompts and messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    Resource {
        resource: ResourceContent,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceContent {
    pub uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blob: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Session prompt params.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPromptParams {
    pub session_id: String,
    pub prompt: Vec<ContentBlock>,
}

/// Session prompt response.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPromptResult {
    pub stop_reason: Option<String>,
}

/// Session update notification payload.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionUpdateParams {
    pub session_id: String,
    pub update: Value,
}

/// Parsed session update types.
#[derive(Debug, Clone)]
pub enum SessionUpdate {
    AgentMessageChunk { text: String },
    AgentThoughtChunk { text: String },
    UserMessageChunk { text: String },
    ToolCall(ToolCallInfo),
    ToolCallUpdate(ToolCallUpdateInfo),
    Plan(PlanInfo),
    AvailableCommandsUpdate(Vec<AgentCommand>),
    CurrentModeUpdate { mode_id: String },
    Unknown(Value),
}

impl SessionUpdate {
    /// Parse from raw update value.
    pub fn from_value(value: &Value) -> Self {
        let update_type = value.get("sessionUpdate").and_then(|v| v.as_str()).unwrap_or("");
        match update_type {
            "agent_message_chunk" => {
                let text = value
                    .get("content")
                    .and_then(|c| c.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string();
                Self::AgentMessageChunk { text }
            }
            "agent_thought_chunk" => {
                let text = value
                    .get("content")
                    .and_then(|c| c.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string();
                Self::AgentThoughtChunk { text }
            }
            "user_message_chunk" => {
                let text = value
                    .get("content")
                    .and_then(|c| c.get("text"))
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string();
                Self::UserMessageChunk { text }
            }
            "tool_call" => {
                let info = ToolCallInfo {
                    tool_call_id: value.get("toolCallId").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    title: value.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    kind: value.get("kind").and_then(|v| v.as_str()).unwrap_or("other").to_string(),
                    status: value.get("status").and_then(|v| v.as_str()).unwrap_or("pending").to_string(),
                    content: value.get("content").cloned(),
                };
                Self::ToolCall(info)
            }
            "tool_call_update" => {
                let info = ToolCallUpdateInfo {
                    tool_call_id: value.get("toolCallId").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    status: value.get("status").and_then(|v| v.as_str()).map(String::from),
                    title: value.get("title").and_then(|v| v.as_str()).map(String::from),
                    content: value.get("content").cloned(),
                };
                Self::ToolCallUpdate(info)
            }
            "plan" => {
                let entries = value
                    .get("entries")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|e| {
                                Some(PlanEntry {
                                    content: e.get("content")?.as_str()?.to_string(),
                                    status: e.get("status").and_then(|s| s.as_str()).unwrap_or("pending").to_string(),
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                Self::Plan(PlanInfo { entries })
            }
            "current_mode_update" => {
                let mode_id = value
                    .get("currentModeId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                Self::CurrentModeUpdate { mode_id }
            }
            _ => Self::Unknown(value.clone()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ToolCallInfo {
    pub tool_call_id: String,
    pub title: String,
    pub kind: String,
    pub status: String,
    pub content: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct ToolCallUpdateInfo {
    pub tool_call_id: String,
    pub status: Option<String>,
    pub title: Option<String>,
    pub content: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct PlanInfo {
    pub entries: Vec<PlanEntry>,
}

#[derive(Debug, Clone)]
pub struct PlanEntry {
    pub content: String,
    pub status: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgentCommand {
    pub name: String,
    pub description: Option<String>,
}

/// Permission request from agent.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestPermissionParams {
    pub session_id: String,
    pub tool_call: Value,
    pub options: Vec<PermissionOption>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionOption {
    pub option_id: String,
    pub name: String,
    pub kind: Option<String>,
}

/// Permission response to agent.
#[derive(Debug, Clone, Serialize)]
pub struct RequestPermissionResponse {
    pub outcome: PermissionOutcome,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionOutcome {
    pub outcome: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub option_id: Option<String>,
}

/// File read request from agent.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsReadParams {
    pub session_id: String,
    pub path: String,
    pub line: Option<u64>,
    pub limit: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_update_parse_agent_message() {
        let value = serde_json::json!({
            "sessionUpdate": "agent_message_chunk",
            "content": {
                "type": "text",
                "text": "Hello from agent"
            }
        });
        match SessionUpdate::from_value(&value) {
            SessionUpdate::AgentMessageChunk { text } => assert_eq!(text, "Hello from agent"),
            other => panic!("Expected AgentMessageChunk, got {:?}", other),
        }
    }

    #[test]
    fn test_session_update_parse_tool_call() {
        let value = serde_json::json!({
            "sessionUpdate": "tool_call",
            "toolCallId": "tc-1",
            "title": "Read file",
            "kind": "read",
            "status": "in_progress"
        });
        match SessionUpdate::from_value(&value) {
            SessionUpdate::ToolCall(info) => {
                assert_eq!(info.tool_call_id, "tc-1");
                assert_eq!(info.title, "Read file");
                assert_eq!(info.kind, "read");
            }
            other => panic!("Expected ToolCall, got {:?}", other),
        }
    }

    #[test]
    fn test_initialize_params_serialization() {
        let params = InitializeParams {
            protocol_version: 1,
            client_capabilities: ClientCapabilities {
                fs: FsCapabilities {
                    read_text_file: true,
                    write_text_file: false,
                },
                terminal: false,
            },
            client_info: ClientInfo {
                name: "par-term".to_string(),
                title: "Par Term".to_string(),
                version: "0.16.0".to_string(),
            },
        };
        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("protocolVersion"));
        assert!(json.contains("par-term"));
        assert!(json.contains("readTextFile"));
    }

    #[test]
    fn test_content_block_serialization() {
        let block = ContentBlock::Text {
            text: "Hello".to_string(),
        };
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains(r#""type":"text"#));

        let block = ContentBlock::Resource {
            resource: ResourceContent {
                uri: "file:///test.rs".to_string(),
                text: Some("fn main() {}".to_string()),
                blob: None,
                mime_type: Some("text/x-rust".to_string()),
            },
        };
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains("resource"));
        assert!(json.contains("file:///test.rs"));
    }
}
```

**Step 2: Build and test**

Run: `cargo build && cargo test protocol`
Expected: All tests pass.

**Step 3: Commit**

```bash
git add src/acp/protocol.rs
git commit -m "feat(acp): define ACP protocol message types"
```

---

### Task 10: Create agent discovery from TOML configs

**Files:**
- Create: `src/acp/agents.rs`
- Create: `agents/claude.com.toml` (bundled agent config)

**Step 1: Define agent config schema in `src/acp/agents.rs`**

```rust
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Agent configuration loaded from TOML.
#[derive(Debug, Clone, Deserialize)]
pub struct AgentConfig {
    pub identity: String,
    pub name: String,
    pub short_name: String,
    #[serde(default = "default_protocol")]
    pub protocol: String,
    #[serde(default = "default_type")]
    pub r#type: String,
    #[serde(default)]
    pub active: Option<bool>,
    pub run_command: HashMap<String, String>,
    #[serde(default)]
    pub actions: HashMap<String, HashMap<String, ActionConfig>>,
}

fn default_protocol() -> String { "acp".to_string() }
fn default_type() -> String { "coding".to_string() }

#[derive(Debug, Clone, Deserialize)]
pub struct ActionConfig {
    pub command: Option<String>,
    pub description: Option<String>,
}

impl AgentConfig {
    /// Get the run command for the current platform.
    pub fn run_command_for_platform(&self) -> Option<&str> {
        let platform = if cfg!(target_os = "macos") {
            "macos"
        } else if cfg!(target_os = "windows") {
            "windows"
        } else {
            "linux"
        };

        self.run_command
            .get(platform)
            .or_else(|| self.run_command.get("*"))
            .map(|s| s.as_str())
    }

    /// Whether this agent is active (defaults to true).
    pub fn is_active(&self) -> bool {
        self.active.unwrap_or(true)
    }
}

/// Discover available agents from bundled and user config directories.
pub fn discover_agents(user_config_dir: &Path) -> Vec<AgentConfig> {
    let mut agents = Vec::new();

    // Load bundled agents from the agents/ directory relative to the executable
    let bundled_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("agents")))
        .unwrap_or_else(|| PathBuf::from("agents"));

    load_agents_from_dir(&bundled_dir, &mut agents);

    // Load user agents (override bundled ones with same identity)
    let user_agents_dir = user_config_dir.join("agents");
    load_agents_from_dir(&user_agents_dir, &mut agents);

    // Filter to active agents
    agents.retain(|a| a.is_active());

    agents
}

fn load_agents_from_dir(dir: &Path, agents: &mut Vec<AgentConfig>) {
    if !dir.exists() {
        return;
    }

    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "toml") {
            match std::fs::read_to_string(&path) {
                Ok(content) => match toml::from_str::<AgentConfig>(&content) {
                    Ok(config) => {
                        // Remove existing agent with same identity (user overrides bundled)
                        agents.retain(|a| a.identity != config.identity);
                        agents.push(config);
                    }
                    Err(e) => {
                        log::error!("Failed to parse agent config {}: {e}", path.display());
                    }
                },
                Err(e) => {
                    log::error!("Failed to read agent config {}: {e}", path.display());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_agent_toml() {
        let toml_str = r#"
identity = "claude.com"
name = "Claude Code"
short_name = "claude"
protocol = "acp"
type = "coding"

[run_command]
"*" = "claude-code-acp"
macos = "claude-code-acp"
"#;
        let config: AgentConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.identity, "claude.com");
        assert_eq!(config.name, "Claude Code");
        assert!(config.is_active());
        assert!(config.run_command_for_platform().is_some());
    }

    #[test]
    fn test_inactive_agent() {
        let toml_str = r#"
identity = "test.agent"
name = "Test"
short_name = "test"
active = false

[run_command]
"*" = "test-agent"
"#;
        let config: AgentConfig = toml::from_str(toml_str).unwrap();
        assert!(!config.is_active());
    }
}
```

**Step 2: Create bundled agent config**

Create `agents/claude.com.toml`:
```toml
identity = "claude.com"
name = "Claude Code"
short_name = "claude"
protocol = "acp"
type = "coding"

[run_command]
"*" = "claude-code-acp"
```

**Step 3: Build and test**

Run: `cargo build && cargo test agents`
Expected: Tests pass.

**Step 4: Commit**

```bash
git add src/acp/agents.rs agents/
git commit -m "feat(acp): add agent discovery from TOML configs"
```

---

### Task 11: Create Agent lifecycle manager

**Files:**
- Create: `src/acp/agent.rs`

**Step 1: Implement Agent struct with spawn, initialize, session management**

```rust
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value;
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, Mutex};

use super::agents::AgentConfig;
use super::jsonrpc::{IncomingMessage, JsonRpcClient, RpcError};
use super::protocol::*;

/// Connection status of an ACP agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentStatus {
    Disconnected,
    Connecting,
    Connected,
    Error(String),
}

/// Messages from the agent to the UI.
#[derive(Debug, Clone)]
pub enum AgentMessage {
    StatusChanged(AgentStatus),
    SessionUpdate(SessionUpdate),
    PermissionRequest {
        request_id: u64,
        tool_call: Value,
        options: Vec<PermissionOption>,
    },
    FileReadRequest {
        request_id: u64,
        path: String,
        line: Option<u64>,
        limit: Option<u64>,
    },
}

/// Manages an ACP agent subprocess.
pub struct Agent {
    pub config: AgentConfig,
    pub status: AgentStatus,
    pub session_id: Option<String>,
    child: Option<Child>,
    client: Option<Arc<JsonRpcClient>>,
    /// Channel to send messages to the UI.
    ui_tx: mpsc::UnboundedSender<AgentMessage>,
    /// Whether to auto-approve permission requests (yolo mode).
    pub auto_approve: bool,
}

impl Agent {
    pub fn new(config: AgentConfig, ui_tx: mpsc::UnboundedSender<AgentMessage>) -> Self {
        Self {
            config,
            status: AgentStatus::Disconnected,
            session_id: None,
            child: None,
            client: None,
            ui_tx,
            auto_approve: false,
        }
    }

    /// Spawn the agent subprocess and perform ACP handshake.
    pub async fn connect(&mut self, cwd: &str) -> Result<(), String> {
        let run_command = self
            .config
            .run_command_for_platform()
            .ok_or_else(|| "No run command for this platform".to_string())?;

        self.status = AgentStatus::Connecting;
        let _ = self.ui_tx.send(AgentMessage::StatusChanged(self.status.clone()));

        // Spawn subprocess
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(run_command)
            .current_dir(cwd)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn agent: {e}"))?;

        let stdin = child.stdin.take().ok_or("No stdin")?;
        let stdout = child.stdout.take().ok_or("No stdout")?;

        let mut client = JsonRpcClient::new(stdin, stdout);
        let incoming_rx = client.take_incoming();
        let client = Arc::new(client);

        // Initialize
        let init_params = InitializeParams {
            protocol_version: 1,
            client_capabilities: ClientCapabilities {
                fs: FsCapabilities {
                    read_text_file: true,
                    write_text_file: false,
                },
                terminal: false,
            },
            client_info: ClientInfo {
                name: "par-term".to_string(),
                title: "Par Term".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        };

        let result = client
            .request(
                "initialize",
                Some(serde_json::to_value(&init_params).unwrap()),
            )
            .await
            .map_err(|e| format!("Initialize failed: {e}"))?;

        let _init_result: InitializeResult =
            serde_json::from_value(result).map_err(|e| format!("Bad init response: {e}"))?;

        // Create new session
        let session_params = SessionNewParams {
            cwd: cwd.to_string(),
            mcp_servers: Some(vec![]),
        };

        let result = client
            .request(
                "session/new",
                Some(serde_json::to_value(&session_params).unwrap()),
            )
            .await
            .map_err(|e| format!("Session creation failed: {e}"))?;

        let session_result: SessionResult =
            serde_json::from_value(result).map_err(|e| format!("Bad session response: {e}"))?;

        self.session_id = Some(session_result.session_id);
        self.child = Some(child);
        self.client = Some(client.clone());
        self.status = AgentStatus::Connected;
        let _ = self.ui_tx.send(AgentMessage::StatusChanged(self.status.clone()));

        // Spawn message handler task
        if let Some(mut rx) = incoming_rx {
            let ui_tx = self.ui_tx.clone();
            let auto_approve = self.auto_approve;
            let client_clone = client;
            tokio::spawn(async move {
                while let Some(msg) = rx.recv().await {
                    Self::handle_incoming(msg, &ui_tx, auto_approve, &client_clone).await;
                }
            });
        }

        Ok(())
    }

    async fn handle_incoming(
        msg: IncomingMessage,
        ui_tx: &mpsc::UnboundedSender<AgentMessage>,
        auto_approve: bool,
        client: &Arc<JsonRpcClient>,
    ) {
        if msg.is_notification() {
            if let Some(method) = &msg.method {
                if method == "session/update" {
                    if let Some(params) = &msg.params {
                        if let Some(update) = params.get("update") {
                            let session_update = SessionUpdate::from_value(update);
                            let _ = ui_tx.send(AgentMessage::SessionUpdate(session_update));
                        }
                    }
                }
            }
        } else if msg.is_rpc_call() {
            if let Some(method) = &msg.method {
                let request_id = msg.id.unwrap_or(0);
                match method.as_str() {
                    "session/request_permission" => {
                        if let Some(params) = &msg.params {
                            if auto_approve {
                                // Yolo mode: auto-approve with first "allow" option
                                if let Ok(perm) = serde_json::from_value::<RequestPermissionParams>(params.clone()) {
                                    let option_id = perm
                                        .options
                                        .iter()
                                        .find(|o| o.kind.as_deref() == Some("allow_once") || o.kind.as_deref() == Some("allow_always"))
                                        .or(perm.options.first())
                                        .map(|o| o.option_id.clone());
                                    let response = RequestPermissionResponse {
                                        outcome: PermissionOutcome {
                                            outcome: "selected".to_string(),
                                            option_id,
                                        },
                                    };
                                    let _ = client
                                        .respond(request_id, serde_json::to_value(&response).unwrap())
                                        .await;
                                }
                            } else {
                                // Send to UI for user decision
                                if let Ok(perm) = serde_json::from_value::<RequestPermissionParams>(params.clone()) {
                                    let _ = ui_tx.send(AgentMessage::PermissionRequest {
                                        request_id,
                                        tool_call: perm.tool_call,
                                        options: perm.options,
                                    });
                                }
                            }
                        }
                    }
                    "fs/read_text_file" => {
                        if let Some(params) = &msg.params {
                            if let Ok(fs_params) = serde_json::from_value::<FsReadParams>(params.clone()) {
                                let _ = ui_tx.send(AgentMessage::FileReadRequest {
                                    request_id,
                                    path: fs_params.path,
                                    line: fs_params.line,
                                    limit: fs_params.limit,
                                });
                            }
                        }
                    }
                    _ => {
                        // Unknown method — respond with error
                        let _ = client
                            .respond(
                                request_id,
                                serde_json::json!({"error": {"code": -32601, "message": "Method not found"}}),
                            )
                            .await;
                    }
                }
            }
        }
    }

    /// Send a prompt to the agent.
    pub async fn send_prompt(&self, content: Vec<ContentBlock>) -> Result<SessionPromptResult, String> {
        let client = self.client.as_ref().ok_or("Not connected")?;
        let session_id = self.session_id.as_ref().ok_or("No session")?;

        let params = SessionPromptParams {
            session_id: session_id.clone(),
            prompt: content,
        };

        let result = client
            .request(
                "session/prompt",
                Some(serde_json::to_value(&params).unwrap()),
            )
            .await
            .map_err(|e| format!("Prompt failed: {e}"))?;

        serde_json::from_value(result).map_err(|e| format!("Bad prompt response: {e}"))
    }

    /// Cancel current operation.
    pub async fn cancel(&self) -> Result<(), String> {
        let client = self.client.as_ref().ok_or("Not connected")?;
        let session_id = self.session_id.as_ref().ok_or("No session")?;

        client
            .notify(
                "session/cancel",
                Some(serde_json::json!({
                    "sessionId": session_id,
                    "_meta": {}
                })),
            )
            .await
            .map_err(|e| format!("Cancel failed: {e}"))?;

        Ok(())
    }

    /// Respond to a permission request.
    pub async fn respond_permission(
        &self,
        request_id: u64,
        option_id: Option<String>,
        cancelled: bool,
    ) -> Result<(), String> {
        let client = self.client.as_ref().ok_or("Not connected")?;

        let response = RequestPermissionResponse {
            outcome: PermissionOutcome {
                outcome: if cancelled {
                    "cancelled".to_string()
                } else {
                    "selected".to_string()
                },
                option_id,
            },
        };

        client
            .respond(request_id, serde_json::to_value(&response).unwrap())
            .await
            .map_err(|e| format!("Permission response failed: {e}"))?;

        Ok(())
    }

    /// Respond to a file read request.
    pub async fn respond_file_read(
        &self,
        request_id: u64,
        content: Result<String, String>,
    ) -> Result<(), String> {
        let client = self.client.as_ref().ok_or("Not connected")?;

        let response = match content {
            Ok(text) => serde_json::json!({"content": text}),
            Err(e) => serde_json::json!({"error": {"code": -32603, "message": e}}),
        };

        client
            .respond(request_id, response)
            .await
            .map_err(|e| format!("File read response failed: {e}"))?;

        Ok(())
    }

    /// Disconnect and kill the agent subprocess.
    pub async fn disconnect(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill().await;
        }
        self.client = None;
        self.session_id = None;
        self.status = AgentStatus::Disconnected;
        let _ = self.ui_tx.send(AgentMessage::StatusChanged(self.status.clone()));
    }

    pub fn is_connected(&self) -> bool {
        self.status == AgentStatus::Connected
    }
}

impl Drop for Agent {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            // Best-effort kill on drop
            let _ = child.start_kill();
        }
    }
}
```

**Step 2: Build**

Run: `cargo build`
Expected: Clean build.

**Step 3: Commit**

```bash
git add src/acp/agent.rs
git commit -m "feat(acp): implement agent lifecycle manager with ACP handshake"
```

---

## Phase 4: Chat UI and Integration

### Task 12: Add chat UI to the inspector panel

**Files:**
- Modify: `src/ai_inspector/panel.rs`
- Create: `src/ai_inspector/chat.rs`
- Modify: `src/ai_inspector/mod.rs`

**Step 1: Create chat data model in `src/ai_inspector/chat.rs`**

```rust
use crate::acp::protocol::SessionUpdate;

/// A message in the chat history.
#[derive(Debug, Clone)]
pub enum ChatMessage {
    User(String),
    Agent(String),
    Thinking(String),
    ToolCall {
        title: String,
        kind: String,
        status: String,
    },
    CommandSuggestion(String),
    Permission {
        request_id: u64,
        description: String,
        options: Vec<(String, String)>, // (option_id, label)
        resolved: bool,
    },
    AutoApproved(String),
    System(String),
}

/// Chat state for the agent conversation.
pub struct ChatState {
    pub messages: Vec<ChatMessage>,
    pub input: String,
    /// Whether a response is currently streaming.
    pub streaming: bool,
    /// Accumulator for streaming agent text.
    agent_text_buffer: String,
}

impl ChatState {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            input: String::new(),
            streaming: false,
            agent_text_buffer: String::new(),
        }
    }

    /// Process a session update from the agent.
    pub fn handle_update(&mut self, update: SessionUpdate) {
        match update {
            SessionUpdate::AgentMessageChunk { text } => {
                self.agent_text_buffer.push_str(&text);
                self.streaming = true;
                // Check for command suggestions (lines starting with `$ ` or backtick blocks)
                // This is a simple heuristic — could be made smarter
            }
            SessionUpdate::AgentThoughtChunk { text } => {
                // Append to last thinking message or create new
                if let Some(ChatMessage::Thinking(existing)) = self.messages.last_mut() {
                    existing.push_str(&text);
                } else {
                    self.messages.push(ChatMessage::Thinking(text));
                }
            }
            SessionUpdate::ToolCall(info) => {
                self.messages.push(ChatMessage::ToolCall {
                    title: info.title,
                    kind: info.kind,
                    status: info.status,
                });
            }
            SessionUpdate::ToolCallUpdate(info) => {
                // Update last matching tool call status
                for msg in self.messages.iter_mut().rev() {
                    if let ChatMessage::ToolCall { status, title, .. } = msg {
                        if let Some(new_status) = &info.status {
                            *status = new_status.clone();
                        }
                        if let Some(new_title) = &info.title {
                            *title = new_title.clone();
                        }
                        break;
                    }
                }
            }
            _ => {}
        }
    }

    /// Flush the streaming text buffer into a completed message.
    /// Call this when streaming ends (stop_reason received).
    pub fn flush_agent_message(&mut self) {
        if !self.agent_text_buffer.is_empty() {
            let text = std::mem::take(&mut self.agent_text_buffer);
            // Extract command suggestions from the text
            let mut remaining = String::new();
            for line in text.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("$ ") || trimmed.starts_with("```") {
                    // Don't extract code blocks as suggestions yet — keep it simple
                }
                remaining.push_str(line);
                remaining.push('\n');
            }
            self.messages.push(ChatMessage::Agent(remaining.trim_end().to_string()));
        }
        self.streaming = false;
    }

    /// Get the current streaming text (partial agent response).
    pub fn streaming_text(&self) -> &str {
        &self.agent_text_buffer
    }

    pub fn add_user_message(&mut self, text: String) {
        self.messages.push(ChatMessage::User(text));
    }

    pub fn add_system_message(&mut self, text: String) {
        self.messages.push(ChatMessage::System(text));
    }

    pub fn add_command_suggestion(&mut self, command: String) {
        self.messages.push(ChatMessage::CommandSuggestion(command));
    }

    pub fn add_auto_approved(&mut self, description: String) {
        self.messages.push(ChatMessage::AutoApproved(description));
    }
}
```

**Step 2: Update `src/ai_inspector/mod.rs`**

```rust
pub mod chat;
pub mod panel;
pub mod snapshot;
```

**Step 3: Add chat rendering to the panel**

Modify `src/ai_inspector/panel.rs` to add agent header and chat area. Add these fields to `AIInspectorPanel`:

```rust
/// Chat state for agent conversation.
pub chat: ChatState,
/// Selected agent identity.
pub selected_agent: String,
/// Agent connection status.
pub agent_status: AgentStatus,
/// Chat input field should request focus.
chat_request_focus: bool,
```

Add chat rendering between the zone content and action bar sections in `show()`. The chat section includes:
- Agent header with status indicator and yolo toggle
- Scrollable message list
- Input field with send button

The implementer should follow the pattern of the zone rendering but add the chat-specific message rendering (user messages right-aligned, agent messages left-aligned, tool calls as collapsible cards, command suggestions as clickable blocks).

**Step 4: Build**

Run: `cargo build`
Expected: Clean build.

**Step 5: Commit**

```bash
git add src/ai_inspector/
git commit -m "feat(ai-inspector): add chat UI for agent conversations"
```

---

### Task 13: Wire agent into WindowState

**Files:**
- Modify: `src/app/window_state.rs`

**Step 1: Add agent management**

Add fields to `WindowState`:
```rust
/// ACP agent message receiver.
pub(crate) agent_rx: Option<mpsc::UnboundedReceiver<AgentMessage>>,
/// ACP agent (managed via tokio).
pub(crate) agent: Option<Arc<tokio::sync::Mutex<Agent>>>,
/// Available agent configs.
pub(crate) available_agents: Vec<AgentConfig>,
```

**Step 2: Load available agents on initialization**

In `WindowState::new()`:
```rust
let config_dir = dirs::config_dir().unwrap_or_default().join("par-term");
available_agents: crate::acp::agents::discover_agents(&config_dir),
```

**Step 3: Process agent messages in the event loop**

Add to the main rendering loop, polling the agent message receiver:
```rust
// Process agent messages
if let Some(rx) = &mut self.agent_rx {
    while let Ok(msg) = rx.try_recv() {
        match msg {
            AgentMessage::StatusChanged(status) => {
                self.ai_inspector.agent_status = status;
                self.needs_redraw = true;
            }
            AgentMessage::SessionUpdate(update) => {
                self.ai_inspector.chat.handle_update(update);
                self.needs_redraw = true;
            }
            AgentMessage::PermissionRequest { request_id, tool_call, options } => {
                // Add permission prompt to chat
                let description = tool_call.get("title")
                    .and_then(|t| t.as_str())
                    .unwrap_or("Permission requested")
                    .to_string();
                self.ai_inspector.chat.messages.push(ChatMessage::Permission {
                    request_id,
                    description,
                    options: options.iter().map(|o| (o.option_id.clone(), o.name.clone())).collect(),
                    resolved: false,
                });
                self.needs_redraw = true;
            }
            AgentMessage::FileReadRequest { request_id, path, line, limit } => {
                // Read the file and respond
                // (Handle auto-approve or prompt user based on config)
                let content = std::fs::read_to_string(&path)
                    .map_err(|e| e.to_string());
                if let Some(agent) = &self.agent {
                    let agent = agent.clone();
                    tokio::spawn(async move {
                        let agent = agent.lock().await;
                        let _ = agent.respond_file_read(request_id, content).await;
                    });
                }
            }
        }
    }
}
```

**Step 4: Handle agent connect/disconnect from panel actions**

Add new `InspectorAction` variants for agent operations:
```rust
ConnectAgent(String), // agent identity
DisconnectAgent,
SendPrompt(String),
RespondPermission { request_id: u64, option_id: Option<String>, cancelled: bool },
```

Handle these in the action match in the event loop.

**Step 5: Build and test**

Run: `cargo build`
Expected: Clean build.

**Step 6: Commit**

```bash
git add src/app/window_state.rs src/ai_inspector/panel.rs
git commit -m "feat(ai-inspector): wire ACP agent into window state event loop"
```

---

### Task 14: Add auto-context feeding on command completion

**Files:**
- Modify: `src/app/window_state.rs`

**Step 1: Detect command completion**

In the main loop where shell integration events are processed, detect when a new command completes (exit code becomes available) and send context to the agent if auto mode is enabled:

```rust
// After processing shell integration events, check for command completion
if self.config.ai_inspector_auto_context
    && self.ai_inspector.open
    && self.ai_inspector.agent_status == AgentStatus::Connected
{
    // Check if a new command completed since last check
    if let Some(tab) = self.tab_manager.active_tab() {
        if let Ok(term) = tab.terminal.try_lock() {
            let history = term.get_command_history();
            let current_count = history.len();
            if current_count > self.ai_inspector.last_command_count {
                // New command completed — send context
                if let Some((cmd, exit_code, duration)) = history.last() {
                    let context = format!(
                        "Command completed:\n$ {}\nExit code: {}\nDuration: {}ms",
                        cmd,
                        exit_code.map(|c| c.to_string()).unwrap_or("N/A".to_string()),
                        duration
                    );
                    // Send to agent asynchronously
                    if let Some(agent) = &self.agent {
                        let agent = agent.clone();
                        let content = vec![ContentBlock::Text { text: context }];
                        tokio::spawn(async move {
                            let agent = agent.lock().await;
                            let _ = agent.send_prompt(content).await;
                        });
                    }
                }
                self.ai_inspector.last_command_count = current_count;
            }
        }
    }
}
```

Add `last_command_count: usize` field to `AIInspectorPanel`.

**Step 2: Build**

Run: `cargo build`
Expected: Clean build.

**Step 3: Commit**

```bash
git add src/app/window_state.rs src/ai_inspector/panel.rs
git commit -m "feat(ai-inspector): auto-feed terminal context to agent on command completion"
```

---

## Phase 5: Settings UI and Polish

### Task 15: Add Settings UI tab for AI Inspector

**Files:**
- Create: `src/settings_ui/ai_inspector_tab.rs`
- Modify: `src/settings_ui/mod.rs`
- Modify: `src/settings_ui/sidebar.rs`

**Step 1: Add tab to `SettingsTab` enum in `sidebar.rs`**

Add `AiInspector,` to the enum, `display_name()`, `icon()`, `all()`, and `tab_search_keywords()`:

```rust
// In enum:
AiInspector,

// In display_name():
Self::AiInspector => "AI Inspector",

// In icon():
Self::AiInspector => "🤖",

// In all():
Self::AiInspector,

// In tab_search_keywords():
SettingsTab::AiInspector => &[
    "ai", "inspector", "agent", "acp", "llm", "assistant",
    "snapshot", "zone", "command", "history", "context",
    "auto", "approve", "yolo", "live", "update", "scope",
    "cards", "timeline", "tree", "export", "json",
],
```

**Step 2: Create the tab UI in `src/settings_ui/ai_inspector_tab.rs`**

Follow the pattern of existing tabs (e.g., `window_tab.rs`). Include controls for all AI inspector config fields:
- Checkbox: `ai_inspector_enabled`
- Slider: `ai_inspector_width` (200-600)
- Dropdown: `ai_inspector_default_scope`
- Dropdown: `ai_inspector_view_mode`
- Checkbox: `ai_inspector_live_update`
- Checkbox: `ai_inspector_show_zones`
- Separator: "Agent Settings"
- Dropdown: `ai_inspector_agent` (populated from available agents)
- Checkbox: `ai_inspector_auto_context`
- Slider: `ai_inspector_context_max_lines` (50-1000)
- Checkbox with warning: `ai_inspector_auto_approve` ("Yolo Mode")

Each change should set `settings.has_changes = true` and `*changes_this_frame = true`.

**Step 3: Wire tab into settings_ui/mod.rs**

Add `mod ai_inspector_tab;` and the match arm in the tab rendering section:
```rust
SettingsTab::AiInspector => {
    ai_inspector_tab::show(ui, self, changes_this_frame, &mut collapsed);
}
```

**Step 4: Build**

Run: `cargo build`
Expected: Clean build.

**Step 5: Commit**

```bash
git add src/settings_ui/
git commit -m "feat(settings): add AI Inspector settings tab"
```

---

### Task 16: Add keybinding action for toggle

**Files:**
- Modify: `src/app/input_events.rs`

**Step 1: Add keybinding action handler**

Find the keybinding action match section (~line 1224) and add:

```rust
"toggle_ai_inspector" => {
    if self.config.ai_inspector_enabled {
        self.ai_inspector.toggle();
        self.needs_redraw = true;
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
    true
}
```

**Step 2: Build**

Run: `cargo build`
Expected: Clean build.

**Step 3: Commit**

```bash
git add src/app/input_events.rs
git commit -m "feat(keybindings): add toggle_ai_inspector action"
```

---

### Task 17: Final integration test and cleanup

**Step 1: Run full test suite**

Run: `make checkall` (or `cargo fmt -- --check && cargo clippy --all-targets -- -D warnings && cargo test`)
Expected: All pass.

**Step 2: Fix any clippy warnings or formatting issues**

Run: `cargo fmt && cargo clippy --fix --allow-dirty`

**Step 3: Manual testing checklist**

- [ ] Cmd+I opens/closes the panel
- [ ] Panel shows environment info (hostname, CWD, shell)
- [ ] Scope selector works (visible, recent, full)
- [ ] View mode selector works (cards, timeline, tree, list+detail)
- [ ] Zones can be hidden
- [ ] Copy JSON copies valid JSON to clipboard
- [ ] Save to file opens dialog and writes JSON
- [ ] Panel resize works by dragging left edge
- [ ] Terminal reflows columns when panel opens/closes
- [ ] Settings UI tab shows all AI inspector options
- [ ] Escape closes the panel
- [ ] Live/Paused toggle works

**Step 4: Commit final cleanup**

```bash
git add -A
git commit -m "feat(ai-inspector): complete AI terminal inspector with ACP agent integration"
```

---

## Task Dependencies

```
Task 1 (Config) ──→ Task 3 (Snapshot) ──→ Task 4 (Gather) ──→ Task 5 (Panel UI)
Task 2 (toml dep) ──→ Task 10 (Agents)
Task 5 (Panel UI) ──→ Task 6 (WindowState) ──→ Task 7 (Reflow) ──→ Task 17 (Final)
Task 8 (JSON-RPC) ──→ Task 9 (Protocol) ──→ Task 11 (Agent) ──→ Task 13 (Wire Agent)
Task 10 (Agents) ──→ Task 11 (Agent)
Task 12 (Chat UI) ──→ Task 13 (Wire Agent) ──→ Task 14 (Auto-context) ──→ Task 17 (Final)
Task 15 (Settings) ──→ Task 17 (Final)
Task 16 (Keybinding) ──→ Task 17 (Final)
```

**Parallel tracks:**
- Track A: Tasks 1→3→4→5→6→7 (Panel UI)
- Track B: Tasks 2→8→9→10→11 (ACP Protocol)
- Track C: Tasks 12→13→14 (Chat + Integration, depends on A+B)
- Track D: Tasks 15→16 (Settings, depends on A)
