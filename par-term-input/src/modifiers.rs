//! Modifier state tracking: shift/ctrl/alt/super and which Alt key is held.
//!
//! Implements [`InputHandler`] methods that record and resolve keyboard
//! modifier state from winit events. Includes a defensive Windows focus-steal
//! workaround (`sync_modifier_from_key_event`) and the translation of that
//! state into the shared encoder's modifier bits, including the `ALT_RIGHT`
//! side bit that selects the right Option-key mode. Split from `lib.rs` for
//! organization (AUDIT.md ARC-006); the key-encoding cluster in
//! `key_encoding.rs` reads this state via shared `impl InputHandler` methods.

use par_term_emu_core_rust::keyboard::{modifiers as term_mods, option_modes};
use winit::event::{ElementState, KeyEvent, Modifiers};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};

use super::InputHandler;

impl InputHandler {
    /// Update the current modifier state
    pub fn update_modifiers(&mut self, modifiers: Modifiers) {
        self.modifiers = modifiers;
    }

    /// Track Alt key press/release to know which Alt is active
    pub fn track_alt_key(&mut self, event: &KeyEvent) {
        self.track_alt_physical_key(event.physical_key, event.state);
    }

    /// [`track_alt_key`](Self::track_alt_key) for callers without a winit
    /// `KeyEvent` (which cannot be constructed outside winit).
    pub fn track_alt_physical_key(&mut self, physical_key: PhysicalKey, state: ElementState) {
        let pressed = state == ElementState::Pressed;
        match physical_key {
            PhysicalKey::Code(KeyCode::AltLeft) => self.left_alt_pressed = pressed,
            PhysicalKey::Code(KeyCode::AltRight) => self.right_alt_pressed = pressed,
            _ => {}
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

    /// The held modifiers as the shared encoder's bits. Super carries no
    /// legacy xterm bit and is left out. `ALT_RIGHT` is set only when the
    /// right Alt alone is held: both held (or neither tracked) reports left.
    pub(crate) fn term_modifiers(&self) -> u8 {
        let state = self.modifiers.state();
        let mut bits = 0;
        if state.shift_key() {
            bits |= term_mods::SHIFT;
        }
        if state.alt_key() {
            bits |= term_mods::ALT;
            if self.right_alt_pressed && !self.left_alt_pressed {
                bits |= term_mods::ALT_RIGHT;
            }
        }
        if state.control_key() {
            bits |= term_mods::CTRL;
        }
        bits
    }

    /// The Option mode the encoder will apply for `mods`, resolved the same
    /// way the core does (side from `ALT_RIGHT`, unknown values as Normal).
    pub(crate) fn active_option_mode(&self, mods: u8) -> u8 {
        let mode = if mods & term_mods::ALT_RIGHT != 0 {
            self.key_options.right_option
        } else {
            self.key_options.left_option
        };
        match mode {
            option_modes::META | option_modes::ESC => mode,
            _ => option_modes::NORMAL,
        }
    }
}
