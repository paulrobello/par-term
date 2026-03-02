//! Cursor animation state for blinking and opacity tracking.
//!
//! Groups the four fields that govern cursor blink animation so they travel
//! together and can be reasoned about in isolation from the rest of `WindowState`.

/// Animation state for the terminal cursor (blink, fade, timing).
#[derive(Debug)]
pub(crate) struct CursorAnimState {
    /// Cursor opacity for smooth fade animation (0.0 = invisible, 1.0 = fully visible)
    pub(crate) cursor_opacity: f32,
    /// Time of last cursor blink toggle
    pub(crate) last_cursor_blink: Option<std::time::Instant>,
    /// Time of last key press (to reset cursor blink)
    pub(crate) last_key_press: Option<std::time::Instant>,
    /// When to blink cursor next (used by about_to_wait scheduling)
    pub(crate) cursor_blink_timer: Option<std::time::Instant>,
}

impl Default for CursorAnimState {
    fn default() -> Self {
        Self {
            cursor_opacity: 1.0,
            last_cursor_blink: None,
            last_key_press: None,
            cursor_blink_timer: None,
        }
    }
}
