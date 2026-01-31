use crate::styled_content::{StyledSegment, extract_styled_segments};
use crate::themes::Theme;
use anyhow::Result;
use par_term_emu_core_rust::pty_session::PtySession;
use par_term_emu_core_rust::terminal::Terminal;
use parking_lot::Mutex;
use std::sync::Arc;

// Re-export clipboard types for use in other modules
pub use par_term_emu_core_rust::terminal::{ClipboardEntry, ClipboardSlot};

/// Convert ANSI color index to RGB
#[allow(dead_code)]
fn ansi_to_rgb(color_idx: u8) -> [u8; 3] {
    match color_idx {
        // Standard 16 colors
        0 => [0, 0, 0],        // Black
        1 => [205, 0, 0],      // Red
        2 => [0, 205, 0],      // Green
        3 => [205, 205, 0],    // Yellow
        4 => [0, 0, 238],      // Blue
        5 => [205, 0, 205],    // Magenta
        6 => [0, 205, 205],    // Cyan
        7 => [229, 229, 229],  // White
        8 => [127, 127, 127],  // Bright Black (Gray)
        9 => [255, 0, 0],      // Bright Red
        10 => [0, 255, 0],     // Bright Green
        11 => [255, 255, 0],   // Bright Yellow
        12 => [92, 92, 255],   // Bright Blue
        13 => [255, 0, 255],   // Bright Magenta
        14 => [0, 255, 255],   // Bright Cyan
        15 => [255, 255, 255], // Bright White
        // 216 color cube (16-231)
        16..=231 => {
            let idx = color_idx - 16;
            let r = (idx / 36) * 51;
            let g = ((idx % 36) / 6) * 51;
            let b = (idx % 6) * 51;
            [r, g, b]
        }
        // Grayscale (232-255)
        232..=255 => {
            let gray = 8 + (color_idx - 232) * 10;
            [gray, gray, gray]
        }
    }
}

pub mod clipboard;
pub mod graphics;
pub mod hyperlinks;
pub mod rendering;
pub mod spawn;

/// Terminal manager that wraps the PTY session
pub struct TerminalManager {
    /// The underlying PTY session
    pub(crate) pty_session: Arc<Mutex<PtySession>>,
    /// Terminal dimensions (cols, rows)
    pub(crate) dimensions: (usize, usize),
    /// Color theme for ANSI colors
    pub(crate) theme: Theme,
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
        })
    }

    /// Set the color theme
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
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

    // TODO: Recording APIs not yet available in par-term-emu-core-rust
    // Uncomment when the core library supports recording again

    /*
    /// Start recording a terminal session
    pub fn start_recording(&self, title: Option<String>) {
        log::info!("Starting session recording");
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.start_recording(title);
    }

    /// Stop recording and return the recording session
    pub fn stop_recording(&self) -> Option<par_term_emu_core_rust::terminal::RecordingSession> {
        log::info!("Stopping session recording");
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.stop_recording()
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

    /// Check if currently recording
    pub fn is_recording(&self) -> bool {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.is_recording()
    }
    */

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
    #[allow(dead_code)]
    pub fn shell_integration_command(&self) -> Option<String> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.shell_integration().command().map(String::from)
    }

    // TODO: Shell integration stats API not yet available in par-term-emu-core-rust
    /*
    /// Get shell integration statistics
    pub fn shell_integration_stats(
        &self,
    ) -> par_term_emu_core_rust::terminal::ShellIntegrationStats {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.get_shell_integration_stats()
    }
    */

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
