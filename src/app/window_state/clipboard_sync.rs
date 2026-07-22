//! OSC 52 clipboard bridge.
//!
//! Programs (locally or over SSH) set the clipboard via the OSC 52 escape
//! sequence. The core terminal stores that payload but does not touch the OS
//! clipboard itself; this module polls it each frame and, when it changes,
//! writes it to the system clipboard via `arboard`. Gated by
//! `config.osc52_clipboard` (default on) so a remote program can reach the
//! local clipboard over a plain terminal/SSH session.

use super::WindowState;

impl WindowState {
    /// Bridge OSC 52 clipboard writes to the system clipboard.
    ///
    /// Called each frame after `check_trigger_actions()`. Polls the active
    /// tab's terminal for OSC 52 content set by a program; when it differs
    /// from the last value we applied, pushes it to the OS clipboard.
    ///
    /// try_lock: intentional — clipboard polling runs in about_to_wait (sync
    /// event loop). On a miss the write is simply retried next frame.
    pub(crate) fn check_clipboard_sync(&mut self) {
        if !self.config.load().osc52_clipboard {
            return;
        }

        let content = if let Some(tab) = self.tab_manager.active_tab()
            && let Ok(term) = tab.terminal.try_read()
        {
            term.get_clipboard()
        } else {
            return;
        };

        // Only act on a real change. `None` means no OSC 52 content exists —
        // never clear the system clipboard for that, since it would clobber
        // local copies (and image clipboards) on tab switches.
        if let Some(content) = content
            && self.last_osc52_clipboard.as_deref() != Some(content.as_str())
        {
            match self.input_handler.copy_to_clipboard(&content) {
                Ok(()) => {
                    crate::debug_info!(
                        "CLIPBOARD",
                        "OSC 52 synced {} chars to system clipboard",
                        content.len()
                    );
                    self.last_osc52_clipboard = Some(content);
                }
                Err(e) => log::error!("OSC 52 clipboard sync failed: {}", e),
            }
        }
    }
}
