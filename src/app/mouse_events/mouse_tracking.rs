use crate::app::window_state::WindowState;
use std::sync::Arc;

impl WindowState {
    /// Send mouse event to terminal if mouse tracking is enabled
    ///
    /// Returns true if event was consumed by terminal (mouse tracking enabled or alt screen active),
    /// false otherwise. When on alt screen, we don't want local text selection.
    ///
    /// In split pane mode, this routes events to the focused pane's terminal with
    /// pane-relative cell coordinates.
    pub(crate) fn try_send_mouse_event(&self, button: u8, pressed: bool) -> bool {
        let tab = if let Some(t) = self.tab_manager.active_tab() {
            t
        } else {
            return false;
        };

        let mouse_position = tab.active_mouse().position;

        // Get the correct terminal, cell coordinates, and (for tmux routing) native pane ID
        let (terminal_arc, col, row, native_pane_id) = if let Some(ref pm) = tab.pane_manager
            && let Some(focused_pane) = pm.focused_pane()
        {
            // Split pane mode: use focused pane's terminal with pane-relative coordinates
            let Some((col, row)) =
                self.pixel_to_pane_cell(mouse_position.0, mouse_position.1, &focused_pane.bounds)
            else {
                return false;
            };
            (
                Arc::clone(&focused_pane.terminal),
                col,
                row,
                Some(focused_pane.id),
            )
        } else {
            // Single pane: use tab's terminal with global coordinates
            let Some((col, row)) = self.pixel_to_cell(mouse_position.0, mouse_position.1) else {
                return false;
            };
            (Arc::clone(&tab.terminal), col, row, None)
        };

        // try_read (not try_write): all operations below are &self on TerminalManager
        // (is_mouse_tracking_enabled, is_alt_screen_active, encode_mouse_event, write).
        // Using a shared read lock eliminates cascading contention where a previous async
        // write task holding the outer write lock (blocked on the inner parking_lot Mutex)
        // prevents new clicks from acquiring the lock — silently dropping them.
        let Ok(term) = terminal_arc.try_read() else {
            return false;
        };

        // Check if alternate screen is active - don't do local selection on alt screen
        // even if mouse tracking isn't enabled (e.g., some TUI apps don't enable mouse)
        let alt_screen_active = term.is_alt_screen_active();

        // Check if mouse tracking is enabled
        if term.is_mouse_tracking_enabled() {
            // Encode mouse event
            let encoded = term.encode_mouse_event(button, col, row, pressed, 0);
            // Release the read lock before any I/O so we don't hold it across awaits
            drop(term);

            if !encoded.is_empty() {
                // For tmux display panes: route via the gateway so the TUI app running
                // inside the real tmux pane actually receives the mouse event.  Writing
                // to the local virtual terminal (no PTY) silently drops the bytes.
                if self.is_tmux_connected()
                    && let Some(native_id) = native_pane_id
                    && let Some(&tmux_pane_id) =
                        self.tmux_state.native_pane_to_tmux_pane.get(&native_id)
                {
                    let escaped = crate::tmux::escape_keys_for_tmux(&encoded);
                    let cmd = format!("send-keys -t %{} {}\n", tmux_pane_id, escaped);
                    self.write_to_gateway(&cmd);
                } else {
                    // Non-tmux path: write directly to the local PTY.
                    // read().await (not write().await): TerminalManager::write() takes &self,
                    // mutation happens behind the inner parking_lot::Mutex<PtySession>.
                    let terminal_clone = Arc::clone(&terminal_arc);
                    let runtime = Arc::clone(&self.runtime);
                    runtime.spawn(async move {
                        let t = terminal_clone.read().await;
                        let _ = t.write(&encoded);
                    });
                }
            }
            return true; // Event consumed by mouse tracking
        }

        // On alt screen without mouse tracking - still consume event to prevent selection
        if alt_screen_active {
            return true;
        }

        false // Event not consumed, handle normally
    }

    pub(crate) fn active_terminal_mouse_tracking_enabled_at(
        &self,
        mouse_position: (f64, f64),
    ) -> bool {
        let Some(tab) = self.tab_manager.active_tab() else {
            return false;
        };

        if let Some(ref pm) = tab.pane_manager
            && let Some(focused_pane) = pm.focused_pane()
        {
            if self
                .pixel_to_pane_cell(mouse_position.0, mouse_position.1, &focused_pane.bounds)
                .is_none()
            {
                return false;
            }
            // try_read (not try_write): is_mouse_tracking_enabled() takes &self.
            // On miss: returns false (no mouse tracking), which may cause a missed mouse
            // event routing. The next mouse move/click will re-query correctly.
            return focused_pane
                .terminal
                .try_read()
                .ok()
                .is_some_and(|term| term.is_mouse_tracking_enabled());
        }

        if self
            .pixel_to_cell(mouse_position.0, mouse_position.1)
            .is_none()
        {
            return false;
        }

        // try_read (not try_write): is_mouse_tracking_enabled() takes &self.
        // On miss: returns false (no tracking detected). Cosmetically incorrect for this
        // event only; the next query will succeed.
        tab.terminal
            .try_read()
            .ok()
            .is_some_and(|term| term.is_mouse_tracking_enabled())
    }
}
