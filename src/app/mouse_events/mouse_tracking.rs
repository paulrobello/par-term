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

        let mouse_position = tab.mouse.position;

        // Get the correct terminal and cell coordinates based on whether split panes exist
        let (terminal_arc, col, row) = if let Some(ref pm) = tab.pane_manager
            && let Some(focused_pane) = pm.focused_pane()
        {
            // Split pane mode: use focused pane's terminal with pane-relative coordinates
            let Some((col, row)) =
                self.pixel_to_pane_cell(mouse_position.0, mouse_position.1, &focused_pane.bounds)
            else {
                return false;
            };
            (Arc::clone(&focused_pane.terminal), col, row)
        } else {
            // Single pane: use tab's terminal with global coordinates
            let Some((col, row)) = self.pixel_to_cell(mouse_position.0, mouse_position.1) else {
                return false;
            };
            (Arc::clone(&tab.terminal), col, row)
        };

        // try_lock: intentional — mouse button handler runs in the sync event loop.
        // On miss: the mouse event is not forwarded to mouse-tracking apps this click.
        // The user can click again; no data is permanently lost.
        let Ok(term) = terminal_arc.try_lock() else {
            return false;
        };

        // Check if alternate screen is active - don't do local selection on alt screen
        // even if mouse tracking isn't enabled (e.g., some TUI apps don't enable mouse)
        let alt_screen_active = term.is_alt_screen_active();

        // Check if mouse tracking is enabled
        if term.is_mouse_tracking_enabled() {
            // Encode mouse event
            let encoded = term.encode_mouse_event(button, col, row, pressed, 0);

            if !encoded.is_empty() {
                // Send to PTY using async lock to ensure write completes
                let terminal_clone = Arc::clone(&terminal_arc);
                let runtime = Arc::clone(&self.runtime);
                runtime.spawn(async move {
                    let t = terminal_clone.lock().await;
                    let _ = t.write(&encoded);
                });
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
            // try_lock: intentional — querying mouse tracking state from the sync event loop.
            // On miss: returns false (no mouse tracking), which may cause a missed mouse
            // event routing. The next mouse move/click will re-query correctly.
            return focused_pane
                .terminal
                .try_lock()
                .ok()
                .is_some_and(|term| term.is_mouse_tracking_enabled());
        }

        if self
            .pixel_to_cell(mouse_position.0, mouse_position.1)
            .is_none()
        {
            return false;
        }

        // try_lock: intentional — querying mouse tracking state for the single-pane path.
        // On miss: returns false (no tracking detected). Cosmetically incorrect for this
        // event only; the next query will succeed.
        tab.terminal
            .try_lock()
            .ok()
            .is_some_and(|term| term.is_mouse_tracking_enabled())
    }
}
