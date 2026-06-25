//! Modifier state tracking: shift/ctrl/alt/super and Option/Alt key modes.
//!
//! Implements [`InputHandler`] methods that record and resolve keyboard
//! modifier state from winit events. Includes a defensive Windows focus-steal
//! workaround (`sync_modifier_from_key_event`) and resolution of the active
//! Option-key mode based on which Alt key is held. Split from `lib.rs` for
//! organization (AUDIT.md ARC-006); the key-encoding cluster in
//! `key_encoding.rs` reads this state via shared `impl InputHandler` methods.

use winit::event::{ElementState, KeyEvent, Modifiers};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};

use par_term_config::OptionKeyMode;

use super::InputHandler;

impl InputHandler {
    /// Update the current modifier state
    pub fn update_modifiers(&mut self, modifiers: Modifiers) {
        self.modifiers = modifiers;
    }

    /// Update Option/Alt key modes from config
    pub fn update_option_key_modes(&mut self, left: OptionKeyMode, right: OptionKeyMode) {
        self.left_option_key_mode = left;
        self.right_option_key_mode = right;
    }

    /// Track Alt key press/release to know which Alt is active
    pub fn track_alt_key(&mut self, event: &KeyEvent) {
        // Check if this is an Alt key event by physical key
        let is_left_alt = matches!(event.physical_key, PhysicalKey::Code(KeyCode::AltLeft));
        let is_right_alt = matches!(event.physical_key, PhysicalKey::Code(KeyCode::AltRight));

        if is_left_alt {
            self.left_alt_pressed = event.state == ElementState::Pressed;
        } else if is_right_alt {
            self.right_alt_pressed = event.state == ElementState::Pressed;
        }
    }

    /// Defensive modifier-state sync from physical key events.
    ///
    /// On Windows, `WM_NCACTIVATE(false)` fires when a notification, popup, or system
    /// dialog briefly steals visual focus. Winit responds by emitting `ModifiersChanged(empty)`,
    /// which clears our modifier state. Because keyboard focus is never actually lost,
    /// no `WM_SETFOCUS` fires to restore the state. Subsequent `WM_KEYDOWN` messages should
    /// re-trigger `update_modifiers` inside winit, but in practice there is a window where
    /// the state stays zeroed, causing Shift/Ctrl/Alt to stop working until the key is
    /// physically released and re-pressed.
    ///
    /// To guard against this, we synthesize modifier updates directly from `KeyboardInput`
    /// events for physical modifier keys. This runs after `ModifiersChanged` has already been
    /// applied (winit guarantees `ModifiersChanged` fires before `KeyboardInput` for the same
    /// key), so it is a no-op in the normal path and only corrects state when winit's
    /// `ModifiersChanged` is stale or missing.
    pub fn sync_modifier_from_key_event(&mut self, event: &KeyEvent) {
        let pressed = event.state == ElementState::Pressed;
        let mut state = self.modifiers.state();

        match event.physical_key {
            PhysicalKey::Code(KeyCode::ShiftLeft | KeyCode::ShiftRight) => {
                state.set(ModifiersState::SHIFT, pressed);
            }
            PhysicalKey::Code(KeyCode::ControlLeft | KeyCode::ControlRight) => {
                state.set(ModifiersState::CONTROL, pressed);
            }
            PhysicalKey::Code(KeyCode::AltLeft | KeyCode::AltRight) => {
                state.set(ModifiersState::ALT, pressed);
            }
            PhysicalKey::Code(KeyCode::SuperLeft | KeyCode::SuperRight) => {
                state.set(ModifiersState::SUPER, pressed);
            }
            _ => return, // Not a modifier key — nothing to do
        }

        self.modifiers = Modifiers::from(state);
    }

    /// Get the active Option key mode based on which Alt key is pressed
    pub(crate) fn get_active_option_mode(&self) -> OptionKeyMode {
        // If both are pressed, prefer left (arbitrary but consistent)
        // If only one is pressed, use that one's mode
        // If neither is pressed (shouldn't happen when alt modifier is set), default to left
        if self.left_alt_pressed {
            self.left_option_key_mode
        } else if self.right_alt_pressed {
            self.right_option_key_mode
        } else {
            // Fallback: both modes are the same in most configs, so use left
            self.left_option_key_mode
        }
    }
}
