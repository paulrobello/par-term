//! Clipboard operations: paste, copy, and X11 primary selection.
//!
//! Implements [`InputHandler`] methods for reading from and writing to the
//! system clipboard via `arboard`. The X11 primary selection (Linux only) is
//! also wired here. Split from `lib.rs` for organization (AUDIT.md ARC-006).

use super::InputHandler;

impl InputHandler {
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
