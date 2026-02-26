use crate::scrollback_metadata::{
    CommandSnapshot, LineMetadata, ScrollbackMark, ScrollbackMetadata,
};
use crate::styled_content::{StyledSegment, extract_styled_segments};
use anyhow::Result;
use par_term_config::Theme;
use par_term_emu_core_rust::pty_session::PtySession;
use par_term_emu_core_rust::shell_integration::ShellIntegrationMarker;
use par_term_emu_core_rust::terminal::Terminal;
use parking_lot::Mutex;
use std::sync::Arc;

/// Events produced by shell-integration markers that the prettifier pipeline
/// needs in order to delineate command output blocks.
#[derive(Debug, Clone)]
pub enum ShellLifecycleEvent {
    /// A command has started executing (OSC 133 C marker).
    CommandStarted {
        command: String,
        absolute_line: usize,
    },
    /// A command has finished executing (OSC 133 D marker).
    CommandFinished { absolute_line: usize },
}

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
    /// Shell lifecycle events queued for the prettifier pipeline.
    shell_lifecycle_events: Vec<ShellLifecycleEvent>,
}

impl TerminalManager {
    /// Create a new terminal manager with the specified dimensions
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
            shell_lifecycle_events: Vec::new(),
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
    fn extract_command_text(
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

    /// Set cell dimensions in pixels for sixel graphics scroll calculations
    pub fn set_cell_dimensions(&self, width: u32, height: u32) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.set_cell_dimensions(width, height);
    }

    /// Write data to the PTY (send user input to shell)
    pub fn write(&self, data: &[u8]) -> Result<()> {
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
    pub fn write_str(&self, data: &str) -> Result<()> {
        let mut pty = self.pty_session.lock();
        pty.write_str(data)
            .map_err(|e| anyhow::anyhow!("Failed to write to PTY: {}", e))?;
        Ok(())
    }

    /// Process raw data through the terminal emulator (for tmux output routing).
    pub fn process_data(&self, data: &[u8]) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.process(data);
    }

    /// Paste text to the terminal with proper bracketed paste handling.
    pub fn paste(&self, content: &str) -> Result<()> {
        if content.is_empty() {
            return Ok(());
        }

        let content = content.replace('\n', "\r");

        log::debug!("Pasting {} chars (bracketed paste check)", content.len());

        let (start, end) = {
            let pty = self.pty_session.lock();
            let terminal = pty.terminal();
            let term = terminal.lock();
            (
                term.bracketed_paste_start().to_vec(),
                term.bracketed_paste_end().to_vec(),
            )
        };

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
    pub async fn paste_with_delay(&self, content: &str, delay_ms: u64) -> Result<()> {
        if content.is_empty() {
            return Ok(());
        }

        let (start, end) = {
            let pty = self.pty_session.lock();
            let terminal = pty.terminal();
            let term = terminal.lock();
            (
                term.bracketed_paste_start().to_vec(),
                term.bracketed_paste_end().to_vec(),
            )
        };

        if !start.is_empty() {
            let mut pty = self.pty_session.lock();
            pty.write(&start)
                .map_err(|e| anyhow::anyhow!("Failed to write bracketed paste start: {}", e))?;
        }

        let lines: Vec<&str> = content.split('\n').collect();
        let delay = tokio::time::Duration::from_millis(delay_ms);

        for (i, line) in lines.iter().enumerate() {
            let mut line_data = line.replace('\n', "\r");
            if i < lines.len() - 1 {
                line_data.push('\r');
            }

            {
                let mut pty = self.pty_session.lock();
                pty.write(line_data.as_bytes())
                    .map_err(|e| anyhow::anyhow!("Failed to write paste line: {}", e))?;
            }

            if i < lines.len() - 1 {
                tokio::time::sleep(delay).await;
            }
        }

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
    pub fn content(&self) -> Result<String> {
        let pty = self.pty_session.lock();
        Ok(pty.content())
    }

    /// Resize the terminal
    pub fn resize(&mut self, cols: usize, rows: usize) -> Result<()> {
        log::info!("Resizing terminal to: {}x{}", cols, rows);

        let mut pty = self.pty_session.lock();
        pty.resize(cols as u16, rows as u16)
            .map_err(|e| anyhow::anyhow!("Failed to resize PTY: {}", e))?;

        self.dimensions = (cols, rows);
        Ok(())
    }

    /// Resize the terminal with pixel dimensions
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
    pub fn set_pixel_size(&mut self, width_px: usize, height_px: usize) -> Result<()> {
        let pty = self.pty_session.lock();
        let term_arc = pty.terminal();
        let mut term = term_arc.lock();
        term.set_pixel_size(width_px, height_px);
        Ok(())
    }

    /// Get the current terminal dimensions
    pub fn dimensions(&self) -> (usize, usize) {
        self.dimensions
    }

    /// Get a clone of the underlying terminal for direct access
    pub fn terminal(&self) -> Arc<Mutex<Terminal>> {
        let pty = self.pty_session.lock();
        pty.terminal()
    }

    /// Check if there have been updates since last check
    pub fn has_updates(&self) -> bool {
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

        let image_format = match format.to_lowercase().as_str() {
            "png" => ImageFormat::Png,
            "jpeg" | "jpg" => ImageFormat::Jpeg,
            "svg" => ImageFormat::Svg,
            _ => {
                log::warn!("Unknown format '{}', defaulting to PNG", format);
                ImageFormat::Png
            }
        };

        let config = ScreenshotConfig {
            format: image_format,
            ..Default::default()
        };

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
            _ => term.export_asciicast(session),
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
    pub fn shell_integration_hostname(&self) -> Option<String> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.shell_integration().hostname().map(String::from)
    }

    /// Get username from shell integration (OSC 7)
    pub fn shell_integration_username(&self) -> Option<String> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.shell_integration().username().map(String::from)
    }

    /// Poll CWD change events from terminal
    pub fn poll_cwd_events(&self) -> Vec<par_term_emu_core_rust::terminal::CwdChange> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.poll_cwd_events()
    }

    /// Poll trigger action results from the core terminal.
    pub fn poll_action_results(&self) -> Vec<par_term_emu_core_rust::terminal::ActionResult> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.poll_action_results()
    }

    // === File Transfer Methods ===

    pub fn get_active_transfers(
        &self,
    ) -> Vec<par_term_emu_core_rust::terminal::file_transfer::FileTransfer> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.get_active_transfers()
    }

    pub fn get_completed_transfers(
        &self,
    ) -> Vec<par_term_emu_core_rust::terminal::file_transfer::FileTransfer> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.get_completed_transfers()
    }

    pub fn take_completed_transfer(
        &self,
        id: u64,
    ) -> Option<par_term_emu_core_rust::terminal::file_transfer::FileTransfer> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.take_completed_transfer(id)
    }

    pub fn cancel_file_transfer(&self, id: u64) -> bool {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.cancel_file_transfer(id)
    }

    pub fn send_upload_data(&self, data: &[u8]) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.send_upload_data(data);
    }

    pub fn cancel_upload(&self) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.cancel_upload();
    }

    pub fn poll_upload_requests(&self) -> Vec<String> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.poll_upload_requests()
    }

    pub fn custom_session_variables(&self) -> std::collections::HashMap<String, String> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.session_variables().custom.clone()
    }

    pub fn shell_integration_stats(
        &self,
    ) -> par_term_emu_core_rust::terminal::ShellIntegrationStats {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.get_shell_integration_stats()
    }

    /// Get cursor position
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

    /// Check if cursor is visible
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

    /// Send focus event to PTY if the application has enabled focus tracking (DECSET 1004).
    /// Returns true if the event was sent.
    pub fn report_focus_change(&self, focused: bool) -> bool {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        let data = if focused {
            term.report_focus_in()
        } else {
            term.report_focus_out()
        };
        if !data.is_empty() {
            drop(term);
            drop(terminal);
            drop(pty);
            // Write the focus event sequence to PTY
            if let Err(e) = self.write(&data) {
                log::error!("Failed to write focus event to PTY: {}", e);
                return false;
            }
            true
        } else {
            false
        }
    }

    /// Check if alternate screen is active
    pub fn is_alt_screen_active(&self) -> bool {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.is_alt_screen_active()
    }

    /// Get the modifyOtherKeys mode
    pub fn modify_other_keys_mode(&self) -> u8 {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.modify_other_keys_mode()
    }

    /// Check if application cursor key mode (DECCKM) is enabled.
    pub fn application_cursor(&self) -> bool {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.application_cursor()
    }

    /// Get the terminal title set by OSC 0, 1, or 2 sequences
    pub fn get_title(&self) -> String {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.title().to_string()
    }

    /// Get the current shell integration marker state
    pub fn shell_integration_marker(
        &self,
    ) -> Option<par_term_emu_core_rust::shell_integration::ShellIntegrationMarker> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.shell_integration().marker()
    }

    /// Check if a command is currently running based on shell integration
    pub fn is_command_running(&self) -> bool {
        use par_term_emu_core_rust::shell_integration::ShellIntegrationMarker;

        matches!(
            self.shell_integration_marker(),
            Some(ShellIntegrationMarker::CommandExecuted)
        )
    }

    /// Get the name of the currently running command (first word only)
    pub fn get_running_command_name(&self) -> Option<String> {
        if !self.is_command_running() {
            return None;
        }

        self.shell_integration_command().and_then(|cmd| {
            let first_word = cmd.split_whitespace().next()?;
            let name = std::path::Path::new(first_word)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(first_word);
            Some(name.to_string())
        })
    }

    /// Check if tab close should show a confirmation dialog
    pub fn should_confirm_close(&self, jobs_to_ignore: &[String]) -> Option<String> {
        let command_name = self.get_running_command_name()?;

        let command_lower = command_name.to_lowercase();
        for ignore in jobs_to_ignore {
            if ignore.to_lowercase() == command_lower {
                return None;
            }
        }

        Some(command_name)
    }

    /// Check if mouse motion events should be reported
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
    pub fn get_styled_segments(&self) -> Vec<StyledSegment> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        let grid = term.active_grid();
        extract_styled_segments(grid)
    }

    /// Get the current generation number for dirty tracking
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

    /// Check if any progress bar is currently active
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
    pub fn set_answerback_string(&self, answerback: Option<String>) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.set_answerback_string(answerback);
    }

    pub fn set_width_config(&self, config: par_term_emu_core_rust::WidthConfig) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.set_width_config(config);
    }

    pub fn set_normalization_form(&self, form: par_term_emu_core_rust::NormalizationForm) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.set_normalization_form(form);
    }

    pub fn set_output_callback<F>(&self, callback: F)
    where
        F: Fn(&[u8]) + Send + Sync + 'static,
    {
        let mut pty = self.pty_session.lock();
        pty.set_output_callback(std::sync::Arc::new(callback));
    }

    pub fn start_recording(&self, title: Option<String>) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.start_recording(title);
    }

    pub fn stop_recording(&self) -> Option<par_term_emu_core_rust::terminal::RecordingSession> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.stop_recording()
    }

    pub fn is_recording(&self) -> bool {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.is_recording()
    }

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
    pub fn start_coprocess(
        &self,
        config: par_term_emu_core_rust::coprocess::CoprocessConfig,
    ) -> std::result::Result<par_term_emu_core_rust::coprocess::CoprocessId, String> {
        let pty = self.pty_session.lock();
        pty.start_coprocess(config)
    }

    pub fn stop_coprocess(
        &self,
        id: par_term_emu_core_rust::coprocess::CoprocessId,
    ) -> std::result::Result<(), String> {
        let pty = self.pty_session.lock();
        pty.stop_coprocess(id)
    }

    pub fn coprocess_status(
        &self,
        id: par_term_emu_core_rust::coprocess::CoprocessId,
    ) -> Option<bool> {
        let pty = self.pty_session.lock();
        pty.coprocess_status(id)
    }

    pub fn read_from_coprocess(
        &self,
        id: par_term_emu_core_rust::coprocess::CoprocessId,
    ) -> std::result::Result<Vec<String>, String> {
        let pty = self.pty_session.lock();
        pty.read_from_coprocess(id)
    }

    pub fn list_coprocesses(&self) -> Vec<par_term_emu_core_rust::coprocess::CoprocessId> {
        let pty = self.pty_session.lock();
        pty.list_coprocesses()
    }

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
    pub fn set_tmux_control_mode(&self, enabled: bool) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.set_tmux_control_mode(enabled);
    }

    pub fn is_tmux_control_mode(&self) -> bool {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.is_tmux_control_mode()
    }

    pub fn drain_tmux_notifications(
        &self,
    ) -> Vec<par_term_emu_core_rust::tmux_control::TmuxNotification> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.drain_tmux_notifications()
    }

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
    pub fn sync_triggers(&self, triggers: &[par_term_config::TriggerConfig]) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();

        let existing: Vec<u64> = term.list_triggers().iter().map(|t| t.id).collect();
        for id in existing {
            term.remove_trigger(id);
        }

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
    pub fn add_observer(
        &self,
        observer: std::sync::Arc<dyn par_term_emu_core_rust::observer::TerminalObserver>,
    ) -> par_term_emu_core_rust::observer::ObserverId {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.add_observer(observer)
    }

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

        if let Some(mut pty) = self.pty_session.try_lock() {
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
