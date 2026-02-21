//! AI Inspector side panel UI.
//!
//! Provides an egui-based side panel for viewing terminal state snapshots,
//! command history, and environment info. Supports multiple view modes
//! (Cards, Timeline, Tree, ListDetail) and interactive controls.

use egui::{Color32, Context, CursorIcon, Frame, Id, Key, Label, Order, Pos2, RichText, Stroke};

use crate::ai_inspector::chat::{parse_text_segments, ChatMessage, ChatState, TextSegment};
use crate::ai_inspector::snapshot::{CommandEntry, SnapshotData, SnapshotScope};
use crate::config::Config;
use par_term_acp::{AgentConfig, AgentStatus};

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
    /// Run a command in the terminal AND notify the agent it was executed.
    RunCommandAndNotify(String),
    /// Connect to an agent by identity string.
    ConnectAgent(String),
    /// Disconnect from the current agent.
    DisconnectAgent,
    /// Send a user prompt to the connected agent.
    SendPrompt(String),
    /// Toggle agent terminal access.
    SetTerminalAccess(bool),
    /// Respond to an agent permission request.
    RespondPermission {
        request_id: u64,
        option_id: String,
        cancelled: bool,
    },
    /// Set the agent's session mode (e.g. "bypassPermissions").
    SetAgentMode(String),
    /// Cancel the current agent prompt.
    CancelPrompt,
    /// Cancel the most recent queued (unsent) user prompt.
    CancelQueuedPrompt,
    /// Clear all chat messages.
    ClearChat,
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

/// User message background.
const USER_MSG_BG: Color32 = Color32::from_rgb(30, 50, 70);

/// Agent message background.
const AGENT_MSG_BG: Color32 = Color32::from_rgb(35, 35, 40);

/// System message color.
const SYSTEM_MSG_COLOR: Color32 = Color32::from_gray(110);

/// Command suggestion background.
const CMD_SUGGEST_BG: Color32 = Color32::from_rgb(40, 45, 30);

/// Code block background.
const CODE_BLOCK_BG: Color32 = Color32::from_rgb(18, 18, 24);

/// Code block language tag color.
const CODE_LANG_COLOR: Color32 = Color32::from_gray(90);

/// Connected status color.
const AGENT_CONNECTED: Color32 = Color32::from_rgb(76, 175, 80);

/// Disconnected status color.
const AGENT_DISCONNECTED: Color32 = Color32::from_gray(100);

/// AI Inspector side panel.
pub struct AIInspectorPanel {
    /// Whether the panel is open.
    pub open: bool,
    /// Current panel width in pixels (configured/drag-resized).
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
    /// Current agent connection status.
    pub agent_status: AgentStatus,
    /// Chat state for the agent conversation.
    pub chat: ChatState,
    /// Whether the agent is allowed to write to the terminal.
    pub agent_terminal_access: bool,
    /// Whether to auto-approve all agent permission requests (YOLO mode).
    pub auto_approve: bool,
    /// Actual rendered width from the last egui frame (may exceed `width` if content overflows).
    rendered_width: f32,
    /// Whether the pointer is hovering over the resize handle (persists between frames
    /// so `is_egui_using_pointer` can block the initial mouse press from reaching the terminal).
    hover_resize_handle: bool,
    /// Maximum allowed width in pixels (computed from viewport * max_width_ratio).
    max_width: f32,
    /// Selected agent index for the multi-agent dropdown.
    selected_agent_index: usize,
}

impl AIInspectorPanel {
    /// Create a new inspector panel initialized from config.
    pub fn new(config: &Config) -> Self {
        Self {
            open: config.ai_inspector_open_on_startup,
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
            agent_status: AgentStatus::Disconnected,
            chat: ChatState::new(),
            agent_terminal_access: config.ai_inspector_agent_terminal_access,
            auto_approve: config.ai_inspector_auto_approve,
            rendered_width: 0.0,
            hover_resize_handle: false,
            max_width: 0.0,
            selected_agent_index: 0,
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
    ///
    /// Uses the actual rendered width (which may exceed the configured `self.width`
    /// if content overflows) to ensure the terminal insets correctly.
    /// Clamps to the maximum allowed width to prevent the panel from taking
    /// over the entire window.
    pub fn consumed_width(&self) -> f32 {
        if self.open {
            let raw_width = self.rendered_width.max(self.width);
            if self.max_width > 0.0 {
                raw_width.min(self.max_width)
            } else {
                raw_width
            }
        } else {
            0.0
        }
    }

    /// Whether the user is currently drag-resizing the panel.
    pub fn is_resizing(&self) -> bool {
        self.resizing
    }

    /// Whether the pointer is interacting with the resize handle (hovering or dragging).
    /// Used by `is_egui_using_pointer()` to block mouse events from reaching the terminal.
    pub fn wants_pointer(&self) -> bool {
        self.resizing || self.hover_resize_handle
    }

    /// Render the inspector panel and return any action to perform.
    pub fn show(&mut self, ctx: &Context, available_agents: &[AgentConfig]) -> InspectorAction {
        if !self.open {
            return InspectorAction::None;
        }

        // Handle Escape key to close — but only when no text input or popup has focus
        let any_text_focused = ctx.memory(|m| m.focused().is_some()) && {
            // Check if the focused widget is a text edit (chat input)
            let focus_id = ctx.memory(|m| m.focused());
            focus_id.is_some()
        };
        if ctx.input(|i| i.key_pressed(Key::Escape)) && !any_text_focused {
            self.open = false;
            return InspectorAction::Close;
        }

        let viewport = ctx.input(|i| i.viewport_rect());
        let max_width = viewport.width() * self.max_width_ratio;
        self.max_width = max_width;
        self.width = self.width.clamp(self.min_width, max_width);

        // --- Resize handle input (BEFORE panel rendering so width updates this frame) ---
        // Use previous frame's consumed_width for hover detection (imperceptible 1-frame lag).
        let prev_panel_x = viewport.max.x - self.consumed_width();
        let handle_left = prev_panel_x - RESIZE_HANDLE_WIDTH / 2.0;
        let handle_right = prev_panel_x + RESIZE_HANDLE_WIDTH / 2.0;
        let pointer_pos = ctx.input(|i| i.pointer.hover_pos());
        let hover = pointer_pos.is_some_and(|pos| {
            pos.x >= handle_left
                && pos.x <= handle_right
                && pos.y >= viewport.min.y
                && pos.y <= viewport.max.y
        });

        let primary_pressed = ctx.input(|i| i.pointer.primary_pressed());
        let primary_down = ctx.input(|i| i.pointer.primary_down());
        let delta = ctx.input(|i| i.pointer.delta());

        if hover && primary_pressed {
            self.resizing = true;
        }
        if self.resizing {
            if primary_down {
                let old_width = self.width;
                self.width = (self.width - delta.x).clamp(self.min_width, max_width);
                // Apply the same clamped delta to rendered_width so consumed_width()
                // moves in lockstep with the drag. This avoids a jump at drag start
                // (when rendered_width > self.width due to content overflow) and also
                // prevents movement when clamped at min/max (clamped_delta == 0).
                let clamped_delta = self.width - old_width;
                self.rendered_width = (self.rendered_width + clamped_delta).max(self.width);
            } else {
                self.resizing = false;
            }
        }
        // Persist hover state so is_egui_using_pointer() can block mouse events
        // from reaching the terminal on the initial click (before resizing is set).
        self.hover_resize_handle = hover;
        if hover || self.resizing {
            ctx.set_cursor_icon(CursorIcon::ResizeHorizontal);
        }

        // Recompute panel_x with potentially drag-updated width (eliminates 1-frame lag).
        let panel_x = viewport.max.x - self.consumed_width();

        // --- Main panel area ---
        // Use Order::Middle so modal dialogs (Order::Foreground) render above.
        let area_response = egui::Area::new(Id::new("ai_inspector_panel"))
            .fixed_pos(Pos2::new(panel_x, viewport.min.y))
            .order(Order::Middle)
            .interactable(true)
            .show(ctx, |ui| {
                let mut close_requested = false;
                let mut action = InspectorAction::None;

                let inner_width = self.width - 18.0; // 8px margin each side + 1px stroke each side
                let panel_frame = Frame::new()
                    .fill(PANEL_BG)
                    .stroke(Stroke::new(1.0, Color32::from_gray(50)))
                    .inner_margin(8.0);

                panel_frame.show(ui, |ui| {
                    let panel_inner_height = (viewport.height() - 18.0).max(0.0);
                    ui.set_min_width(inner_width);
                    ui.set_max_width(inner_width);
                    // Constrain both min AND max height so that
                    // ui.available_height() returns a finite value.
                    // Without set_max_height, Areas report near-infinite
                    // available space which pushes pinned-bottom elements
                    // off-screen.
                    ui.set_min_height(panel_inner_height);
                    ui.set_max_height(panel_inner_height);

                    // === Title bar ===
                    ui.horizontal(|ui| {
                        ui.heading(
                            RichText::new("Assistant")
                                .strong()
                                .color(Color32::from_gray(220)),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .button(RichText::new("X").size(14.0))
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

                    // === Agent connection bar (above terminal capture) ===
                    let agent_action = self.render_agent_bar(ui, available_agents);
                    if !matches!(agent_action, InspectorAction::None) {
                        action = agent_action;
                    }

                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // === Collapsible terminal capture section ===
                    egui::CollapsingHeader::new(
                        RichText::new("Terminal Capture")
                            .color(Color32::from_gray(180))
                            .strong(),
                    )
                    .id_salt("terminal_capture_section")
                    .default_open(false)
                    .show(ui, |ui| {
                        // --- Controls row ---
                        self.render_controls(ui);

                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(4.0);

                        // --- Environment strip ---
                        if let Some(ref snapshot) = self.snapshot {
                            self.render_environment(ui, snapshot);
                            ui.add_space(4.0);
                            ui.separator();
                            ui.add_space(4.0);
                        }

                        // --- Commands content ---
                        // Use a fixed max height within the collapsible section
                        let cmd_height = (ui.available_height() * 0.5).clamp(100.0, 300.0);
                        egui::ScrollArea::vertical()
                            .id_salt("capture_commands_scroll")
                            .max_height(cmd_height)
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
                                            RichText::new(
                                                "Click Refresh to capture terminal state.",
                                            )
                                            .color(Color32::from_gray(80))
                                            .small(),
                                        );
                                    });
                                }
                            });
                    });

                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // Reserve space for pinned bottom elements:
                    // chat input ~34-80 (multiline), checkbox ~22, action bar ~30, separators+spacing ~20
                    let input_lines = if self.agent_status == AgentStatus::Connected {
                        self.chat.input.lines().count().clamp(1, 6) as f32
                    } else {
                        0.0
                    };
                    let bottom_reserve = if self.agent_status == AgentStatus::Connected {
                        90.0 + (input_lines - 1.0).max(0.0) * 14.0
                    } else {
                        36.0
                    };
                    let available_height = (ui.available_height() - bottom_reserve).max(50.0);

                    // === Scrollable content: chat messages ===
                    egui::ScrollArea::vertical()
                        .id_salt("inspector_scroll")
                        .max_height(available_height)
                        .auto_shrink([false, false])
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            // --- Chat messages section ---
                            if !self.chat.messages.is_empty() || self.chat.streaming {
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new("Chat")
                                        .color(Color32::from_gray(160))
                                        .small()
                                        .strong(),
                                );
                                ui.add_space(4.0);

                                let chat_action = Self::render_chat_messages(ui, &self.chat);
                                if !matches!(chat_action, InspectorAction::None) {
                                    action = chat_action;
                                }
                            }
                        });

                    // === Pinned bottom: Chat input + checkbox + action bar ===
                    if self.agent_status == AgentStatus::Connected {
                        ui.add_space(4.0);
                        ui.separator();
                        ui.add_space(2.0);
                        let input_action = self.render_chat_input(ui);
                        if !matches!(input_action, InspectorAction::None) {
                            action = input_action;
                        }
                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            if ui
                                .checkbox(
                                    &mut self.agent_terminal_access,
                                    RichText::new("Terminal access")
                                        .small()
                                        .color(Color32::from_gray(160)),
                                )
                                .changed()
                            {
                                action =
                                    InspectorAction::SetTerminalAccess(self.agent_terminal_access);
                            }
                            let yolo_color = if self.auto_approve {
                                Color32::from_rgb(255, 193, 7)
                            } else {
                                Color32::from_gray(160)
                            };
                            if ui
                                .checkbox(
                                    &mut self.auto_approve,
                                    RichText::new("YOLO").small().color(yolo_color),
                                )
                                .on_hover_text(
                                    "Auto-approve all agent permission requests",
                                )
                                .changed()
                            {
                                let mode = if self.auto_approve {
                                    "bypassPermissions"
                                } else {
                                    "default"
                                };
                                action = InspectorAction::SetAgentMode(mode.to_string());
                            }
                        });
                    }

                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(2.0);
                    let bar_action = self.render_action_bar(ui);
                    if !matches!(bar_action, InspectorAction::None) {
                        action = bar_action;
                    }
                });

                if close_requested {
                    InspectorAction::Close
                } else {
                    action
                }
            });

        // Track the actual rendered width (used by consumed_width() next frame).
        // Skip during active drag to prevent oscillation: the drag handler sets
        // rendered_width = self.width, but the area may render wider than self.width
        // (content overflow). Updating here would cause consumed_width() to bounce
        // between the two values on alternating frames, making the scrollbar jitter.
        if !self.resizing {
            self.rendered_width = area_response.response.rect.width();
        }

        // --- Paint resize handle line (Order::Background so modal dialogs render above) ---
        let line_color = if hover || self.resizing {
            Color32::from_gray(120)
        } else {
            Color32::from_gray(60)
        };
        let painter = ctx.layer_painter(egui::LayerId::new(
            Order::Background,
            Id::new("ai_inspector_resize_line"),
        ));
        painter.line_segment(
            [
                Pos2::new(panel_x, viewport.min.y),
                Pos2::new(panel_x, viewport.max.y),
            ],
            Stroke::new(2.0, line_color),
        );

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
                RichText::new("* Live").color(EXIT_SUCCESS).small()
            } else {
                RichText::new("o Paused")
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
                .button(RichText::new("~ Refresh").small())
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
                ui.label(RichText::new("|").color(dim_color).small());
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
                ui.label(RichText::new("|").color(dim_color).small());
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

                // Command text (wrap to prevent panel overflow)
                ui.add(
                    Label::new(
                        RichText::new(&cmd.command)
                            .color(Color32::from_gray(230))
                            .monospace(),
                    )
                    .wrap(),
                );

                ui.add_space(4.0);

                // Exit code badge + duration
                ui.horizontal(|ui| {
                    if let Some(code) = cmd.exit_code {
                        let (color, text) = if code == 0 {
                            (EXIT_SUCCESS, format!("OK {code}"))
                        } else {
                            (EXIT_FAILURE, format!("FAIL {code}"))
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
                        ui.add(
                            Label::new(
                                RichText::new(truncated)
                                    .color(Color32::from_gray(160))
                                    .monospace()
                                    .small(),
                            )
                            .wrap(),
                        );
                    });
                }
            });

            ui.add_space(4.0);
        }
    }

    /// Render timeline view: flat list with icons and durations.
    fn render_timeline(ui: &mut egui::Ui, commands: &[CommandEntry]) {
        // Reserve space for icon (~15px), duration label (~60px), and spacing
        let avail = ui.available_width();
        let max_cmd_chars = ((avail - 90.0) / 7.5).max(10.0) as usize;

        for (i, cmd) in commands.iter().enumerate() {
            ui.horizontal(|ui| {
                // Status icon
                let icon = match cmd.exit_code {
                    Some(0) => RichText::new("*").color(EXIT_SUCCESS),
                    Some(_) => RichText::new("*").color(EXIT_FAILURE),
                    None => RichText::new("o").color(Color32::from_gray(100)),
                };
                ui.label(icon);

                // Command text (truncated to fit, char-boundary-safe)
                let cmd_display = if cmd.command.len() > max_cmd_chars {
                    format!("{}...", truncate_chars(&cmd.command, max_cmd_chars))
                } else {
                    cmd.command.clone()
                };
                ui.label(
                    RichText::new(cmd_display)
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
        // Truncate to fit within panel - account for collapsing header icon (~20px)
        // and monospace char width (~7-8px). Use available width dynamically.
        let avail = ui.available_width();
        let max_chars = ((avail - 20.0) / 7.5).max(10.0) as usize;

        for (i, cmd) in commands.iter().enumerate() {
            let header_text = if cmd.command.len() > max_chars {
                format!("{}...", truncate_chars(&cmd.command, max_chars))
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
                    ui.add(
                        Label::new(
                            RichText::new(truncated)
                                .color(Color32::from_gray(160))
                                .monospace()
                                .small(),
                        )
                        .wrap(),
                    );
                }
            });
        }
    }

    /// Render list detail view: simple list with icon and command text.
    fn render_list_detail(ui: &mut egui::Ui, commands: &[CommandEntry]) {
        // Truncate command to fit in horizontal layout
        let avail = ui.available_width();
        let max_cmd_chars = ((avail - 50.0) / 7.5).max(10.0) as usize;

        for cmd in commands {
            ui.horizontal(|ui| {
                // Status icon
                let icon = match cmd.exit_code {
                    Some(0) => RichText::new("OK").color(EXIT_SUCCESS),
                    Some(_) => RichText::new("FAIL").color(EXIT_FAILURE),
                    None => RichText::new("-").color(Color32::from_gray(100)),
                };
                ui.label(icon);

                // Command text (truncated to fit, char-boundary-safe)
                let cmd_display = if cmd.command.len() > max_cmd_chars {
                    format!("{}...", truncate_chars(&cmd.command, max_cmd_chars))
                } else {
                    cmd.command.clone()
                };
                ui.label(
                    RichText::new(cmd_display)
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
                .button(RichText::new(" Copy JSON").small())
                .on_hover_text("Copy snapshot as JSON to clipboard")
                .clicked()
                && let Some(ref snapshot) = self.snapshot
                && let Ok(json) = snapshot.to_json()
            {
                action = InspectorAction::CopyJson(json);
            }

            // Save to file button
            if ui
                .button(RichText::new(" Save").small())
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

    /// Render the agent connection status bar with connect/disconnect controls.
    fn render_agent_bar(
        &mut self,
        ui: &mut egui::Ui,
        available_agents: &[AgentConfig],
    ) -> InspectorAction {
        let mut action = InspectorAction::None;

        ui.horizontal(|ui| {
            // Status indicator
            let (status_icon, status_color, status_text) = match &self.agent_status {
                AgentStatus::Connected => ("*", AGENT_CONNECTED, "Connected".to_string()),
                AgentStatus::Connecting => (
                    "o",
                    Color32::from_rgb(255, 193, 7),
                    "Connecting...".to_string(),
                ),
                AgentStatus::Disconnected => ("o", AGENT_DISCONNECTED, "Disconnected".to_string()),
                AgentStatus::Error(msg) => ("*", EXIT_FAILURE, format!("Error: {msg}")),
            };
            ui.label(RichText::new(status_icon).color(status_color).small());
            let status_response = ui.label(RichText::new(&status_text).color(status_color).small());
            if let AgentStatus::Error(msg) = &self.agent_status {
                status_response.on_hover_text(msg);
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                match self.agent_status {
                    AgentStatus::Connected => {
                        if ui
                            .button(RichText::new("Disconnect").small())
                            .on_hover_text("Disconnect from agent")
                            .clicked()
                        {
                            action = InspectorAction::DisconnectAgent;
                        }
                    }
                    AgentStatus::Disconnected | AgentStatus::Error(_) => {
                        if !available_agents.is_empty() {
                            // Clamp selected index to valid range
                            let idx = self.selected_agent_index.min(available_agents.len() - 1);
                            let agent = &available_agents[idx];
                            if ui
                                .button(RichText::new("Connect").small())
                                .on_hover_text(format!("Connect to {}", agent.name))
                                .clicked()
                            {
                                action = InspectorAction::ConnectAgent(agent.identity.clone());
                            }

                            // Agent selector dropdown (if multiple)
                            if available_agents.len() > 1 {
                                let selected_name = &available_agents[idx].short_name;
                                egui::ComboBox::from_id_salt("agent_selector")
                                    .selected_text(selected_name)
                                    .width(80.0)
                                    .show_ui(ui, |ui| {
                                        for (i, agent) in available_agents.iter().enumerate() {
                                            if ui.selectable_label(i == idx, &agent.name).clicked()
                                            {
                                                self.selected_agent_index = i;
                                            }
                                        }
                                    });
                            }
                        } else {
                            ui.label(
                                RichText::new("No agents found")
                                    .color(Color32::from_gray(80))
                                    .small()
                                    .italics(),
                            );
                        }
                    }
                    AgentStatus::Connecting => {
                        ui.spinner();
                    }
                }
            });
        });

        // Show install buttons only for agents whose connector binary is not in PATH
        if matches!(
            self.agent_status,
            AgentStatus::Disconnected | AgentStatus::Error(_)
        ) {
            let installable: Vec<_> = available_agents
                .iter()
                .filter(|a| a.install_command.is_some() && !a.connector_installed)
                .collect();
            if !installable.is_empty() {
                ui.add_space(4.0);
                ui.label(
                    RichText::new("Install ACP connectors:")
                        .color(Color32::from_gray(130))
                        .small(),
                );
                ui.horizontal_wrapped(|ui| {
                    for agent in installable {
                        let cmd = agent.install_command.as_deref().unwrap();
                        if ui
                            .button(RichText::new(format!("Install {}", agent.short_name)).small())
                            .on_hover_text(format!("Paste '{cmd}' into terminal"))
                            .clicked()
                        {
                            action = InspectorAction::WriteToTerminal(format!("{cmd}\n"));
                        }
                    }
                });
            }
        }

        action
    }

    /// Render chat messages from the conversation history.
    fn render_chat_messages(ui: &mut egui::Ui, chat: &ChatState) -> InspectorAction {
        let mut action = InspectorAction::None;

        for msg in &chat.messages {
            match msg {
                ChatMessage::User { text, pending } => {
                    let frame = Frame::new()
                        .fill(USER_MSG_BG)
                        .corner_radius(4.0)
                        .inner_margin(6.0);
                    frame.show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("You:")
                                    .color(Color32::from_rgb(100, 160, 230))
                                    .small()
                                    .strong(),
                            );
                            if *pending {
                                ui.label(
                                    RichText::new("(queued)")
                                        .color(Color32::from_gray(100))
                                        .small()
                                        .italics(),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui
                                            .button(
                                                RichText::new("Cancel")
                                                    .small()
                                                    .color(Color32::from_rgb(255, 100, 100)),
                                            )
                                            .on_hover_text("Cancel this queued message")
                                            .clicked()
                                        {
                                            action = InspectorAction::CancelQueuedPrompt;
                                        }
                                    },
                                );
                            }
                        });
                        ui.add(
                            Label::new(RichText::new(text).color(Color32::from_gray(220)))
                                .selectable(true)
                                .wrap(),
                        );
                    });
                    ui.add_space(4.0);
                }
                ChatMessage::Agent(text) => {
                    let frame = Frame::new()
                        .fill(AGENT_MSG_BG)
                        .corner_radius(4.0)
                        .inner_margin(6.0);
                    frame.show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.label(
                            RichText::new("Agent:")
                                .color(AGENT_CONNECTED)
                                .small()
                                .strong(),
                        );
                        Self::render_rich_text(ui, text);
                    });
                    ui.add_space(4.0);
                }
                ChatMessage::Thinking(text) => {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("").color(Color32::from_gray(80)).small());
                        ui.add(
                            Label::new(
                                RichText::new(text)
                                    .color(Color32::from_gray(90))
                                    .small()
                                    .italics(),
                            )
                            .selectable(true)
                            .wrap(),
                        );
                    });
                    ui.add_space(2.0);
                }
                ChatMessage::ToolCall { title, status, .. } => {
                    ui.horizontal(|ui| {
                        let status_icon = if status == "completed" {
                            RichText::new("OK").color(AGENT_CONNECTED).small()
                        } else if status == "error" || status == "failed" {
                            RichText::new("FAIL").color(EXIT_FAILURE).small()
                        } else if status == "in_progress" || status == "running" {
                            RichText::new(".")
                                .color(Color32::from_rgb(255, 193, 7))
                                .small()
                        } else {
                            // Empty or unknown status — show neutral pending indicator
                            RichText::new("-").color(Color32::from_gray(120)).small()
                        };
                        ui.label(status_icon);
                        ui.add(
                            Label::new(
                                RichText::new(title)
                                    .color(Color32::from_gray(150))
                                    .small()
                                    .monospace(),
                            )
                            .selectable(true)
                            .wrap(),
                        );
                    });
                    ui.add_space(2.0);
                }
                ChatMessage::CommandSuggestion(cmd) => {
                    let frame = Frame::new()
                        .fill(CMD_SUGGEST_BG)
                        .corner_radius(4.0)
                        .inner_margin(6.0);
                    frame.show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.label(
                            RichText::new("Suggested command:")
                                .color(Color32::from_gray(130))
                                .small(),
                        );
                        ui.add(
                            Label::new(
                                RichText::new(format!("$ {cmd}"))
                                    .color(Color32::from_gray(220))
                                    .monospace(),
                            )
                            .selectable(true)
                            .wrap(),
                        );
                        ui.horizontal(|ui| {
                            if ui
                                .button(RichText::new("> Run").small())
                                .on_hover_text("Execute this command in the terminal")
                                .clicked()
                            {
                                // Send command + Enter to terminal and notify agent
                                action = InspectorAction::RunCommandAndNotify(cmd.clone());
                            }
                            if ui
                                .button(RichText::new("# Paste").small())
                                .on_hover_text("Paste command into terminal without executing")
                                .clicked()
                            {
                                action = InspectorAction::WriteToTerminal(cmd.clone());
                            }
                        });
                    });
                    ui.add_space(4.0);
                }
                ChatMessage::Permission {
                    request_id,
                    description,
                    options,
                    resolved,
                } => {
                    let frame = Frame::new()
                        .fill(Color32::from_rgb(50, 35, 20))
                        .corner_radius(4.0)
                        .inner_margin(6.0);
                    frame.show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.label(
                            RichText::new(if *resolved {
                                "OK Permission granted"
                            } else {
                                "! Permission requested"
                            })
                            .color(Color32::from_rgb(255, 193, 7))
                            .small()
                            .strong(),
                        );
                        ui.add(
                            Label::new(
                                RichText::new(description.as_str())
                                    .color(Color32::from_gray(180))
                                    .small(),
                            )
                            .selectable(true)
                            .wrap(),
                        );
                        if !*resolved {
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                for (opt_id, opt_label) in options {
                                    if ui
                                        .button(RichText::new(opt_label.as_str()).small())
                                        .clicked()
                                    {
                                        action = InspectorAction::RespondPermission {
                                            request_id: *request_id,
                                            option_id: opt_id.clone(),
                                            cancelled: false,
                                        };
                                    }
                                }
                                if ui
                                    .button(
                                        RichText::new("Deny")
                                            .small()
                                            .color(Color32::from_rgb(255, 100, 100)),
                                    )
                                    .clicked()
                                {
                                    action = InspectorAction::RespondPermission {
                                        request_id: *request_id,
                                        option_id: String::new(),
                                        cancelled: true,
                                    };
                                }
                            });
                        }
                    });
                    ui.add_space(4.0);
                }
                ChatMessage::AutoApproved(desc) => {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("OK").color(Color32::from_gray(100)).small());
                        ui.add(
                            Label::new(
                                RichText::new(format!("Auto-approved: {desc}"))
                                    .color(Color32::from_gray(100))
                                    .small()
                                    .italics(),
                            )
                            .selectable(true)
                            .wrap(),
                        );
                    });
                    ui.add_space(2.0);
                }
                ChatMessage::System(text) => {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("i").color(SYSTEM_MSG_COLOR).small());
                        ui.add(
                            Label::new(
                                RichText::new(text)
                                    .color(SYSTEM_MSG_COLOR)
                                    .small()
                                    .italics(),
                            )
                            .selectable(true)
                            .wrap(),
                        );
                    });
                    ui.add_space(2.0);
                }
            }
        }

        // Show streaming text if agent is currently responding
        if chat.streaming {
            let streaming = chat.streaming_text();
            if !streaming.is_empty() {
                let frame = Frame::new()
                    .fill(AGENT_MSG_BG)
                    .corner_radius(4.0)
                    .inner_margin(6.0);
                frame.show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Agent:")
                                .color(AGENT_CONNECTED)
                                .small()
                                .strong(),
                        );
                        ui.spinner();
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .button(
                                    RichText::new("Cancel")
                                        .small()
                                        .color(Color32::from_rgb(255, 100, 100)),
                                )
                                .on_hover_text("Cancel current prompt")
                                .clicked()
                            {
                                action = InspectorAction::CancelPrompt;
                            }
                        });
                    });
                    Self::render_rich_text(ui, streaming);
                });
            } else {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(
                        RichText::new("Agent is thinking...")
                            .color(Color32::from_gray(120))
                            .small()
                            .italics(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(
                                RichText::new("Cancel")
                                    .small()
                                    .color(Color32::from_rgb(255, 100, 100)),
                            )
                            .on_hover_text("Cancel current prompt")
                            .clicked()
                        {
                            action = InspectorAction::CancelPrompt;
                        }
                    });
                });
            }
        }

        action
    }

    /// Render agent text with code block formatting.
    ///
    /// Parses the text into plain text and fenced code block segments, rendering
    /// code blocks with a distinct background and monospace font.
    fn render_rich_text(ui: &mut egui::Ui, text: &str) {
        let segments = parse_text_segments(text);
        for segment in &segments {
            match segment {
                TextSegment::Plain(t) => {
                    if !t.is_empty() {
                        ui.add(
                            Label::new(RichText::new(t).color(Color32::from_gray(210)))
                                .selectable(true)
                                .wrap(),
                        );
                    }
                }
                TextSegment::CodeBlock { lang, code } => {
                    let code_frame = Frame::new()
                        .fill(CODE_BLOCK_BG)
                        .corner_radius(3.0)
                        .inner_margin(6.0)
                        .stroke(Stroke::new(1.0, Color32::from_gray(40)));
                    code_frame.show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        if !lang.is_empty() {
                            ui.label(RichText::new(lang.as_str()).color(CODE_LANG_COLOR).small());
                        }
                        ui.add(
                            Label::new(
                                RichText::new(code.as_str())
                                    .color(Color32::from_gray(200))
                                    .monospace(),
                            )
                            .selectable(true)
                            .wrap(),
                        );
                    });
                }
            }
        }
    }

    /// Render the chat text input and send/clear buttons.
    ///
    /// Multiline: Enter sends, Shift+Enter inserts a newline.
    fn render_chat_input(&mut self, ui: &mut egui::Ui) -> InspectorAction {
        let mut action = InspectorAction::None;

        // Determine input height based on line count (min 1 row, max 6 rows)
        let line_count = self.chat.input.lines().count().clamp(1, 6);
        let input_height = 20.0 + (line_count as f32 - 1.0) * 14.0;

        let button_width = 60.0; // space for Send + Clear buttons
        let input_width = ui.available_width() - button_width;

        // Check for Enter (without Shift) before rendering the TextEdit,
        // since egui may consume the key event.
        let enter_pressed = ui.input(|i| {
            i.key_pressed(Key::Enter)
                && !i.modifiers.shift
                && !i.modifiers.ctrl
                && !i.modifiers.command
        });

        ui.horizontal(|ui| {
            let response = ui.add_sized(
                [input_width, input_height],
                egui::TextEdit::multiline(&mut self.chat.input)
                    .hint_text("Message... (Shift+Enter for newline)")
                    .desired_width(input_width)
                    .desired_rows(line_count),
            );

            let is_focused = response.has_focus();
            let should_send = is_focused && enter_pressed;

            ui.vertical(|ui| {
                let send_clicked = ui
                    .button(RichText::new(">").size(14.0))
                    .on_hover_text("Send message (Enter)")
                    .clicked();

                if ui
                    .button(RichText::new("C").size(12.0))
                    .on_hover_text("Clear conversation")
                    .clicked()
                {
                    action = InspectorAction::ClearChat;
                }

                if (should_send || send_clicked) && !self.chat.input.trim().is_empty() {
                    let text = self.chat.input.trim().to_string();
                    self.chat.input.clear();
                    action = InspectorAction::SendPrompt(text);
                }

                // Remove the trailing newline that Enter adds before we send
                if should_send {
                    // egui inserts the newline from Enter; strip it
                    while self.chat.input.ends_with('\n') {
                        self.chat.input.pop();
                    }
                }
            });

            // Re-focus input after sending
            if should_send {
                response.request_focus();
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

/// Truncate a string to at most `max_chars` characters, respecting UTF-8
/// char boundaries (never panics on multi-byte characters like emoji or CJK).
fn truncate_chars(s: &str, max_chars: usize) -> &str {
    if s.len() <= max_chars {
        return s;
    }
    // Find the last char boundary at or before max_chars bytes
    let mut end = max_chars.min(s.len());
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
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
    fn test_truncate_chars_ascii() {
        assert_eq!(truncate_chars("hello", 10), "hello");
        assert_eq!(truncate_chars("hello world", 5), "hello");
        assert_eq!(truncate_chars("", 5), "");
    }

    #[test]
    fn test_truncate_chars_multibyte() {
        // Each emoji is 4 bytes; slicing at byte 2 would panic without boundary check
        let emoji = "🚀🎉🌍";
        let result = truncate_chars(emoji, 4); // first emoji is 4 bytes
        assert_eq!(result, "🚀");
        // Ensure we don't panic on a mid-char boundary
        let result = truncate_chars(emoji, 5);
        assert_eq!(result, "🚀"); // can't include partial second emoji
    }

    #[test]
    fn test_truncate_chars_cjk() {
        // CJK characters are 3 bytes each
        let cjk = "你好世界";
        let result = truncate_chars(cjk, 3);
        assert_eq!(result, "你");
        let result = truncate_chars(cjk, 4);
        assert_eq!(result, "你"); // can't include partial second char
        let result = truncate_chars(cjk, 6);
        assert_eq!(result, "你好");
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
        assert_eq!(panel.view_mode, ViewMode::Tree);
        assert!(!panel.live_update);
        assert!(panel.show_zones);
    }
}
