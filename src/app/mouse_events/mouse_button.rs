use crate::app::window_state::WindowState;
use crate::terminal::ClipboardSlot;
use crate::url_detection;
use std::sync::Arc;
use winit::event::{ElementState, MouseButton};

impl WindowState {
    pub(crate) fn handle_mouse_button(&mut self, button: MouseButton, state: ElementState) {
        // Get mouse position from active tab for shader interaction
        let mouse_position = self
            .tab_manager
            .active_tab()
            .map(|t| t.mouse.position)
            .unwrap_or((0.0, 0.0));

        let suppress_terminal_mouse_click = self
            .should_suppress_terminal_mouse_click_for_image_guard(button, state, mouse_position);

        // Check if profile drawer is open - let egui handle all mouse events
        if self.overlay_ui.profile_drawer_ui.expanded {
            self.request_redraw();
            return;
        }

        // Check if click is on the profile drawer toggle button
        let in_toggle_button = self.with_window(|window| {
            let size = window.inner_size();
            self.overlay_ui.profile_drawer_ui.is_point_in_toggle_button(
                mouse_position.0 as f32,
                mouse_position.1 as f32,
                size.width as f32,
                size.height as f32,
            )
        });
        if in_toggle_button == Some(true) {
            // Let egui handle the toggle button click
            self.request_redraw();
            return;
        }

        // Check if click is in the tab bar area - if so, let egui handle it
        // IMPORTANT: Do this BEFORE setting button_pressed to avoid selection state issues
        // Tab bar dimensions are in logical pixels (egui); mouse_position is physical pixels (winit)
        let tab_count = self.tab_manager.tab_count();
        let tab_bar_height = self.tab_bar_ui.get_height(tab_count, &self.config);
        let tab_bar_width = self.tab_bar_ui.get_width(tab_count, &self.config);
        let scale_factor = self
            .window
            .as_ref()
            .map(|w| w.scale_factor())
            .unwrap_or(1.0);
        let in_tab_bar = match self.config.tab_bar_position {
            crate::config::TabBarPosition::Top => {
                mouse_position.1 < tab_bar_height as f64 * scale_factor
            }
            crate::config::TabBarPosition::Bottom => {
                let window_height = self
                    .window
                    .as_ref()
                    .map(|w| w.inner_size().height as f64)
                    .unwrap_or(0.0);
                mouse_position.1 > window_height - tab_bar_height as f64 * scale_factor
            }
            crate::config::TabBarPosition::Left => {
                mouse_position.0 < tab_bar_width as f64 * scale_factor
            }
        };
        if in_tab_bar {
            // Request redraw so egui can process the click event
            self.request_redraw();
            return; // Click is on tab bar, don't process as terminal event
        }

        // Check if tab context menu is open - if so, let egui handle all clicks.
        // Request a redraw so egui can process click-away dismissal immediately.
        if self.tab_bar_ui.is_context_menu_open() {
            self.request_redraw();
            return;
        }

        // --- 1. Shader Interaction ---
        // Update shader mouse state for left button (matches Shadertoy iMouse convention)
        if button == MouseButton::Left
            && let Some(ref mut renderer) = self.renderer
        {
            renderer.set_shader_mouse_button(
                state == ElementState::Pressed,
                mouse_position.0 as f32,
                mouse_position.1 as f32,
            );
        }

        match button {
            MouseButton::Left => {
                self.handle_left_mouse_button(state, mouse_position, suppress_terminal_mouse_click);
            }
            MouseButton::Middle => {
                // Try to send to terminal if mouse tracking is enabled
                if self.try_send_mouse_event(1, state == ElementState::Pressed) {
                    return; // Event consumed by terminal
                }

                // Handle middle-click paste if configured (with bracketed paste support)
                if state == ElementState::Pressed
                    && self.config.middle_click_paste
                    && let Some(text) = self.input_handler.paste_from_primary_selection()
                    && let Some(tab) = self.tab_manager.active_tab()
                {
                    let text = crate::paste_transform::sanitize_paste_content(&text);
                    let terminal_clone = Arc::clone(&tab.terminal);
                    self.runtime.spawn(async move {
                        let term = terminal_clone.write().await;
                        let _ = term.paste(&text);
                    });
                }
            }
            MouseButton::Right => {
                // Try to send to terminal if mouse tracking is enabled
                let _ = self.try_send_mouse_event(2, state == ElementState::Pressed);
                // Event consumed by terminal (or ignored)
            }
            _ => {}
        }
    }

    fn handle_left_mouse_button(
        &mut self,
        state: ElementState,
        mouse_position: (f64, f64),
        suppress_terminal_mouse_click: bool,
    ) {
        // --- 2. URL Clicking ---
        // Check for modifier+Click on URL to open it in default browser
        // macOS: Cmd+Click (matches iTerm2 and system conventions)
        // Windows/Linux: Ctrl+Click (matches platform conventions)
        #[cfg(target_os = "macos")]
        let url_modifier_pressed = self.input_handler.modifiers.state().super_key();
        #[cfg(not(target_os = "macos"))]
        let url_modifier_pressed = self.input_handler.modifiers.state().control_key();

        if state == ElementState::Pressed
            && url_modifier_pressed
            && let Some((col, row)) = self.pixel_to_cell(mouse_position.0, mouse_position.1)
            && let Some(tab) = self.tab_manager.active_tab()
        {
            let adjusted_row = row + tab.scroll_state.offset;

            if let Some(item) =
                url_detection::find_url_at_position(&tab.mouse.detected_urls, col, adjusted_row)
            {
                match &item.item_type {
                    url_detection::DetectedItemType::Url => {
                        if let Err(e) =
                            url_detection::open_url(&item.url, &self.config.link_handler_command)
                        {
                            log::error!("Failed to open URL: {}", e);
                        }
                    }
                    url_detection::DetectedItemType::FilePath { line, column } => {
                        let editor_mode = self.config.semantic_history_editor_mode;
                        let editor_cmd = &self.config.semantic_history_editor;
                        let cwd = tab.get_cwd();
                        crate::debug_info!(
                            "SEMANTIC",
                            "Opening file path: {:?} line={:?} col={:?} mode={:?} editor_cmd={:?} cwd={:?}",
                            item.url,
                            line,
                            column,
                            editor_mode,
                            editor_cmd,
                            cwd
                        );
                        if let Err(e) = url_detection::open_file_in_editor(
                            &item.url,
                            *line,
                            *column,
                            editor_mode,
                            editor_cmd,
                            cwd.as_deref(),
                        ) {
                            crate::debug_error!("SEMANTIC", "Failed to open file: {}", e);
                        }
                    }
                }
                return; // Exit early: click handled
            }
        }

        // --- 3. Option+Click Cursor Positioning ---
        // NOTE: This must be checked BEFORE setting button_pressed to avoid triggering selection
        // Move cursor to clicked position when Option/Alt is pressed (without Cmd/Super)
        // This sends arrow key sequences to move the cursor within the shell line
        // macOS: Option+Click (matches iTerm2)
        // Windows/Linux: Alt+Click
        // Note: Option+Cmd is reserved for rectangular selection (matching iTerm2)
        if state == ElementState::Pressed
            && self.config.option_click_moves_cursor
            && self.input_handler.modifiers.state().alt_key()
            && !self.input_handler.modifiers.state().super_key() // Not Cmd/Super (that's for rectangular selection)
            && let Some((target_col, _target_row)) =
                self.pixel_to_cell(mouse_position.0, mouse_position.1)
            && let Some(tab) = self.tab_manager.active_tab()
        {
            // Only move cursor if we're at the bottom of scrollback (current view)
            // and not on the alternate screen (where apps handle their own cursor)
            let at_bottom = tab.scroll_state.offset == 0;
            // try_lock: intentional — double-click cursor-position query in sync loop.
            // On miss: defaults to (alt_screen=true, col=0) which skips the arrow-key
            // reposition logic. The cursor stays where it was — acceptable UX.
            let (is_alt_screen, current_col) = tab
                .terminal
                .try_write()
                .ok()
                .map(|t| (t.is_alt_screen_active(), t.cursor_position().0))
                .unwrap_or((true, 0));

            if at_bottom && !is_alt_screen {
                // Calculate horizontal movement needed
                // Send arrow keys: \x1b[C (right) or \x1b[D (left)
                let move_seq = if target_col > current_col {
                    // Move right
                    let count = target_col - current_col;
                    "\x1b[C".repeat(count)
                } else if target_col < current_col {
                    // Move left
                    let count = current_col - target_col;
                    "\x1b[D".repeat(count)
                } else {
                    // Already at target column
                    String::new()
                };

                if !move_seq.is_empty() {
                    let terminal_clone = Arc::clone(&tab.terminal);
                    let runtime = Arc::clone(&self.runtime);
                    runtime.spawn(async move {
                        let t = terminal_clone.write().await;
                        let _ = t.write(move_seq.as_bytes());
                    });
                }
                return; // Exit early: cursor move handled
            }
        }

        // --- 4. Mouse Tracking Forwarding ---
        // Forward events to the PTY if terminal application requested tracking.
        // Shift held bypasses mouse tracking to allow local text selection
        // (standard terminal convention: iTerm2, Kitty, Alacritty all honour this).
        let shift_held = self.input_handler.modifiers.state().shift_key();
        if !suppress_terminal_mouse_click
            && !shift_held
            && self.try_send_mouse_event(0, state == ElementState::Pressed)
        {
            // Still track button state so mouse motion reporting works correctly.
            // ButtonEvent mode only reports motion when button_pressed is true,
            // so we must set this even though the click was consumed by tracking.
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                tab.mouse.button_pressed = state == ElementState::Pressed;
            }
            return; // Exit early: terminal app handled the input
        }
        if suppress_terminal_mouse_click {
            crate::debug_log!(
                "MOUSE",
                "Suppressing terminal mouse click forwarding to preserve image clipboard"
            );
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                // Fully consume the protected click so it doesn't become a local
                // selection anchor and affect the next drag-selection gesture.
                tab.mouse.button_pressed = false;
                tab.selection_mouse_mut().is_selecting = false;
            }
            return;
        }

        // Track button press state for motion tracking logic (drag selection, motion reporting)
        // This is set AFTER special handlers (URL click, Option+click, mouse tracking) to avoid
        // triggering selection when those features handle the click
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.mouse.button_pressed = state == ElementState::Pressed;
        }

        if state == ElementState::Pressed {
            self.handle_left_mouse_press(mouse_position);
        } else {
            self.handle_left_mouse_release();
        }
    }

    fn handle_left_mouse_press(&mut self, mouse_position: (f64, f64)) {
        // --- 5. Scrollbar Interaction ---
        // Check if clicking/dragging the scrollbar track or thumb
        let mouse_x = mouse_position.0 as f32;
        let mouse_y = mouse_position.1 as f32;

        if let Some(renderer) = &self.renderer
            && renderer.scrollbar_track_contains_x(mouse_x)
        {
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                tab.scroll_state.dragging = true;
                tab.scroll_state.last_activity = std::time::Instant::now();

                let thumb_bounds = renderer.scrollbar_thumb_bounds();
                if renderer.scrollbar_contains_point(mouse_x, mouse_y) {
                    // Clicked on thumb: track offset from thumb top for precise dragging
                    tab.scroll_state.drag_offset = thumb_bounds
                        .map(|(thumb_top, thumb_height)| {
                            (mouse_y - thumb_top).clamp(0.0, thumb_height)
                        })
                        .unwrap_or(0.0);
                } else {
                    // Clicked on track: center thumb on mouse position
                    tab.scroll_state.drag_offset = thumb_bounds
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
            tab.mouse.dragging_divider = Some(divider_idx);
            log::debug!("Started dragging divider {}", divider_idx);
            return; // Exit early: divider drag started
        }

        // --- 5c. Pane Focus ---
        // If tab has multiple panes, focus the clicked pane.
        // Return early to prevent falling through to selection anchoring —
        // without this, slight mouse movement during the click creates an
        // accidental micro-selection that overwrites clipboard contents.
        if let Some(tab) = self.tab_manager.active_tab_mut()
            && tab.has_multiple_panes()
        {
            // End any active drag on the OLD focused pane before switching focus.
            // The selection itself persists (visible but inactive), matching iTerm2 behavior.
            tab.selection_mouse_mut().is_selecting = false;

            if let Some(pane_id) = tab.focus_pane_at(mouse_x, mouse_y) {
                log::debug!("Focused pane {} via mouse click", pane_id);
                // Also update tmux focused pane for correct input routing
                self.set_tmux_focused_pane_from_native(pane_id);
                // Reset scroll to bottom when switching pane focus so the
                // newly-focused pane doesn't inherit the previous pane's scroll offset.
                self.set_scroll_target(0);
                self.focus_state.needs_redraw = true;
                return;
            }
        }

        // --- 5d. Prettifier Gutter Click ---
        // Check if clicking in the gutter area to toggle a prettified block
        if let Some((col, row)) = self.pixel_to_cell(mouse_position.0, mouse_position.1) {
            let viewport_rows = self
                .renderer
                .as_ref()
                .map(|r| r.grid_size().1)
                .unwrap_or(24);
            let handled = if let Some(tab) = self.tab_manager.active_tab_mut() {
                if let Some(ref pipeline) = tab.prettifier {
                    let scroll_offset = tab.scroll_state.offset;
                    let indicators = tab.gutter_manager.indicators_for_viewport(
                        pipeline,
                        scroll_offset,
                        viewport_rows,
                    );
                    if let Some(block_id) = tab.gutter_manager.hit_test(col, row, &indicators) {
                        if let Some(ref mut p) = tab.prettifier {
                            p.toggle_block(block_id);
                        }
                        self.focus_state.needs_redraw = true;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            };
            if handled {
                return;
            }
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
                self.config.mouse_double_click_threshold
            } else {
                self.config.mouse_triple_click_threshold
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

    fn handle_left_mouse_release(&mut self) {
        // End scrollbar drag
        let is_dragging = self
            .tab_manager
            .active_tab()
            .map(|t| t.scroll_state.dragging)
            .unwrap_or(false);

        if is_dragging && let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.scroll_state.dragging = false;
            tab.scroll_state.drag_offset = 0.0;
            return;
        }

        // End divider drag
        let divider_info = self.tab_manager.active_tab().and_then(|t| {
            let idx = t.mouse.dragging_divider?;
            let divider = t.get_divider(idx)?;
            Some((idx, divider.is_horizontal))
        });

        if let Some((_divider_idx, is_horizontal)) = divider_info {
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                tab.mouse.dragging_divider = None;
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
            .and_then(|t| t.mouse.dragging_divider)
            .is_some()
        {
            // Fallback: divider was being dragged but we couldn't get info
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                tab.mouse.dragging_divider = None;
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
            if self.config.auto_copy_selection {
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
