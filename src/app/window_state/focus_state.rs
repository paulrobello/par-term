//! Focus and redraw tracking state for the window manager.
//!
//! Extracted from `WindowState` as part of the God Object decomposition (ARC-001).

use crate::tab::TabId;
use std::time::Instant;

/// State for window focus, redraw tracking, and render throttling.
pub(crate) struct FocusState {
    /// Whether the window currently has OS-level focus
    pub(crate) is_focused: bool,
    /// Whether the window needs a redraw (set by event handlers)
    pub(crate) needs_redraw: bool,
    /// A `RedrawRequested` was delivered but `should_render_frame()` rejected it
    /// because the FPS gate had not yet elapsed. Events delivered to egui (e.g. a
    /// tab click's press/release) are now sitting in `egui_winit`'s `raw_input`
    /// accumulator with nothing scheduled to consume them. `about_to_wait` re-arms
    /// a frame when the gate opens so the stall self-heals instead of waiting for
    /// an unrelated wake.
    pub(crate) pending_egui_repaint: bool,
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
    /// Tab to switch to on the next post-render pass when a focus-click landed
    /// directly on a tab.  Set by the native event handler (using cached tab
    /// rects) as a fallback in case egui's own click detection doesn't fire
    /// (e.g. pointer state was stale when the window was unfocused).
    pub(crate) pending_focus_tab_switch: Option<TabId>,
}

impl Default for FocusState {
    fn default() -> Self {
        Self {
            is_focused: true, // Assume focused on creation
            needs_redraw: true,
            pending_egui_repaint: false,
            last_render_time: None,
            cursor_hidden_since: None,
            flicker_pending_render: false,
            throughput_batch_start: None,
            ui_consumed_mouse_press: false,
            focus_click_pending: false,
            focus_click_suppressed_while_unfocused_at: None,
            pending_focus_tab_switch: None,
        }
    }
}
