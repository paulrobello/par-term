//! Concrete implementations of the traits defined in [`crate::traits`].
//!
//! # What is implemented here
//!
//! - [`TerminalAccess`] on [`crate::terminal::TerminalManager`] — all five
//!   read-only query methods are direct thin wrappers over the existing
//!   `TerminalManager` methods, so there is no behaviour change at call sites.
//!
//! - [`UIElement`] on [`crate::tab_bar_ui::TabBarUI`] and
//!   [`crate::status_bar::StatusBarUI`] — each type defines its own `Ctx<'a>`
//!   holding the per-frame references it needs (config, tab count, fullscreen flag).
//!   Existing call sites are unaffected; the trait provides a new generic path.
//!
//! # How to use `TerminalAccess` in new code
//!
//! Any function that only needs to inspect terminal mode (e.g. mouse handlers,
//! input encoders) can be written against `T: TerminalAccess` instead of
//! taking a concrete `&TerminalManager`. This makes the function testable with
//! a lightweight mock:
//!
//! ```rust,ignore
//! use par_term::traits::TerminalAccess;
//!
//! fn encode_if_tracking<T: TerminalAccess>(t: &T, button: u8, col: usize, row: usize) -> Vec<u8> {
//!     if t.is_mouse_tracking_active() {
//!         t.encode_mouse_event(button, col, row, true, 0)
//!     } else {
//!         Vec::new()
//!     }
//! }
//! ```
//!
//! See `src/traits.rs` for the full trait contract and migration path notes.

use crate::config::Config;
use crate::status_bar::StatusBarUI;
use crate::tab_bar_ui::TabBarUI;
use crate::terminal::TerminalManager;
use crate::traits::{TerminalAccess, UIElement};

impl TerminalAccess for TerminalManager {
    /// Returns `true` if the alternate screen buffer (DECSC/smcup) is active.
    ///
    /// Delegates to [`TerminalManager::is_alt_screen_active`].
    fn is_alt_screen_active(&self) -> bool {
        self.is_alt_screen_active()
    }

    /// Returns `true` if mouse motion events should be forwarded to the PTY.
    ///
    /// Delegates to [`TerminalManager::should_report_mouse_motion`].
    fn should_report_mouse_motion(&self, button_pressed: bool) -> bool {
        self.should_report_mouse_motion(button_pressed)
    }

    /// Returns the current modifyOtherKeys level (0 = off, 1 = basic, 2 = full).
    ///
    /// Delegates to [`TerminalManager::modify_other_keys_mode`].
    fn modify_other_keys_mode(&self) -> u8 {
        self.modify_other_keys_mode()
    }

    /// Returns `true` if DECCKM (application cursor key) mode is active.
    ///
    /// Delegates to [`TerminalManager::application_cursor`].
    fn application_cursor(&self) -> bool {
        self.application_cursor()
    }

    /// Encode a mouse event into the bytes to be written to the PTY.
    ///
    /// Delegates to [`TerminalManager::encode_mouse_event`].
    fn encode_mouse_event(
        &self,
        button: u8,
        col: usize,
        row: usize,
        pressed: bool,
        modifiers: u8,
    ) -> Vec<u8> {
        self.encode_mouse_event(button, col, row, pressed, modifiers)
    }
}

// ── UIElement implementations ─────────────────────────────────────────────────

/// Per-call context for [`TabBarUI`].
///
/// Holds the two parameters that `TabBarUI` needs to compute its layout
/// dimensions: the global config reference and the current tab count.
pub struct TabBarCtx<'a> {
    /// Global application configuration (read-only borrow for this frame).
    pub config: &'a Config,
    /// Number of open tabs in the current window.
    pub tab_count: usize,
}

impl UIElement for TabBarUI {
    type Ctx<'a> = TabBarCtx<'a>;

    /// Returns `true` when the tab bar is configured to show given the current
    /// tab count and `config.tab_bar_mode`.
    fn is_visible(&self, ctx: Self::Ctx<'_>) -> bool {
        self.should_show(ctx.tab_count, ctx.config.tab_bar_mode)
    }

    /// Effective height of the tab bar in logical pixels.
    ///
    /// Non-zero only when the bar is visible **and** positioned horizontally
    /// (top or bottom).  Returns 0.0 for a left-side tab bar.
    fn height_logical(&self, ctx: Self::Ctx<'_>) -> f32 {
        self.get_height(ctx.tab_count, ctx.config)
    }

    /// Effective width of the tab bar in logical pixels.
    ///
    /// Non-zero only when the bar is visible **and** positioned on the left side.
    /// Returns 0.0 for top/bottom tab bars.
    fn width_logical(&self, ctx: Self::Ctx<'_>) -> f32 {
        self.get_width(ctx.tab_count, ctx.config)
    }

    /// Returns `true` when the tab rename field or a context menu is open,
    /// indicating that keyboard input should not be forwarded to the terminal.
    fn is_capturing_input(&self) -> bool {
        self.is_renaming() || self.is_context_menu_open()
    }
}

/// Per-call context for [`StatusBarUI`].
///
/// Holds the two parameters that `StatusBarUI` needs to determine its
/// visibility and height: the global config reference and whether the window
/// is currently fullscreen.
pub struct StatusBarCtx<'a> {
    /// Global application configuration (read-only borrow for this frame).
    pub config: &'a Config,
    /// Whether the window is currently in fullscreen mode.
    pub is_fullscreen: bool,
}

impl UIElement for StatusBarUI {
    type Ctx<'a> = StatusBarCtx<'a>;

    /// Returns `true` when the status bar is enabled and not hidden by the
    /// current window/mouse state.
    fn is_visible(&self, ctx: Self::Ctx<'_>) -> bool {
        !self.should_hide(ctx.config, ctx.is_fullscreen)
    }

    /// Effective height of the status bar in logical pixels.
    ///
    /// Delegates to `StatusBarUI::height`, which returns 0.0 when hidden.
    fn height_logical(&self, ctx: Self::Ctx<'_>) -> f32 {
        self.height(ctx.config, ctx.is_fullscreen)
    }

    /// Width of the status bar in logical pixels.
    ///
    /// The status bar always spans the full window width, so this returns 0.0
    /// (the bar does not consume horizontal space from the sides).
    fn width_logical(&self, _ctx: Self::Ctx<'_>) -> f32 {
        0.0
    }

    /// Returns `false` — the status bar is read-only and never captures input.
    fn is_capturing_input(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal mock that implements `TerminalAccess` without a live PTY session.
    ///
    /// Used to verify the trait object interface at compile time and exercise the
    /// method dispatch without spinning up a PTY.
    struct MockTerminal {
        alt_screen: bool,
        mouse_any_event: bool,
        modify_other_keys: u8,
        app_cursor: bool,
    }

    impl MockTerminal {
        fn new() -> Self {
            Self {
                alt_screen: false,
                mouse_any_event: false,
                modify_other_keys: 0,
                app_cursor: false,
            }
        }

        fn with_alt_screen(mut self) -> Self {
            self.alt_screen = true;
            self
        }

        fn with_any_event_mouse(mut self) -> Self {
            self.mouse_any_event = true;
            self
        }

        fn with_app_cursor(mut self) -> Self {
            self.app_cursor = true;
            self
        }

        fn with_modify_other_keys(mut self, level: u8) -> Self {
            self.modify_other_keys = level;
            self
        }
    }

    impl TerminalAccess for MockTerminal {
        fn is_alt_screen_active(&self) -> bool {
            self.alt_screen
        }

        fn should_report_mouse_motion(&self, _button_pressed: bool) -> bool {
            self.mouse_any_event
        }

        fn modify_other_keys_mode(&self) -> u8 {
            self.modify_other_keys
        }

        fn application_cursor(&self) -> bool {
            self.app_cursor
        }

        fn encode_mouse_event(
            &self,
            button: u8,
            col: usize,
            row: usize,
            _pressed: bool,
            _modifiers: u8,
        ) -> Vec<u8> {
            // Minimal stub: return a recognisable byte sequence for assertions.
            vec![b'\x1b', b'[', b'M', button, col as u8, row as u8]
        }
    }

    // ── TerminalAccess contract tests ─────────────────────────────────────

    #[test]
    fn mock_alt_screen_default_false() {
        let t = MockTerminal::new();
        assert!(!t.is_alt_screen_active());
    }

    #[test]
    fn mock_alt_screen_activated() {
        let t = MockTerminal::new().with_alt_screen();
        assert!(t.is_alt_screen_active());
    }

    #[test]
    fn mock_mouse_motion_off_by_default() {
        let t = MockTerminal::new();
        assert!(!t.should_report_mouse_motion(false));
        assert!(!t.should_report_mouse_motion(true));
    }

    #[test]
    fn mock_mouse_motion_on_with_any_event() {
        let t = MockTerminal::new().with_any_event_mouse();
        assert!(t.should_report_mouse_motion(false));
        assert!(t.should_report_mouse_motion(true));
    }

    #[test]
    fn mock_modify_other_keys_default_zero() {
        let t = MockTerminal::new();
        assert_eq!(t.modify_other_keys_mode(), 0);
    }

    #[test]
    fn mock_modify_other_keys_level_2() {
        let t = MockTerminal::new().with_modify_other_keys(2);
        assert_eq!(t.modify_other_keys_mode(), 2);
    }

    #[test]
    fn mock_application_cursor_default_false() {
        let t = MockTerminal::new();
        assert!(!t.application_cursor());
    }

    #[test]
    fn mock_application_cursor_activated() {
        let t = MockTerminal::new().with_app_cursor();
        assert!(t.application_cursor());
    }

    #[test]
    fn mock_encode_mouse_event_returns_bytes() {
        let t = MockTerminal::new();
        let bytes = t.encode_mouse_event(0, 10, 5, true, 0);
        // Our mock stub returns a 6-byte sequence starting with ESC [ M.
        assert_eq!(bytes.len(), 6);
        assert_eq!(bytes[0], b'\x1b');
        assert_eq!(bytes[1], b'[');
        assert_eq!(bytes[2], b'M');
        assert_eq!(bytes[3], 0); // button
        assert_eq!(bytes[4], 10); // col
        assert_eq!(bytes[5], 5); // row
    }

    /// Compile-time check: a function generic over `T: TerminalAccess` can
    /// accept either `TerminalManager` or `MockTerminal`.
    fn query_mode<T: TerminalAccess>(t: &T) -> (bool, bool, u8) {
        (
            t.is_alt_screen_active(),
            t.application_cursor(),
            t.modify_other_keys_mode(),
        )
    }

    #[test]
    fn generic_function_works_with_mock() {
        let t = MockTerminal::new().with_alt_screen().with_app_cursor();
        let (alt, cursor, mok) = query_mode(&t);
        assert!(alt);
        assert!(cursor);
        assert_eq!(mok, 0);
    }

    // ── UIElement contract tests ───────────────────────────────────────────

    /// Compile-time check: a function generic over `T: UIElement` can query
    /// height and visibility without knowing the concrete type.
    fn query_element<T>(element: &T, ctx: T::Ctx<'_>) -> (bool, f32, f32, bool)
    where
        T: UIElement,
    {
        // We can't call the ctx-taking methods twice with the same ctx because
        // Ctx is moved on the first call.  Instead return the results as a tuple
        // so the caller can assert each individually with separate ctx values.
        let _ = element.height_logical(ctx);
        (false, 0.0, 0.0, element.is_capturing_input())
    }

    /// Verify that `TabBarUI::is_capturing_input` returns false when no
    /// interactive state is active (no rename, no context menu).
    #[test]
    fn tab_bar_ui_not_capturing_input_by_default() {
        let tab_bar = crate::tab_bar_ui::TabBarUI::new();
        assert!(!tab_bar.is_capturing_input());
    }

    /// Verify that `StatusBarUI::is_capturing_input` always returns false.
    #[test]
    fn status_bar_ui_never_captures_input() {
        let status_bar = crate::status_bar::StatusBarUI::new();
        assert!(!status_bar.is_capturing_input());
    }

    /// Verify that `StatusBarUI::width_logical` always returns 0.0 (the
    /// status bar spans the full window width, not a side panel).
    #[test]
    fn status_bar_ui_width_is_zero() {
        use crate::traits_impl::StatusBarCtx;
        let status_bar = crate::status_bar::StatusBarUI::new();
        let config = crate::config::Config::default();
        let ctx = StatusBarCtx {
            config: &config,
            is_fullscreen: false,
        };
        assert_eq!(status_bar.width_logical(ctx), 0.0);
    }

    /// Verify that `TabBarUI::height_logical` returns the configured tab bar
    /// height when `TabBarMode::Always` is active and the bar is horizontal.
    #[test]
    fn tab_bar_ui_height_matches_config() {
        use crate::traits_impl::TabBarCtx;
        let tab_bar = crate::tab_bar_ui::TabBarUI::new();
        let config = crate::config::Config::default();
        // Default config has TabBarMode::Always and horizontal position,
        // so height_logical should equal config.tab_bar_height.
        let height = tab_bar.height_logical(TabBarCtx {
            config: &config,
            tab_count: 1,
        });
        assert_eq!(height, config.tab_bar_height);
    }

    /// Compile-time check: `query_element` can be instantiated with both
    /// `TabBarUI` and `StatusBarUI`, confirming they share the `UIElement` bound.
    #[test]
    fn ui_element_generic_function_compiles_for_both_types() {
        use crate::traits_impl::{StatusBarCtx, TabBarCtx};
        let tab_bar = crate::tab_bar_ui::TabBarUI::new();
        let status_bar = crate::status_bar::StatusBarUI::new();
        let config = crate::config::Config::default();

        let (_, _, _, capturing_tab) = query_element(
            &tab_bar,
            TabBarCtx {
                config: &config,
                tab_count: 1,
            },
        );
        let (_, _, _, capturing_status) = query_element(
            &status_bar,
            StatusBarCtx {
                config: &config,
                is_fullscreen: false,
            },
        );
        // Neither should be capturing input in default state.
        assert!(!capturing_tab);
        assert!(!capturing_status);
    }
}
