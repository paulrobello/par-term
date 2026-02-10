//! Copy mode integration for WindowState.
//!
//! Handles entering/exiting copy mode, key dispatch, selection synchronization,
//! clipboard operations, and the egui status bar overlay.

use crate::app::window_state::WindowState;
use crate::copy_mode::{SearchDirection, VisualMode};
use winit::event::KeyEvent;
use winit::keyboard::{Key, NamedKey};

impl WindowState {
    /// Check if copy mode is currently active
    pub(crate) fn is_copy_mode_active(&self) -> bool {
        self.copy_mode.active
    }

    /// Enter copy mode, anchoring the cursor at the current terminal cursor position
    pub(crate) fn enter_copy_mode(&mut self) {
        if !self.config.copy_mode_enabled {
            return;
        }

        let (cursor_col, cursor_row, cols, rows, scrollback_len) =
            if let Some(tab) = self.tab_manager.active_tab() {
                if let Ok(term) = tab.terminal.try_lock() {
                    let (col, row) = term.cursor_position();
                    let (cols, rows) = term.dimensions();
                    let sb_len = term.scrollback_len();
                    (col, row, cols, rows, sb_len)
                } else {
                    return;
                }
            } else {
                return;
            };

        self.copy_mode
            .enter(cursor_col, cursor_row, cols, rows, scrollback_len);
        self.sync_copy_mode_selection();
        self.needs_redraw = true;
        self.request_redraw();
        crate::debug_info!(
            "COPY_MODE",
            "Entered copy mode at ({}, {})",
            cursor_col,
            cursor_row
        );
    }

    /// Exit copy mode, clearing selection and restoring scroll
    pub(crate) fn exit_copy_mode(&mut self) {
        self.copy_mode.exit();
        // Clear selection
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.mouse.selection = None;
            tab.cache.cells = None; // Invalidate cache
        }
        // Scroll back to bottom
        self.set_scroll_target(0);
        self.needs_redraw = true;
        self.request_redraw();
        crate::debug_info!("COPY_MODE", "Exited copy mode");
    }

    /// Handle a key event in copy mode. Returns true if the key was consumed.
    pub(crate) fn handle_copy_mode_key(&mut self, event: &KeyEvent) {
        // Handle search input mode
        if self.copy_mode.is_searching {
            self.handle_copy_mode_search_key(event);
            return;
        }

        // Handle pending mark set (waiting for mark name after 'm')
        if self.copy_mode.pending_mark_set {
            self.copy_mode.pending_mark_set = false;
            if let Key::Character(ref ch) = event.logical_key
                && let Some(c) = ch.chars().next()
                && c.is_ascii_lowercase()
            {
                self.copy_mode.set_mark(c);
                crate::debug_info!("COPY_MODE", "Set mark '{}'", c);
                self.request_redraw();
                return;
            }
            return;
        }

        // Handle pending mark goto (waiting for mark name after "'")
        if self.copy_mode.pending_mark_goto {
            self.copy_mode.pending_mark_goto = false;
            if let Key::Character(ref ch) = event.logical_key
                && let Some(c) = ch.chars().next()
                && c.is_ascii_lowercase()
                && self.copy_mode.goto_mark(c)
            {
                crate::debug_info!("COPY_MODE", "Jumped to mark '{}'", c);
                self.after_copy_mode_motion();
                return;
            }
            return;
        }

        // Handle pending 'g' (waiting for second 'g' in 'gg')
        if self.copy_mode.pending_g {
            self.copy_mode.pending_g = false;
            if let Key::Character(ref ch) = event.logical_key
                && ch.as_str() == "g"
            {
                self.copy_mode.goto_top();
                self.after_copy_mode_motion();
                return;
            }
            // Not 'g', ignore the pending state
            return;
        }

        // Check modifiers for Ctrl key combinations
        let modifiers = &self.input_handler.modifiers;
        let ctrl = modifiers.state().control_key();

        match &event.logical_key {
            // === Directional motions ===
            Key::Character(ch) if !ctrl => match ch.as_str() {
                "h" => {
                    self.copy_mode.move_left();
                    self.after_copy_mode_motion();
                }
                "j" => {
                    self.copy_mode.move_down();
                    self.after_copy_mode_motion();
                }
                "k" => {
                    self.copy_mode.move_up();
                    self.after_copy_mode_motion();
                }
                "l" => {
                    self.copy_mode.move_right();
                    self.after_copy_mode_motion();
                }

                // === Line motions ===
                "0" => {
                    // '0' can be part of a count or line start
                    if self.copy_mode.count.is_some() {
                        self.copy_mode.push_count_digit(0);
                    } else {
                        self.copy_mode.move_to_line_start();
                        self.after_copy_mode_motion();
                    }
                }
                "$" => {
                    self.copy_mode.move_to_line_end();
                    self.after_copy_mode_motion();
                }
                "^" => {
                    if let Some(text) = self.get_copy_mode_line_text() {
                        self.copy_mode.move_to_first_non_blank(&text);
                        self.after_copy_mode_motion();
                    }
                }

                // === Word motions ===
                "w" => {
                    if let Some(text) = self.get_copy_mode_line_text() {
                        let word_chars = self.config.word_characters.clone();
                        self.copy_mode.move_word_forward(&text, &word_chars);
                        self.after_copy_mode_motion();
                    }
                }
                "b" => {
                    if let Some(text) = self.get_copy_mode_line_text() {
                        let word_chars = self.config.word_characters.clone();
                        self.copy_mode.move_word_backward(&text, &word_chars);
                        self.after_copy_mode_motion();
                    }
                }
                "e" => {
                    if let Some(text) = self.get_copy_mode_line_text() {
                        let word_chars = self.config.word_characters.clone();
                        self.copy_mode.move_word_end(&text, &word_chars);
                        self.after_copy_mode_motion();
                    }
                }
                "W" => {
                    if let Some(text) = self.get_copy_mode_line_text() {
                        self.copy_mode.move_big_word_forward(&text);
                        self.after_copy_mode_motion();
                    }
                }
                "B" => {
                    if let Some(text) = self.get_copy_mode_line_text() {
                        self.copy_mode.move_big_word_backward(&text);
                        self.after_copy_mode_motion();
                    }
                }
                "E" => {
                    if let Some(text) = self.get_copy_mode_line_text() {
                        self.copy_mode.move_big_word_end(&text);
                        self.after_copy_mode_motion();
                    }
                }

                // === Page/buffer motions ===
                "G" => {
                    if let Some(count) = self.copy_mode.count.take() {
                        // {count}G goes to absolute line
                        self.copy_mode.goto_line(count.saturating_sub(1));
                    } else {
                        self.copy_mode.goto_bottom();
                    }
                    self.after_copy_mode_motion();
                }
                "g" => {
                    self.copy_mode.pending_g = true;
                }

                // === Visual modes ===
                "v" => {
                    if self.copy_mode.pending_operator.is_some() {
                        // yv = yank to current pos (do nothing special)
                        self.copy_mode.pending_operator = None;
                    } else {
                        self.copy_mode.toggle_visual_char();
                        self.after_copy_mode_motion();
                    }
                }
                "V" => {
                    self.copy_mode.toggle_visual_line();
                    self.after_copy_mode_motion();
                }

                // === Yank ===
                "y" => {
                    if self.copy_mode.visual_mode != VisualMode::None {
                        // In visual mode, yank the selection
                        self.yank_copy_mode_selection();
                    } else {
                        // Set pending yank operator (yy = yank line, yw = yank word, etc.)
                        // For simplicity, just yank current line on 'y' in normal mode
                        self.copy_mode.pending_operator =
                            Some(crate::copy_mode::PendingOperator::Yank);
                    }
                }

                // === Search ===
                "/" => {
                    self.copy_mode.start_search(SearchDirection::Forward);
                    self.needs_redraw = true;
                    self.request_redraw();
                }
                "?" => {
                    self.copy_mode.start_search(SearchDirection::Backward);
                    self.needs_redraw = true;
                    self.request_redraw();
                }
                "n" => {
                    self.execute_copy_mode_search(false);
                }
                "N" => {
                    self.execute_copy_mode_search(true);
                }

                // === Marks ===
                "m" => {
                    self.copy_mode.pending_mark_set = true;
                }
                "'" => {
                    self.copy_mode.pending_mark_goto = true;
                }

                // === Count prefix ===
                "1" => self.copy_mode.push_count_digit(1),
                "2" => self.copy_mode.push_count_digit(2),
                "3" => self.copy_mode.push_count_digit(3),
                "4" => self.copy_mode.push_count_digit(4),
                "5" => self.copy_mode.push_count_digit(5),
                "6" => self.copy_mode.push_count_digit(6),
                "7" => self.copy_mode.push_count_digit(7),
                "8" => self.copy_mode.push_count_digit(8),
                "9" => self.copy_mode.push_count_digit(9),

                // === Exit ===
                "q" => {
                    self.exit_copy_mode();
                }

                _ => {}
            },

            // === Ctrl key combinations ===
            Key::Character(ch) if ctrl => match ch.as_str() {
                "u" => {
                    self.copy_mode.half_page_up();
                    self.after_copy_mode_motion();
                }
                "d" => {
                    self.copy_mode.half_page_down();
                    self.after_copy_mode_motion();
                }
                "b" => {
                    self.copy_mode.page_up();
                    self.after_copy_mode_motion();
                }
                "f" => {
                    self.copy_mode.page_down();
                    self.after_copy_mode_motion();
                }
                "v" => {
                    self.copy_mode.toggle_visual_block();
                    self.after_copy_mode_motion();
                }
                _ => {}
            },

            // === Named keys ===
            Key::Named(NamedKey::Escape) => {
                if self.copy_mode.visual_mode != VisualMode::None {
                    // Exit visual mode first
                    self.copy_mode.visual_mode = VisualMode::None;
                    self.copy_mode.selection_anchor = None;
                    if let Some(tab) = self.tab_manager.active_tab_mut() {
                        tab.mouse.selection = None;
                        tab.cache.cells = None;
                    }
                    self.needs_redraw = true;
                    self.request_redraw();
                } else {
                    self.exit_copy_mode();
                }
            }
            Key::Named(NamedKey::ArrowLeft) => {
                self.copy_mode.move_left();
                self.after_copy_mode_motion();
            }
            Key::Named(NamedKey::ArrowRight) => {
                self.copy_mode.move_right();
                self.after_copy_mode_motion();
            }
            Key::Named(NamedKey::ArrowUp) => {
                self.copy_mode.move_up();
                self.after_copy_mode_motion();
            }
            Key::Named(NamedKey::ArrowDown) => {
                self.copy_mode.move_down();
                self.after_copy_mode_motion();
            }
            Key::Named(NamedKey::Home) => {
                self.copy_mode.move_to_line_start();
                self.after_copy_mode_motion();
            }
            Key::Named(NamedKey::End) => {
                self.copy_mode.move_to_line_end();
                self.after_copy_mode_motion();
            }
            Key::Named(NamedKey::PageUp) => {
                self.copy_mode.page_up();
                self.after_copy_mode_motion();
            }
            Key::Named(NamedKey::PageDown) => {
                self.copy_mode.page_down();
                self.after_copy_mode_motion();
            }

            _ => {}
        }
    }

    /// Handle key events during search input mode
    fn handle_copy_mode_search_key(&mut self, event: &KeyEvent) {
        match &event.logical_key {
            Key::Named(NamedKey::Escape) => {
                self.copy_mode.cancel_search();
                self.needs_redraw = true;
                self.request_redraw();
            }
            Key::Named(NamedKey::Enter) => {
                self.copy_mode.is_searching = false;
                self.execute_copy_mode_search(false);
            }
            Key::Named(NamedKey::Backspace) => {
                self.copy_mode.search_backspace();
                self.needs_redraw = true;
                self.request_redraw();
            }
            Key::Character(ch) => {
                for c in ch.chars() {
                    self.copy_mode.search_input(c);
                }
                self.needs_redraw = true;
                self.request_redraw();
            }
            _ => {}
        }
    }

    /// Execute search in the current direction (or reversed if `reverse` is true)
    fn execute_copy_mode_search(&mut self, reverse: bool) {
        if self.copy_mode.search_query.is_empty() {
            return;
        }

        let query = self.copy_mode.search_query.clone();
        let forward = match self.copy_mode.search_direction {
            SearchDirection::Forward => !reverse,
            SearchDirection::Backward => reverse,
        };

        let current_line = self.copy_mode.cursor_absolute_line;
        let current_col = self.copy_mode.cursor_col;

        // Get all lines from terminal for searching
        let found = if let Some(tab) = self.tab_manager.active_tab() {
            if let Ok(term) = tab.terminal.try_lock() {
                let total = self.copy_mode.scrollback_len + self.copy_mode.rows;
                if forward {
                    // Search forward from current position
                    self.search_lines_forward(&term, &query, current_line, current_col, total)
                } else {
                    // Search backward from current position
                    self.search_lines_backward(&term, &query, current_line, current_col)
                }
            } else {
                None
            }
        } else {
            None
        };

        if let Some((line, col)) = found {
            self.copy_mode.cursor_absolute_line = line;
            self.copy_mode.cursor_col = col;
            self.after_copy_mode_motion();
            crate::debug_info!("COPY_MODE", "Search found '{}' at {}:{}", query, line, col);
        } else {
            self.show_toast("Pattern not found");
            self.needs_redraw = true;
            self.request_redraw();
        }
    }

    /// Search forward through lines for a query string
    fn search_lines_forward(
        &self,
        term: &crate::terminal::TerminalManager,
        query: &str,
        start_line: usize,
        start_col: usize,
        total_lines: usize,
    ) -> Option<(usize, usize)> {
        let query_lower = query.to_lowercase();

        // Search from current position to end
        for abs_line in start_line..total_lines {
            if let Some(text) = term.line_text_at_absolute(abs_line) {
                let search_start = if abs_line == start_line {
                    start_col + 1
                } else {
                    0
                };
                let text_lower = text.to_lowercase();
                if let Some(pos) = text_lower[search_start..].find(&query_lower) {
                    return Some((abs_line, search_start + pos));
                }
            }
        }
        // Wrap around from beginning
        for abs_line in 0..start_line {
            if let Some(text) = term.line_text_at_absolute(abs_line) {
                let text_lower = text.to_lowercase();
                if let Some(pos) = text_lower.find(&query_lower) {
                    return Some((abs_line, pos));
                }
            }
        }
        None
    }

    /// Search backward through lines for a query string
    fn search_lines_backward(
        &self,
        term: &crate::terminal::TerminalManager,
        query: &str,
        start_line: usize,
        start_col: usize,
    ) -> Option<(usize, usize)> {
        let query_lower = query.to_lowercase();

        // Search from current position to beginning
        for abs_line in (0..=start_line).rev() {
            if let Some(text) = term.line_text_at_absolute(abs_line) {
                let text_lower = text.to_lowercase();
                let search_end = if abs_line == start_line {
                    start_col
                } else {
                    text_lower.len()
                };
                if let Some(pos) = text_lower[..search_end].rfind(&query_lower) {
                    return Some((abs_line, pos));
                }
            }
        }
        // Wrap around from end
        let total = self.copy_mode.scrollback_len + self.copy_mode.rows;
        for abs_line in (start_line + 1..total).rev() {
            if let Some(text) = term.line_text_at_absolute(abs_line) {
                let text_lower = text.to_lowercase();
                if let Some(pos) = text_lower.rfind(&query_lower) {
                    return Some((abs_line, pos));
                }
            }
        }
        None
    }

    /// Get the text of the line at the copy mode cursor's current absolute line
    fn get_copy_mode_line_text(&self) -> Option<String> {
        let abs_line = self.copy_mode.cursor_absolute_line;
        if let Some(tab) = self.tab_manager.active_tab()
            && let Ok(term) = tab.terminal.try_lock()
        {
            return term.line_text_at_absolute(abs_line);
        }
        None
    }

    /// Post-motion housekeeping: sync selection, follow cursor, redraw
    fn after_copy_mode_motion(&mut self) {
        // Handle pending yank operator (e.g. yw, yj, etc.)
        if self.copy_mode.pending_operator.is_some() {
            // For pending yank, we need a visual selection first
            // Simple approach: just clear the pending operator
            // Full vi would create a temporary selection, but that's complex
            self.copy_mode.pending_operator = None;
        }

        self.sync_copy_mode_selection();
        self.follow_copy_mode_cursor();
        self.needs_redraw = true;
        self.request_redraw();
    }

    /// Synchronize the copy mode visual selection with the tab's mouse selection
    fn sync_copy_mode_selection(&mut self) {
        let scroll_offset = self
            .tab_manager
            .active_tab()
            .map(|t| t.scroll_state.offset)
            .unwrap_or(0);

        let selection = self.copy_mode.compute_selection(scroll_offset);

        if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.mouse.selection = selection;
            tab.cache.cells = None; // Invalidate cache to re-render selection
        }
    }

    /// Scroll the viewport to follow the copy mode cursor if it moved offscreen
    fn follow_copy_mode_cursor(&mut self) {
        let current_offset = self
            .tab_manager
            .active_tab()
            .map(|t| t.scroll_state.offset)
            .unwrap_or(0);

        if let Some(new_offset) = self.copy_mode.required_scroll_offset(current_offset) {
            self.set_scroll_target(new_offset);
            // After scrolling, re-sync selection coordinates
            self.sync_copy_mode_selection();
        }
    }

    /// Yank the current visual selection to clipboard, optionally exiting copy mode
    fn yank_copy_mode_selection(&mut self) {
        if let Some(text) = self.get_selected_text() {
            let text_len = text.len();
            let auto_exit = self.config.copy_mode_auto_exit_on_yank;
            match self.input_handler.copy_to_clipboard(&text) {
                Ok(()) => {
                    let line_count = text.lines().count();
                    let msg = if line_count > 1 {
                        format!("{} lines yanked", line_count)
                    } else {
                        format!("{} chars yanked", text_len)
                    };
                    if auto_exit {
                        self.exit_copy_mode();
                    } else {
                        // Stay in copy mode but clear visual selection
                        self.copy_mode.visual_mode = crate::copy_mode::VisualMode::None;
                        self.copy_mode.selection_anchor = None;
                        if let Some(tab) = self.tab_manager.active_tab_mut() {
                            tab.mouse.selection = None;
                            tab.cache.cells = None;
                        }
                        self.needs_redraw = true;
                        self.request_redraw();
                    }
                    self.show_toast(msg);
                }
                Err(e) => {
                    crate::debug_error!("COPY_MODE", "Failed to copy to clipboard: {}", e);
                    self.show_toast("Failed to copy to clipboard");
                }
            }
        } else if self.config.copy_mode_auto_exit_on_yank {
            self.exit_copy_mode();
        }
    }
}
