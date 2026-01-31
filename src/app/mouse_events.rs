use crate::app::window_state::WindowState;
use crate::selection::{Selection, SelectionMode};
use crate::terminal::ClipboardSlot;
use crate::url_detection;
use std::sync::Arc;
use winit::event::{ElementState, MouseButton, MouseScrollDelta};

impl WindowState {
    /// Send mouse event to terminal if mouse tracking is enabled
    ///
    /// Returns true if event was consumed by terminal (mouse tracking enabled or alt screen active),
    /// false otherwise. When on alt screen, we don't want local text selection.
    pub(crate) fn try_send_mouse_event(&self, button: u8, pressed: bool) -> bool {
        let tab = if let Some(t) = self.tab_manager.active_tab() {
            t
        } else {
            return false;
        };

        let mouse_position = tab.mouse.position;
        let Some((col, row)) = self.pixel_to_cell(mouse_position.0, mouse_position.1) else {
            return false;
        };

        let Ok(term) = tab.terminal.try_lock() else {
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
                let terminal_clone = Arc::clone(&tab.terminal);
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

    pub(crate) fn handle_mouse_button(&mut self, button: MouseButton, state: ElementState) {
        // Get mouse position from active tab for shader interaction
        let mouse_position = self
            .tab_manager
            .active_tab()
            .map(|t| t.mouse.position)
            .unwrap_or((0.0, 0.0));

        // Check if profile modal or drawer is open - let egui handle all mouse events
        if self.profile_modal_ui.visible || self.profile_drawer_ui.expanded {
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            return;
        }

        // Check if click is on the profile drawer toggle button
        if let Some(window) = &self.window {
            let size = window.inner_size();
            if self.profile_drawer_ui.is_point_in_toggle_button(
                mouse_position.0 as f32,
                mouse_position.1 as f32,
                size.width as f32,
                size.height as f32,
            ) {
                // Let egui handle the toggle button click
                window.request_redraw();
                return;
            }
        }

        // Check if click is in the tab bar area - if so, let egui handle it
        // IMPORTANT: Do this BEFORE setting button_pressed to avoid selection state issues
        let tab_bar_height = self
            .tab_bar_ui
            .get_height(self.tab_manager.tab_count(), &self.config);
        if mouse_position.1 < tab_bar_height as f64 {
            // Request redraw so egui can process the click event
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            return; // Click is on tab bar, don't process as terminal event
        }

        // Track button press state for motion tracking logic (drag selection, motion reporting)
        // Only set this for clicks in the terminal area, not on tab bar
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.mouse.button_pressed = state == ElementState::Pressed;
        }

        // Check if tab context menu is open - if so, let egui handle all clicks
        if self.tab_bar_ui.is_context_menu_open() {
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
                // --- 2. URL Clicking ---
                // Check for Ctrl+Click on URL to open it in default browser
                if state == ElementState::Pressed
                    && self.input_handler.modifiers.state().control_key()
                    && let Some((col, row)) = self.pixel_to_cell(mouse_position.0, mouse_position.1)
                    && let Some(tab) = self.tab_manager.active_tab()
                {
                    let adjusted_row = row + tab.scroll_state.offset;

                    if let Some(url) = url_detection::find_url_at_position(
                        &tab.mouse.detected_urls,
                        col,
                        adjusted_row,
                    ) {
                        if let Err(e) = url_detection::open_url(&url.url) {
                            log::error!("Failed to open URL: {}", e);
                        }
                        return; // Exit early: URL click handled
                    }
                }

                // --- 3. Mouse Tracking Forwarding ---
                // Forward events to the PTY if terminal application requested tracking
                if self.try_send_mouse_event(0, state == ElementState::Pressed) {
                    return; // Exit early: terminal app handled the input
                }

                if state == ElementState::Pressed {
                    // --- 4. Scrollbar Interaction ---
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

                    // --- 5. Selection Anchoring & Click Counting ---
                    // Handle complex selection modes based on click sequence
                    if let Some((col, row)) = self.pixel_to_cell(mouse_position.0, mouse_position.1)
                    {
                        let now = std::time::Instant::now();

                        // Read current click state
                        let (same_position, click_count, last_click_time) = self
                            .tab_manager
                            .active_tab()
                            .map(|t| {
                                (
                                    t.mouse.click_position == Some((col, row)),
                                    t.mouse.click_count,
                                    t.mouse.last_click_time,
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
                            && last_click_time
                                .is_some_and(|t| now.duration_since(t) < click_threshold)
                        {
                            (click_count + 1).min(3)
                        } else {
                            1
                        };

                        // Update mouse state
                        if let Some(tab) = self.tab_manager.active_tab_mut() {
                            if new_click_count == 1 {
                                // Clear previous selection on new single click
                                tab.mouse.selection = None;
                            }
                            tab.mouse.click_count = new_click_count;
                            tab.mouse.last_click_time = Some(now);
                            tab.mouse.click_position = Some((col, row));
                        }

                        // Apply immediate selection based on click count
                        if new_click_count == 2 {
                            // Double-click: Anchor word selection
                            self.select_word_at(col, row);
                            if let Some(tab) = self.tab_manager.active_tab_mut() {
                                tab.mouse.is_selecting = false; // Word selection is static until drag starts
                            }
                            if let Some(window) = &self.window {
                                window.request_redraw();
                            }
                        } else if new_click_count == 3 {
                            // Triple-click: Anchor full-line selection
                            self.select_line_at(row);
                            if let Some(tab) = self.tab_manager.active_tab_mut() {
                                tab.mouse.is_selecting = true; // Triple-click usually implies immediate drag intent
                            }
                            if let Some(window) = &self.window {
                                window.request_redraw();
                            }
                        } else {
                            // Single click: Reset state and wait for drag to start Normal/Rectangular selection
                            if let Some(tab) = self.tab_manager.active_tab_mut() {
                                tab.mouse.is_selecting = false;
                                tab.mouse.selection = None;
                            }
                            if let Some(window) = &self.window {
                                window.request_redraw();
                            }
                        }
                    }
                } else {
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

                    // End selection and optionally copy to clipboard/primary selection
                    if let Some(tab) = self.tab_manager.active_tab_mut() {
                        tab.mouse.is_selecting = false;
                    }

                    if let Some(mut selected_text) = self.get_selected_text()
                        && !selected_text.is_empty()
                    {
                        // Strip trailing newline if configured (inverted logic: copy_trailing_newline=false means strip)
                        if !self.config.copy_trailing_newline {
                            while selected_text.ends_with('\n') || selected_text.ends_with('\r') {
                                selected_text.pop();
                            }
                        }

                        // Always copy to primary selection (Linux X11 - no-op on other platforms)
                        if let Err(e) = self.input_handler.copy_to_primary_selection(&selected_text)
                        {
                            log::debug!("Failed to copy to primary selection: {}", e);
                        } else {
                            log::debug!(
                                "Copied {} chars to primary selection",
                                selected_text.len()
                            );
                        }

                        // Copy to clipboard if auto_copy is enabled
                        if self.config.auto_copy_selection {
                            if let Err(e) = self.input_handler.copy_to_clipboard(&selected_text) {
                                log::error!("Failed to copy to clipboard: {}", e);
                            } else {
                                log::debug!("Copied {} chars to clipboard", selected_text.len());
                            }
                        }

                        // Add to clipboard history (once, regardless of which clipboard was used)
                        if let Some(tab) = self.tab_manager.active_tab()
                            && let Ok(term) = tab.terminal.try_lock()
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
                    let terminal_clone = Arc::clone(&tab.terminal);
                    self.runtime.spawn(async move {
                        let term = terminal_clone.lock().await;
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

    pub(crate) fn handle_mouse_move(&mut self, position: (f64, f64)) {
        // Update mouse position in active tab (always needed for egui)
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.mouse.position = position;
        }

        // Check if profile modal or drawer is open - let egui handle mouse events
        if self.profile_modal_ui.visible || self.profile_drawer_ui.expanded {
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            return;
        }

        // Check if mouse is in the tab bar area - if so, skip terminal-specific processing
        // Position update above is still needed for proper event handling
        let tab_bar_height = self
            .tab_bar_ui
            .get_height(self.tab_manager.tab_count(), &self.config);
        if position.1 < tab_bar_height as f64 {
            // Request redraw so egui can update hover states
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            return; // Mouse is on tab bar, let egui handle it
        }

        // --- 1. Shader Uniform Updates ---
        // Update current mouse position for custom shaders (iMouse.xy)
        if let Some(ref mut renderer) = self.renderer {
            renderer.set_shader_mouse_position(position.0 as f32, position.1 as f32);
        }

        // --- 2. URL Hover Detection ---
        // Identify if mouse is over a clickable link and update window UI (cursor/title)
        if let Some((col, row)) = self.pixel_to_cell(position.0, position.1) {
            // Get scroll offset and terminal title from active tab (clone to avoid borrow conflicts)
            let (scroll_offset, terminal_title, detected_urls, hovered_url) = self
                .tab_manager
                .active_tab()
                .map(|t| {
                    (
                        t.scroll_state.offset,
                        t.cache.terminal_title.clone(),
                        t.mouse.detected_urls.clone(),
                        t.mouse.hovered_url.clone(),
                    )
                })
                .unwrap_or((0, String::new(), Vec::new(), None));

            let adjusted_row = row + scroll_offset;
            let url_opt = url_detection::find_url_at_position(&detected_urls, col, adjusted_row);

            if let Some(url) = url_opt {
                // Hovering over a new/different URL
                if hovered_url.as_ref() != Some(&url.url) {
                    if let Some(tab) = self.tab_manager.active_tab_mut() {
                        tab.mouse.hovered_url = Some(url.url.clone());
                    }
                    if let Some(window) = &self.window {
                        // Visual feedback: hand pointer + URL tooltip in title
                        window.set_cursor(winit::window::CursorIcon::Pointer);
                        let tooltip_title = format!("{} - {}", self.config.window_title, url.url);
                        window.set_title(&tooltip_title);
                    }
                }
            } else if hovered_url.is_some() {
                // Mouse left a URL area: restore default state
                if let Some(tab) = self.tab_manager.active_tab_mut() {
                    tab.mouse.hovered_url = None;
                }
                if let Some(window) = &self.window {
                    window.set_cursor(winit::window::CursorIcon::Text);
                    // Restore terminal-controlled title or config default
                    if self.config.allow_title_change && !terminal_title.is_empty() {
                        window.set_title(&terminal_title);
                    } else {
                        window.set_title(&self.config.window_title);
                    }
                }
            }
        }

        // --- 3. Mouse Motion Reporting ---
        // Forward motion events to PTY if requested by terminal app (e.g., mouse tracking in vim)
        if let Some((col, row)) = self.pixel_to_cell(position.0, position.1) {
            let should_report = self.tab_manager.active_tab().is_some_and(|tab| {
                tab.terminal
                    .try_lock()
                    .ok()
                    .is_some_and(|term| term.should_report_mouse_motion(tab.mouse.button_pressed))
            });

            if should_report
                && let Some(tab) = self.tab_manager.active_tab()
                && let Ok(term) = tab.terminal.try_lock()
            {
                // Encode button+motion (button 32 marker)
                let button = if tab.mouse.button_pressed {
                    32 // Motion while button pressed
                } else {
                    35 // Motion without button pressed
                };

                let encoded = term.encode_mouse_event(button, col, row, true, 0);
                if !encoded.is_empty() {
                    let terminal_clone = Arc::clone(&tab.terminal);
                    let runtime = Arc::clone(&self.runtime);
                    runtime.spawn(async move {
                        let t = terminal_clone.lock().await;
                        let _ = t.write(&encoded);
                    });
                }
                return; // Exit early: terminal app is handling mouse motion
            }
        }

        // --- 4. Scrollbar Dragging ---
        let is_dragging = self
            .tab_manager
            .active_tab()
            .map(|t| t.scroll_state.dragging)
            .unwrap_or(false);

        if is_dragging {
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                tab.scroll_state.last_activity = std::time::Instant::now();
            }
            self.drag_scrollbar_to(position.1 as f32);
            return; // Exit early: scrollbar dragging takes precedence over selection
        }

        // --- 5. Drag Selection Logic ---
        // Perform local text selection if mouse tracking is NOT active
        let alt_screen_active = self.tab_manager.active_tab().is_some_and(|tab| {
            tab.terminal
                .try_lock()
                .ok()
                .is_some_and(|term| term.is_alt_screen_active())
        });

        // Get mouse state for selection logic
        let (button_pressed, click_count, is_selecting, click_position, selection_mode) = self
            .tab_manager
            .active_tab()
            .map(|t| {
                (
                    t.mouse.button_pressed,
                    t.mouse.click_count,
                    t.mouse.is_selecting,
                    t.mouse.click_position,
                    t.mouse.selection.as_ref().map(|s| s.mode),
                )
            })
            .unwrap_or((false, 0, false, None, None));

        if let Some((col, row)) = self.pixel_to_cell(position.0, position.1)
            && button_pressed
            && !alt_screen_active
        {
            if click_count == 1
                && !is_selecting
                && let Some(click_pos) = click_position
                && click_pos != (col, row)
            {
                // Initial drag move: Start selection if we've moved past the click threshold
                // Alt key triggers Rectangular/Block selection mode
                let mode = if self.input_handler.modifiers.state().alt_key() {
                    SelectionMode::Rectangular
                } else {
                    SelectionMode::Normal
                };

                if let Some(tab) = self.tab_manager.active_tab_mut() {
                    tab.mouse.is_selecting = true;
                    tab.mouse.selection = Some(Selection::new(click_pos, (col, row), mode));
                }
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            } else if is_selecting && let Some(mode) = selection_mode {
                // Dragging in progress: Update selection endpoints
                if mode == SelectionMode::Line {
                    // Triple-click mode: Selection always covers whole lines
                    self.extend_line_selection(row);
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                } else {
                    // Normal/Rectangular mode: update end cell
                    if let Some(tab) = self.tab_manager.active_tab_mut()
                        && let Some(ref mut sel) = tab.mouse.selection
                    {
                        sel.end = (col, row);
                    }
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            }
        }
    }

    pub(crate) fn handle_mouse_wheel(&mut self, delta: MouseScrollDelta) {
        // Check if profile modal or drawer is open - let egui handle scroll events
        if self.profile_modal_ui.visible || self.profile_drawer_ui.expanded {
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            return;
        }

        // --- 1. Mouse Tracking Protocol ---
        // Check if the terminal application (e.g., vim, htop) has requested mouse tracking.
        // If enabled, we forward wheel events to the PTY instead of scrolling locally.
        let is_mouse_tracking = self.tab_manager.active_tab().is_some_and(|tab| {
            tab.terminal
                .try_lock()
                .ok()
                .is_some_and(|term| term.is_mouse_tracking_enabled())
        });

        if is_mouse_tracking {
            // Calculate scroll lines based on delta type (Line vs Pixel)
            let scroll_lines = match delta {
                MouseScrollDelta::LineDelta(_x, y) => y as i32,
                MouseScrollDelta::PixelDelta(pos) => (pos.y / 20.0) as i32,
            };

            // Get mouse position and terminal from active tab
            let mouse_position = self
                .tab_manager
                .active_tab()
                .map(|t| t.mouse.position)
                .unwrap_or((0.0, 0.0));

            // Map pixel position to terminal cell coordinates
            if let Some((col, row)) = self.pixel_to_cell(mouse_position.0, mouse_position.1) {
                // XTerm mouse protocol buttons: 64 = scroll up, 65 = scroll down
                let button = if scroll_lines > 0 { 64 } else { 65 };
                // Limit burst to 10 events to avoid flooding the PTY
                let count = scroll_lines.unsigned_abs().min(10);

                if let Some(tab) = self.tab_manager.active_tab()
                    && let Ok(term) = tab.terminal.try_lock()
                {
                    // Encode and send to terminal via async task
                    let mut all_encoded = Vec::new();
                    for _ in 0..count {
                        let encoded = term.encode_mouse_event(button, col, row, true, 0);
                        if !encoded.is_empty() {
                            all_encoded.extend(encoded);
                        }
                    }

                    if !all_encoded.is_empty() {
                        let terminal_clone = Arc::clone(&tab.terminal);
                        let runtime = Arc::clone(&self.runtime);
                        runtime.spawn(async move {
                            let t = terminal_clone.lock().await;
                            let _ = t.write(&all_encoded);
                        });
                    }
                }
            }
            return; // Exit early: terminal app handled the input
        }

        // --- 2. Local Scrolling ---
        // Normal behavior: scroll through the local scrollback buffer.
        let scroll_lines = match delta {
            MouseScrollDelta::LineDelta(_x, y) => (y * self.config.mouse_scroll_speed) as i32,
            MouseScrollDelta::PixelDelta(pos) => (pos.y / 20.0) as i32,
        };

        let scrollback_len = self
            .tab_manager
            .active_tab()
            .map(|t| t.cache.scrollback_len)
            .unwrap_or(0);

        // Calculate new scroll target (positive delta = scroll up = increase offset)
        let new_target = if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.scroll_state.apply_scroll(scroll_lines, scrollback_len)
        } else {
            return;
        };

        // Update target and trigger interpolation animation
        self.set_scroll_target(new_target);
    }

    /// Set scroll target and initiate smooth interpolation animation.
    pub(crate) fn set_scroll_target(&mut self, new_offset: usize) {
        let target_set = if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.scroll_state.set_target(new_offset)
        } else {
            false
        };

        if target_set {
            // Request redraw to start the animation loop
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }

    fn drag_scrollbar_to(&mut self, mouse_y: f32) {
        let drag_offset = self
            .tab_manager
            .active_tab()
            .map(|t| t.scroll_state.drag_offset)
            .unwrap_or(0.0);

        let current_offset = self
            .tab_manager
            .active_tab()
            .map(|t| t.scroll_state.offset)
            .unwrap_or(0);

        if let Some(renderer) = &self.renderer {
            let adjusted_y = mouse_y - drag_offset;
            if let Some(new_offset) = renderer.scrollbar_mouse_y_to_scroll_offset(adjusted_y)
                && current_offset != new_offset
            {
                // Instant update for dragging (no animation)
                if let Some(tab) = self.tab_manager.active_tab_mut() {
                    tab.scroll_state.offset = new_offset;
                    tab.scroll_state.target_offset = new_offset;
                    tab.scroll_state.animated_offset = new_offset as f64;
                    tab.scroll_state.animation_start = None;
                }

                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
        }
    }

    /// Convert pixel coordinates to terminal cell coordinates
    pub(crate) fn pixel_to_cell(&self, x: f64, y: f64) -> Option<(usize, usize)> {
        if let Some(renderer) = &self.renderer {
            // Use actual cell dimensions from renderer for accurate coordinate mapping
            let cell_width = renderer.cell_width() as f64;
            let cell_height = renderer.cell_height() as f64;
            let padding = renderer.window_padding() as f64;
            let content_offset_y = renderer.content_offset_y() as f64;

            // Account for window padding (all sides) and content offset (tab bar height)
            let adjusted_x = (x - padding).max(0.0);
            let adjusted_y = (y - padding - content_offset_y).max(0.0);

            let col = (adjusted_x / cell_width) as usize;
            let row = (adjusted_y / cell_height) as usize;

            Some((col, row))
        } else {
            None
        }
    }
}
