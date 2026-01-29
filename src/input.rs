use arboard::Clipboard;
use winit::event::{ElementState, KeyEvent, Modifiers};
use winit::keyboard::{Key, NamedKey};

/// Input handler for converting winit events to terminal input
pub struct InputHandler {
    pub modifiers: Modifiers,
    clipboard: Option<Clipboard>,
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
        }
    }

    /// Update the current modifier state
    pub fn update_modifiers(&mut self, modifiers: Modifiers) {
        self.modifiers = modifiers;
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

                // Regular character input
                let mut bytes = s.as_bytes().to_vec();

                // Handle Alt key (sends ESC prefix)
                if alt {
                    bytes.insert(0, 0x1b);
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
