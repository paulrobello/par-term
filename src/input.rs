use arboard::Clipboard;
use winit::event::{ElementState, KeyEvent, Modifiers};
use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};

use crate::config::OptionKeyMode;

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
    pub fn handle_key_event(&mut self, event: KeyEvent) -> Option<Vec<u8>> {
        if event.state != ElementState::Pressed {
            return None;
        }

        let ctrl = self.modifiers.state().control_key();
        let alt = self.modifiers.state().alt_key();

        match event.logical_key {
            // Character keys
            Key::Character(ref s) => {
                if ctrl {
                    // Handle Ctrl+key combinations
                    let ch = s.chars().next()?;

                    // Note: Ctrl+V paste is handled at higher level for bracketed paste support

                    if ch.is_ascii_alphabetic() {
                        // Ctrl+A through Ctrl+Z map to ASCII 1-26
                        let byte = (ch.to_ascii_lowercase() as u8) - b'a' + 1;
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

                let seq = match named_key {
                    NamedKey::Enter => "\r",
                    NamedKey::Tab => "\t",
                    NamedKey::Space => " ",
                    NamedKey::Backspace => "\x7f",
                    NamedKey::Escape => "\x1b",
                    NamedKey::Insert => "\x1b[2~",
                    NamedKey::Delete => "\x1b[3~",

                    // Arrow keys
                    NamedKey::ArrowUp => "\x1b[A",
                    NamedKey::ArrowDown => "\x1b[B",
                    NamedKey::ArrowRight => "\x1b[C",
                    NamedKey::ArrowLeft => "\x1b[D",

                    // Navigation keys
                    NamedKey::Home => "\x1b[H",
                    NamedKey::End => "\x1b[F",
                    NamedKey::PageUp => "\x1b[5~",
                    NamedKey::PageDown => "\x1b[6~",

                    // Function keys
                    NamedKey::F1 => "\x1bOP",
                    NamedKey::F2 => "\x1bOQ",
                    NamedKey::F3 => "\x1bOR",
                    NamedKey::F4 => "\x1bOS",
                    NamedKey::F5 => "\x1b[15~",
                    NamedKey::F6 => "\x1b[17~",
                    NamedKey::F7 => "\x1b[18~",
                    NamedKey::F8 => "\x1b[19~",
                    NamedKey::F9 => "\x1b[20~",
                    NamedKey::F10 => "\x1b[21~",
                    NamedKey::F11 => "\x1b[23~",
                    NamedKey::F12 => "\x1b[24~",

                    _ => return None,
                };

                Some(seq.as_bytes().to_vec())
            }

            _ => None,
        }
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
