//! The clipboard history toggle and `paste_text`.
//!
//! Opening the clipboard-history panel resolves through the registry's
//! `toggle_clipboard_history` default (UX K2). While it (or paste special)
//! is open, its keys are read by its own `show()` on the egui side, behind
//! the overlay stack (UX.md OV2).

use crate::app::window_state::WindowState;
use par_term_terminal::ClipboardSlot;

impl WindowState {
    pub(crate) fn toggle_clipboard_history(&mut self) {
        // Refresh clipboard history entries from terminal before showing.
        // read_terminal_handle, not tab.terminal: OSC 52 payloads are parsed
        // into the FOCUSED pane's terminal — in a split (or a mux tab, where
        // tab.terminal is the hidden login shell) tab.terminal's history is
        // empty and the panel would open blank (B70).
        // try_lock: intentional — called from keyboard handler in sync event loop.
        // On miss: clipboard history UI shows stale entries. Acceptable for a UI toggle;
        // the user can dismiss and re-open to get fresh entries.
        if let Some(tab) = self.tab_manager.active_tab() {
            let terminal = tab.read_terminal_handle();
            if let Ok(term) = terminal.try_read() {
                // Get history for all slots and merge
                let mut all_entries = Vec::new();
                all_entries.extend(term.get_clipboard_history(ClipboardSlot::Primary));
                all_entries.extend(term.get_clipboard_history(ClipboardSlot::Clipboard));
                all_entries.extend(term.get_clipboard_history(ClipboardSlot::Selection));

                // Sort by timestamp (newest first)
                all_entries.sort_by_key(|e| std::cmp::Reverse(e.timestamp));

                self.overlay_ui
                    .clipboard_history_ui
                    .update_entries(all_entries);
            }
        }

        self.overlay_ui.clipboard_history_ui.toggle();
        self.focus_state.needs_redraw = true;
        log::debug!(
            "Clipboard history UI toggled: {}",
            self.overlay_ui.clipboard_history_ui.visible
        );
    }

    pub(crate) fn paste_text(&mut self, text: &str) {
        // SEC-007: Warn when paste content contains control characters that will be stripped.
        // Control characters in clipboard content (ESC, C0, C1) can inject terminal escape
        // sequences. The sanitizer always strips them; this warning alerts the user that
        // clipboard content was modified before pasting.
        if self.config.load().selection.warn_paste_control_chars
            && crate::paste_transform::paste_contains_control_chars(text)
        {
            log::warn!(
                "Clipboard paste content contained control characters (ESC, C0, C1) that were \
                 stripped before pasting to prevent terminal escape sequence injection. \
                 This may indicate the clipboard contains crafted or binary content. \
                 Set `warn_paste_control_chars: false` in config to suppress this warning."
            );
            crate::debug_info!(
                "PASTE",
                "SECURITY: paste content contained control chars — stripped before PTY write \
                 ({} chars original)",
                text.len(),
            );
        }

        // Sanitize clipboard content to strip dangerous control characters
        // (escape sequences, C0/C1 controls) before sending to PTY
        let text = crate::paste_transform::sanitize_paste_content(text);

        // Broadcast includes pastes (UX.md V5).
        if self.broadcast_paste(&text) {
            return;
        }

        // Try to paste via tmux if connected
        if self.paste_via_tmux(&text) {
            return; // Paste was routed through tmux
        }

        // Fall back to direct terminal paste
        if let Some(tab) = self.tab_manager.active_tab() {
            use std::sync::Arc;
            // Route to focused pane's terminal in split-pane mode.
            // In single-pane mode the focused pane wraps Tab::terminal (same Arc).
            let terminal_clone = tab
                .pane_manager
                .as_ref()
                .and_then(|pm| pm.focused_pane())
                .map(|pane| Arc::clone(&pane.terminal))
                .unwrap_or_else(|| Arc::clone(&tab.terminal));
            let delay_ms = self.config.load().selection.paste_delay_ms;
            self.runtime.spawn(async move {
                let term = terminal_clone.read().await;
                if delay_ms > 0 && text.contains('\n') {
                    let _ = term.paste_with_delay(&text, delay_ms).await;
                } else {
                    let _ = term.paste(&text);
                }
                log::debug!("Pasted text ({} chars)", text.len());
            });
        }
    }
}
