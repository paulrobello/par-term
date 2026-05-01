//! Left-mouse press and release handlers.
//!
//! Extracted from `mouse_button` to keep that file under 500 lines.
//!
//! Contains:
//! - `handle_left_mouse_press`   — scrollbar, divider, pane-focus, gutter, selection anchoring
//! - `handle_left_mouse_release` — end drag (scrollbar/divider), copy selection to clipboard

use crate::app::window_state::WindowState;
use crate::terminal::ClipboardSlot;

impl WindowState {
    pub(super) fn handle_left_mouse_press(&mut self, mouse_position: (f64, f64)) {
        // --- 5. Scrollbar Interaction ---
        // Check if clicking/dragging the scrollbar track or thumb
        let mouse_x = mouse_position.0 as f32;
        let mouse_y = mouse_position.1 as f32;

        if let Some(renderer) = &self.renderer
            && renderer.scrollbar_track_contains_x(mouse_x)
        {
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                tab.active_scroll_state_mut().dragging = true;
                tab.active_scroll_state_mut().last_activity = std::time::Instant::now();

                let thumb_bounds = renderer.scrollbar_thumb_bounds();
                if renderer.scrollbar_contains_point(mouse_x, mouse_y) {
                    // Clicked on thumb: track offset from thumb top for precise dragging
                    tab.active_scroll_state_mut().drag_offset = thumb_bounds
                        .map(|(thumb_top, thumb_height)| {
                            (mouse_y - thumb_top).clamp(0.0, thumb_height)
                        })
                        .unwrap_or(0.0);
                } else {
                    // Clicked on track: center thumb on mouse position
                    tab.active_scroll_state_mut().drag_offset = thumb_bounds
                        .map(|(_, thumb_height)| thumb_height / 2.0)
                        .unwrap_or(0.0);
                }
            }

            self.drag_scrollbar_to(mouse_y);
            return; // Exit early: scrollbar handling takes precedence over selection
        }

        // --- 5b. Divider Click ---
        // Check if clicking on a pane divider to start resize
        if let Some(tab) = self.tab_manager.active_tab_mut()
            && let Some(divider_idx) = tab.find_divider_at(mouse_x, mouse_y)
        {
            // Start divider drag
            tab.active_mouse_mut().dragging_divider = Some(divider_idx);
            log::debug!("Started dragging divider {}", divider_idx);
            return; // Exit early: divider drag started
        }

        // --- 5c. Pane Focus ---
        // If tab has multiple panes, focus the clicked pane.
        // Only return early when switching to a DIFFERENT pane — clicking within
        // the already-focused pane must fall through to selection anchoring.
        // (Returning early for same-pane clicks was the bug: it prevented drag-select
        // from ever starting because the selection anchor was never stored.)
        //
        // prev_focused is captured via an immutable borrow before the mutable
        // borrow used for focus_pane_at, so the two borrows do not overlap.
        let prev_focused = self
            .tab_manager
            .active_tab()
            .filter(|t| t.has_multiple_panes())
            .and_then(|t| t.focused_pane_id());

        if let Some(tab) = self.tab_manager.active_tab_mut()
            && tab.has_multiple_panes()
            && let Some(pane_id) = tab.focus_pane_at(mouse_x, mouse_y)
            && prev_focused != Some(pane_id)
        {
            log::debug!(
                "Focused pane {} via mouse click (switched from {:?})",
                pane_id,
                prev_focused
            );
            // End any active drag on the OLD focused pane before switching focus.
            // The selection itself persists (visible but inactive), matching iTerm2 behavior.
            tab.selection_mouse_mut().is_selecting = false;
            // Clear button_pressed on the OLD pane. handle_left_mouse_button sets
            // button_pressed=true on the currently-focused pane *before* calling us,
            // so after focus_pane_at() the old pane retains a stale button_pressed=true.
            // On mouse-move after the click, handle_mouse_move reads the NEW focused
            // pane's state, so the stale flag is invisible there — but when the user
            // later clicks back to the old pane, handle_left_mouse_press returns early
            // again (pane-switch path) without setting click_pixel_position. The next
            // mouse-move then sees button_pressed=true + an old click_pixel_position
            // potentially far from the current position, triggering an accidental
            // drag-selection that highlights text.
            if let Some(old_id) = prev_focused
                && let Some(pm) = tab.pane_manager.as_mut()
                && let Some(old_pane) = pm.get_pane_mut(old_id)
            {
                old_pane.mouse.button_pressed = false;
            }
            // Also update tmux focused pane for correct input routing
            self.set_tmux_focused_pane_from_native(pane_id);
            // Reset scroll to bottom when switching pane focus so the
            // newly-focused pane doesn't inherit the previous pane's scroll offset.
            self.set_scroll_target(0);
            self.focus_state.needs_redraw = true;
            return;
            // Same pane clicked: fall through to selection anchoring below.
        }

        // --- 6. Selection Anchoring & Click Counting ---
        // Handle complex selection modes based on click sequence
        // Use pane-relative coordinates in split-pane mode so selections
        // are stored relative to the focused pane's terminal buffer.
        if let Some((col, row)) = self.pixel_to_selection_cell(mouse_position.0, mouse_position.1) {
            let now = std::time::Instant::now();

            // Read current click state from per-pane selection mouse
            let (same_position, click_count, last_click_time) = self
                .tab_manager
                .active_tab()
                .map(|t| {
                    let sm = t.selection_mouse();
                    (
                        sm.click_position == Some((col, row)),
                        sm.click_count,
                        sm.last_click_time,
                    )
                })
                .unwrap_or((false, 0, None));

            // Thresholds for sequential clicks (double/triple)
            let threshold_ms = if click_count == 1 {
                self.config.load().mouse.mouse_double_click_threshold
            } else {
                self.config.load().mouse.mouse_triple_click_threshold
            };
            let click_threshold = std::time::Duration::from_millis(threshold_ms);

            // Determine new click count
            let new_click_count = if same_position
                && last_click_time.is_some_and(|t| now.duration_since(t) < click_threshold)
            {
                (click_count + 1).min(3)
            } else {
                1
            };

            // Update selection mouse state (per-pane in split mode)
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                let sm = tab.selection_mouse_mut();
                if new_click_count == 1 {
                    // Clear previous selection on new single click
                    sm.selection = None;
                }
                sm.click_count = new_click_count;
                sm.last_click_time = Some(now);
                sm.click_position = Some((col, row));
                sm.click_pixel_position = Some(mouse_position);
            }

            // Apply immediate selection based on click count
            if new_click_count == 2 {
                // Double-click: Anchor word selection
                self.select_word_at(col, row);
                if let Some(tab) = self.tab_manager.active_tab_mut() {
                    tab.selection_mouse_mut().is_selecting = false; // Word selection is static until drag starts
                }
                self.request_redraw();
            } else if new_click_count == 3 {
                // Triple-click: Anchor full-line selection
                self.select_line_at(row);
                if let Some(tab) = self.tab_manager.active_tab_mut() {
                    tab.selection_mouse_mut().is_selecting = true; // Triple-click usually implies immediate drag intent
                }
                self.request_redraw();
            } else {
                // Single click: Reset state and wait for drag to start Normal/Rectangular selection
                if let Some(tab) = self.tab_manager.active_tab_mut() {
                    let sm = tab.selection_mouse_mut();
                    sm.is_selecting = false;
                    sm.selection = None;
                }
                self.request_redraw();
            }
        }
    }

    pub(super) fn handle_left_mouse_release(&mut self) {
        // End scrollbar drag
        let is_dragging = self
            .tab_manager
            .active_tab()
            .map(|t| t.active_scroll_state().dragging)
            .unwrap_or(false);

        if is_dragging && let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.active_scroll_state_mut().dragging = false;
            tab.active_scroll_state_mut().drag_offset = 0.0;
            return;
        }

        // End divider drag
        let divider_info = self.tab_manager.active_tab().and_then(|t| {
            let idx = t.active_mouse().dragging_divider?;
            let divider = t.get_divider(idx)?;
            Some((idx, divider.is_horizontal))
        });

        if let Some((_divider_idx, is_horizontal)) = divider_info {
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                tab.active_mouse_mut().dragging_divider = None;
                log::debug!("Ended divider drag");
            }
            // Sync pane resize to tmux if gateway is active
            // Pass whether this was a horizontal divider (affects height) or vertical (affects width)
            self.sync_pane_resize_to_tmux(is_horizontal);
            self.focus_state.needs_redraw = true;
            self.request_redraw();
            return;
        } else if self
            .tab_manager
            .active_tab()
            .and_then(|t| t.active_mouse().dragging_divider)
            .is_some()
        {
            // Fallback: divider was being dragged but we couldn't get info
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                tab.active_mouse_mut().dragging_divider = None;
                log::debug!("Ended divider drag (no info)");
            }
            self.focus_state.needs_redraw = true;
            self.request_redraw();
            return;
        }

        // End selection and optionally copy to clipboard/primary selection
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.selection_mouse_mut().is_selecting = false;
        }

        if let Some(selected_text) = self.get_selected_text_for_copy() {
            // Always copy to primary selection (Linux X11 - no-op on other platforms)
            if let Err(e) = self.input_handler.copy_to_primary_selection(&selected_text) {
                log::debug!("Failed to copy to primary selection: {}", e);
            } else {
                log::debug!("Copied {} chars to primary selection", selected_text.len());
            }

            // Copy to clipboard if auto_copy is enabled
            if self.config.load().auto_copy_selection {
                if let Err(e) = self.input_handler.copy_to_clipboard(&selected_text) {
                    log::error!("Failed to copy to clipboard: {}", e);
                } else {
                    log::debug!("Copied {} chars to clipboard", selected_text.len());
                    // Sync to tmux paste buffer if connected
                    self.sync_clipboard_to_tmux(&selected_text);
                }
            }

            // Add to clipboard history (once, regardless of which clipboard was used)
            // try_lock: intentional — called from mouse release handler in sync loop.
            // On miss: this selection is not added to clipboard history. The clipboard
            // content itself was already copied above (separate operation).
            if let Some(tab) = self.tab_manager.active_tab()
                && let Ok(term) = tab.terminal.try_write()
            {
                term.add_to_clipboard_history(
                    ClipboardSlot::Clipboard,
                    selected_text.clone(),
                    None,
                );
            }
        }
    }
}
