//! Clipboard history, paste special, and paste_text key handling.

use crate::app::window_state::WindowState;
use crate::terminal::ClipboardSlot;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{Key, NamedKey};

impl WindowState {
    pub(crate) fn handle_clipboard_history_keys(&mut self, event: &KeyEvent) -> bool {
        // Handle Escape to close clipboard history UI
        if self.overlay_ui.clipboard_history_ui.visible {
            if event.state == ElementState::Pressed {
                match &event.logical_key {
                    Key::Named(NamedKey::Escape) => {
                        self.overlay_ui.clipboard_history_ui.visible = false;
                        self.focus_state.needs_redraw = true;
                        return true;
                    }
                    Key::Named(NamedKey::ArrowUp) => {
                        self.overlay_ui.clipboard_history_ui.select_previous();
                        self.focus_state.needs_redraw = true;
                        return true;
                    }
                    Key::Named(NamedKey::ArrowDown) => {
                        self.overlay_ui.clipboard_history_ui.select_next();
                        self.focus_state.needs_redraw = true;
                        return true;
                    }
                    Key::Named(NamedKey::Enter) => {
                        // Check if Shift is held for paste special
                        let shift = self.input_handler.modifiers.state().shift_key();
                        if let Some(entry) = self.overlay_ui.clipboard_history_ui.selected_entry() {
                            let content = entry.content.clone();
                            self.overlay_ui.clipboard_history_ui.visible = false;

                            if shift {
                                // Shift+Enter: Open paste special UI with the selected content
                                self.overlay_ui.paste_special_ui.open(content);
                                log::info!("Paste special UI opened from clipboard history");
                            } else {
                                // Enter: Paste directly
                                self.paste_text(&content);
                            }
                            self.focus_state.needs_redraw = true;
                        }
                        return true;
                    }
                    _ => {}
                }
            }
            // While clipboard history is visible, consume all key events
            return true;
        }

        // Ctrl+Shift+H: Toggle clipboard history UI
        if event.state == ElementState::Pressed {
            let ctrl = self.input_handler.modifiers.state().control_key();
            let shift = self.input_handler.modifiers.state().shift_key();

            if ctrl
                && shift
                && matches!(event.logical_key, Key::Character(ref c) if c.as_str() == "h" || c.as_str() == "H")
            {
                self.toggle_clipboard_history();
                return true;
            }
        }

        false
    }

    pub(crate) fn toggle_clipboard_history(&mut self) {
        // Refresh clipboard history entries from terminal before showing
        // try_lock: intentional — called from keyboard handler in sync event loop.
        // On miss: clipboard history UI shows stale entries. Acceptable for a UI toggle;
        // the user can dismiss and re-open to get fresh entries.
        if let Some(tab) = self.tab_manager.active_tab()
            && let Ok(term) = tab.terminal.try_write()
        {
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

        self.overlay_ui.clipboard_history_ui.toggle();
        self.focus_state.needs_redraw = true;
        log::debug!(
            "Clipboard history UI toggled: {}",
            self.overlay_ui.clipboard_history_ui.visible
        );
    }

    pub(crate) fn handle_paste_special_keys(&mut self, event: &KeyEvent) -> bool {
        // Handle keys when paste special UI is visible
        if self.overlay_ui.paste_special_ui.visible {
            if event.state == ElementState::Pressed {
                match &event.logical_key {
                    Key::Named(NamedKey::Escape) => {
                        self.overlay_ui.paste_special_ui.close();
                        self.focus_state.needs_redraw = true;
                        return true;
                    }
                    Key::Named(NamedKey::ArrowUp) => {
                        self.overlay_ui.paste_special_ui.select_previous();
                        self.focus_state.needs_redraw = true;
                        return true;
                    }
                    Key::Named(NamedKey::ArrowDown) => {
                        self.overlay_ui.paste_special_ui.select_next();
                        self.focus_state.needs_redraw = true;
                        return true;
                    }
                    Key::Named(NamedKey::Enter) => {
                        // Apply the selected transformation and paste
                        if let Some(result) = self.overlay_ui.paste_special_ui.apply_selected() {
                            self.overlay_ui.paste_special_ui.close();
                            self.paste_text(&result);
                            self.focus_state.needs_redraw = true;
                        }
                        return true;
                    }
                    _ => {}
                }
            }
            // While paste special is visible, consume all key events
            // to prevent them from going to the terminal
            return true;
        }
        false
    }

    pub(crate) fn paste_text(&mut self, text: &str) {
        // SEC-007: Warn when paste content contains control characters that will be stripped.
        // Control characters in clipboard content (ESC, C0, C1) can inject terminal escape
        // sequences. The sanitizer always strips them; this warning alerts the user that
        // clipboard content was modified before pasting.
        if self.config.warn_paste_control_chars
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
            let delay_ms = self.config.paste_delay_ms;
            self.runtime.spawn(async move {
                let term = terminal_clone.write().await;
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
