//! AI Inspector side panel UI.
//!
//! Provides an egui-based side panel for viewing terminal state snapshots,
//! command history, and environment info. Supports multiple view modes
//! (Cards, Timeline, Tree, ListDetail) and interactive controls.

use egui::{
    Color32, Context, CursorIcon, Frame, Id, Key, Order, Pos2, Rect, RichText, Sense, Stroke, Vec2,
};

use crate::ai_inspector::snapshot::{CommandEntry, SnapshotData, SnapshotScope};
use crate::config::Config;

/// View mode for displaying snapshot data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Cards,
    Timeline,
    Tree,
    ListDetail,
}

impl ViewMode {
    /// Human-readable label for this view mode.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Cards => "Cards",
            Self::Timeline => "Timeline",
            Self::Tree => "Tree",
            Self::ListDetail => "List Detail",
        }
    }

    /// All available view modes.
    pub fn all() -> &'static [ViewMode] {
        &[
            ViewMode::Cards,
            ViewMode::Timeline,
            ViewMode::Tree,
            ViewMode::ListDetail,
        ]
    }

    /// Parse a view mode from config string.
    fn from_config_str(s: &str) -> Self {
        match s {
            "timeline" => Self::Timeline,
            "tree" => Self::Tree,
            "list_detail" => Self::ListDetail,
            _ => Self::Cards,
        }
    }
}

/// Actions returned from the inspector panel to the caller.
#[derive(Debug, Clone)]
pub enum InspectorAction {
    /// No action needed.
    None,
    /// Close the panel.
    Close,
    /// Copy JSON string to clipboard.
    CopyJson(String),
    /// Save JSON string to a file.
    SaveToFile(String),
    /// Write text into the active terminal.
    WriteToTerminal(String),
}

/// Predefined scope options for the dropdown.
struct ScopeOption {
    label: &'static str,
    scope: SnapshotScope,
}

const SCOPE_OPTIONS: &[ScopeOption] = &[
    ScopeOption {
        label: "Visible",
        scope: SnapshotScope::Visible,
    },
    ScopeOption {
        label: "Recent 5",
        scope: SnapshotScope::Recent(5),
    },
    ScopeOption {
        label: "Recent 10",
        scope: SnapshotScope::Recent(10),
    },
    ScopeOption {
        label: "Recent 25",
        scope: SnapshotScope::Recent(25),
    },
    ScopeOption {
        label: "Recent 50",
        scope: SnapshotScope::Recent(50),
    },
    ScopeOption {
        label: "Full",
        scope: SnapshotScope::Full,
    },
];

/// Width of the resize handle on the left edge of the panel.
const RESIZE_HANDLE_WIDTH: f32 = 8.0;

/// Panel background color (opaque dark).
const PANEL_BG: Color32 = Color32::from_rgba_premultiplied(24, 24, 24, 255);

/// Card background color.
const CARD_BG: Color32 = Color32::from_gray(32);

/// Card border stroke.
const CARD_BORDER: Stroke = Stroke {
    width: 1.0,
    color: Color32::from_gray(50),
};

/// Exit code success color (green).
const EXIT_SUCCESS: Color32 = Color32::from_rgb(76, 175, 80);

/// Exit code failure color (red).
const EXIT_FAILURE: Color32 = Color32::from_rgb(244, 67, 54);

/// AI Inspector side panel.
pub struct AIInspectorPanel {
    /// Whether the panel is open.
    pub open: bool,
    /// Current panel width in pixels.
    pub width: f32,
    /// Minimum panel width.
    min_width: f32,
    /// Maximum width as ratio of viewport width.
    max_width_ratio: f32,
    /// Whether the user is currently resizing via drag.
    resizing: bool,
    /// Current snapshot scope.
    pub scope: SnapshotScope,
    /// Current view mode.
    pub view_mode: ViewMode,
    /// Whether to auto-refresh on terminal changes.
    pub live_update: bool,
    /// Whether to show zone boundaries.
    pub show_zones: bool,
    /// Current snapshot data (populated by the app layer).
    pub snapshot: Option<SnapshotData>,
    /// Whether the panel needs a data refresh.
    pub needs_refresh: bool,
    /// Last known command count (for detecting changes).
    pub last_command_count: usize,
}

impl AIInspectorPanel {
    /// Create a new inspector panel initialized from config.
    pub fn new(config: &Config) -> Self {
        Self {
            open: false,
            width: config.ai_inspector_width,
            min_width: 200.0,
            max_width_ratio: 0.5,
            resizing: false,
            scope: SnapshotScope::from_config_str(&config.ai_inspector_default_scope),
            view_mode: ViewMode::from_config_str(&config.ai_inspector_view_mode),
            live_update: config.ai_inspector_live_update,
            show_zones: config.ai_inspector_show_zones,
            snapshot: None,
            needs_refresh: true,
            last_command_count: 0,
        }
    }

    /// Toggle the panel open/closed.
    ///
    /// Returns `true` if the panel was just opened (useful for auto-launch).
    pub fn toggle(&mut self) -> bool {
        self.open = !self.open;
        if self.open {
            self.needs_refresh = true;
        }
        self.open
    }

    /// Returns the width consumed by the panel (0 if closed).
    pub fn consumed_width(&self) -> f32 {
        if self.open { self.width } else { 0.0 }
    }

    /// Render the inspector panel and return any action to perform.
    pub fn show(&mut self, ctx: &Context) -> InspectorAction {
        if !self.open {
            return InspectorAction::None;
        }

        // Handle Escape key to close
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.open = false;
            return InspectorAction::Close;
        }

        let viewport = ctx.input(|i| i.viewport_rect());
        let max_width = viewport.width() * self.max_width_ratio;
        self.width = self.width.clamp(self.min_width, max_width);

        let panel_x = viewport.max.x - self.width;

        // --- Resize handle ---
        let handle_rect = Rect::from_min_size(
            Pos2::new(panel_x - RESIZE_HANDLE_WIDTH / 2.0, viewport.min.y),
            Vec2::new(RESIZE_HANDLE_WIDTH, viewport.height()),
        );
        egui::Area::new(Id::new("ai_inspector_resize_handle"))
            .fixed_pos(handle_rect.min)
            .order(Order::Foreground)
            .interactable(true)
            .show(ctx, |ui| {
                let (_, handle_response) = ui.allocate_exact_size(
                    Vec2::new(RESIZE_HANDLE_WIDTH, viewport.height()),
                    Sense::drag(),
                );
                if handle_response.hovered() || self.resizing {
                    ctx.set_cursor_icon(CursorIcon::ResizeHorizontal);
                }
                if handle_response.drag_started() {
                    self.resizing = true;
                }
                if self.resizing {
                    if handle_response.dragged() {
                        let delta = -handle_response.drag_delta().x;
                        self.width = (self.width + delta).clamp(self.min_width, max_width);
                    }
                    if handle_response.drag_stopped() {
                        self.resizing = false;
                    }
                }
            });

        // --- Main panel area ---
        let area_response = egui::Area::new(Id::new("ai_inspector_panel"))
            .fixed_pos(Pos2::new(panel_x, viewport.min.y))
            .order(Order::Foreground)
            .interactable(true)
            .show(ctx, |ui| {
                let mut close_requested = false;

                let panel_frame = Frame::new()
                    .fill(PANEL_BG)
                    .stroke(Stroke::new(1.0, Color32::from_gray(50)))
                    .inner_margin(8.0);

                let frame_response = panel_frame.show(ui, |ui| {
                    ui.set_min_size(Vec2::new(
                        self.width - 16.0, // account for inner margin
                        viewport.height() - 16.0,
                    ));
                    ui.set_max_width(self.width - 16.0);

                    // === Title bar ===
                    ui.horizontal(|ui| {
                        ui.heading(
                            RichText::new("AI Inspector")
                                .strong()
                                .color(Color32::from_gray(220)),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .button(RichText::new("\u{2715}").size(14.0))
                                .on_hover_text("Close (Escape)")
                                .clicked()
                            {
                                close_requested = true;
                            }
                        });
                    });

                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // === Controls row ===
                    self.render_controls(ui);

                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // === Environment strip ===
                    if let Some(ref snapshot) = self.snapshot {
                        self.render_environment(ui, snapshot);
                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(4.0);
                    }

                    // === Scrollable zone content ===
                    let available_height = ui.available_height() - 36.0; // reserve for action bar
                    egui::ScrollArea::vertical()
                        .max_height(available_height)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            if let Some(ref snapshot) = self.snapshot {
                                if snapshot.commands.is_empty() {
                                    ui.vertical_centered(|ui| {
                                        ui.add_space(20.0);
                                        ui.label(
                                            RichText::new("No commands captured yet")
                                                .color(Color32::from_gray(100))
                                                .italics(),
                                        );
                                        ui.add_space(4.0);
                                        ui.label(
                                            RichText::new(
                                                "Run some commands in the terminal\nto see them here.",
                                            )
                                            .color(Color32::from_gray(80))
                                            .small(),
                                        );
                                    });
                                } else {
                                    match self.view_mode {
                                        ViewMode::Cards => {
                                            Self::render_cards(ui, &snapshot.commands);
                                        }
                                        ViewMode::Timeline => {
                                            Self::render_timeline(ui, &snapshot.commands);
                                        }
                                        ViewMode::Tree => {
                                            Self::render_tree(ui, &snapshot.commands);
                                        }
                                        ViewMode::ListDetail => {
                                            Self::render_list_detail(ui, &snapshot.commands);
                                        }
                                    }
                                }
                            } else {
                                ui.vertical_centered(|ui| {
                                    ui.add_space(20.0);
                                    ui.label(
                                        RichText::new("No snapshot available")
                                            .color(Color32::from_gray(100))
                                            .italics(),
                                    );
                                    ui.add_space(4.0);
                                    ui.label(
                                        RichText::new("Click Refresh to capture terminal state.")
                                            .color(Color32::from_gray(80))
                                            .small(),
                                    );
                                });
                            }
                        });

                    // === Action bar (bottom) ===
                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(2.0);
                    self.render_action_bar(ui)
                });

                if close_requested {
                    InspectorAction::Close
                } else {
                    frame_response.inner
                }
            });

        let action = area_response.inner;

        // Handle close action
        if matches!(action, InspectorAction::Close) {
            self.open = false;
        }

        action
    }

    /// Render the controls row (scope, view mode, live/paused, refresh).
    fn render_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // Scope dropdown
            let current_scope_label = SCOPE_OPTIONS
                .iter()
                .find(|opt| opt.scope == self.scope)
                .map_or("Visible", |opt| opt.label);

            egui::ComboBox::from_id_salt("ai_inspector_scope")
                .selected_text(current_scope_label)
                .width(90.0)
                .show_ui(ui, |ui| {
                    for opt in SCOPE_OPTIONS {
                        if ui
                            .selectable_label(self.scope == opt.scope, opt.label)
                            .clicked()
                        {
                            self.scope = opt.scope.clone();
                            self.needs_refresh = true;
                        }
                    }
                });

            // View mode dropdown
            egui::ComboBox::from_id_salt("ai_inspector_view_mode")
                .selected_text(self.view_mode.label())
                .width(80.0)
                .show_ui(ui, |ui| {
                    for mode in ViewMode::all() {
                        if ui
                            .selectable_label(self.view_mode == *mode, mode.label())
                            .clicked()
                        {
                            self.view_mode = *mode;
                        }
                    }
                });
        });

        ui.horizontal(|ui| {
            // Live/Paused toggle
            let live_label = if self.live_update {
                RichText::new("\u{25CF} Live").color(EXIT_SUCCESS).small()
            } else {
                RichText::new("\u{25CB} Paused")
                    .color(Color32::from_gray(140))
                    .small()
            };
            if ui
                .button(live_label)
                .on_hover_text(if self.live_update {
                    "Click to pause auto-refresh"
                } else {
                    "Click to enable auto-refresh"
                })
                .clicked()
            {
                self.live_update = !self.live_update;
            }

            // Refresh button
            if ui
                .button(RichText::new("\u{21BB} Refresh").small())
                .on_hover_text("Refresh snapshot now")
                .clicked()
            {
                self.needs_refresh = true;
            }
        });
    }

    /// Render the environment info strip.
    fn render_environment(&self, ui: &mut egui::Ui, snapshot: &SnapshotData) {
        let env = &snapshot.environment;
        let dim_color = Color32::from_gray(120);
        let val_color = Color32::from_gray(190);

        ui.horizontal_wrapped(|ui| {
            // user@host
            if let Some(ref user) = env.username {
                ui.label(RichText::new(user).color(val_color).small());
                if env.hostname.is_some() {
                    ui.label(RichText::new("@").color(dim_color).small());
                }
            }
            if let Some(ref host) = env.hostname {
                ui.label(RichText::new(host).color(val_color).small());
            }

            // Separator
            if env.username.is_some() || env.hostname.is_some() {
                ui.label(RichText::new("\u{2502}").color(dim_color).small());
            }

            // CWD
            if let Some(ref cwd) = env.cwd {
                ui.label(RichText::new(cwd).color(val_color).small());
            }
        });

        ui.horizontal(|ui| {
            // Shell
            if let Some(ref shell) = env.shell {
                ui.label(RichText::new("Shell:").color(dim_color).small());
                ui.label(RichText::new(shell).color(val_color).small());
                ui.label(RichText::new("\u{2502}").color(dim_color).small());
            }

            // Command count
            let cmd_count = snapshot.commands.len();
            ui.label(RichText::new("Commands:").color(dim_color).small());
            ui.label(
                RichText::new(cmd_count.to_string())
                    .color(val_color)
                    .small(),
            );
        });
    }

    /// Render cards view: each command in a framed card.
    fn render_cards(ui: &mut egui::Ui, commands: &[CommandEntry]) {
        for (i, cmd) in commands.iter().enumerate() {
            let card_frame = Frame::new()
                .fill(CARD_BG)
                .stroke(CARD_BORDER)
                .corner_radius(4.0)
                .inner_margin(8.0);

            card_frame.show(ui, |ui| {
                ui.set_min_width(ui.available_width());

                // Command text
                ui.label(
                    RichText::new(&cmd.command)
                        .color(Color32::from_gray(230))
                        .monospace(),
                );

                ui.add_space(4.0);

                // Exit code badge + duration
                ui.horizontal(|ui| {
                    if let Some(code) = cmd.exit_code {
                        let (color, text) = if code == 0 {
                            (EXIT_SUCCESS, format!("\u{2713} {code}"))
                        } else {
                            (EXIT_FAILURE, format!("\u{2717} {code}"))
                        };
                        ui.label(RichText::new(text).color(color).small().strong());
                    }

                    // Duration
                    let duration_str = format_duration(cmd.duration_ms);
                    ui.label(
                        RichText::new(duration_str)
                            .color(Color32::from_gray(120))
                            .small(),
                    );
                });

                // CWD if present
                if let Some(ref cwd) = cmd.cwd {
                    ui.label(
                        RichText::new(cwd)
                            .color(Color32::from_gray(90))
                            .small()
                            .italics(),
                    );
                }

                // Collapsible output
                if let Some(ref output) = cmd.output
                    && !output.is_empty()
                {
                    egui::CollapsingHeader::new(
                        RichText::new("Output")
                            .color(Color32::from_gray(140))
                            .small(),
                    )
                    .id_salt(format!("card_output_{i}"))
                    .show(ui, |ui| {
                        let truncated = truncate_output(output, 20);
                        ui.label(
                            RichText::new(truncated)
                                .color(Color32::from_gray(160))
                                .monospace()
                                .small(),
                        );
                    });
                }
            });

            ui.add_space(4.0);
        }
    }

    /// Render timeline view: flat list with icons and durations.
    fn render_timeline(ui: &mut egui::Ui, commands: &[CommandEntry]) {
        for (i, cmd) in commands.iter().enumerate() {
            ui.horizontal(|ui| {
                // Status icon
                let icon = match cmd.exit_code {
                    Some(0) => RichText::new("\u{25CF}").color(EXIT_SUCCESS),
                    Some(_) => RichText::new("\u{25CF}").color(EXIT_FAILURE),
                    None => RichText::new("\u{25CB}").color(Color32::from_gray(100)),
                };
                ui.label(icon);

                // Command text
                ui.label(
                    RichText::new(&cmd.command)
                        .color(Color32::from_gray(210))
                        .monospace(),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format_duration(cmd.duration_ms))
                            .color(Color32::from_gray(100))
                            .small(),
                    );
                });
            });

            // Separator between entries
            if i < commands.len() - 1 {
                ui.separator();
            }
        }
    }

    /// Render tree view: collapsing headers per command with detail children.
    fn render_tree(ui: &mut egui::Ui, commands: &[CommandEntry]) {
        for (i, cmd) in commands.iter().enumerate() {
            let header_text = if cmd.command.len() > 40 {
                format!("{}...", &cmd.command[..40])
            } else {
                cmd.command.clone()
            };

            egui::CollapsingHeader::new(
                RichText::new(header_text)
                    .color(Color32::from_gray(210))
                    .monospace(),
            )
            .id_salt(format!("tree_cmd_{i}"))
            .show(ui, |ui| {
                // Exit code
                if let Some(code) = cmd.exit_code {
                    let (color, label) = if code == 0 {
                        (EXIT_SUCCESS, "Success")
                    } else {
                        (EXIT_FAILURE, "Failed")
                    };
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Exit:")
                                .color(Color32::from_gray(120))
                                .small(),
                        );
                        ui.label(
                            RichText::new(format!("{code} ({label})"))
                                .color(color)
                                .small(),
                        );
                    });
                }

                // Duration
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Duration:")
                            .color(Color32::from_gray(120))
                            .small(),
                    );
                    ui.label(
                        RichText::new(format_duration(cmd.duration_ms))
                            .color(Color32::from_gray(180))
                            .small(),
                    );
                });

                // CWD
                if let Some(ref cwd) = cmd.cwd {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("CWD:").color(Color32::from_gray(120)).small());
                        ui.label(RichText::new(cwd).color(Color32::from_gray(180)).small());
                    });
                }

                // Output
                if let Some(ref output) = cmd.output
                    && !output.is_empty()
                {
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new("Output:")
                            .color(Color32::from_gray(120))
                            .small(),
                    );
                    let truncated = truncate_output(output, 20);
                    ui.label(
                        RichText::new(truncated)
                            .color(Color32::from_gray(160))
                            .monospace()
                            .small(),
                    );
                }
            });
        }
    }

    /// Render list detail view: simple list with icon and command text.
    fn render_list_detail(ui: &mut egui::Ui, commands: &[CommandEntry]) {
        for cmd in commands {
            ui.horizontal(|ui| {
                // Status icon
                let icon = match cmd.exit_code {
                    Some(0) => RichText::new("\u{2713}").color(EXIT_SUCCESS),
                    Some(_) => RichText::new("\u{2717}").color(EXIT_FAILURE),
                    None => RichText::new("\u{2022}").color(Color32::from_gray(100)),
                };
                ui.label(icon);

                // Command text
                ui.label(
                    RichText::new(&cmd.command)
                        .color(Color32::from_gray(210))
                        .monospace(),
                );
            });
        }
    }

    /// Render the action bar at the bottom of the panel.
    fn render_action_bar(&self, ui: &mut egui::Ui) -> InspectorAction {
        let mut action = InspectorAction::None;

        ui.horizontal(|ui| {
            // Copy JSON button
            if ui
                .button(RichText::new("\u{1F4CB} Copy JSON").small())
                .on_hover_text("Copy snapshot as JSON to clipboard")
                .clicked()
                && let Some(ref snapshot) = self.snapshot
                && let Ok(json) = snapshot.to_json()
            {
                action = InspectorAction::CopyJson(json);
            }

            // Save to file button
            if ui
                .button(RichText::new("\u{1F4BE} Save").small())
                .on_hover_text("Save snapshot JSON to file")
                .clicked()
                && let Some(ref snapshot) = self.snapshot
                && let Ok(json) = snapshot.to_json()
            {
                action = InspectorAction::SaveToFile(json);
            }
        });

        action
    }
}

/// Format a duration in milliseconds to a human-readable string.
fn format_duration(ms: u64) -> String {
    if ms < 1000 {
        format!("{ms}ms")
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else {
        let minutes = ms / 60_000;
        let seconds = (ms % 60_000) / 1000;
        format!("{minutes}m {seconds}s")
    }
}

/// Truncate output text to a maximum number of lines.
fn truncate_output(output: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = output.lines().take(max_lines + 1).collect();
    if lines.len() > max_lines {
        let mut result: String = lines[..max_lines].join("\n");
        result.push_str("\n... (truncated)");
        result
    } else {
        output.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_view_mode_label() {
        assert_eq!(ViewMode::Cards.label(), "Cards");
        assert_eq!(ViewMode::Timeline.label(), "Timeline");
        assert_eq!(ViewMode::Tree.label(), "Tree");
        assert_eq!(ViewMode::ListDetail.label(), "List Detail");
    }

    #[test]
    fn test_view_mode_all() {
        let all = ViewMode::all();
        assert_eq!(all.len(), 4);
    }

    #[test]
    fn test_view_mode_from_config_str() {
        assert_eq!(ViewMode::from_config_str("cards"), ViewMode::Cards);
        assert_eq!(ViewMode::from_config_str("timeline"), ViewMode::Timeline);
        assert_eq!(ViewMode::from_config_str("tree"), ViewMode::Tree);
        assert_eq!(
            ViewMode::from_config_str("list_detail"),
            ViewMode::ListDetail
        );
        assert_eq!(ViewMode::from_config_str("unknown"), ViewMode::Cards);
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(0), "0ms");
        assert_eq!(format_duration(500), "500ms");
        assert_eq!(format_duration(1000), "1.0s");
        assert_eq!(format_duration(1500), "1.5s");
        assert_eq!(format_duration(60000), "1m 0s");
        assert_eq!(format_duration(90000), "1m 30s");
    }

    #[test]
    fn test_truncate_output() {
        let short = "line1\nline2\nline3";
        assert_eq!(truncate_output(short, 5), short);

        let long = (0..30)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let truncated = truncate_output(&long, 5);
        assert!(truncated.ends_with("... (truncated)"));
        // 5 lines + the truncation notice
        assert_eq!(truncated.lines().count(), 6);
    }

    #[test]
    fn test_inspector_panel_toggle() {
        let config = Config::default();
        let mut panel = AIInspectorPanel::new(&config);
        assert!(!panel.open);
        assert_eq!(panel.consumed_width(), 0.0);

        let opened = panel.toggle();
        assert!(opened);
        assert!(panel.open);
        assert!(panel.needs_refresh);
        assert!(panel.consumed_width() > 0.0);

        let opened = panel.toggle();
        assert!(!opened);
        assert!(!panel.open);
        assert_eq!(panel.consumed_width(), 0.0);
    }

    #[test]
    fn test_inspector_panel_new_from_config() {
        let config = Config::default();
        let panel = AIInspectorPanel::new(&config);
        assert!(!panel.open);
        assert_eq!(panel.width, 300.0);
        assert_eq!(panel.scope, SnapshotScope::Visible);
        assert_eq!(panel.view_mode, ViewMode::Cards);
        assert!(panel.live_update);
        assert!(panel.show_zones);
    }
}
