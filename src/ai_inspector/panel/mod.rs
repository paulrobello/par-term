//! AI Inspector side panel UI.
//!
//! Provides an egui-based side panel for viewing terminal state snapshots,
//! command history, and environment info. Supports multiple view modes
//! (Cards, Timeline, Tree, ListDetail) and interactive controls.
//!
//! Sub-modules:
//! - `types`         — `ViewMode`, `InspectorAction`, constants
//! - `snapshot_view` — controls row, environment strip, Cards/Timeline/Tree/ListDetail renderers
//! - `chat_view`     — agent bar, action bar, chat messages, rich text, chat input

mod chat_view;
mod message_render;
mod panel_body;
mod snapshot_view;
pub mod types;

pub use types::{InspectorAction, ViewMode};

use egui::{Color32, Context, CursorIcon, Id, Key, Order, Pos2, Stroke};

use crate::ai_inspector::chat::ChatState;
use crate::ai_inspector::snapshot::{SnapshotData, SnapshotScope};
use crate::config::Config;
use crate::ui_constants::{AI_PANEL_MAX_WIDTH_RATIO, AI_PANEL_MIN_WIDTH};
use par_term_acp::{AgentConfig, AgentStatus};
use std::path::Path;

use types::RESIZE_HANDLE_WIDTH;

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
    /// Display name of the most recently requested/connected agent.
    pub connected_agent_name: Option<String>,
    /// Identity of the most recently requested/connected agent.
    pub connected_agent_identity: Option<String>,
    /// Project root resolved for the active agent session.
    pub connected_agent_project_root: Option<String>,
    /// Working directory sent to the active agent session.
    pub connected_agent_cwd: Option<String>,
    /// Font size for chat message body text (points).
    pub chat_font_size: f32,
    /// Id of the chat input text field, used to check focus for Escape key handling.
    chat_input_id: Option<Id>,
}

impl AIInspectorPanel {
    /// Create a new inspector panel initialized from config.
    pub fn new(config: &Config) -> Self {
        Self {
            open: config.ai_inspector.ai_inspector_open_on_startup,
            width: config.ai_inspector.ai_inspector_width,
            min_width: AI_PANEL_MIN_WIDTH,
            max_width_ratio: AI_PANEL_MAX_WIDTH_RATIO,
            resizing: false,
            scope: SnapshotScope::from_config_str(&config.ai_inspector.ai_inspector_default_scope),
            view_mode: ViewMode::from_config_str(&config.ai_inspector.ai_inspector_view_mode),
            live_update: config.ai_inspector.ai_inspector_live_update,
            show_zones: config.ai_inspector.ai_inspector_show_zones,
            snapshot: None,
            needs_refresh: true,
            last_command_count: 0,
            agent_status: AgentStatus::Disconnected,
            chat: ChatState::new(),
            agent_terminal_access: config.ai_inspector.ai_inspector_agent_terminal_access,
            auto_approve: config.ai_inspector.ai_inspector_auto_approve,
            rendered_width: 0.0,
            hover_resize_handle: false,
            max_width: 0.0,
            selected_agent_index: 0,
            connected_agent_name: None,
            connected_agent_identity: None,
            connected_agent_project_root: None,
            connected_agent_cwd: None,
            chat_font_size: config.ai_inspector.ai_inspector_chat_font_size,
            chat_input_id: None,
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

    pub(super) fn agent_project_label(&self) -> Option<String> {
        let root = self.connected_agent_project_root.as_deref()?;
        let name = Path::new(root)
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .unwrap_or(root);
        Some(format!("Project: {name}"))
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

        // Handle Escape key to close — but only when the chat input is NOT focused
        let chat_input_focused = self
            .chat_input_id
            .is_some_and(|id| ctx.memory(|m| m.has_focus(id)));
        if ctx.input(|i| i.key_pressed(Key::Escape)) && !chat_input_focused {
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
                let (action, close_requested) = self.render_panel_body(ui, available_agents);
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_inspector::panel_helpers::{format_duration, truncate_chars, truncate_output};
    use crate::ai_inspector::snapshot::SnapshotScope;

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
        // Each emoji is 4 bytes; function now correctly uses character count
        let emoji = "🚀🎉🌍";
        let result = truncate_chars(emoji, 1); // first character (1 emoji)
        assert_eq!(result, "🚀");
        // Multiple emoji characters
        let result = truncate_chars(emoji, 2);
        assert_eq!(result, "🚀🎉");
        // All emoji
        let result = truncate_chars(emoji, 3);
        assert_eq!(result, "🚀🎉🌍");
        // More than string length returns full string
        let result = truncate_chars(emoji, 10);
        assert_eq!(result, "🚀🎉🌍");
    }

    #[test]
    fn test_truncate_chars_cjk() {
        // CJK characters are 3 bytes each; function now correctly uses character count
        let cjk = "你好世界";
        let result = truncate_chars(cjk, 1);
        assert_eq!(result, "你");
        let result = truncate_chars(cjk, 2);
        assert_eq!(result, "你好");
        let result = truncate_chars(cjk, 4);
        assert_eq!(result, "你好世界");
        // More than string length returns full string
        let result = truncate_chars(cjk, 10);
        assert_eq!(result, "你好世界");
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
        assert_eq!(panel.connected_agent_project_root, None);
        assert_eq!(panel.connected_agent_cwd, None);
    }

    #[test]
    fn test_agent_project_label_uses_project_directory_name() {
        let config = Config::default();
        let mut panel = AIInspectorPanel::new(&config);
        panel.connected_agent_project_root = Some("/Users/example/Repos/par-term".to_string());

        assert_eq!(
            panel.agent_project_label(),
            Some("Project: par-term".to_string())
        );
    }

    #[test]
    fn test_agent_project_label_falls_back_to_full_path_for_root() {
        let config = Config::default();
        let mut panel = AIInspectorPanel::new(&config);
        panel.connected_agent_project_root = Some("/".to_string());

        assert_eq!(panel.agent_project_label(), Some("Project: /".to_string()));
    }
}
