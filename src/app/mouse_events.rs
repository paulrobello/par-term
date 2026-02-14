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

        // Check if profile drawer is open - let egui handle all mouse events
        if self.profile_drawer_ui.expanded {
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
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            return; // Click is on tab bar, don't process as terminal event
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

                    if let Some(item) = url_detection::find_url_at_position(
                        &tab.mouse.detected_urls,
                        col,
                        adjusted_row,
                    ) {
                        match &item.item_type {
                            url_detection::DetectedItemType::Url => {
                                if let Err(e) = url_detection::open_url(
                                    &item.url,
                                    &self.config.link_handler_command,
                                ) {
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
                    let (is_alt_screen, current_col) = tab
                        .terminal
                        .try_lock()
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
                                let t = terminal_clone.lock().await;
                                let _ = t.write(move_seq.as_bytes());
                            });
                        }
                        return; // Exit early: cursor move handled
                    }
                }

                // --- 4. Mouse Tracking Forwarding ---
                // Forward events to the PTY if terminal application requested tracking
                if self.try_send_mouse_event(0, state == ElementState::Pressed) {
                    // Still track button state so mouse motion reporting works correctly.
                    // ButtonEvent mode only reports motion when button_pressed is true,
                    // so we must set this even though the click was consumed by tracking.
                    if let Some(tab) = self.tab_manager.active_tab_mut() {
                        tab.mouse.button_pressed = state == ElementState::Pressed;
                    }
                    return; // Exit early: terminal app handled the input
                }

                // Track button press state for motion tracking logic (drag selection, motion reporting)
                // This is set AFTER special handlers (URL click, Option+click, mouse tracking) to avoid
                // triggering selection when those features handle the click
                if let Some(tab) = self.tab_manager.active_tab_mut() {
                    tab.mouse.button_pressed = state == ElementState::Pressed;
                }

                if state == ElementState::Pressed {
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
                    // If tab has multiple panes, focus the clicked pane
                    if let Some(tab) = self.tab_manager.active_tab_mut()
                        && tab.has_multiple_panes()
                        && let Some(pane_id) = tab.focus_pane_at(mouse_x, mouse_y)
                    {
                        log::debug!("Focused pane {} via mouse click", pane_id);
                        // Also update tmux focused pane for correct input routing
                        self.set_tmux_focused_pane_from_native(pane_id);
                        self.needs_redraw = true;
                    }

                    // --- 6. Selection Anchoring & Click Counting ---
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
                        self.needs_redraw = true;
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
                        self.needs_redraw = true;
                        self.request_redraw();
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
                                // Sync to tmux paste buffer if connected
                                self.sync_clipboard_to_tmux(&selected_text);
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

        // Notify status bar of mouse activity (for auto-hide timer)
        self.status_bar_ui.on_mouse_activity();

        // Check if profile drawer is open - let egui handle mouse events
        if self.profile_drawer_ui.expanded {
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            return;
        }

        // Check if mouse is in the tab bar area - if so, skip terminal-specific processing
        // Position update above is still needed for proper event handling
        // Tab bar height is in logical pixels (egui); position is physical pixels (winit)
        let tab_bar_height = self
            .tab_bar_ui
            .get_height(self.tab_manager.tab_count(), &self.config);
        let scale_factor = self
            .window
            .as_ref()
            .map(|w| w.scale_factor())
            .unwrap_or(1.0);
        if position.1 < tab_bar_height as f64 * scale_factor {
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
                        let base_title = self.format_title(&self.config.window_title);
                        let tooltip_title = format!("{} - {}", base_title, url.url);
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
                        window.set_title(&self.format_title(&terminal_title));
                    } else {
                        window.set_title(&self.format_title(&self.config.window_title));
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

        // --- 4b. Divider Dragging ---
        // Handle pane divider drag resize
        let divider_dragging = self
            .tab_manager
            .active_tab()
            .and_then(|t| t.mouse.dragging_divider);

        if let Some(divider_index) = divider_dragging {
            // Actively dragging a divider
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                tab.drag_divider(divider_index, position.0 as f32, position.1 as f32);
            }
            self.needs_redraw = true;
            self.request_redraw();
            return; // Exit early: divider dragging takes precedence
        }

        // --- 4c. Divider Hover Detection ---
        // Check if mouse is hovering over a pane divider
        let is_on_divider = self
            .tab_manager
            .active_tab()
            .is_some_and(|t| t.is_on_divider(position.0 as f32, position.1 as f32));

        let was_hovering = self
            .tab_manager
            .active_tab()
            .is_some_and(|t| t.mouse.divider_hover);

        if is_on_divider != was_hovering {
            // Hover state changed
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                tab.mouse.divider_hover = is_on_divider;
                tab.mouse.hovered_divider_index = if is_on_divider {
                    tab.find_divider_at(position.0 as f32, position.1 as f32)
                } else {
                    None
                };
            }
            if let Some(window) = &self.window {
                if is_on_divider {
                    // Get divider orientation to set correct cursor
                    if let Some(tab) = self.tab_manager.active_tab()
                        && let Some(divider_idx) =
                            tab.find_divider_at(position.0 as f32, position.1 as f32)
                        && let Some(divider) = tab.get_divider(divider_idx)
                    {
                        let cursor = if divider.is_horizontal {
                            winit::window::CursorIcon::RowResize
                        } else {
                            winit::window::CursorIcon::ColResize
                        };
                        window.set_cursor(cursor);
                    }
                } else {
                    window.set_cursor(winit::window::CursorIcon::Text);
                }
            }
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
                // Option+Cmd (Alt+Super) triggers Rectangular/Block selection mode (matches iTerm2)
                // Option alone is for cursor positioning, not selection
                let mode = if self.input_handler.modifiers.state().alt_key()
                    && self.input_handler.modifiers.state().super_key()
                {
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
        // Check if profile drawer is open - let egui handle scroll events
        if self.profile_drawer_ui.expanded {
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
            // Calculate scroll amounts based on delta type (Line vs Pixel)
            let (scroll_x, scroll_y) = match delta {
                MouseScrollDelta::LineDelta(x, y) => (x as i32, y as i32),
                MouseScrollDelta::PixelDelta(pos) => ((pos.x / 20.0) as i32, (pos.y / 20.0) as i32),
            };

            // Get mouse position and terminal from active tab
            let mouse_position = self
                .tab_manager
                .active_tab()
                .map(|t| t.mouse.position)
                .unwrap_or((0.0, 0.0));

            // Map pixel position to terminal cell coordinates
            if let Some((col, row)) = self.pixel_to_cell(mouse_position.0, mouse_position.1) {
                let mut all_encoded = Vec::new();

                // --- 1a. Vertical scroll events ---
                // XTerm mouse protocol buttons: 64 = scroll up, 65 = scroll down
                if scroll_y != 0 {
                    let button = if scroll_y > 0 { 64 } else { 65 };
                    // Limit burst to 10 events to avoid flooding the PTY
                    let count = scroll_y.unsigned_abs().min(10);

                    if let Some(tab) = self.tab_manager.active_tab()
                        && let Ok(term) = tab.terminal.try_lock()
                    {
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

                    if let Some(tab) = self.tab_manager.active_tab()
                        && let Ok(term) = tab.terminal.try_lock()
                    {
                        for _ in 0..count {
                            let encoded = term.encode_mouse_event(button, col, row, true, 0);
                            if !encoded.is_empty() {
                                all_encoded.extend(encoded);
                            }
                        }
                    }
                }

                // Send all encoded events to terminal
                if !all_encoded.is_empty()
                    && let Some(tab) = self.tab_manager.active_tab()
                {
                    let terminal_clone = Arc::clone(&tab.terminal);
                    let runtime = Arc::clone(&self.runtime);
                    runtime.spawn(async move {
                        let t = terminal_clone.lock().await;
                        let _ = t.write(&all_encoded);
                    });
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
            let content_offset_x = renderer.content_offset_x() as f64;

            // Account for window padding (all sides) and content offsets (tab bar)
            let adjusted_x = (x - padding - content_offset_x).max(0.0);
            let adjusted_y = (y - padding - content_offset_y).max(0.0);

            let col = (adjusted_x / cell_width) as usize;
            let row = (adjusted_y / cell_height) as usize;

            Some((col, row))
        } else {
            None
        }
    }

    /// Handle a file being dropped into the terminal window.
    ///
    /// Quotes the file path according to the configured style and writes it
    /// to the active terminal session.
    pub(crate) fn handle_dropped_file(&mut self, path: std::path::PathBuf) {
        use crate::shell_quote::quote_path;

        // Quote the path according to the configured style
        let quoted_path = quote_path(&path, self.config.dropped_file_quote_style);

        log::debug!(
            "File dropped: {:?} -> {} (style: {:?})",
            path,
            quoted_path,
            self.config.dropped_file_quote_style
        );

        // Write the quoted path to the terminal
        if let Some(tab) = self.tab_manager.active_tab() {
            let terminal_clone = Arc::clone(&tab.terminal);
            let runtime = Arc::clone(&self.runtime);

            runtime.spawn(async move {
                let term = terminal_clone.lock().await;
                let bytes = quoted_path.as_bytes().to_vec();
                if let Err(e) = term.write(&bytes) {
                    log::error!("Failed to write dropped file path to terminal: {}", e);
                }
            });

            // Request redraw in case terminal needs to update
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }
}
