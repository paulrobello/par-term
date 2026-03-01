use crate::app::window_state::WindowState;
use std::sync::Arc;
use winit::event::MouseScrollDelta;

impl WindowState {
    pub(crate) fn handle_mouse_wheel(&mut self, delta: MouseScrollDelta) {
        // Check if profile drawer is open - let egui handle scroll events
        if self.overlay_ui.profile_drawer_ui.expanded {
            self.request_redraw();
            return;
        }

        // --- 1. Mouse Tracking Protocol ---
        // Check if the terminal application (e.g., vim, htop) has requested mouse tracking.
        // If enabled, we forward wheel events to the PTY instead of scrolling locally.
        // In split pane mode, check and route to the focused pane's terminal.
        let (terminal_for_tracking, is_mouse_tracking) =
            if let Some(tab) = self.tab_manager.active_tab() {
                if let Some(ref pm) = tab.pane_manager
                    && let Some(focused_pane) = pm.focused_pane()
                {
                    // try_lock: intentional — scroll wheel handler in sync event loop.
                    // On miss: tracking check returns false; scroll is handled locally.
                    let tracking = focused_pane
                        .terminal
                        .try_write()
                        .ok()
                        .is_some_and(|term| term.is_mouse_tracking_enabled());
                    (Some(Arc::clone(&focused_pane.terminal)), tracking)
                } else {
                    // try_lock: intentional — same rationale as focused_pane path above.
                    let tracking = tab
                        .terminal
                        .try_write()
                        .ok()
                        .is_some_and(|term| term.is_mouse_tracking_enabled());
                    (Some(Arc::clone(&tab.terminal)), tracking)
                }
            } else {
                (None, false)
            };

        if is_mouse_tracking && let Some(terminal_arc) = terminal_for_tracking {
            // Calculate scroll amounts based on delta type (Line vs Pixel)
            let (scroll_x, scroll_y) = match delta {
                MouseScrollDelta::LineDelta(x, y) => (x as i32, y as i32),
                MouseScrollDelta::PixelDelta(pos) => ((pos.x / 20.0) as i32, (pos.y / 20.0) as i32),
            };

            // Get mouse position from active tab
            let mouse_position = self
                .tab_manager
                .active_tab()
                .map(|t| t.active_mouse().position)
                .unwrap_or((0.0, 0.0));

            // Map pixel position to cell coordinates (pane-relative if split panes exist)
            // For scroll events, fall back to (0, 0) if outside pane bounds — scroll
            // should still reach the focused pane even when mouse drifts onto a divider.
            let (col, row) = if let Some(tab) = self.tab_manager.active_tab()
                && let Some(ref pm) = tab.pane_manager
                && let Some(focused_pane) = pm.focused_pane()
            {
                self.pixel_to_pane_cell(mouse_position.0, mouse_position.1, &focused_pane.bounds)
                    .unwrap_or((0, 0))
            } else {
                self.pixel_to_cell(mouse_position.0, mouse_position.1)
                    .unwrap_or((0, 0))
            };

            let mut all_encoded = Vec::new();

            // --- 1a. Vertical scroll events ---
            // XTerm mouse protocol buttons: 64 = scroll up, 65 = scroll down
            if scroll_y != 0 {
                let button = if scroll_y > 0 { 64 } else { 65 };
                // Limit burst to 10 events to avoid flooding the PTY
                let count = scroll_y.unsigned_abs().min(10);

                // try_lock: intentional — scroll wheel encoding in sync event loop.
                // On miss: the scroll events are not encoded for this wheel tick.
                // The next wheel tick will succeed. Terminal apps may notice skipped ticks.
                if let Ok(term) = terminal_arc.try_write() {
                    for _ in 0..count {
                        let encoded = term.encode_mouse_event(button, col, row, true, 0);
                        if !encoded.is_empty() {
                            all_encoded.extend(encoded);
                        }
                    }
                }
            }

            // --- 1b. Horizontal scroll events (if enabled) ---
            // XTerm mouse protocol buttons: 66 = scroll left, 67 = scroll right
            if self.config.report_horizontal_scroll && scroll_x != 0 {
                let button = if scroll_x > 0 { 67 } else { 66 };
                // Limit burst to 10 events to avoid flooding the PTY
                let count = scroll_x.unsigned_abs().min(10);

                // try_lock: intentional — horizontal scroll encoding, same as vertical above.
                if let Ok(term) = terminal_arc.try_write() {
                    for _ in 0..count {
                        let encoded = term.encode_mouse_event(button, col, row, true, 0);
                        if !encoded.is_empty() {
                            all_encoded.extend(encoded);
                        }
                    }
                }
            }

            // Send all encoded events to terminal
            if !all_encoded.is_empty() {
                let terminal_clone = Arc::clone(&terminal_arc);
                let runtime = Arc::clone(&self.runtime);
                runtime.spawn(async move {
                    let t = terminal_clone.write().await;
                    let _ = t.write(&all_encoded);
                });
            }
            return; // Exit early: terminal app handled the input
        }

        // --- 2. Local Scrolling ---
        // Normal behavior: scroll through the local scrollback buffer.
        let scroll_lines = match delta {
            MouseScrollDelta::LineDelta(_x, y) => (y * self.config.mouse_scroll_speed) as i32,
            MouseScrollDelta::PixelDelta(pos) => (pos.y / 20.0) as i32,
        };

        let scrollback_len = self.get_active_scrollback_len();

        // Calculate new scroll target (positive delta = scroll up = increase offset)
        let new_target = if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.active_scroll_state_mut()
                .apply_scroll(scroll_lines, scrollback_len)
        } else {
            return;
        };

        // Update target and trigger interpolation animation
        self.set_scroll_target(new_target);
    }

    /// Set scroll target and initiate smooth interpolation animation.
    pub(crate) fn set_scroll_target(&mut self, new_offset: usize) {
        let target_set = if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.active_scroll_state_mut().set_target(new_offset)
        } else {
            false
        };

        if target_set {
            // Request redraw to start the animation loop
            self.request_redraw();
        }
    }

    pub(crate) fn drag_scrollbar_to(&mut self, mouse_y: f32) {
        let drag_offset = self
            .tab_manager
            .active_tab()
            .map(|t| t.active_scroll_state().drag_offset)
            .unwrap_or(0.0);

        let current_offset = self
            .tab_manager
            .active_tab()
            .map(|t| t.active_scroll_state().offset)
            .unwrap_or(0);

        if let Some(renderer) = &self.renderer {
            let adjusted_y = mouse_y - drag_offset;
            if let Some(new_offset) = renderer.scrollbar_mouse_y_to_scroll_offset(adjusted_y)
                && current_offset != new_offset
            {
                // Instant update for dragging (no animation)
                if let Some(tab) = self.tab_manager.active_tab_mut() {
                    tab.active_scroll_state_mut().offset = new_offset;
                    tab.active_scroll_state_mut().target_offset = new_offset;
                    tab.active_scroll_state_mut().animated_offset = new_offset as f64;
                    tab.active_scroll_state_mut().animation_start = None;
                }

                self.request_redraw();
            }
        }
    }
}
