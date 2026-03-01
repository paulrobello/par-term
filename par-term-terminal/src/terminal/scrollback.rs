use super::{ShellLifecycleEvent, TerminalManager};
use crate::scrollback_metadata::{CommandSnapshot, LineMetadata, ScrollbackMark};
use par_term_emu_core_rust::shell_integration::ShellIntegrationMarker;
use par_term_emu_core_rust::terminal::Terminal;

impl TerminalManager {
    /// Update scrollback metadata based on shell integration events from the core.
    ///
    /// Drains the queued `ShellIntegrationEvent` events from the terminal, each of
    /// which carries the `cursor_line` at the exact moment the OSC 133 marker was
    /// parsed. This eliminates the batching problem where multiple markers arrive
    /// between frames and only the last one was visible via `marker()`.
    ///
    /// Command text is captured from the terminal grid when the marker changes
    /// away from CommandStart, then injected into scrollback marks after
    /// `apply_event()` creates them.
    pub fn update_scrollback_metadata(&mut self, scrollback_len: usize, cursor_row: usize) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();

        // Drain queued shell integration events with their recorded cursor positions.
        let shell_events = term.poll_shell_integration_events();

        // Read cumulative history state (used only for CommandFinished events).
        let history = term.get_command_history();
        let history_len = history.len();
        let last_command = history
            .last()
            .map(|c| CommandSnapshot::from_core(c, history_len.saturating_sub(1)));

        // Process each queued event at its recorded cursor position.
        if !shell_events.is_empty() {
            for (event_type, event_command, exit_code, _timestamp, cursor_line) in &shell_events {
                let marker = match event_type.as_str() {
                    "prompt_start" => Some(ShellIntegrationMarker::PromptStart),
                    "command_start" => Some(ShellIntegrationMarker::CommandStart),
                    "command_executed" => Some(ShellIntegrationMarker::CommandExecuted),
                    "command_finished" => Some(ShellIntegrationMarker::CommandFinished),
                    _ => None,
                };

                let abs_line = cursor_line.unwrap_or(scrollback_len + cursor_row);

                // Track cursor position at CommandStart (B) for command text extraction.
                let prev_marker = self.last_shell_marker;
                if marker != prev_marker {
                    let cursor_col = term.cursor().col;
                    match marker {
                        Some(ShellIntegrationMarker::CommandStart) => {
                            self.command_start_pos = Some((abs_line, cursor_col));
                        }
                        _ => {
                            if let Some((start_abs_line, start_col)) = self.command_start_pos.take()
                            {
                                let text = Self::extract_command_text(
                                    &term,
                                    start_abs_line,
                                    start_col,
                                    scrollback_len,
                                );
                                if !text.is_empty() {
                                    self.captured_command_text = Some((start_abs_line, text));
                                }
                            }
                        }
                    }
                    self.last_shell_marker = marker;
                }

                // Only pass history/exit info for CommandFinished events.
                let is_finished = matches!(marker, Some(ShellIntegrationMarker::CommandFinished));

                self.scrollback_metadata.apply_event(
                    marker,
                    abs_line,
                    if is_finished { history_len } else { 0 },
                    if is_finished {
                        last_command.clone()
                    } else {
                        None
                    },
                    if is_finished { *exit_code } else { None },
                );

                // Feed command lifecycle into the core library's command history.
                match event_type.as_str() {
                    "command_executed" => {
                        let cmd_text = event_command
                            .clone()
                            .or_else(|| self.captured_command_text.as_ref().map(|(_, t)| t.clone()))
                            .unwrap_or_default();
                        if !cmd_text.is_empty() {
                            term.start_command_execution(cmd_text.clone());
                            self.shell_lifecycle_events
                                .push(ShellLifecycleEvent::CommandStarted {
                                    command: cmd_text,
                                    absolute_line: abs_line,
                                });
                        }
                    }
                    "command_finished" => {
                        term.end_command_execution(*exit_code);
                        self.shell_lifecycle_events
                            .push(ShellLifecycleEvent::CommandFinished {
                                absolute_line: abs_line,
                            });
                    }
                    _ => {}
                }
            }
        }

        drop(term);
        drop(terminal);
        drop(pty);

        // If we captured command text, apply it to the mark at the command's
        // absolute line.
        if let Some((abs_line, cmd)) = self.captured_command_text.take() {
            self.scrollback_metadata.set_mark_command_at(abs_line, cmd);
        }
    }

    /// Extract command text from the terminal using absolute line positioning.
    pub(crate) fn extract_command_text(
        term: &Terminal,
        start_abs_line: usize,
        start_col: usize,
        current_scrollback_len: usize,
    ) -> String {
        let grid = term.active_grid();
        let mut parts = Vec::new();
        for offset in 0..5 {
            let abs_line = start_abs_line + offset;
            let (text, is_wrapped) = if abs_line < current_scrollback_len {
                let t = Self::scrollback_line_text(grid, abs_line);
                let w = grid.is_scrollback_wrapped(abs_line);
                (t, w)
            } else {
                let grid_row = abs_line - current_scrollback_len;
                let t = grid.row_text(grid_row);
                let w = grid.is_line_wrapped(grid_row);
                (t, w)
            };
            let trimmed = if offset == 0 {
                text.chars()
                    .skip(start_col)
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            } else {
                text.trim_end().to_string()
            };
            if !trimmed.is_empty() {
                parts.push(trimmed);
            }
            if !is_wrapped {
                break;
            }
        }
        parts.join("").trim().to_string()
    }

    /// Read text from a scrollback line, converting cells to a string.
    pub(crate) fn scrollback_line_text(
        grid: &par_term_emu_core_rust::grid::Grid,
        scrollback_index: usize,
    ) -> String {
        if let Some(cells) = grid.scrollback_line(scrollback_index) {
            cells
                .iter()
                .filter(|cell| !cell.flags.wide_char_spacer())
                .map(|cell| cell.get_grapheme())
                .collect::<Vec<String>>()
                .join("")
        } else {
            String::new()
        }
    }

    /// Get rendered scrollback marks (prompt/command boundaries).
    pub fn scrollback_marks(&self) -> Vec<ScrollbackMark> {
        self.scrollback_metadata.marks()
    }

    /// Find previous prompt mark before the given absolute line (if any).
    pub fn scrollback_previous_mark(&self, line: usize) -> Option<usize> {
        self.scrollback_metadata.previous_mark(line)
    }

    /// Find next prompt mark after the given absolute line (if any).
    pub fn scrollback_next_mark(&self, line: usize) -> Option<usize> {
        self.scrollback_metadata.next_mark(line)
    }

    /// Retrieve metadata for a specific absolute line index, if available.
    pub fn scrollback_metadata_for_line(&self, line: usize) -> Option<LineMetadata> {
        self.scrollback_metadata.metadata_for_line(line)
    }

    /// Get command history from the core library (commands tracked via shell integration).
    ///
    /// Returns commands as `(command_text, exit_code, duration_ms)` tuples.
    pub fn core_command_history(&self) -> Vec<(String, Option<i32>, Option<u64>)> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.get_command_history()
            .iter()
            .map(|cmd| (cmd.command.clone(), cmd.exit_code, cmd.duration_ms))
            .collect()
    }

    /// Get scrollback lines
    pub fn scrollback(&self) -> Vec<String> {
        let pty = self.pty_session.lock();
        pty.scrollback()
    }

    /// Get scrollback length
    pub fn scrollback_len(&self) -> usize {
        let pty = self.pty_session.lock();
        pty.scrollback_len()
    }

    /// Get text of a line at an absolute index (scrollback + screen).
    pub fn line_text_at_absolute(&self, absolute_line: usize) -> Option<String> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        let grid = term.active_grid();
        let scrollback_len = grid.scrollback_len();

        if absolute_line < scrollback_len {
            Some(Self::scrollback_line_text(grid, absolute_line))
        } else {
            let screen_row = absolute_line - scrollback_len;
            if screen_row < grid.rows() {
                Some(grid.row_text(screen_row))
            } else {
                None
            }
        }
    }

    /// Get all lines in a range as text (for search in copy mode).
    pub fn lines_text_range(&self, start: usize, end: usize) -> Vec<(String, usize)> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        let grid = term.active_grid();
        let scrollback_len = grid.scrollback_len();
        let max_line = scrollback_len + grid.rows();

        let start = start.min(max_line);
        let end = end.min(max_line);

        let mut result = Vec::with_capacity(end.saturating_sub(start));
        for abs_line in start..end {
            let text = if abs_line < scrollback_len {
                Self::scrollback_line_text(grid, abs_line)
            } else {
                let screen_row = abs_line - scrollback_len;
                if screen_row < grid.rows() {
                    grid.row_text(screen_row)
                } else {
                    break;
                }
            };
            result.push((text, abs_line));
        }
        result
    }

    /// Get all scrollback lines as Cell arrays.
    pub fn scrollback_as_cells(&self) -> Vec<Vec<par_term_config::Cell>> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        let grid = term.active_grid();

        let scrollback_len = grid.scrollback_len();
        let cols = grid.cols();
        let mut result = Vec::with_capacity(scrollback_len);

        for line_idx in 0..scrollback_len {
            let mut row_cells = Vec::with_capacity(cols);
            if let Some(line) = grid.scrollback_line(line_idx) {
                Self::push_line_from_slice(
                    line,
                    &mut crate::terminal::rendering::RowRenderContext {
                        cols,
                        dest: &mut row_cells,
                        screen_row: 0, // screen_row (unused for our purposes)
                        selection: None,
                        rectangular: false,
                        cursor: None,
                        theme: &self.theme,
                    },
                );
            } else {
                Self::push_empty_cells(cols, &mut row_cells);
            }
            result.push(row_cells);
        }

        result
    }

    /// Clear scrollback buffer
    pub fn clear_scrollback(&self) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.process(b"\x1b[3J");
    }

    /// Clear scrollback metadata (prompt marks, command history, timestamps).
    pub fn clear_scrollback_metadata(&mut self) {
        self.scrollback_metadata.clear();
        self.last_shell_marker = None;
        self.command_start_pos = None;
        self.captured_command_text = None;
    }

    /// Drain queued shell lifecycle events for the prettifier pipeline.
    pub fn drain_shell_lifecycle_events(&mut self) -> Vec<ShellLifecycleEvent> {
        std::mem::take(&mut self.shell_lifecycle_events)
    }

    /// Search for text in the visible screen.
    pub fn search(
        &self,
        query: &str,
        case_sensitive: bool,
    ) -> Vec<par_term_emu_core_rust::terminal::SearchMatch> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.search_text(query, case_sensitive)
    }

    /// Search for text in the scrollback buffer.
    pub fn search_scrollback(
        &self,
        query: &str,
        case_sensitive: bool,
        max_lines: Option<usize>,
    ) -> Vec<par_term_emu_core_rust::terminal::SearchMatch> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.search_scrollback(query, case_sensitive, max_lines)
    }

    /// Search for text in both visible screen and scrollback.
    pub fn search_all(&self, query: &str, case_sensitive: bool) -> Vec<crate::SearchMatch> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();

        let scrollback_len = term.active_grid().scrollback_len();
        let mut results = Vec::new();

        let scrollback_matches = term.search_scrollback(query, case_sensitive, None);
        for m in scrollback_matches {
            let abs_line = scrollback_len as isize + m.row;
            if abs_line >= 0 {
                results.push(crate::SearchMatch::new(abs_line as usize, m.col, m.length));
            }
        }

        let screen_matches = term.search_text(query, case_sensitive);
        for m in screen_matches {
            let abs_line = scrollback_len + m.row as usize;
            results.push(crate::SearchMatch::new(abs_line, m.col, m.length));
        }

        results.sort_by(|a, b| a.line.cmp(&b.line).then_with(|| a.column.cmp(&b.column)));

        results
    }
}
