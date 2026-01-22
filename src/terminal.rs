use crate::cell_renderer::Cell;
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

/// Terminal manager that wraps the PTY session
pub struct TerminalManager {
    /// The underlying PTY session
    pty_session: Arc<Mutex<PtySession>>,
    /// Terminal dimensions (cols, rows)
    dimensions: (usize, usize),
    /// Color theme for ANSI colors
    theme: Theme,
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

    /// Spawn a shell in the terminal
    #[allow(dead_code)]
    pub fn spawn_shell(&mut self) -> Result<()> {
        log::info!("Spawning shell in PTY");
        let mut pty = self.pty_session.lock();
        pty.spawn_shell()
            .map_err(|e| anyhow::anyhow!("Failed to spawn shell: {}", e))?;
        Ok(())
    }

    /// Spawn a custom shell command in the terminal
    ///
    /// # Arguments
    /// * `command` - The shell command to execute (e.g., "/bin/zsh", "fish")
    #[allow(dead_code)]
    pub fn spawn_custom_shell(&mut self, command: &str) -> Result<()> {
        log::info!("Spawning custom shell: {}", command);
        let mut pty = self.pty_session.lock();
        let args: Vec<&str> = Vec::new();
        pty.spawn(command, &args)
            .map_err(|e| anyhow::anyhow!("Failed to spawn custom shell: {}", e))?;
        Ok(())
    }

    /// Spawn a custom shell with arguments
    ///
    /// # Arguments
    /// * `command` - The shell command to execute
    /// * `args` - Arguments to pass to the shell
    #[allow(dead_code)]
    pub fn spawn_custom_shell_with_args(&mut self, command: &str, args: &[String]) -> Result<()> {
        log::info!("Spawning custom shell: {} with args: {:?}", command, args);
        let mut pty = self.pty_session.lock();
        let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        pty.spawn(command, &args_refs)
            .map_err(|e| anyhow::anyhow!("Failed to spawn custom shell: {}", e))?;
        Ok(())
    }

    /// Spawn shell with optional working directory and environment variables
    ///
    /// # Arguments
    /// * `working_dir` - Optional working directory path
    /// * `env_vars` - Optional environment variables to set
    #[allow(dead_code)]
    pub fn spawn_shell_with_dir(
        &mut self,
        working_dir: Option<&str>,
        env_vars: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<()> {
        log::info!(
            "Spawning shell with dir: {:?}, env: {:?}",
            working_dir,
            env_vars
        );
        let mut pty = self.pty_session.lock();
        pty.spawn_shell_with_env(env_vars, working_dir)
            .map_err(|e| anyhow::anyhow!("Failed to spawn shell with env: {}", e))
    }

    /// Spawn custom shell with args, optional working directory, and environment variables
    ///
    /// # Arguments
    /// * `command` - The shell command to execute
    /// * `args` - Arguments to pass to the shell
    /// * `working_dir` - Optional working directory path
    /// * `env_vars` - Optional environment variables to set
    pub fn spawn_custom_shell_with_dir(
        &mut self,
        command: &str,
        args: Option<&[String]>,
        working_dir: Option<&str>,
        env_vars: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<()> {
        log::info!(
            "Spawning custom shell: {} with dir: {:?}, env: {:?}",
            command,
            working_dir,
            env_vars
        );

        let args_refs: Vec<&str> = args
            .map(|a| a.iter().map(|s| s.as_str()).collect())
            .unwrap_or_default();

        let mut pty = self.pty_session.lock();
        pty.spawn_with_env(command, &args_refs, env_vars, working_dir)
            .map_err(|e| anyhow::anyhow!("Failed to spawn custom shell with env: {}", e))
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
    #[allow(dead_code)]
    pub fn has_updates(&self) -> bool {
        // For now, always assume there are updates since we poll at 60fps
        // In the future, we could track update generation to optimize
        true
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

    /// Get all graphics (Sixel, iTerm2, Kitty)
    /// Returns a vector of cloned TerminalGraphic objects for rendering
    #[allow(dead_code)]
    pub fn get_graphics(&self) -> Vec<par_term_emu_core_rust::graphics::TerminalGraphic> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        let graphics: Vec<_> = term.all_graphics().to_vec();
        if !graphics.is_empty() {
            debug_info!(
                "TERMINAL",
                "Returning {} graphics from core library",
                graphics.len()
            );
            for (i, g) in graphics.iter().enumerate() {
                debug_trace!(
                    "TERMINAL",
                    "  [{}] protocol={:?}, pos=({},{}), size={}x{}",
                    i,
                    g.protocol,
                    g.position.0,
                    g.position.1,
                    g.width,
                    g.height
                );
            }
        }
        graphics
    }

    /// Get graphics at a specific row
    #[allow(dead_code)]
    pub fn get_graphics_at_row(
        &self,
        row: usize,
    ) -> Vec<par_term_emu_core_rust::graphics::TerminalGraphic> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.graphics_at_row(row)
            .iter()
            .map(|g| (*g).clone())
            .collect()
    }

    /// Get total graphics count
    #[allow(dead_code)]
    pub fn graphics_count(&self) -> usize {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.graphics_count()
    }

    /// Get all OSC 8 hyperlinks from the terminal
    pub fn get_all_hyperlinks(&self) -> Vec<par_term_emu_core_rust::terminal::HyperlinkInfo> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.get_all_hyperlinks()
    }

    /// Get the URL for a specific hyperlink ID
    #[allow(dead_code)]
    pub fn get_hyperlink_url(&self, hyperlink_id: u32) -> Option<String> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.get_hyperlink_url(hyperlink_id)
    }

    /// Get all scrollback graphics
    pub fn get_scrollback_graphics(
        &self,
    ) -> Vec<par_term_emu_core_rust::graphics::TerminalGraphic> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.all_scrollback_graphics().to_vec()
    }

    /// Update animations and return true if any frames changed
    ///
    /// This should be called periodically (e.g., in the redraw loop) to advance
    /// animation frames based on timing. Returns true if any animation advanced
    /// to a new frame, indicating that a redraw is needed.
    pub fn update_animations(&self) -> bool {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        let changed_images = term.graphics_store_mut().update_animations();
        !changed_images.is_empty()
    }

    /// Get all graphics with current animation frames
    ///
    /// For animated graphics, returns the current frame based on animation state.
    /// For static graphics, returns the original graphic unchanged.
    pub fn get_graphics_with_animations(
        &self,
    ) -> Vec<par_term_emu_core_rust::graphics::TerminalGraphic> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();

        let mut graphics = Vec::new();

        // First, collect all base graphics
        let base_graphics: Vec<_> = term.all_graphics().to_vec();

        debug_log!(
            "TERMINAL",
            "get_graphics_with_animations() - base_graphics count: {}",
            base_graphics.len()
        );

        // Then, for each graphic, check if it has an animation and get current frame
        for (idx, graphic) in base_graphics.iter().enumerate() {
            debug_trace!(
                "TERMINAL",
                "Processing graphic {} - pos=({},{}), size={}x{}, kitty_id={:?}",
                idx,
                graphic.position.0,
                graphic.position.1,
                graphic.width,
                graphic.height,
                graphic.kitty_image_id
            );

            // Check if this graphic has an active animation
            if let Some(image_id) = graphic.kitty_image_id
                && let Some(anim) = term.graphics_store().get_animation(image_id)
                && let Some(current_frame) = anim.current_frame()
            {
                // Create a graphic from the current animation frame
                let mut animated_graphic = graphic.clone();
                animated_graphic.pixels = current_frame.pixels.clone();
                animated_graphic.width = current_frame.width;
                animated_graphic.height = current_frame.height;

                debug_info!(
                    "TERMINAL",
                    "Using animated frame {} for image {}",
                    anim.current_frame,
                    image_id
                );

                graphics.push(animated_graphic);
                continue;
            }
            // Not animated or no current frame - use original graphic
            debug_trace!("TERMINAL", "Using static graphic {}", idx);
            graphics.push(graphic.clone());
        }

        debug_log!("TERMINAL", "Returning {} graphics total", graphics.len());
        graphics
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

    /// Get terminal grid with scrollback offset as Cell array for CellRenderer
    ///
    /// # Arguments
    /// * `scroll_offset` - Number of lines to scroll back (0 = current view at bottom)
    /// * `selection` - Optional selection range (start_col, start_row, end_col, end_row) in screen coordinates
    /// * `rectangular` - Whether the selection is rectangular/block mode (default: false)
    /// * `cursor` - Optional cursor (position, opacity) for smooth fade animations
    pub fn get_cells_with_scrollback(
        &self,
        scroll_offset: usize,
        selection: Option<((usize, usize), (usize, usize))>,
        rectangular: bool,
        _cursor: Option<((usize, usize), f32)>,
    ) -> Vec<Cell> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        let grid = term.active_grid();

        // Don't pass cursor to cells - we'll render it separately as geometry
        let cursor_with_style = None;

        let rows = grid.rows();
        let cols = grid.cols();
        let scrollback_len = grid.scrollback_len();
        let clamped_offset = scroll_offset.min(scrollback_len);
        let total_lines = scrollback_len + rows;
        let end_line = total_lines.saturating_sub(clamped_offset);
        let start_line = end_line.saturating_sub(rows);

        let mut cells = Vec::with_capacity(rows * cols);

        for line_idx in start_line..end_line {
            let screen_row = line_idx - start_line;

            if line_idx < scrollback_len {
                if let Some(line) = grid.scrollback_line(line_idx) {
                    Self::push_line_from_slice(
                        line,
                        cols,
                        &mut cells,
                        screen_row,
                        selection,
                        rectangular,
                        cursor_with_style,
                        &self.theme,
                    );
                } else {
                    Self::push_empty_cells(cols, &mut cells);
                }
            } else {
                let grid_row = line_idx - scrollback_len;
                Self::push_grid_row(
                    grid,
                    grid_row,
                    cols,
                    &mut cells,
                    screen_row,
                    selection,
                    rectangular,
                    cursor_with_style,
                    &self.theme,
                );
            }
        }

        cells
    }
}

impl TerminalManager {
    #[allow(clippy::too_many_arguments)]
    fn push_line_from_slice(
        line: &[par_term_emu_core_rust::cell::Cell],
        cols: usize,
        dest: &mut Vec<Cell>,
        screen_row: usize,
        selection: Option<((usize, usize), (usize, usize))>,
        rectangular: bool,
        cursor: Option<(
            (usize, usize),
            f32,
            par_term_emu_core_rust::cursor::CursorStyle,
        )>,
        theme: &Theme,
    ) {
        let copy_len = cols.min(line.len());
        for (col, cell) in line[..copy_len].iter().enumerate() {
            let is_selected = Self::is_cell_selected(col, screen_row, selection, rectangular);
            let cursor_info = cursor.and_then(|((cx, cy), opacity, style)| {
                if cx == col && cy == screen_row {
                    Some((opacity, style))
                } else {
                    None
                }
            });
            dest.push(Self::convert_term_cell_with_theme(
                cell,
                is_selected,
                cursor_info,
                theme,
            ));
        }

        if copy_len < cols {
            Self::push_empty_cells(cols - copy_len, dest);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn push_grid_row(
        grid: &par_term_emu_core_rust::grid::Grid,
        row: usize,
        cols: usize,
        dest: &mut Vec<Cell>,
        screen_row: usize,
        selection: Option<((usize, usize), (usize, usize))>,
        rectangular: bool,
        cursor: Option<(
            (usize, usize),
            f32,
            par_term_emu_core_rust::cursor::CursorStyle,
        )>,
        theme: &Theme,
    ) {
        for col in 0..cols {
            let is_selected = Self::is_cell_selected(col, screen_row, selection, rectangular);
            let cursor_info = cursor.and_then(|((cx, cy), opacity, style)| {
                if cx == col && cy == screen_row {
                    Some((opacity, style))
                } else {
                    None
                }
            });
            if let Some(cell) = grid.get(col, row) {
                dest.push(Self::convert_term_cell_with_theme(
                    cell,
                    is_selected,
                    cursor_info,
                    theme,
                ));
            } else {
                dest.push(Cell::default());
            }
        }
    }

    fn push_empty_cells(count: usize, dest: &mut Vec<Cell>) {
        for _ in 0..count {
            dest.push(Cell::default());
        }
    }

    /// Check if a cell at (col, row) is within the selection range
    fn is_cell_selected(
        col: usize,
        row: usize,
        selection: Option<((usize, usize), (usize, usize))>,
        rectangular: bool,
    ) -> bool {
        if let Some(((start_col, start_row), (end_col, end_row))) = selection {
            if rectangular {
                // Rectangular selection: select cells within the column and row bounds
                let min_col = start_col.min(end_col);
                let max_col = start_col.max(end_col);
                let min_row = start_row.min(end_row);
                let max_row = start_row.max(end_row);

                return col >= min_col && col <= max_col && row >= min_row && row <= max_row;
            }

            // Normal line-based selection
            // Single line selection
            if start_row == end_row {
                return row == start_row && col >= start_col && col <= end_col;
            }

            // Multi-line selection
            if row == start_row {
                // First line - from start_col to end of line
                return col >= start_col;
            } else if row == end_row {
                // Last line - from start of line to end_col
                return col <= end_col;
            } else if row > start_row && row < end_row {
                // Middle lines - entire line selected
                return true;
            }
        }
        false
    }

    fn convert_term_cell_with_theme(
        term_cell: &par_term_emu_core_rust::cell::Cell,
        is_selected: bool,
        cursor_info: Option<(f32, par_term_emu_core_rust::cursor::CursorStyle)>,
        theme: &Theme,
    ) -> Cell {
        use par_term_emu_core_rust::color::{Color as TermColor, NamedColor};
        use par_term_emu_core_rust::cursor::CursorStyle as TermCursorStyle;

        // Debug: Log cells with non-default backgrounds OR reverse flag (likely status bar)
        // This helps diagnose TMUX status bar background rendering issues
        let bg_rgb = term_cell.bg.to_rgb();
        let fg_rgb = term_cell.fg.to_rgb();
        let has_colored_bg = bg_rgb != (0, 0, 0); // Not black background
        let has_reverse = term_cell.flags.reverse();

        if has_colored_bg || has_reverse {
            debug_info!(
                "TERMINAL",
                "Cell with colored BG or REVERSE: '{}' (U+{:04X}): fg={:?} (RGB:{},{},{}), bg={:?} (RGB:{},{},{}), reverse={}, flags={:?}",
                if term_cell.c.is_control() {
                    '?'
                } else {
                    term_cell.c
                },
                term_cell.c as u32,
                term_cell.fg,
                fg_rgb.0,
                fg_rgb.1,
                fg_rgb.2,
                term_cell.bg,
                bg_rgb.0,
                bg_rgb.1,
                bg_rgb.2,
                has_reverse,
                term_cell.flags
            );
        }

        // Apply theme colors for ANSI colors (Named colors)
        let fg = match &term_cell.fg {
            TermColor::Named(named) => {
                #[allow(unreachable_patterns)]
                let theme_color = match named {
                    NamedColor::Black => theme.black,
                    NamedColor::Red => theme.red,
                    NamedColor::Green => theme.green,
                    NamedColor::Yellow => theme.yellow,
                    NamedColor::Blue => theme.blue,
                    NamedColor::Magenta => theme.magenta,
                    NamedColor::Cyan => theme.cyan,
                    NamedColor::White => theme.white,
                    NamedColor::BrightBlack => theme.bright_black,
                    NamedColor::BrightRed => theme.bright_red,
                    NamedColor::BrightGreen => theme.bright_green,
                    NamedColor::BrightYellow => theme.bright_yellow,
                    NamedColor::BrightBlue => theme.bright_blue,
                    NamedColor::BrightMagenta => theme.bright_magenta,
                    NamedColor::BrightCyan => theme.bright_cyan,
                    NamedColor::BrightWhite => theme.bright_white,
                    _ => theme.foreground, // Other colors default to foreground
                };
                (theme_color.r, theme_color.g, theme_color.b)
            }
            _ => term_cell.fg.to_rgb(), // Keep 256-color and RGB as-is
        };

        let bg = match &term_cell.bg {
            TermColor::Named(named) => {
                #[allow(unreachable_patterns)]
                let theme_color = match named {
                    NamedColor::Black => theme.black,
                    NamedColor::Red => theme.red,
                    NamedColor::Green => theme.green,
                    NamedColor::Yellow => theme.yellow,
                    NamedColor::Blue => theme.blue,
                    NamedColor::Magenta => theme.magenta,
                    NamedColor::Cyan => theme.cyan,
                    NamedColor::White => theme.white,
                    NamedColor::BrightBlack => theme.bright_black,
                    NamedColor::BrightRed => theme.bright_red,
                    NamedColor::BrightGreen => theme.bright_green,
                    NamedColor::BrightYellow => theme.bright_yellow,
                    NamedColor::BrightBlue => theme.bright_blue,
                    NamedColor::BrightMagenta => theme.bright_magenta,
                    NamedColor::BrightCyan => theme.bright_cyan,
                    NamedColor::BrightWhite => theme.bright_white,
                    _ => theme.background, // Other colors default to background
                };
                (theme_color.r, theme_color.g, theme_color.b)
            }
            _ => term_cell.bg.to_rgb(), // Keep 256-color and RGB as-is
        };

        // Check if cell has reverse video flag (SGR 7) - TMUX uses this for status bar
        let is_reverse = term_cell.flags.reverse();

        // Blend colors for smooth cursor fade animation, or invert for selection/reverse
        let (fg_color, bg_color) = if let Some((opacity, style)) = cursor_info {
            // Smooth cursor: blend between normal and inverted colors based on opacity and style
            let blend = |normal: u8, inverted: u8, opacity: f32| -> u8 {
                (normal as f32 * (1.0 - opacity) + inverted as f32 * opacity) as u8
            };

            // Different cursor styles - for now, all use inversion
            // TODO: Implement proper geometric rendering for beam/underline cursors
            // This requires adding cursor geometry to the cell renderer
            match style {
                // Block cursor: full inversion (default behavior)
                TermCursorStyle::SteadyBlock | TermCursorStyle::BlinkingBlock => (
                    [
                        blend(fg.0, bg.0, opacity),
                        blend(fg.1, bg.1, opacity),
                        blend(fg.2, bg.2, opacity),
                        255,
                    ],
                    [
                        blend(bg.0, fg.0, opacity),
                        blend(bg.1, fg.1, opacity),
                        blend(bg.2, fg.2, opacity),
                        255,
                    ],
                ),
                // Beam and Underline: Use same inversion for now
                // Proper implementation would draw thin lines in the renderer
                TermCursorStyle::SteadyBar
                | TermCursorStyle::BlinkingBar
                | TermCursorStyle::SteadyUnderline
                | TermCursorStyle::BlinkingUnderline => (
                    [
                        blend(fg.0, bg.0, opacity),
                        blend(fg.1, bg.1, opacity),
                        blend(fg.2, bg.2, opacity),
                        255,
                    ],
                    [
                        blend(bg.0, fg.0, opacity),
                        blend(bg.1, fg.1, opacity),
                        blend(bg.2, fg.2, opacity),
                        255,
                    ],
                ),
            }
        } else if is_selected || is_reverse {
            // Selection or Reverse video (SGR 7): invert colors
            (
                [bg.0, bg.1, bg.2, 255], // Swap: background becomes foreground
                [fg.0, fg.1, fg.2, 255], // Swap: foreground becomes background
            )
        } else {
            // Normal cell
            ([fg.0, fg.1, fg.2, 255], [bg.0, bg.1, bg.2, 255])
        };

        // Optimization: Avoid String allocation for cells without combining chars
        let grapheme = if term_cell.has_combining_chars() {
            term_cell.get_grapheme()
        } else {
            term_cell.base_char().to_string()
        };

        Cell {
            grapheme,
            fg_color,
            bg_color,
            bold: term_cell.flags.bold(),
            italic: term_cell.flags.italic(),
            underline: term_cell.flags.underline(),
            strikethrough: term_cell.flags.strikethrough(),
            hyperlink_id: term_cell.flags.hyperlink_id,
            wide_char: term_cell.flags.wide_char(),
            wide_char_spacer: term_cell.flags.wide_char_spacer(),
        }
    }
}

// ========================================================================
// Clipboard History Methods
// ========================================================================

impl TerminalManager {
    /// Get clipboard history for a specific slot
    pub fn get_clipboard_history(&self, slot: ClipboardSlot) -> Vec<ClipboardEntry> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.get_clipboard_history(slot)
    }

    /// Get the most recent clipboard entry for a slot
    #[allow(dead_code)]
    #[allow(dead_code)]
    pub fn get_latest_clipboard(&self, slot: ClipboardSlot) -> Option<ClipboardEntry> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.get_latest_clipboard(slot)
    }

    /// Search clipboard history across all slots or a specific slot
    #[allow(dead_code)]
    #[allow(dead_code)]
    pub fn search_clipboard_history(
        &self,
        query: &str,
        slot: Option<ClipboardSlot>,
    ) -> Vec<ClipboardEntry> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.search_clipboard_history(query, slot)
    }

    /// Add content to clipboard history
    pub fn add_to_clipboard_history(
        &self,
        slot: ClipboardSlot,
        content: String,
        label: Option<String>,
    ) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.add_to_clipboard_history(slot, content, label);
    }

    /// Clear clipboard history for a specific slot
    pub fn clear_clipboard_history(&self, slot: ClipboardSlot) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.clear_clipboard_history(slot);
    }

    /// Clear all clipboard history
    pub fn clear_all_clipboard_history(&self) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.clear_all_clipboard_history();
    }

    /// Set maximum clipboard sync events retained
    pub fn set_max_clipboard_sync_events(&self, max: usize) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.set_max_clipboard_sync_events(max);
    }

    /// Set maximum bytes cached per clipboard event
    pub fn set_max_clipboard_event_bytes(&self, max_bytes: usize) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.set_max_clipboard_event_bytes(max_bytes);
    }

    /// Set maximum clipboard history entries per slot
    #[allow(dead_code)]
    #[allow(dead_code)]
    pub fn set_max_clipboard_sync_history(&self, max: usize) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.set_max_clipboard_sync_history(max);
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
