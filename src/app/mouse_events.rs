use crate::app::window_state::WindowState;
use crate::selection::{Selection, SelectionMode};
use crate::terminal::ClipboardSlot;
use crate::url_detection;
use std::sync::Arc;
use winit::event::{ElementState, MouseButton, MouseScrollDelta};

impl WindowState {
    pub(crate) fn select_word_at(&mut self, col: usize, row: usize) {
        if let Some(terminal) = &self.terminal
            && let Ok(term) = terminal.try_lock()
        {
            let (cols, _rows) = term.dimensions();
            let visible_cells =
                term.get_cells_with_scrollback(self.scroll_state.offset, None, false, None);
            if visible_cells.is_empty() || cols == 0 {
                return;
            }

            let cell_idx = row * cols + col;
            if cell_idx >= visible_cells.len() {
                return;
            }

            // Find word boundaries
            let mut start_col = col;
            let mut end_col = col;

            // Expand left
            for c in (0..col).rev() {
                let idx = row * cols + c;
                if idx >= visible_cells.len() {
                    break;
                }
                let ch = visible_cells[idx].grapheme.chars().next().unwrap_or('\0');
                if ch.is_alphanumeric() || ch == '_' {
                    start_col = c;
                } else {
                    break;
                }
            }

            // Expand right
            for c in col..cols {
                let idx = row * cols + c;
                if idx >= visible_cells.len() {
                    break;
                }
                let ch = visible_cells[idx].grapheme.chars().next().unwrap_or('\0');
                if ch.is_alphanumeric() || ch == '_' {
                    end_col = c;
                } else {
                    break;
                }
            }

            self.mouse.selection = Some(Selection::new(
                (start_col, row),
                (end_col, row),
                SelectionMode::Normal,
            ));
        }
    }

    /// Select entire line at the given row (used for triple-click)
    pub(crate) fn select_line_at(&mut self, row: usize) {
        if let Some(terminal) = &self.terminal
            && let Ok(term) = terminal.try_lock()
        {
            let (cols, _rows) = term.dimensions();
            if cols == 0 {
                return;
            }

            // Store the row in start/end - Line mode uses rows only
            self.mouse.selection = Some(Selection::new(
                (0, row),
                (cols.saturating_sub(1), row),
                SelectionMode::Line,
            ));
        }
    }

    /// Extend line selection to include rows from anchor to current row
    pub(crate) fn extend_line_selection(&mut self, current_row: usize) {
        if let Some(terminal) = &self.terminal
            && let Ok(term) = terminal.try_lock()
        {
            let (cols, _rows) = term.dimensions();
            if cols == 0 {
                return;
            }

            // Use click_position as the anchor row (the originally triple-clicked row)
            let anchor_row = self
                .mouse
                .click_position
                .map(|(_, r)| r)
                .unwrap_or(current_row);

            if let Some(ref mut selection) = self.mouse.selection
                && selection.mode == SelectionMode::Line
            {
                // For line selection, always ensure full lines are selected
                // by setting columns appropriately based on drag direction
                if current_row >= anchor_row {
                    // Dragging down or same row: start at col 0, end at last col
                    selection.start = (0, anchor_row);
                    selection.end = (cols.saturating_sub(1), current_row);
                } else {
                    // Dragging up: start at last col (anchor row), end at col 0 (current row)
                    // After normalization, this becomes: start=(0, current_row), end=(cols-1, anchor_row)
                    selection.start = (cols.saturating_sub(1), anchor_row);
                    selection.end = (0, current_row);
                }
            }
        }
    }

    /// Extract selected text from terminal
    pub(crate) fn get_selected_text(&self) -> Option<String> {
        if let (Some(selection), Some(terminal)) = (&self.mouse.selection, &self.terminal) {
            if let Ok(term) = terminal.try_lock() {
                let (start, end) = selection.normalized();
                let (start_col, start_row) = start;
                let (end_col, end_row) = end;

                let (cols, rows) = term.dimensions();
                let visible_cells =
                    term.get_cells_with_scrollback(self.scroll_state.offset, None, false, None);
                if visible_cells.is_empty() || cols == 0 {
                    return None;
                }

                let mut visible_lines = Vec::with_capacity(rows);
                for row in 0..rows {
                    let start_idx = row * cols;
                    let end_idx = start_idx.saturating_add(cols);
                    if end_idx > visible_cells.len() {
                        break;
                    }

                    let mut line = String::with_capacity(cols);
                    for cell in &visible_cells[start_idx..end_idx] {
                        line.push_str(&cell.grapheme);
                    }
                    visible_lines.push(line);
                }

                if visible_lines.is_empty() {
                    return None;
                }

                let mut selected_text = String::new();
                let max_row = visible_lines.len().saturating_sub(1);
                let start_row = start_row.min(max_row);
                let end_row = end_row.min(max_row);

                if selection.mode == SelectionMode::Line {
                    // Line selection: extract full lines
                    #[allow(clippy::needless_range_loop)]
                    for row in start_row..=end_row {
                        if row > start_row {
                            selected_text.push('\n');
                        }
                        let line = &visible_lines[row];
                        // Trim trailing spaces from each line but keep the content
                        selected_text.push_str(line.trim_end());
                    }
                } else if selection.mode == SelectionMode::Rectangular {
                    // Rectangular selection: extract same columns from each row
                    let min_col = start_col.min(end_col);
                    let max_col = start_col.max(end_col);

                    #[allow(clippy::needless_range_loop)]
                    for row in start_row..=end_row {
                        if row > start_row {
                            selected_text.push('\n');
                        }
                        let line = &visible_lines[row];
                        selected_text.push_str(&Self::extract_columns(
                            line,
                            min_col,
                            Some(max_col),
                        ));
                    }
                } else if start_row == end_row {
                    // Normal single-line selection
                    let line = &visible_lines[start_row];
                    selected_text = Self::extract_columns(line, start_col, Some(end_col));
                } else {
                    // Normal multi-line selection
                    for (idx, row) in (start_row..=end_row).enumerate() {
                        let line = &visible_lines[row];
                        if idx == 0 {
                            selected_text.push_str(&Self::extract_columns(line, start_col, None));
                        } else if row == end_row {
                            selected_text.push('\n');
                            selected_text.push_str(&Self::extract_columns(line, 0, Some(end_col)));
                        } else {
                            selected_text.push('\n');
                            selected_text.push_str(line);
                        }
                    }
                }

                Some(selected_text)
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Detect URLs in the visible terminal area (both regex-detected and OSC 8 hyperlinks)
    pub(crate) fn detect_urls(&mut self) {
        self.mouse.detected_urls.clear();

        if let Some(terminal) = &self.terminal
            && let Ok(term) = terminal.try_lock()
        {
            let (cols, rows) = term.dimensions();
            let visible_cells =
                term.get_cells_with_scrollback(self.scroll_state.offset, None, false, None);

            if visible_cells.is_empty() || cols == 0 {
                return;
            }

            // Build hyperlink ID to URL mapping from terminal
            let mut hyperlink_urls = std::collections::HashMap::new();
            let all_hyperlinks = term.get_all_hyperlinks();
            for hyperlink_info in all_hyperlinks {
                // Get the hyperlink ID from the first position
                if let Some((col, row)) = hyperlink_info.positions.first() {
                    // Get the cell at this position to find the hyperlink_id
                    let cell_idx = row * cols + col;
                    if let Some(cell) = visible_cells.get(cell_idx)
                        && let Some(id) = cell.hyperlink_id
                    {
                        hyperlink_urls.insert(id, hyperlink_info.url.clone());
                    }
                }
            }

            // Extract text from each visible line and detect URLs
            for row in 0..rows {
                let start_idx = row * cols;
                let end_idx = start_idx.saturating_add(cols);
                if end_idx > visible_cells.len() {
                    break;
                }

                let row_cells = &visible_cells[start_idx..end_idx];

                let mut line = String::with_capacity(cols);
                for cell in row_cells {
                    line.push_str(&cell.grapheme);
                }

                // Adjust row to account for scroll offset
                let absolute_row = row + self.scroll_state.offset;

                // Detect regex-based URLs in this line
                let regex_urls = url_detection::detect_urls_in_line(&line, absolute_row);
                self.mouse.detected_urls.extend(regex_urls);

                // Detect OSC 8 hyperlinks in this row
                let osc8_urls =
                    url_detection::detect_osc8_hyperlinks(row_cells, absolute_row, &hyperlink_urls);
                self.mouse.detected_urls.extend(osc8_urls);
            }
        }
    }

    /// Apply visual styling to cells that are part of detected URLs
    /// Changes the foreground color to indicate clickable URLs
    pub(crate) fn apply_url_underlines(
        &self,
        cells: &mut [crate::cell_renderer::Cell],
        renderer_size: &winit::dpi::PhysicalSize<u32>,
    ) {
        if self.mouse.detected_urls.is_empty() {
            return;
        }

        // Calculate grid dimensions from renderer size
        let char_width = self.config.font_size * 0.6;
        let cols = (renderer_size.width as f32 / char_width) as usize;

        // URL color: bright cyan (#4FC3F7) for visibility
        let url_color = [79, 195, 247, 255];

        // Apply color styling to cells that are part of URLs
        for url in &self.mouse.detected_urls {
            // Convert absolute row (with scroll offset) to viewport-relative row
            if url.row < self.scroll_state.offset {
                continue; // URL is above the visible area
            }
            let viewport_row = url.row - self.scroll_state.offset;

            // Calculate cell indices for this URL
            for col in url.start_col..url.end_col {
                let cell_idx = viewport_row * cols + col;
                if cell_idx < cells.len() {
                    cells[cell_idx].fg_color = url_color;
                    cells[cell_idx].underline = true; // Set for future underline rendering support
                }
            }
        }
    }

    /// Send mouse event to terminal if mouse tracking is enabled
    ///
    /// Returns true if event was consumed by terminal (mouse tracking enabled or alt screen active),
    /// false otherwise. When on alt screen, we don't want local text selection.
    pub(crate) fn try_send_mouse_event(&self, button: u8, pressed: bool) -> bool {
        if let Some(terminal) = &self.terminal
            && let Some((col, row)) =
                self.pixel_to_cell(self.mouse.position.0, self.mouse.position.1)
            && let Ok(term) = terminal.try_lock()
        {
            // Check if alternate screen is active - don't do local selection on alt screen
            // even if mouse tracking isn't enabled (e.g., some TUI apps don't enable mouse)
            let alt_screen_active = term.is_alt_screen_active();

            // Check if mouse tracking is enabled
            if term.is_mouse_tracking_enabled() {
                // Encode mouse event
                let encoded = term.encode_mouse_event(button, col, row, pressed, 0);

                if !encoded.is_empty() {
                    // Send to PTY using async lock to ensure write completes
                    let terminal_clone = Arc::clone(terminal);
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
        }
        false // Event not consumed, handle normally
    }

    pub(crate) fn handle_mouse_button(&mut self, button: MouseButton, state: ElementState) {
        // Track button press state for motion tracking logic (drag selection, motion reporting)
        self.mouse.button_pressed = state == ElementState::Pressed;

        // --- 1. Shader Interaction ---
        // Update shader mouse state for left button (matches Shadertoy iMouse convention)
        if button == MouseButton::Left
            && let Some(ref mut renderer) = self.renderer
        {
            renderer.set_shader_mouse_button(
                state == ElementState::Pressed,
                self.mouse.position.0 as f32,
                self.mouse.position.1 as f32,
            );
        }

        match button {
            MouseButton::Left => {
                // --- 2. URL Clicking ---
                // Check for Ctrl+Click on URL to open it in default browser
                if state == ElementState::Pressed
                    && self.input_handler.modifiers.state().control_key()
                    && let Some((col, row)) =
                        self.pixel_to_cell(self.mouse.position.0, self.mouse.position.1)
                {
                    // Adjust row for scroll offset
                    let adjusted_row = row + self.scroll_state.offset;

                    if let Some(url) = url_detection::find_url_at_position(
                        &self.mouse.detected_urls,
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
                    let mouse_x = self.mouse.position.0 as f32;
                    let mouse_y = self.mouse.position.1 as f32;

                    if let Some(renderer) = &self.renderer
                        && renderer.scrollbar_track_contains_x(mouse_x)
                    {
                        self.scroll_state.dragging = true;
                        self.scroll_state.last_activity = std::time::Instant::now();

                        let thumb_bounds = renderer.scrollbar_thumb_bounds();
                        if renderer.scrollbar_contains_point(mouse_x, mouse_y) {
                            // Clicked on thumb: track offset from thumb top for precise dragging
                            self.scroll_state.drag_offset = thumb_bounds
                                .map(|(thumb_top, thumb_height)| {
                                    (mouse_y - thumb_top).clamp(0.0, thumb_height)
                                })
                                .unwrap_or(0.0);
                        } else {
                            // Clicked on track: center thumb on mouse position
                            self.scroll_state.drag_offset = thumb_bounds
                                .map(|(_, thumb_height)| thumb_height / 2.0)
                                .unwrap_or(0.0);
                        }

                        self.drag_scrollbar_to(mouse_y);
                        return; // Exit early: scrollbar handling takes precedence over selection
                    }

                    // --- 5. Selection Anchoring & Click Counting ---
                    // Handle complex selection modes based on click sequence
                    if let Some((col, row)) =
                        self.pixel_to_cell(self.mouse.position.0, self.mouse.position.1)
                    {
                        let now = std::time::Instant::now();
                        let same_position = self.mouse.click_position == Some((col, row));

                        // Thresholds for sequential clicks (double/triple)
                        let threshold_ms = if self.mouse.click_count == 1 {
                            self.config.mouse_double_click_threshold
                        } else {
                            self.config.mouse_triple_click_threshold
                        };
                        let click_threshold = std::time::Duration::from_millis(threshold_ms);

                        // Increment click counter if within time/space constraints
                        if same_position
                            && let Some(last_time) = self.mouse.last_click_time
                            && now.duration_since(last_time) < click_threshold
                        {
                            self.mouse.click_count = (self.mouse.click_count + 1).min(3);
                        } else {
                            self.mouse.click_count = 1;
                            // Clear previous selection on new single click
                            self.mouse.selection = None;
                        }

                        self.mouse.last_click_time = Some(now);
                        self.mouse.click_position = Some((col, row));

                        // Apply immediate selection based on click count
                        if self.mouse.click_count == 2 {
                            // Double-click: Anchor word selection
                            self.select_word_at(col, row);
                            self.mouse.is_selecting = false; // Word selection is static until drag starts
                            if let Some(window) = &self.window {
                                window.request_redraw();
                            }
                        } else if self.mouse.click_count == 3 {
                            // Triple-click: Anchor full-line selection
                            self.select_line_at(row);
                            self.mouse.is_selecting = true; // Triple-click usually implies immediate drag intent
                            if let Some(window) = &self.window {
                                window.request_redraw();
                            }
                        } else {
                            // Single click: Reset state and wait for drag to start Normal/Rectangular selection
                            self.mouse.is_selecting = false;
                            self.mouse.selection = None;
                            if let Some(window) = &self.window {
                                window.request_redraw();
                            }
                        }
                    }
                } else {
                    // End scrollbar drag
                    if self.scroll_state.dragging {
                        self.scroll_state.dragging = false;
                        self.scroll_state.drag_offset = 0.0;
                        return;
                    }

                    // End selection and optionally copy to clipboard/primary selection
                    self.mouse.is_selecting = false;

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
                        if let Some(terminal) = &self.terminal
                            && let Ok(term) = terminal.try_lock()
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

                // Handle middle-click paste if configured
                if state == ElementState::Pressed && self.config.middle_click_paste {
                    // Paste from primary selection (Linux X11) or clipboard (fallback)
                    if let Some(bytes) = self.input_handler.paste_from_primary_selection()
                        && let Some(terminal) = &self.terminal
                    {
                        let terminal_clone = Arc::clone(terminal);
                        self.runtime.spawn(async move {
                            let term = terminal_clone.lock().await;
                            let _ = term.write(&bytes);
                        });
                    }
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
        self.mouse.position = position;

        // --- 1. Shader Uniform Updates ---
        // Update current mouse position for custom shaders (iMouse.xy)
        if let Some(ref mut renderer) = self.renderer {
            renderer.set_shader_mouse_position(position.0 as f32, position.1 as f32);
        }

        // --- 2. URL Hover Detection ---
        // Identify if mouse is over a clickable link and update window UI (cursor/title)
        if let Some((col, row)) = self.pixel_to_cell(position.0, position.1) {
            let adjusted_row = row + self.scroll_state.offset;
            let url_opt =
                url_detection::find_url_at_position(&self.mouse.detected_urls, col, adjusted_row);

            if let Some(url) = url_opt {
                // Hovering over a new/different URL
                if self.mouse.hovered_url.as_ref() != Some(&url.url) {
                    self.mouse.hovered_url = Some(url.url.clone());
                    if let Some(window) = &self.window {
                        // Visual feedback: hand pointer + URL tooltip in title
                        window.set_cursor(winit::window::CursorIcon::Pointer);
                        let tooltip_title = format!("{} - {}", self.config.window_title, url.url);
                        window.set_title(&tooltip_title);
                    }
                }
            } else {
                // Mouse left a URL area: restore default state
                if self.mouse.hovered_url.is_some() {
                    self.mouse.hovered_url = None;
                    if let Some(window) = &self.window {
                        window.set_cursor(winit::window::CursorIcon::Text);
                        // Restore terminal-controlled title or config default
                        if self.config.allow_title_change && !self.cache.terminal_title.is_empty() {
                            window.set_title(&self.cache.terminal_title);
                        } else {
                            window.set_title(&self.config.window_title);
                        }
                    }
                }
            }
        }

        // --- 3. Mouse Motion Reporting ---
        // Forward motion events to PTY if requested by terminal app (e.g., mouse tracking in vim)
        if let Some(terminal) = &self.terminal
            && let Some((col, row)) = self.pixel_to_cell(position.0, position.1)
            && let Ok(term) = terminal.try_lock()
            && term.should_report_mouse_motion(self.mouse.button_pressed)
        {
            // Encode button+motion (button 32 marker)
            let button = if self.mouse.button_pressed {
                32 // Motion while button pressed
            } else {
                35 // Motion without button pressed
            };

            let encoded = term.encode_mouse_event(button, col, row, true, 0);
            if !encoded.is_empty() {
                let terminal_clone = Arc::clone(terminal);
                let runtime = Arc::clone(&self.runtime);
                runtime.spawn(async move {
                    let t = terminal_clone.lock().await;
                    let _ = t.write(&encoded);
                });
            }
            return; // Exit early: terminal app is handling mouse motion
        }

        // --- 4. Scrollbar Dragging ---
        if self.scroll_state.dragging {
            self.scroll_state.last_activity = std::time::Instant::now();
            self.drag_scrollbar_to(position.1 as f32);
            return; // Exit early: scrollbar dragging takes precedence over selection
        }

        // --- 5. Drag Selection Logic ---
        // Perform local text selection if mouse tracking is NOT active
        let alt_screen_active = self
            .terminal
            .as_ref()
            .and_then(|t| t.try_lock().ok())
            .is_some_and(|term| term.is_alt_screen_active());

        if let Some((col, row)) = self.pixel_to_cell(position.0, position.1)
            && self.mouse.button_pressed
            && !alt_screen_active
        {
            if self.mouse.click_count == 1 && !self.mouse.is_selecting {
                // Initial drag move: Start selection if we've moved past the click threshold
                if let Some(click_pos) = self.mouse.click_position
                    && click_pos != (col, row)
                {
                    self.mouse.is_selecting = true;
                    // Alt key triggers Rectangular/Block selection mode
                    let mode = if self.input_handler.modifiers.state().alt_key() {
                        SelectionMode::Rectangular
                    } else {
                        SelectionMode::Normal
                    };
                    self.mouse.selection = Some(Selection::new(
                        self.mouse.click_position.unwrap(),
                        (col, row),
                        mode,
                    ));
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                }
            } else if self.mouse.is_selecting {
                // Dragging in progress: Update selection endpoints
                if let Some(ref selection) = self.mouse.selection {
                    if selection.mode == SelectionMode::Line {
                        // Triple-click mode: Selection always covers whole lines
                        self.extend_line_selection(row);
                        if let Some(window) = &self.window {
                            window.request_redraw();
                        }
                    } else {
                        // Normal/Rectangular mode: update end cell
                        if let Some(ref mut sel) = self.mouse.selection {
                            sel.end = (col, row);
                            if let Some(window) = &self.window {
                                window.request_redraw();
                            }
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn handle_mouse_wheel(&mut self, delta: MouseScrollDelta) {
        // --- 1. Mouse Tracking Protocol ---
        // Check if the terminal application (e.g., vim, htop) has requested mouse tracking.
        // If enabled, we forward wheel events to the PTY instead of scrolling locally.
        if let Some(terminal) = &self.terminal
            && let Ok(term) = terminal.try_lock()
            && term.is_mouse_tracking_enabled()
        {
            // Calculate scroll lines based on delta type (Line vs Pixel)
            let scroll_lines = match delta {
                MouseScrollDelta::LineDelta(_x, y) => y as i32,
                MouseScrollDelta::PixelDelta(pos) => (pos.y / 20.0) as i32,
            };

            // Map pixel position to terminal cell coordinates
            if let Some((col, row)) =
                self.pixel_to_cell(self.mouse.position.0, self.mouse.position.1)
            {
                // XTerm mouse protocol buttons: 64 = scroll up, 65 = scroll down
                let button = if scroll_lines > 0 { 64 } else { 65 };
                // Limit burst to 10 events to avoid flooding the PTY
                let count = scroll_lines.unsigned_abs().min(10);

                // Encode and send to terminal via async task
                let mut all_encoded = Vec::new();
                for _ in 0..count {
                    let encoded = term.encode_mouse_event(button, col, row, true, 0);
                    if !encoded.is_empty() {
                        all_encoded.extend(encoded);
                    }
                }

                if !all_encoded.is_empty() {
                    let terminal_clone = Arc::clone(terminal);
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

        let scrollback_len = self.cache.scrollback_len;

        // Calculate new scroll target (positive delta = scroll up = increase offset)
        let new_target = self.scroll_state.apply_scroll(scroll_lines, scrollback_len);

        // Update target and trigger interpolation animation
        self.set_scroll_target(new_target);
    }

    /// Set scroll target and initiate smooth interpolation animation.
    pub(crate) fn set_scroll_target(&mut self, new_offset: usize) {
        if self.scroll_state.set_target(new_offset) {
            // Request redraw to start the animation loop
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }

    fn drag_scrollbar_to(&mut self, mouse_y: f32) {
        if let Some(renderer) = &self.renderer {
            let adjusted_y = mouse_y - self.scroll_state.drag_offset;
            if let Some(new_offset) = renderer.scrollbar_mouse_y_to_scroll_offset(adjusted_y)
                && self.scroll_state.offset != new_offset
            {
                // Instant update for dragging (no animation)
                self.scroll_state.offset = new_offset;
                self.scroll_state.target_offset = new_offset;
                self.scroll_state.animated_offset = new_offset as f64;
                self.scroll_state.animation_start = None;

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

            // Account for window padding (all sides)
            let adjusted_x = (x - padding).max(0.0);
            let adjusted_y = (y - padding).max(0.0);

            let col = (adjusted_x / cell_width) as usize;
            let row = (adjusted_y / cell_height) as usize;

            Some((col, row))
        } else {
            None
        }
    }
}
