use crate::scrollback_metadata::{CommandSnapshot, ScrollbackMark, ScrollbackMetadata};
use crate::styled_content::{StyledSegment, extract_styled_segments};
use crate::themes::Theme;
use anyhow::Result;
use par_term_emu_core_rust::pty_session::PtySession;
use par_term_emu_core_rust::shell_integration::ShellIntegrationMarker;
use par_term_emu_core_rust::terminal::Terminal;
use parking_lot::Mutex;
use std::sync::Arc;

// Re-export clipboard types for use in other modules
pub use par_term_emu_core_rust::terminal::{ClipboardEntry, ClipboardSlot};

pub mod clipboard;
pub mod graphics;
pub mod hyperlinks;
pub mod rendering;
pub mod spawn;

/// Resolve the user's login shell PATH and return environment variables for coprocess spawning.
///
/// On macOS (and other Unix), app bundles have a minimal PATH that doesn't include
/// user-installed paths like `/opt/homebrew/bin`, `/usr/local/bin`, etc.
/// This function runs the user's login shell once to resolve the full PATH,
/// caches the result, and returns it as a HashMap suitable for `CoprocessConfig.env`.
pub fn coprocess_env() -> std::collections::HashMap<String, String> {
    use std::sync::OnceLock;
    static CACHED_PATH: OnceLock<Option<String>> = OnceLock::new();

    let resolved_path = CACHED_PATH.get_or_init(|| {
        #[cfg(unix)]
        {
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
            match std::process::Command::new(&shell)
                .args(["-lc", "printf '%s' \"$PATH\""])
                .output()
            {
                Ok(output) if output.status.success() => {
                    let path = String::from_utf8_lossy(&output.stdout).to_string();
                    if !path.is_empty() {
                        log::debug!("Resolved login shell PATH: {}", path);
                        Some(path)
                    } else {
                        log::warn!("Login shell returned empty PATH");
                        None
                    }
                }
                Ok(output) => {
                    log::warn!(
                        "Login shell PATH resolution failed (exit={})",
                        output.status
                    );
                    None
                }
                Err(e) => {
                    log::warn!("Failed to run login shell for PATH resolution: {}", e);
                    None
                }
            }
        }
        #[cfg(not(unix))]
        {
            None
        }
    });

    let mut env = std::collections::HashMap::new();
    if let Some(path) = resolved_path {
        env.insert("PATH".to_string(), path.clone());
    }
    env
}

/// Terminal manager that wraps the PTY session
pub struct TerminalManager {
    /// The underlying PTY session
    pub(crate) pty_session: Arc<Mutex<PtySession>>,
    /// Terminal dimensions (cols, rows)
    pub(crate) dimensions: (usize, usize),
    /// Color theme for ANSI colors
    pub(crate) theme: Theme,
    /// Scrollback metadata for shell integration markers
    pub(crate) scrollback_metadata: ScrollbackMetadata,
    /// Previous shell integration marker for detecting transitions
    last_shell_marker: Option<ShellIntegrationMarker>,
    /// Absolute line and column at CommandStart (B marker) for extracting command text.
    /// Stored as (absolute_line, col) where absolute_line = scrollback_len + cursor_row
    /// at the time the B marker was seen. Using absolute line rather than grid row
    /// ensures we can still find the command text even after it scrolls into scrollback.
    command_start_pos: Option<(usize, usize)>,
    /// Command text captured from the terminal (waiting to be applied to a mark).
    /// Stored as (absolute_line, text) so we can target the correct mark.
    captured_command_text: Option<(usize, String)>,
}

impl TerminalManager {
    /// Create a new terminal manager with the specified dimensions
    #[allow(dead_code)]
    pub fn new(cols: usize, rows: usize) -> Result<Self> {
        Self::new_with_scrollback(cols, rows, 10000)
    }

    /// Create a new terminal manager with specified dimensions and scrollback size
    pub fn new_with_scrollback(cols: usize, rows: usize, scrollback_size: usize) -> Result<Self> {
        log::info!(
            "Creating terminal with dimensions: {}x{}, scrollback: {}",
            cols,
            rows,
            scrollback_size
        );

        let pty_session = PtySession::new(cols, rows, scrollback_size);
        let pty_session = Arc::new(Mutex::new(pty_session));

        Ok(Self {
            pty_session,
            dimensions: (cols, rows),
            theme: Theme::default(),
            scrollback_metadata: ScrollbackMetadata::new(),
            last_shell_marker: None,
            command_start_pos: None,
            captured_command_text: None,
        })
    }

    /// Set the color theme
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }

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
            for (event_type, _command, exit_code, _timestamp, cursor_line) in &shell_events {
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
                // Other markers (A, B, C) should NOT trigger the history or
                // exit-code synthesis fallbacks inside apply_event — those
                // fallbacks exist for shells that miss D markers entirely.
                // Passing them for every event causes phantom commands that
                // inflate last_recorded_history_len, preventing real D events
                // from recording.
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
            }
        }
        // When no queued events, do nothing. With the event queue in place,
        // all OSC 133 markers arrive as events. Running apply_event on every
        // frame with the stale marker and shifting cursor position would create
        // phantom commands that inflate last_recorded_history_len.

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
    ///
    /// `start_abs_line` is the absolute line (scrollback_len + grid_row) recorded
    /// at CommandStart (B marker). `start_col` is the column after the prompt.
    /// `current_scrollback_len` is the current scrollback length, used to determine
    /// whether the command line is still in the visible grid or has scrolled into
    /// the scrollback buffer.
    ///
    /// For fast commands (e.g., `ls`), by the time we detect the marker transition,
    /// the command line may have scrolled into scrollback. This method handles both
    /// cases by checking whether the absolute line falls within scrollback or the
    /// visible grid.
    fn extract_command_text(
        term: &Terminal,
        start_abs_line: usize,
        start_col: usize,
        current_scrollback_len: usize,
    ) -> String {
        let grid = term.active_grid();
        // Read the command line, continuing only if the line is soft-wrapped
        // (indicating a long command that wraps to the next line). Non-wrapped
        // lines mark the end of the command — what follows is command output.
        let mut parts = Vec::new();
        for offset in 0..5 {
            let abs_line = start_abs_line + offset;
            let (text, is_wrapped) = if abs_line < current_scrollback_len {
                // Line has scrolled into scrollback
                let t = Self::scrollback_line_text(grid, abs_line);
                let w = grid.is_scrollback_wrapped(abs_line);
                (t, w)
            } else {
                // Line is still in the visible grid
                let grid_row = abs_line - current_scrollback_len;
                let t = grid.row_text(grid_row);
                let w = grid.is_line_wrapped(grid_row);
                (t, w)
            };
            let trimmed = if offset == 0 {
                // Skip prompt characters on the first line
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
            // Only continue to the next line if this line is soft-wrapped
            if !is_wrapped {
                break;
            }
        }
        parts.join("").trim().to_string()
    }

    /// Read text from a scrollback line, converting cells to a string.
    fn scrollback_line_text(
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

    /// Set cell dimensions in pixels for sixel graphics scroll calculations
    ///
    /// This should be called when the renderer is initialized or cell size changes.
    /// Default is (1, 2) for TUI half-block rendering.
    pub fn set_cell_dimensions(&self, width: u32, height: u32) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.set_cell_dimensions(width, height);
    }

    /// Write data to the PTY (send user input to shell)
    pub fn write(&self, data: &[u8]) -> Result<()> {
        // Debug log to track what we're sending
        if !data.is_empty() {
            log::debug!(
                "Writing to PTY: {:?} (bytes: {:?})",
                String::from_utf8_lossy(data),
                data
            );
        }
        let mut pty = self.pty_session.lock();
        pty.write(data)
            .map_err(|e| anyhow::anyhow!("Failed to write to PTY: {}", e))?;
        Ok(())
    }

    /// Write string to the PTY
    #[allow(dead_code)]
    pub fn write_str(&self, data: &str) -> Result<()> {
        let mut pty = self.pty_session.lock();
        pty.write_str(data)
            .map_err(|e| anyhow::anyhow!("Failed to write to PTY: {}", e))?;
        Ok(())
    }

    /// Process raw data through the terminal emulator (for tmux output routing).
    ///
    /// This feeds data directly to the terminal parser without going through the PTY.
    /// Used when receiving %output notifications from tmux control mode.
    pub fn process_data(&self, data: &[u8]) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.process(data);
    }

    /// Paste text to the terminal with proper bracketed paste handling.
    /// Converts `\n` to `\r` and wraps with bracketed paste sequences if mode is enabled.
    pub fn paste(&self, content: &str) -> Result<()> {
        if content.is_empty() {
            return Ok(());
        }

        // Convert newlines to carriage returns for terminal
        let content = content.replace('\n', "\r");

        log::debug!("Pasting {} chars (bracketed paste check)", content.len());

        // Query bracketed paste state and copy sequences (release lock before writing)
        let (start, end) = {
            let pty = self.pty_session.lock();
            let terminal = pty.terminal();
            let term = terminal.lock();
            (
                term.bracketed_paste_start().to_vec(),
                term.bracketed_paste_end().to_vec(),
            )
        };

        // Write to PTY: [start] + content + [end]
        let mut pty = self.pty_session.lock();
        if !start.is_empty() {
            log::debug!("Sending bracketed paste start sequence");
            pty.write(&start)
                .map_err(|e| anyhow::anyhow!("Failed to write bracketed paste start: {}", e))?;
        }
        pty.write(content.as_bytes())
            .map_err(|e| anyhow::anyhow!("Failed to write paste content: {}", e))?;
        if !end.is_empty() {
            log::debug!("Sending bracketed paste end sequence");
            pty.write(&end)
                .map_err(|e| anyhow::anyhow!("Failed to write bracketed paste end: {}", e))?;
        }

        Ok(())
    }

    /// Paste text with a delay between lines.
    /// Splits content on newlines and sends each line with a configurable delay,
    /// useful for slow terminals or remote connections.
    pub async fn paste_with_delay(&self, content: &str, delay_ms: u64) -> Result<()> {
        if content.is_empty() {
            return Ok(());
        }

        // Query bracketed paste state (release lock before async work)
        let (start, end) = {
            let pty = self.pty_session.lock();
            let terminal = pty.terminal();
            let term = terminal.lock();
            (
                term.bracketed_paste_start().to_vec(),
                term.bracketed_paste_end().to_vec(),
            )
        };

        // Send bracketed paste start
        if !start.is_empty() {
            let mut pty = self.pty_session.lock();
            pty.write(&start)
                .map_err(|e| anyhow::anyhow!("Failed to write bracketed paste start: {}", e))?;
        }

        // Split on newlines and send each line with delay
        let lines: Vec<&str> = content.split('\n').collect();
        let delay = tokio::time::Duration::from_millis(delay_ms);

        for (i, line) in lines.iter().enumerate() {
            // Convert newline to carriage return for terminal
            let mut line_data = line.replace('\n', "\r");
            // Add carriage return between lines (not after the last one if original didn't end with newline)
            if i < lines.len() - 1 {
                line_data.push('\r');
            }

            {
                let mut pty = self.pty_session.lock();
                pty.write(line_data.as_bytes())
                    .map_err(|e| anyhow::anyhow!("Failed to write paste line: {}", e))?;
            }

            // Delay between lines (not after the last line)
            if i < lines.len() - 1 {
                tokio::time::sleep(delay).await;
            }
        }

        // Send bracketed paste end
        if !end.is_empty() {
            let mut pty = self.pty_session.lock();
            pty.write(&end)
                .map_err(|e| anyhow::anyhow!("Failed to write bracketed paste end: {}", e))?;
        }

        log::debug!(
            "Pasted {} lines with {}ms delay ({} chars total)",
            lines.len(),
            delay_ms,
            content.len()
        );

        Ok(())
    }

    /// Get the terminal content as a string
    #[allow(dead_code)]
    pub fn content(&self) -> Result<String> {
        let pty = self.pty_session.lock();
        Ok(pty.content())
    }

    /// Resize the terminal
    #[allow(dead_code)]
    pub fn resize(&mut self, cols: usize, rows: usize) -> Result<()> {
        log::info!("Resizing terminal to: {}x{}", cols, rows);

        let mut pty = self.pty_session.lock();
        pty.resize(cols as u16, rows as u16)
            .map_err(|e| anyhow::anyhow!("Failed to resize PTY: {}", e))?;

        self.dimensions = (cols, rows);
        Ok(())
    }

    /// Resize the terminal with pixel dimensions
    /// This sets both the character dimensions AND pixel dimensions in the PTY winsize struct,
    /// which is required for applications like kitty icat that query pixel dimensions via TIOCGWINSZ
    pub fn resize_with_pixels(
        &mut self,
        cols: usize,
        rows: usize,
        width_px: usize,
        height_px: usize,
    ) -> Result<()> {
        log::info!(
            "Resizing terminal to: {}x{} ({}x{} pixels)",
            cols,
            rows,
            width_px,
            height_px
        );

        let mut pty = self.pty_session.lock();
        pty.resize_with_pixels(cols as u16, rows as u16, width_px as u16, height_px as u16)
            .map_err(|e| anyhow::anyhow!("Failed to resize PTY with pixels: {}", e))?;

        self.dimensions = (cols, rows);
        Ok(())
    }

    /// Set pixel dimensions for XTWINOPS CSI 14 t query support
    #[allow(dead_code)]
    pub fn set_pixel_size(&mut self, width_px: usize, height_px: usize) -> Result<()> {
        let pty = self.pty_session.lock();
        let term_arc = pty.terminal();
        let mut term = term_arc.lock();
        term.set_pixel_size(width_px, height_px);
        Ok(())
    }

    /// Get the current terminal dimensions
    #[allow(dead_code)]
    pub fn dimensions(&self) -> (usize, usize) {
        self.dimensions
    }

    /// Get a clone of the underlying terminal for direct access
    #[allow(dead_code)]
    pub fn terminal(&self) -> Arc<Mutex<Terminal>> {
        let pty = self.pty_session.lock();
        pty.terminal()
    }

    /// Check if there have been updates since last check
    ///
    /// This now properly delegates to the terminal's update tracking instead of
    /// always returning true. The refresh task already tracks generation changes,
    /// so this is mainly used as a fallback for edge cases.
    #[allow(dead_code)]
    pub fn has_updates(&self) -> bool {
        // Delegate to the terminal's update generation tracking
        // The refresh task already compares generations, so this fallback
        // returns false to avoid redundant redraws
        false
    }

    /// Check if the PTY is still running
    pub fn is_running(&self) -> bool {
        let pty = self.pty_session.lock();
        pty.is_running()
    }

    /// Kill the PTY process
    pub fn kill(&mut self) -> Result<()> {
        let mut pty = self.pty_session.lock();
        pty.kill()
            .map_err(|e| anyhow::anyhow!("Failed to kill PTY: {:?}", e))
    }

    /// Get the current bell event count
    pub fn bell_count(&self) -> u64 {
        let pty = self.pty_session.lock();
        pty.bell_count()
    }

    /// Get scrollback lines
    #[allow(dead_code)]
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
    ///
    /// Line 0 = oldest scrollback line.
    /// Line `scrollback_len` = first visible screen row.
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
    ///
    /// Returns `(line_text, absolute_line_index)` pairs.
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
    ///
    /// This ensures consistent handling of wide characters when searching,
    /// by using the same cell-to-string conversion as visible content.
    pub fn scrollback_as_cells(&self) -> Vec<Vec<crate::cell_renderer::Cell>> {
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
                    cols,
                    &mut row_cells,
                    0,     // screen_row (unused for our purposes)
                    None,  // no selection
                    false, // not rectangular
                    None,  // no cursor
                    &self.theme,
                );
            } else {
                Self::push_empty_cells(cols, &mut row_cells);
            }
            result.push(row_cells);
        }

        result
    }

    /// Clear scrollback buffer
    ///
    /// Removes all scrollback history while preserving the current screen content.
    /// Uses CSI 3 J (ED 3) escape sequence which is the standard way to clear scrollback.
    pub fn clear_scrollback(&self) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        // CSI 3 J = ESC [ 3 J - Erase Scrollback (ED 3)
        term.process(b"\x1b[3J");
    }

    /// Clear scrollback metadata (prompt marks, command history, timestamps).
    ///
    /// Should be called alongside `clear_scrollback()` so that stale marks
    /// don't persist on the scrollbar or as separator lines.
    pub fn clear_scrollback_metadata(&mut self) {
        self.scrollback_metadata.clear();
        self.last_shell_marker = None;
        self.command_start_pos = None;
        self.captured_command_text = None;
    }

    /// Search for text in the visible screen.
    ///
    /// Returns matches with row indices 0+ for visible screen rows.
    pub fn search(
        &self,
        query: &str,
        case_sensitive: bool,
    ) -> Vec<par_term_emu_core_rust::terminal::SearchMatch> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.search(query, case_sensitive)
    }

    /// Search for text in the scrollback buffer.
    ///
    /// Returns matches with negative row indices (e.g., -1 is the most recent scrollback line).
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
    ///
    /// Returns all matches with normalized row indices where:
    /// - Row 0 is the oldest scrollback line
    /// - Rows increase towards the current screen
    pub fn search_all(&self, query: &str, case_sensitive: bool) -> Vec<crate::search::SearchMatch> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();

        let scrollback_len = term.active_grid().scrollback_len();
        let mut results = Vec::new();

        // Search scrollback (returns negative row indices)
        let scrollback_matches = term.search_scrollback(query, case_sensitive, None);
        for m in scrollback_matches {
            // Convert negative row index to absolute line index
            // -1 = most recent scrollback = scrollback_len - 1
            // -(scrollback_len) = oldest scrollback = 0
            let abs_line = scrollback_len as isize + m.row; // m.row is negative
            if abs_line >= 0 {
                results.push(crate::search::SearchMatch::new(
                    abs_line as usize,
                    m.col,
                    m.length,
                ));
            }
        }

        // Search visible screen (returns 0+ row indices)
        let screen_matches = term.search(query, case_sensitive);
        for m in screen_matches {
            // Screen row 0 = scrollback_len in absolute terms
            let abs_line = scrollback_len + m.row as usize;
            results.push(crate::search::SearchMatch::new(abs_line, m.col, m.length));
        }

        // Sort by line, then by column
        results.sort_by(|a, b| a.line.cmp(&b.line).then_with(|| a.column.cmp(&b.column)));

        results
    }

    /// Take all pending OSC 9/777 notifications
    pub fn take_notifications(&self) -> Vec<par_term_emu_core_rust::terminal::Notification> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.take_notifications()
    }

    /// Check if there are pending OSC 9/777 notifications
    pub fn has_notifications(&self) -> bool {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.has_notifications()
    }

    /// Take a screenshot of the terminal and save to file
    ///
    /// # Arguments
    /// * `path` - Path to save the screenshot
    /// * `format` - Screenshot format ("png", "jpeg", "svg", "html")
    /// * `scrollback_lines` - Number of scrollback lines to include (0 for none)
    #[allow(dead_code)]
    pub fn screenshot_to_file(
        &self,
        path: &std::path::Path,
        format: &str,
        scrollback_lines: usize,
    ) -> Result<()> {
        use par_term_emu_core_rust::screenshot::{ImageFormat, ScreenshotConfig};

        log::info!(
            "Taking screenshot to: {} (format: {}, scrollback: {})",
            path.display(),
            format,
            scrollback_lines
        );

        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();

        // Map format string to ImageFormat enum
        let image_format = match format.to_lowercase().as_str() {
            "png" => ImageFormat::Png,
            "jpeg" | "jpg" => ImageFormat::Jpeg,
            "svg" => ImageFormat::Svg,
            _ => {
                log::warn!("Unknown format '{}', defaulting to PNG", format);
                ImageFormat::Png
            }
        };

        // Create screenshot config
        let config = ScreenshotConfig {
            format: image_format,
            ..Default::default()
        };

        // Call the core library's screenshot method
        term.screenshot_to_file(path, config, scrollback_lines)
            .map_err(|e| anyhow::anyhow!("Failed to save screenshot: {}", e))?;

        log::info!("Screenshot saved successfully");
        Ok(())
    }

    /// Add a marker to the recording
    pub fn record_marker(&self, label: String) {
        log::debug!("Recording marker: {}", label);
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.record_marker(label);
    }

    /// Export recording to file (asciicast or JSON format)
    pub fn export_recording_to_file(
        &self,
        session: &par_term_emu_core_rust::terminal::RecordingSession,
        path: &std::path::Path,
        format: &str,
    ) -> Result<()> {
        log::info!("Exporting recording to {}: {}", format, path.display());
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();

        let content = match format.to_lowercase().as_str() {
            "json" => term.export_json(session),
            _ => term.export_asciicast(session), // default to asciicast
        };

        std::fs::write(path, content)?;
        log::info!("Recording exported successfully");
        Ok(())
    }

    /// Get current working directory from shell integration (OSC 7)
    pub fn shell_integration_cwd(&self) -> Option<String> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.shell_integration().cwd().map(String::from)
    }

    /// Get last command exit code from shell integration (OSC 133)
    pub fn shell_integration_exit_code(&self) -> Option<i32> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.shell_integration().exit_code()
    }

    /// Get current command from shell integration
    pub fn shell_integration_command(&self) -> Option<String> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.shell_integration().command().map(String::from)
    }

    /// Get hostname from shell integration (OSC 7)
    ///
    /// Returns the hostname extracted from OSC 7 `file://hostname/path` format.
    /// Returns None for localhost (implicit in `file:///path` format).
    pub fn shell_integration_hostname(&self) -> Option<String> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.shell_integration().hostname().map(String::from)
    }

    /// Get username from shell integration (OSC 7)
    ///
    /// Returns the username extracted from OSC 7 `file://user@hostname/path` format.
    /// Returns None if no username was provided.
    pub fn shell_integration_username(&self) -> Option<String> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.shell_integration().username().map(String::from)
    }

    /// Poll CWD change events from terminal
    ///
    /// Returns all pending CwdChange events and removes them from the queue.
    /// Each event contains: old_cwd, new_cwd, hostname, username, timestamp
    pub fn poll_cwd_events(&self) -> Vec<par_term_emu_core_rust::terminal::CwdChange> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.poll_cwd_events()
    }

    /// Poll trigger action results from the core terminal.
    ///
    /// Returns all pending ActionResult events and removes them from the queue.
    /// Called by the event loop to dispatch frontend-handled trigger actions
    /// (RunCommand, PlaySound, SendText).
    pub fn poll_action_results(&self) -> Vec<par_term_emu_core_rust::terminal::ActionResult> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.poll_action_results()
    }

    /// Get custom session variables set by trigger SetVariable actions.
    ///
    /// Returns a clone of the core terminal's custom variables HashMap.
    pub fn custom_session_variables(&self) -> std::collections::HashMap<String, String> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.session_variables().custom.clone()
    }

    /// Get shell integration statistics
    pub fn shell_integration_stats(
        &self,
    ) -> par_term_emu_core_rust::terminal::ShellIntegrationStats {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.get_shell_integration_stats()
    }

    /// Get cursor position
    #[allow(dead_code)]
    pub fn cursor_position(&self) -> (usize, usize) {
        let pty = self.pty_session.lock();
        pty.cursor_position()
    }

    /// Get cursor style from terminal for rendering
    pub fn cursor_style(&self) -> par_term_emu_core_rust::cursor::CursorStyle {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.cursor().style()
    }

    /// Set cursor style for the terminal
    pub fn set_cursor_style(&mut self, style: par_term_emu_core_rust::cursor::CursorStyle) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.set_cursor_style(style);
    }

    /// Check if cursor is visible (controlled by DECTCEM escape sequence)
    ///
    /// TUI applications typically hide the cursor when entering alternate screen mode.
    /// Returns false when the terminal has received CSI ?25l (hide cursor).
    pub fn is_cursor_visible(&self) -> bool {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.cursor().visible
    }

    /// Check if mouse tracking is enabled
    pub fn is_mouse_tracking_enabled(&self) -> bool {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        !matches!(
            term.mouse_mode(),
            par_term_emu_core_rust::mouse::MouseMode::Off
        )
    }

    /// Check if alternate screen is active (used by TUI applications)
    ///
    /// When the alternate screen is active, text selection should typically be disabled
    /// as the content is controlled by an application (vim, htop, etc.) rather than
    /// being scrollback history.
    pub fn is_alt_screen_active(&self) -> bool {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.is_alt_screen_active()
    }

    /// Get the modifyOtherKeys mode (XTerm extension for enhanced keyboard input)
    ///
    /// Returns:
    /// - 0: Disabled (normal key handling)
    /// - 1: Report modifiers for special keys only
    /// - 2: Report modifiers for all keys
    pub fn modify_other_keys_mode(&self) -> u8 {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.modify_other_keys_mode()
    }

    /// Check if application cursor key mode (DECCKM) is enabled.
    ///
    /// When enabled, arrow keys should send SS3 sequences (ESC O A/B/C/D)
    /// instead of CSI sequences (ESC [ A/B/C/D).
    /// Applications like `less` enable this mode for arrow key navigation.
    pub fn application_cursor(&self) -> bool {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.application_cursor()
    }

    /// Get the terminal title set by OSC 0, 1, or 2 sequences
    ///
    /// Returns the title string that applications have set via escape sequences.
    /// Returns empty string if no title has been set.
    pub fn get_title(&self) -> String {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.title().to_string()
    }

    /// Get the current shell integration marker state
    ///
    /// Returns the marker indicating what phase of command execution we're in:
    /// - PromptStart: Shell is at prompt, waiting for input
    /// - CommandStart: User has started typing a command
    /// - CommandExecuted: Command has been submitted and is running
    /// - CommandFinished: Command has completed
    pub fn shell_integration_marker(
        &self,
    ) -> Option<par_term_emu_core_rust::shell_integration::ShellIntegrationMarker> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.shell_integration().marker()
    }

    /// Check if a command is currently running based on shell integration
    ///
    /// Returns true if the shell integration indicates a command is executing
    /// (marker is CommandExecuted and no CommandFinished has been received).
    ///
    /// Note: This only works when shell integration is enabled (e.g., via iTerm2
    /// shell integration scripts). Without shell integration, this always returns false.
    pub fn is_command_running(&self) -> bool {
        use par_term_emu_core_rust::shell_integration::ShellIntegrationMarker;

        matches!(
            self.shell_integration_marker(),
            Some(ShellIntegrationMarker::CommandExecuted)
        )
    }

    /// Get the name of the currently running command (first word only)
    ///
    /// Returns the command name extracted from the shell integration command,
    /// or None if no command is running or shell integration is not available.
    pub fn get_running_command_name(&self) -> Option<String> {
        if !self.is_command_running() {
            return None;
        }

        self.shell_integration_command().and_then(|cmd| {
            // Extract just the command name (first word)
            // Handle cases like "sudo vim file.txt" -> "sudo"
            // or "./script.sh" -> "script.sh"
            // or "/usr/bin/python" -> "python"
            let first_word = cmd.split_whitespace().next()?;

            // Extract basename from path
            let name = std::path::Path::new(first_word)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(first_word);

            Some(name.to_string())
        })
    }

    /// Check if tab close should show a confirmation dialog
    ///
    /// Returns Some(command_name) if a confirmation should be shown,
    /// or None if the tab can be closed immediately.
    ///
    /// A confirmation is shown when:
    /// - A command is currently running (via shell integration)
    /// - The command name is NOT in the jobs_to_ignore list
    pub fn should_confirm_close(&self, jobs_to_ignore: &[String]) -> Option<String> {
        let command_name = self.get_running_command_name()?;

        // Check if this command is in the ignore list (case-insensitive)
        let command_lower = command_name.to_lowercase();
        for ignore in jobs_to_ignore {
            if ignore.to_lowercase() == command_lower {
                return None;
            }
        }

        Some(command_name)
    }

    /// Check if mouse motion events should be reported
    /// Returns true if mode is ButtonEvent or AnyEvent
    pub fn should_report_mouse_motion(&self, button_pressed: bool) -> bool {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();

        match term.mouse_mode() {
            par_term_emu_core_rust::mouse::MouseMode::AnyEvent => true,
            par_term_emu_core_rust::mouse::MouseMode::ButtonEvent => button_pressed,
            _ => false,
        }
    }

    /// Send a mouse event to the terminal and get the encoded bytes
    ///
    /// # Arguments
    /// * `button` - Mouse button (0 = left, 1 = middle, 2 = right)
    /// * `col` - Column position (0-indexed)
    /// * `row` - Row position (0-indexed)
    /// * `pressed` - true for press, false for release
    /// * `modifiers` - Modifier keys bit mask
    ///
    /// # Returns
    /// Encoded mouse event bytes to send to PTY, or empty vec if tracking is disabled
    pub fn encode_mouse_event(
        &self,
        button: u8,
        col: usize,
        row: usize,
        pressed: bool,
        modifiers: u8,
    ) -> Vec<u8> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();

        let mouse_event =
            par_term_emu_core_rust::mouse::MouseEvent::new(button, col, row, pressed, modifiers);
        term.report_mouse(mouse_event)
    }

    /// Get styled segments from the terminal for rendering
    #[allow(dead_code)]
    pub fn get_styled_segments(&self) -> Vec<StyledSegment> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        let grid = term.active_grid();
        extract_styled_segments(grid)
    }

    /// Get the current generation number for dirty tracking
    ///
    /// The generation number increments whenever the terminal content changes.
    /// This can be used to detect when a cached representation needs to be updated.
    pub fn update_generation(&self) -> u64 {
        let pty = self.pty_session.lock();
        pty.update_generation()
    }
}

// ========================================================================
// Clipboard History Methods
// ========================================================================

impl TerminalManager {}

// ========================================================================
// Progress Bar Methods (OSC 9;4 and OSC 934)
// ========================================================================

impl TerminalManager {
    /// Get the simple progress bar state (OSC 9;4)
    pub fn progress_bar(&self) -> par_term_emu_core_rust::terminal::ProgressBar {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        *term.progress_bar()
    }

    /// Get all named progress bars (OSC 934)
    pub fn named_progress_bars(
        &self,
    ) -> std::collections::HashMap<String, par_term_emu_core_rust::terminal::NamedProgressBar> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.named_progress_bars().clone()
    }

    /// Check if any progress bar is currently active (either simple or named)
    pub fn has_any_progress(&self) -> bool {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.has_progress() || !term.named_progress_bars().is_empty()
    }
}

// ========================================================================
// Answerback String (ENQ Response)
// ========================================================================

impl TerminalManager {
    /// Set the answerback string sent in response to ENQ (0x05) control character
    ///
    /// The answerback string is sent back to the PTY when the terminal receives
    /// an ENQ (enquiry, ASCII 0x05) character. This was historically used for
    /// terminal identification in multi-terminal environments.
    ///
    /// # Security Note
    /// Default is empty (disabled) for security. Setting this may expose
    /// terminal identification information to applications.
    ///
    /// # Arguments
    /// * `answerback` - The string to send, or None/empty to disable
    pub fn set_answerback_string(&self, answerback: Option<String>) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.set_answerback_string(answerback);
    }

    /// Set the Unicode width configuration for character width calculations
    ///
    /// This affects how the terminal calculates character widths for cursor
    /// positioning and text layout, particularly for:
    /// - Emoji (different Unicode versions have different width assignments)
    /// - East Asian Ambiguous characters (can be narrow or wide)
    ///
    /// # Arguments
    /// * `config` - The width configuration to use
    pub fn set_width_config(&self, config: par_term_emu_core_rust::WidthConfig) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.set_width_config(config);
    }

    /// Set the Unicode normalization form for text processing
    ///
    /// Controls how Unicode text is normalized before being stored in terminal cells.
    /// NFC (default) composes characters where possible.
    /// NFD decomposes (macOS HFS+ style).
    /// NFKC/NFKD also resolve compatibility characters.
    /// None disables normalization entirely.
    pub fn set_normalization_form(&self, form: par_term_emu_core_rust::NormalizationForm) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.set_normalization_form(form);
    }

    /// Set a callback to receive raw PTY output data
    ///
    /// This is useful for session logging - the callback receives the raw bytes
    /// from the PTY before they are processed by the terminal emulator.
    ///
    /// # Arguments
    /// * `callback` - A function that takes a byte slice for each chunk of PTY output
    pub fn set_output_callback<F>(&self, callback: F)
    where
        F: Fn(&[u8]) + Send + Sync + 'static,
    {
        let mut pty = self.pty_session.lock();
        pty.set_output_callback(std::sync::Arc::new(callback));
    }

    /// Start recording the terminal session
    ///
    /// # Arguments
    /// * `title` - Optional title for the recording session
    pub fn start_recording(&self, title: Option<String>) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.start_recording(title);
    }

    /// Stop recording and return the recording session
    pub fn stop_recording(&self) -> Option<par_term_emu_core_rust::terminal::RecordingSession> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.stop_recording()
    }

    /// Check if recording is active
    pub fn is_recording(&self) -> bool {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.is_recording()
    }

    /// Export a recording session to asciicast format
    pub fn export_asciicast(
        &self,
        session: &par_term_emu_core_rust::terminal::RecordingSession,
    ) -> String {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.export_asciicast(session)
    }
}

// ========================================================================
// Coprocess Management Methods
// ========================================================================

impl TerminalManager {
    /// Start a new coprocess via the PtySession's built-in coprocess manager.
    ///
    /// The coprocess receives terminal output on its stdin (if `copy_terminal_output` is true)
    /// and its stdout is buffered for reading via `read_from_coprocess()`.
    pub fn start_coprocess(
        &self,
        config: par_term_emu_core_rust::coprocess::CoprocessConfig,
    ) -> std::result::Result<par_term_emu_core_rust::coprocess::CoprocessId, String> {
        let pty = self.pty_session.lock();
        pty.start_coprocess(config)
    }

    /// Stop a coprocess by ID.
    pub fn stop_coprocess(
        &self,
        id: par_term_emu_core_rust::coprocess::CoprocessId,
    ) -> std::result::Result<(), String> {
        let pty = self.pty_session.lock();
        pty.stop_coprocess(id)
    }

    /// Check if a coprocess is still running.
    pub fn coprocess_status(
        &self,
        id: par_term_emu_core_rust::coprocess::CoprocessId,
    ) -> Option<bool> {
        let pty = self.pty_session.lock();
        pty.coprocess_status(id)
    }

    /// Read buffered output from a coprocess (drains the buffer).
    pub fn read_from_coprocess(
        &self,
        id: par_term_emu_core_rust::coprocess::CoprocessId,
    ) -> std::result::Result<Vec<String>, String> {
        let pty = self.pty_session.lock();
        pty.read_from_coprocess(id)
    }

    /// List all coprocess IDs.
    pub fn list_coprocesses(&self) -> Vec<par_term_emu_core_rust::coprocess::CoprocessId> {
        let pty = self.pty_session.lock();
        pty.list_coprocesses()
    }

    /// Read buffered stderr output from a coprocess (drains the buffer).
    pub fn read_coprocess_errors(
        &self,
        id: par_term_emu_core_rust::coprocess::CoprocessId,
    ) -> std::result::Result<Vec<String>, String> {
        let pty = self.pty_session.lock();
        pty.read_coprocess_errors(id)
    }
}

// ========================================================================
// tmux Control Mode Methods
// ========================================================================

impl TerminalManager {
    /// Enable or disable tmux control mode parsing.
    ///
    /// When enabled, incoming PTY data is parsed for tmux control protocol
    /// messages. Regular terminal output within control mode is handled
    /// via `%output` notifications.
    ///
    /// This is used for gateway mode where `tmux -CC` is run in the terminal.
    pub fn set_tmux_control_mode(&self, enabled: bool) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.set_tmux_control_mode(enabled);
    }

    /// Check if tmux control mode is enabled.
    pub fn is_tmux_control_mode(&self) -> bool {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.is_tmux_control_mode()
    }

    /// Drain and return tmux control protocol notifications.
    ///
    /// This consumes all pending notifications from the terminal's tmux parser.
    /// Call this in the event loop to process tmux events.
    ///
    /// Returns core library notification types. Use `ParserBridge::convert_all()`
    /// to convert them to frontend notification types.
    pub fn drain_tmux_notifications(
        &self,
    ) -> Vec<par_term_emu_core_rust::tmux_control::TmuxNotification> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.drain_tmux_notifications()
    }

    /// Get a reference to pending tmux notifications without consuming them.
    pub fn tmux_notifications(
        &self,
    ) -> Vec<par_term_emu_core_rust::tmux_control::TmuxNotification> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.tmux_notifications().to_vec()
    }
}

// ========================================================================
// Trigger Sync Methods
// ========================================================================

impl TerminalManager {
    /// Sync trigger configs from Config into the core TriggerRegistry.
    ///
    /// Clears existing triggers and re-adds from config. Called on startup
    /// and when settings are saved.
    pub fn sync_triggers(&self, triggers: &[crate::config::automation::TriggerConfig]) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();

        // Clear existing triggers by removing all
        let existing: Vec<u64> = term.list_triggers().iter().map(|t| t.id).collect();
        for id in existing {
            term.remove_trigger(id);
        }

        // Add triggers from config
        for trigger_config in triggers {
            let actions: Vec<par_term_emu_core_rust::terminal::TriggerAction> = trigger_config
                .actions
                .iter()
                .map(|a| a.to_core_action())
                .collect();

            match term.add_trigger(
                trigger_config.name.clone(),
                trigger_config.pattern.clone(),
                actions,
            ) {
                Ok(id) => {
                    if !trigger_config.enabled {
                        term.set_trigger_enabled(id, false);
                    }
                    log::info!("Trigger '{}' registered (id={})", trigger_config.name, id);
                }
                Err(e) => {
                    log::error!(
                        "Failed to register trigger '{}': {}",
                        trigger_config.name,
                        e
                    );
                }
            }
        }
    }
}

// ========================================================================
// Observer Management Methods
// ========================================================================

impl TerminalManager {
    /// Register a terminal observer for push-based event delivery.
    ///
    /// The observer will receive `TerminalEvent` callbacks on the PTY reader
    /// thread after each `process()` call. Use this to attach a
    /// [`ScriptEventForwarder`](crate::scripting::observer::ScriptEventForwarder)
    /// so that terminal events are forwarded to script sub-processes.
    pub fn add_observer(
        &self,
        observer: std::sync::Arc<dyn par_term_emu_core_rust::observer::TerminalObserver>,
    ) -> par_term_emu_core_rust::observer::ObserverId {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.add_observer(observer)
    }

    /// Remove a previously registered observer.
    ///
    /// Returns `true` if an observer with the given ID was found and removed.
    pub fn remove_observer(&self, id: par_term_emu_core_rust::observer::ObserverId) -> bool {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.remove_observer(id)
    }
}

impl Drop for TerminalManager {
    fn drop(&mut self) {
        log::info!("Shutting down terminal manager");

        // Explicitly clean up PTY session to ensure proper shutdown
        if let Some(mut pty) = self.pty_session.try_lock() {
            // Kill any running process
            if pty.is_running() {
                log::info!("Killing PTY process during shutdown");
                if let Err(e) = pty.kill() {
                    log::warn!("Failed to kill PTY process: {:?}", e);
                }
            }
        } else {
            log::warn!("Could not acquire PTY lock during terminal manager shutdown");
        }

        log::info!("Terminal manager shutdown complete");
    }
}
