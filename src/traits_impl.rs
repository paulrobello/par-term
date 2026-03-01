//! Concrete implementations of the traits defined in [`crate::traits`].
//!
//! # What is implemented here
//!
//! - [`TerminalAccess`] on [`crate::terminal::TerminalManager`] — all five
//!   read-only query methods are direct thin wrappers over the existing
//!   `TerminalManager` methods, so there is no behaviour change at call sites.
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

use crate::terminal::TerminalManager;
use crate::traits::TerminalAccess;

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
}
