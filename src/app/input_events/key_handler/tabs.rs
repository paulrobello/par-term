//! Tab management keyboard shortcuts (new, close, navigate, move, number-switch).

use crate::app::window_state::WindowState;
use crate::platform::{primary_modifier, primary_modifier_with_shift};
use winit::event::{ElementState, KeyEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, NamedKey};

impl WindowState {
    pub(crate) fn handle_tab_shortcuts(
        &mut self,
        event: &KeyEvent,
        _event_loop: &ActiveEventLoop,
    ) -> bool {
        if event.state != ElementState::Pressed {
            return false;
        }

        let mods = self.input_handler.modifiers.state();
        let ctrl = mods.control_key();
        let shift = mods.shift_key();
        let alt = mods.alt_key();

        // macOS: Cmd is the primary modifier (doesn't conflict with terminal control codes).
        // Windows/Linux: Ctrl+Shift is used to avoid conflicts with Ctrl+T (transpose),
        // Ctrl+W (delete word), Ctrl+N (next history), etc.
        //
        // `primary_modifier`/`primary_modifier_with_shift` from `crate::platform` encapsulate
        // the per-platform modifier selection so each shortcut needs no inline `#[cfg]` block.

        // New Tab: Cmd+T (macOS) / Ctrl+Shift+T (other)
        let is_new_tab = {
            #[cfg(target_os = "macos")]
            {
                primary_modifier(&mods)
            }
            #[cfg(not(target_os = "macos"))]
            {
                primary_modifier_with_shift(&mods)
            }
        } && !alt
            && matches!(event.logical_key, Key::Character(ref c) if c.eq_ignore_ascii_case("t"));

        if is_new_tab {
            self.new_tab_or_show_profiles();
            return true;
        }

        // Close Tab: Cmd+W (macOS) / Ctrl+Shift+W (other)
        // Ctrl+W is "delete word backward" in terminals, must not be intercepted on non-macOS.
        let is_close = {
            #[cfg(target_os = "macos")]
            {
                primary_modifier(&mods)
            }
            #[cfg(not(target_os = "macos"))]
            {
                primary_modifier_with_shift(&mods)
            }
        } && !alt
            && matches!(event.logical_key, Key::Character(ref c) if c.eq_ignore_ascii_case("w"));

        if is_close {
            let should_close_window = self.close_current_tab();
            log::info!("Tab closed (should_close_window: {})", should_close_window);
            if should_close_window {
                self.is_shutting_down = true;
            }
            return true;
        }

        // Next Tab: Cmd+Shift+] (macOS) / Ctrl+Shift+] (other)
        let is_next_bracket = primary_modifier_with_shift(&mods)
            && matches!(event.logical_key, Key::Character(ref c) if c.as_str() == "]");

        if is_next_bracket {
            self.next_tab();
            log::debug!("Switched to next tab");
            return true;
        }

        // Previous Tab: Cmd+Shift+[ (macOS) / Ctrl+Shift+[ (other)
        let is_prev_bracket = primary_modifier_with_shift(&mods)
            && matches!(event.logical_key, Key::Character(ref c) if c.as_str() == "[");

        if is_prev_bracket {
            self.prev_tab();
            log::debug!("Switched to previous tab");
            return true;
        }

        // Ctrl+Tab: Next tab (alternative, universal)
        if ctrl && !shift && matches!(event.logical_key, Key::Named(NamedKey::Tab)) {
            self.next_tab();
            log::debug!("Switched to next tab via Ctrl+Tab");
            return true;
        }

        // Ctrl+Shift+Tab: Previous tab (alternative, universal)
        if ctrl && shift && matches!(event.logical_key, Key::Named(NamedKey::Tab)) {
            self.prev_tab();
            log::debug!("Switched to previous tab via Ctrl+Shift+Tab");
            return true;
        }

        // Move Tab Left: Cmd+Shift+Left (macOS) / Ctrl+Shift+Left (other)
        let is_move_left = primary_modifier_with_shift(&mods)
            && matches!(event.logical_key, Key::Named(NamedKey::ArrowLeft));

        if is_move_left {
            self.move_tab_left();
            log::debug!("Moved tab left");
            return true;
        }

        // Move Tab Right: Cmd+Shift+Right (macOS) / Ctrl+Shift+Right (other)
        let is_move_right = primary_modifier_with_shift(&mods)
            && matches!(event.logical_key, Key::Named(NamedKey::ArrowRight));

        if is_move_right {
            self.move_tab_right();
            log::debug!("Moved tab right");
            return true;
        }

        // Tab switching by number:
        // macOS: Cmd+1-9 / Windows/Linux: Alt+1-9
        // (Ctrl+1-9 don't conflict, but Alt+1-9 is the convention on Linux/Windows)
        #[cfg(target_os = "macos")]
        let is_tab_switch_mod = primary_modifier(&mods);
        #[cfg(not(target_os = "macos"))]
        let is_tab_switch_mod = alt && !shift && !ctrl;

        if is_tab_switch_mod {
            let tab_num = match &event.logical_key {
                Key::Character(c) => match c.as_str() {
                    "1" => Some(1),
                    "2" => Some(2),
                    "3" => Some(3),
                    "4" => Some(4),
                    "5" => Some(5),
                    "6" => Some(6),
                    "7" => Some(7),
                    "8" => Some(8),
                    "9" => Some(9),
                    _ => None,
                },
                _ => None,
            };

            if let Some(n) = tab_num {
                self.switch_to_tab_index(n);
                log::debug!("Switched to tab {}", n);
                return true;
            }
        }

        false
    }
}
