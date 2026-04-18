//! Keyboard input handling and VT byte sequence generation for par-term.
//!
//! This crate converts `winit` keyboard events into the terminal input byte
//! sequences expected by shell applications. It handles character input,
//! named keys, function keys, modifier combinations, Option/Alt key modes,
//! clipboard operations, and the modifyOtherKeys protocol extension.
//!
//! The primary entry point is [`InputHandler`], which tracks modifier state
//! and translates each [`winit::event::KeyEvent`] into a `Vec<u8>` suitable
//! for writing directly to the PTY.

use arboard::Clipboard;
use winit::event::{ElementState, KeyEvent, Modifiers};
use winit::keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey};

use par_term_config::OptionKeyMode;

/// Input handler for converting winit events to terminal input
pub struct InputHandler {
    pub modifiers: Modifiers,
    clipboard: Option<Clipboard>,
    /// Option key mode for left Option/Alt key
    pub left_option_key_mode: OptionKeyMode,
    /// Option key mode for right Option/Alt key
    pub right_option_key_mode: OptionKeyMode,
    /// Track which Alt key is currently pressed (for determining mode on character input)
    /// True = left Alt is pressed, False = right Alt or no Alt
    left_alt_pressed: bool,
    /// True = right Alt is pressed
    right_alt_pressed: bool,
}

impl InputHandler {
    /// Create a new input handler
    pub fn new() -> Self {
        let clipboard = Clipboard::new().ok();
        if clipboard.is_none() {
            log::warn!("Failed to initialize clipboard support");
        }

        Self {
            modifiers: Modifiers::default(),
            clipboard,
            left_option_key_mode: OptionKeyMode::default(),
            right_option_key_mode: OptionKeyMode::default(),
            left_alt_pressed: false,
            right_alt_pressed: false,
        }
    }

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
    fn get_active_option_mode(&self) -> OptionKeyMode {
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

    /// Apply Option/Alt key transformation based on the configured mode
    fn apply_option_key_mode(&self, bytes: &mut Vec<u8>, original_char: char) {
        let mode = self.get_active_option_mode();

        match mode {
            OptionKeyMode::Normal => {
                // Normal mode: the character is already the special character from the OS
                // (e.g., Option+f = ƒ on macOS). Don't modify it.
                // The bytes already contain the correct character from winit.
            }
            OptionKeyMode::Meta => {
                // Meta mode: set the high bit (8th bit) on the character
                // This only works for ASCII characters (0-127)
                if original_char.is_ascii() {
                    let meta_byte = (original_char as u8) | 0x80;
                    bytes.clear();
                    bytes.push(meta_byte);
                }
                // For non-ASCII, fall through to ESC mode behavior
                else {
                    bytes.insert(0, 0x1b);
                }
            }
            OptionKeyMode::Esc => {
                // Esc mode: send ESC prefix before the character
                // First, we need to use the base character, not the special character
                // This requires getting the unmodified key
                if original_char.is_ascii() {
                    bytes.clear();
                    bytes.push(0x1b); // ESC
                    bytes.push(original_char as u8);
                } else {
                    // For non-ASCII original characters, just prepend ESC to what we have
                    bytes.insert(0, 0x1b);
                }
            }
        }
    }

    /// Convert a keyboard event to terminal input bytes
    ///
    /// If `modify_other_keys_mode` is > 0, keys with modifiers will be reported
    /// using the XTerm modifyOtherKeys format: CSI 27 ; modifier ; keycode ~
    pub fn handle_key_event(&mut self, event: KeyEvent) -> Option<Vec<u8>> {
        self.handle_key_event_with_mode(event, 0, false)
    }

    /// Convert a keyboard event to terminal input bytes with modifyOtherKeys support
    ///
    /// `modify_other_keys_mode`:
    /// - 0: Disabled (normal key handling)
    /// - 1: Report modifiers for special keys only
    /// - 2: Report modifiers for all keys
    ///
    /// `application_cursor`: When true (DECCKM mode enabled), arrow keys send
    /// SS3 sequences (ESC O A) instead of CSI sequences (ESC [ A).
    pub fn handle_key_event_with_mode(
        &mut self,
        event: KeyEvent,
        modify_other_keys_mode: u8,
        application_cursor: bool,
    ) -> Option<Vec<u8>> {
        if event.state != ElementState::Pressed {
            return None;
        }

        let ctrl = self.modifiers.state().control_key();
        let alt = self.modifiers.state().alt_key();

        // Check if we should use modifyOtherKeys encoding.
        //
        // Both mode 1 and mode 2 use the same encoding path here — the per-mode routing
        // decisions are made inside `try_modify_other_keys_encoding` (e.g. the Shift-only
        // exemption that matches iTerm2's reference implementation).
        if modify_other_keys_mode > 0
            && let Some(bytes) = self.try_modify_other_keys_encoding(&event)
        {
            return Some(bytes);
        }

        match event.logical_key {
            // Character keys
            Key::Character(ref s) => {
                if ctrl {
                    // Handle Ctrl+key combinations
                    let ch = s.chars().next()?;

                    // Note: Ctrl+V paste is handled at higher level for bracketed paste support

                    if ch.is_ascii_alphabetic() {
                        // Ctrl+A through Ctrl+Z map to ASCII 1-26.
                        // When Alt/Option is also held and enhanced modifier reporting is
                        // unavailable, preserve the Alt modifier using the configured
                        // Option-key mode so Ctrl+Alt+letter stays distinct from plain
                        // Ctrl+letter for terminal applications that rely on Meta+Ctrl.
                        let byte = (ch.to_ascii_lowercase() as u8) - b'a' + 1;
                        if alt {
                            return Some(match self.get_active_option_mode() {
                                OptionKeyMode::Meta => vec![byte | 0x80],
                                OptionKeyMode::Normal | OptionKeyMode::Esc => vec![0x1b, byte],
                            });
                        }
                        return Some(vec![byte]);
                    }
                }

                // Get the base character (without Alt modification) for Option key modes
                // We need to look at the physical key to get the unmodified character
                let base_char = self.get_base_character(&event);

                // Regular character input
                let mut bytes = s.as_bytes().to_vec();

                // Handle Alt/Option key based on configured mode
                if alt {
                    if let Some(base) = base_char {
                        self.apply_option_key_mode(&mut bytes, base);
                    } else {
                        // Fallback: if we can't determine base character, use the first char
                        let ch = s.chars().next().unwrap_or('\0');
                        self.apply_option_key_mode(&mut bytes, ch);
                    }
                }

                Some(bytes)
            }

            // Special keys
            Key::Named(named_key) => {
                // Handle Ctrl+Space specially - sends NUL (0x00)
                if ctrl && matches!(named_key, NamedKey::Space) {
                    return Some(vec![0x00]);
                }

                // Note: Shift+Insert paste is handled at higher level for bracketed paste support

                let shift = self.modifiers.state().shift_key();

                // Compute xterm modifier parameter for named keys.
                // Standard: bit0=Shift, bit1=Alt, bit2=Ctrl; value = bits + 1.
                // Only applied when at least one modifier is held.
                let has_modifier = shift || alt || ctrl;
                let modifier_param = if has_modifier {
                    let mut bits = 0u8;
                    if shift {
                        bits |= 1;
                    }
                    if alt {
                        bits |= 2;
                    }
                    if ctrl {
                        bits |= 4;
                    }
                    Some(bits + 1)
                } else {
                    None
                };

                // Keys that use the "letter" form: CSI 1;modifier letter (with modifier)
                // or CSI letter / SS3 letter (without modifier).
                // Note: SS3 (application cursor mode) is only used when no modifier is
                // present — with a modifier the sequence switches to CSI form per xterm.
                if let Some(suffix) = match named_key {
                    NamedKey::ArrowUp => Some('A'),
                    NamedKey::ArrowDown => Some('B'),
                    NamedKey::ArrowRight => Some('C'),
                    NamedKey::ArrowLeft => Some('D'),
                    NamedKey::Home => Some('H'),
                    NamedKey::End => Some('F'),
                    _ => None,
                } {
                    return if let Some(m) = modifier_param {
                        // CSI 1 ; modifier letter
                        Some(format!("\x1b[1;{m}{suffix}").into_bytes())
                    } else if application_cursor
                        && matches!(
                            named_key,
                            NamedKey::ArrowUp
                                | NamedKey::ArrowDown
                                | NamedKey::ArrowRight
                                | NamedKey::ArrowLeft
                        )
                    {
                        // SS3 letter (application cursor, no modifier)
                        Some(format!("\x1bO{suffix}").into_bytes())
                    } else {
                        // CSI letter (normal mode, no modifier)
                        Some(format!("\x1b[{suffix}").into_bytes())
                    };
                }

                // Keys that use the "tilde" form: CSI keycode ; modifier ~ (with modifier)
                // or CSI keycode ~ (without modifier).
                if let Some(keycode) = match named_key {
                    NamedKey::Insert => Some(2),
                    NamedKey::Delete => Some(3),
                    NamedKey::PageUp => Some(5),
                    NamedKey::PageDown => Some(6),
                    NamedKey::F5 => Some(15),
                    NamedKey::F6 => Some(17),
                    NamedKey::F7 => Some(18),
                    NamedKey::F8 => Some(19),
                    NamedKey::F9 => Some(20),
                    NamedKey::F10 => Some(21),
                    NamedKey::F11 => Some(23),
                    NamedKey::F12 => Some(24),
                    _ => None,
                } {
                    return if let Some(m) = modifier_param {
                        Some(format!("\x1b[{keycode};{m}~").into_bytes())
                    } else {
                        Some(format!("\x1b[{keycode}~").into_bytes())
                    };
                }

                // F1-F4 use SS3 form without modifier, CSI form with modifier.
                // SS3 P/Q/R/S → CSI 1;modifier P/Q/R/S
                if let Some(suffix) = match named_key {
                    NamedKey::F1 => Some('P'),
                    NamedKey::F2 => Some('Q'),
                    NamedKey::F3 => Some('R'),
                    NamedKey::F4 => Some('S'),
                    _ => None,
                } {
                    return if let Some(m) = modifier_param {
                        Some(format!("\x1b[1;{m}{suffix}").into_bytes())
                    } else {
                        Some(format!("\x1bO{suffix}").into_bytes())
                    };
                }

                // Remaining keys with special handling (no modifier encoding)
                let seq = match named_key {
                    // Shift+Enter sends LF (newline) for soft line breaks (like iTerm2)
                    // Regular Enter sends CR (carriage return) for command execution
                    NamedKey::Enter => {
                        if shift {
                            "\n"
                        } else {
                            "\r"
                        }
                    }
                    // Shift+Tab sends reverse-tab escape sequence (CSI Z)
                    // Regular Tab sends HT (horizontal tab)
                    NamedKey::Tab => {
                        if shift {
                            "\x1b[Z"
                        } else {
                            "\t"
                        }
                    }
                    NamedKey::Space => " ",
                    NamedKey::Backspace => "\x7f",
                    NamedKey::Escape => "\x1b",

                    _ => return None,
                };

                Some(seq.as_bytes().to_vec())
            }

            _ => None,
        }
    }

    /// Try to encode a key event using modifyOtherKeys format
    ///
    /// Returns Some(bytes) if the key should be encoded with modifyOtherKeys,
    /// None if normal handling should be used.
    ///
    /// modifyOtherKeys format: CSI 27 ; modifier ; keycode ~
    /// Where modifier is:
    /// - 2 = Shift
    /// - 3 = Alt
    /// - 4 = Shift+Alt
    /// - 5 = Ctrl
    /// - 6 = Shift+Ctrl
    /// - 7 = Alt+Ctrl
    /// - 8 = Shift+Alt+Ctrl
    fn try_modify_other_keys_encoding(&self, event: &KeyEvent) -> Option<Vec<u8>> {
        let ctrl = self.modifiers.state().control_key();
        let alt = self.modifiers.state().alt_key();
        let shift = self.modifiers.state().shift_key();

        // No modifiers means no special encoding needed
        if !ctrl && !alt && !shift {
            return None;
        }

        // Get the base character for the key
        let base_char = self.get_base_character(event)?;

        // Skip modifyOtherKeys encoding for any Shift-only combination on printable
        // characters, regardless of mode or character class.
        //
        // This matches iTerm2's reference implementation (sources/iTermModifyOtherKeysMapper.m
        // and iTermModifyOtherKeysMapper1.m), which is confirmed by its `iTermModifyOtherKeys1Test`
        // suite: for Shift+letter, Shift+digit, and Shift+symbol iTerm2 returns `nil` from
        // `keyMapperStringForPreCocoaEvent` and lets Cocoa's text-input system emit the OS-
        // resolved shifted character (`A`, `!`, `@`, `{`, etc.) directly to the PTY. The same
        // rule applies in both mode 1 (via `shouldModifyOtherKeysForNumberEvent` / ...Symbol /
        // ...RegularEvent returning NO) and mode 2 (via the base mapper returning nil unless
        // Control is held).
        //
        // Why this is necessary: winit's `logical_key` already contains the layout-correct
        // shifted character. TUI applications built on crossterm (Claude Code, etc.) that see
        // a `CSI 27;2;49~` sequence cannot reverse-map the base codepoint `49` ('1') to the
        // shifted codepoint `33` ('!') because they have no access to the OS keyboard layout
        // tables — they just render the base character. Falling through to the normal
        // `Key::Character` path below sends the winit-provided shifted character as raw bytes,
        // which every application handles correctly.
        //
        // We intentionally keep Shift+Alt/Shift+Ctrl etc. encoded here because those carry
        // modifier information that cannot be recovered from the character alone.
        if shift && !ctrl && !alt {
            return None;
        }

        // Calculate the modifier value
        // bit 0 (1) = Shift
        // bit 1 (2) = Alt
        // bit 2 (4) = Ctrl
        // The final value is bits + 1
        let mut modifier_bits = 0u8;
        if shift {
            modifier_bits |= 1;
        }
        if alt {
            modifier_bits |= 2;
        }
        if ctrl {
            modifier_bits |= 4;
        }

        // Add 1 to get the XTerm modifier value (so no modifiers would be 1, but we already checked for that)
        let modifier_value = modifier_bits + 1;

        // Get the Unicode codepoint of the base character
        let keycode = base_char as u32;

        // Format: CSI 27 ; modifier ; keycode ~
        // CSI = ESC [
        Some(format!("\x1b[27;{};{}~", modifier_value, keycode).into_bytes())
    }

    /// Get the base character from a key event (the character without Alt modification)
    /// This maps physical key codes to their unmodified ASCII characters
    fn get_base_character(&self, event: &KeyEvent) -> Option<char> {
        // Map physical key codes to their base characters
        // This is needed because on macOS, Option+key produces a different logical character
        match event.physical_key {
            PhysicalKey::Code(code) => match code {
                KeyCode::KeyA => Some('a'),
                KeyCode::KeyB => Some('b'),
                KeyCode::KeyC => Some('c'),
                KeyCode::KeyD => Some('d'),
                KeyCode::KeyE => Some('e'),
                KeyCode::KeyF => Some('f'),
                KeyCode::KeyG => Some('g'),
                KeyCode::KeyH => Some('h'),
                KeyCode::KeyI => Some('i'),
                KeyCode::KeyJ => Some('j'),
                KeyCode::KeyK => Some('k'),
                KeyCode::KeyL => Some('l'),
                KeyCode::KeyM => Some('m'),
                KeyCode::KeyN => Some('n'),
                KeyCode::KeyO => Some('o'),
                KeyCode::KeyP => Some('p'),
                KeyCode::KeyQ => Some('q'),
                KeyCode::KeyR => Some('r'),
                KeyCode::KeyS => Some('s'),
                KeyCode::KeyT => Some('t'),
                KeyCode::KeyU => Some('u'),
                KeyCode::KeyV => Some('v'),
                KeyCode::KeyW => Some('w'),
                KeyCode::KeyX => Some('x'),
                KeyCode::KeyY => Some('y'),
                KeyCode::KeyZ => Some('z'),
                KeyCode::Digit0 => Some('0'),
                KeyCode::Digit1 => Some('1'),
                KeyCode::Digit2 => Some('2'),
                KeyCode::Digit3 => Some('3'),
                KeyCode::Digit4 => Some('4'),
                KeyCode::Digit5 => Some('5'),
                KeyCode::Digit6 => Some('6'),
                KeyCode::Digit7 => Some('7'),
                KeyCode::Digit8 => Some('8'),
                KeyCode::Digit9 => Some('9'),
                KeyCode::Minus => Some('-'),
                KeyCode::Equal => Some('='),
                KeyCode::BracketLeft => Some('['),
                KeyCode::BracketRight => Some(']'),
                KeyCode::Backslash => Some('\\'),
                KeyCode::Semicolon => Some(';'),
                KeyCode::Quote => Some('\''),
                KeyCode::Backquote => Some('`'),
                KeyCode::Comma => Some(','),
                KeyCode::Period => Some('.'),
                KeyCode::Slash => Some('/'),
                KeyCode::Space => Some(' '),
                _ => None,
            },
            _ => None,
        }
    }

    /// Paste text from clipboard (returns raw text, caller handles terminal conversion)
    pub fn paste_from_clipboard(&mut self) -> Option<String> {
        if let Some(ref mut clipboard) = self.clipboard {
            match clipboard.get_text() {
                Ok(text) => {
                    log::debug!("Pasting from clipboard: {} chars", text.len());
                    Some(text)
                }
                Err(e) => {
                    log::error!("Failed to get clipboard text: {}", e);
                    None
                }
            }
        } else {
            log::warn!("Clipboard not available");
            None
        }
    }

    /// Check if clipboard contains an image (used when text paste returns None
    /// to determine if we should forward the paste event to the terminal for
    /// image-aware applications like Claude Code)
    pub fn clipboard_has_image(&mut self) -> bool {
        if let Some(ref mut clipboard) = self.clipboard {
            let has_image = clipboard.get_image().is_ok();
            log::debug!("Clipboard image check: {}", has_image);
            has_image
        } else {
            false
        }
    }

    /// Copy text to clipboard
    pub fn copy_to_clipboard(&mut self, text: &str) -> Result<(), String> {
        if let Some(ref mut clipboard) = self.clipboard {
            clipboard
                .set_text(text.to_string())
                .map_err(|e| format!("Failed to set clipboard text: {}", e))
        } else {
            Err("Clipboard not available".to_string())
        }
    }

    /// Copy text to primary selection (Linux X11 only)
    #[cfg(target_os = "linux")]
    pub fn copy_to_primary_selection(&mut self, text: &str) -> Result<(), String> {
        use arboard::SetExtLinux;

        if let Some(ref mut clipboard) = self.clipboard {
            clipboard
                .set()
                .clipboard(arboard::LinuxClipboardKind::Primary)
                .text(text.to_string())
                .map_err(|e| format!("Failed to set primary selection: {}", e))?;
            Ok(())
        } else {
            Err("Clipboard not available".to_string())
        }
    }

    /// Paste text from primary selection (Linux X11 only, returns raw text)
    #[cfg(target_os = "linux")]
    pub fn paste_from_primary_selection(&mut self) -> Option<String> {
        use arboard::GetExtLinux;

        if let Some(ref mut clipboard) = self.clipboard {
            match clipboard
                .get()
                .clipboard(arboard::LinuxClipboardKind::Primary)
                .text()
            {
                Ok(text) => {
                    log::debug!("Pasting from primary selection: {} chars", text.len());
                    Some(text)
                }
                Err(e) => {
                    log::error!("Failed to get primary selection text: {}", e);
                    None
                }
            }
        } else {
            log::warn!("Clipboard not available");
            None
        }
    }

    /// Fallback for non-Linux platforms - copy to primary selection not supported
    #[cfg(not(target_os = "linux"))]
    pub fn copy_to_primary_selection(&mut self, _text: &str) -> Result<(), String> {
        Ok(()) // No-op on non-Linux platforms
    }

    /// Fallback for non-Linux platforms - paste from primary selection uses regular clipboard
    #[cfg(not(target_os = "linux"))]
    pub fn paste_from_primary_selection(&mut self) -> Option<String> {
        self.paste_from_clipboard()
    }
}

impl Default for InputHandler {
    fn default() -> Self {
        Self::new()
    }
}
