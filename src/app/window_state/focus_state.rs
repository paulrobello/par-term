//! Focus and redraw tracking state for the window manager.
//!
//! Extracted from `WindowState` as part of the God Object decomposition (ARC-001).

use std::time::Instant;

/// State for window focus, redraw tracking, and render throttling.
pub(crate) struct FocusState {
    /// Whether the window currently has OS-level focus
    pub(crate) is_focused: bool,
    /// Whether the window needs a redraw (set by event handlers)
    pub(crate) needs_redraw: bool,
    /// When the last frame was rendered
    pub(crate) last_render_time: Option<Instant>,

    /// When the cursor was last hidden (for flicker reduction)
    pub(crate) cursor_hidden_since: Option<Instant>,
    /// Whether a render is pending due to flicker reduction delay
    pub(crate) flicker_pending_render: bool,

    /// Start of the current throughput batch (None if not in bulk output)
    pub(crate) throughput_batch_start: Option<Instant>,

    /// Whether the UI (egui) consumed the last mouse press event
    pub(crate) ui_consumed_mouse_press: bool,
    /// Whether a focus click is pending (first click that focused the window)
    pub(crate) focus_click_pending: bool,
    /// Timestamp when a focus click was suppressed (to prevent double-handling)
    pub(crate) focus_click_suppressed_while_unfocused_at: Option<Instant>,
}

impl Default for FocusState {
    fn default() -> Self {
        Self {
            is_focused: true, // Assume focused on creation
            needs_redraw: true,
            last_render_time: None,
            cursor_hidden_since: None,
            flicker_pending_render: false,
            throughput_batch_start: None,
            ui_consumed_mouse_press: false,
            focus_click_pending: false,
            focus_click_suppressed_while_unfocused_at: None,
        }
    }
}
