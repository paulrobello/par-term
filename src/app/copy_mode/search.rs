//! Copy mode search helpers for WindowState.
//!
//! Extracted from `copy_mode_handler` to keep that file focused on key dispatch.
//! Contains:
//! - `handle_copy_mode_search_key` — search input mode key handling
//! - `execute_copy_mode_search` — search execution (forward/backward)
//! - `search_lines_forward` / `search_lines_backward` — line scanning helpers
//! - `get_copy_mode_line_text` — line text accessor
//! - `after_copy_mode_motion` — post-motion housekeeping
//! - `sync_copy_mode_selection` — selection synchronization
//! - `follow_copy_mode_cursor` — viewport scrolling to follow cursor
//! - `yank_copy_mode_selection` — clipboard yank

use crate::app::window_state::WindowState;
use crate::copy_mode::SearchDirection;
use winit::event::KeyEvent;
use winit::keyboard::{Key, NamedKey};

impl WindowState {
    /// Handle key events during search input mode
    pub(crate) fn handle_copy_mode_search_key(&mut self, event: &KeyEvent) {
        match &event.logical_key {
            Key::Named(NamedKey::Escape) => {
                self.copy_mode.cancel_search();
                self.focus_state.needs_redraw = true;
                self.request_redraw();
            }
            Key::Named(NamedKey::Enter) => {
                self.copy_mode.is_searching = false;
                self.execute_copy_mode_search(false);
            }
            Key::Named(NamedKey::Backspace) => {
                self.copy_mode.search_backspace();
                self.focus_state.needs_redraw = true;
                self.request_redraw();
            }
            Key::Character(ch) => {
                for c in ch.chars() {
                    self.copy_mode.search_input(c);
                }
                self.focus_state.needs_redraw = true;
                self.request_redraw();
            }
            _ => {}
        }
    }

    /// Execute search in the current direction (or reversed if `reverse` is true)
    pub(crate) fn execute_copy_mode_search(&mut self, reverse: bool) {
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
        // try_lock: intentional — copy mode search in sync event loop.
        // On miss: search is skipped this keypress; result stays at current position.
        let total = self.copy_mode.scrollback_len + self.copy_mode.rows;
        let found = self
            .tab_manager
            .active_tab()
            .and_then(|tab| {
                tab.try_with_terminal_mut(|term| {
                    if forward {
                        self.search_lines_forward(term, &query, current_line, current_col, total)
                    } else {
                        self.search_lines_backward(term, &query, current_line, current_col)
                    }
                })
            })
            .flatten();

        if let Some((line, col)) = found {
            self.copy_mode.cursor_absolute_line = line;
            self.copy_mode.cursor_col = col;
            self.after_copy_mode_motion();
            crate::debug_info!("COPY_MODE", "Search found '{}' at {}:{}", query, line, col);
        } else {
            self.show_toast("Pattern not found");
            self.focus_state.needs_redraw = true;
            self.request_redraw();
        }
    }

    /// Search forward through lines for a query string
    pub(crate) fn search_lines_forward(
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
    pub(crate) fn search_lines_backward(
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
    pub(crate) fn get_copy_mode_line_text(&self) -> Option<String> {
        let abs_line = self.copy_mode.cursor_absolute_line;
        // try_lock: intentional — reading line text for copy mode in sync event loop.
        // On miss: returns None (no text). The line action (yank/open) is skipped.
        self.tab_manager
            .active_tab()
            .and_then(|tab| tab.try_with_terminal_mut(|term| term.line_text_at_absolute(abs_line)))
            .flatten()
    }

    /// Post-motion housekeeping: sync selection, follow cursor, redraw
    pub(crate) fn after_copy_mode_motion(&mut self) {
        // Handle pending yank operator (e.g. yw, yj, etc.)
        if self.copy_mode.pending_operator.is_some() {
            // For pending yank, we need a visual selection first
            // Simple approach: just clear the pending operator
            // Full vi would create a temporary selection, but that's complex
            self.copy_mode.pending_operator = None;
        }

        self.sync_copy_mode_selection();
        self.follow_copy_mode_cursor();
        self.focus_state.needs_redraw = true;
        self.request_redraw();
    }

    /// Synchronize the copy mode visual selection with the tab's mouse selection
    pub(crate) fn sync_copy_mode_selection(&mut self) {
        let scroll_offset = self
            .with_active_tab(|t| t.active_scroll_state().offset)
            .unwrap_or(0);

        let selection = self.copy_mode.compute_selection(scroll_offset);

        self.with_active_tab_mut(|tab| {
            tab.selection_mouse_mut().selection = selection;
            tab.active_cache_mut().cells = None; // Invalidate cache to re-render selection
        });
    }

    /// Scroll the viewport to follow the copy mode cursor if it moved offscreen
    pub(crate) fn follow_copy_mode_cursor(&mut self) {
        let current_offset = self
            .with_active_tab(|t| t.active_scroll_state().offset)
            .unwrap_or(0);

        if let Some(new_offset) = self.copy_mode.required_scroll_offset(current_offset) {
            self.set_scroll_target(new_offset);
            // After scrolling, re-sync selection coordinates
            self.sync_copy_mode_selection();
        }
    }

    /// Yank the current visual selection to clipboard, optionally exiting copy mode
    pub(crate) fn yank_copy_mode_selection(&mut self) {
        if let Some(text) = self.get_selected_text_for_copy() {
            let text_len = text.len();
            let auto_exit = self.config.copy_mode.copy_mode_auto_exit_on_yank;
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
                        self.with_active_tab_mut(|tab| {
                            tab.selection_mouse_mut().selection = None;
                            tab.active_cache_mut().cells = None;
                        });
                        self.focus_state.needs_redraw = true;
                        self.request_redraw();
                    }
                    self.show_toast(msg);
                }
                Err(e) => {
                    crate::debug_error!("COPY_MODE", "Failed to copy to clipboard: {}", e);
                    self.show_toast("Failed to copy to clipboard");
                }
            }
        } else if self.config.copy_mode.copy_mode_auto_exit_on_yank {
            self.exit_copy_mode();
        }
    }
}
