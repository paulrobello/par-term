use crate::scrollback_metadata::ScrollbackMetadata;
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
pub mod scrollback;
pub mod spawn;

// Re-export coprocess_env from spawn so existing callers keep working
pub use spawn::coprocess_env;

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

    /// Set cell dimensions in pixels for sixel graphics scroll calculations
    pub fn set_cell_dimensions(&self, width: u32, height: u32) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.set_cell_dimensions(width, height);
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
    pub fn get_styled_segments(&self) -> Vec<crate::styled_content::StyledSegment> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        let grid = term.active_grid();
        crate::styled_content::extract_styled_segments(grid)
    }

    /// Get the current generation number for dirty tracking
    pub fn update_generation(&self) -> u64 {
        let pty = self.pty_session.lock();
        pty.update_generation()
    }
}

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
// Answerback String (ENQ Response) and Terminal Configuration
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
    ///
    /// Returns a map of `trigger_id -> require_user_action` for each
    /// successfully registered trigger, so the frontend can enforce
    /// security restrictions on dangerous actions.
    pub fn sync_triggers(
        &self,
        triggers: &[par_term_config::TriggerConfig],
    ) -> std::collections::HashMap<u64, bool> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();

        let existing: Vec<u64> = term.list_triggers().iter().map(|t| t.id).collect();
        for id in existing {
            term.remove_trigger(id);
        }

        let mut security_map = std::collections::HashMap::new();

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
                    security_map.insert(id, trigger_config.require_user_action);
                    log::info!(
                        "Trigger '{}' registered (id={}, require_user_action={})",
                        trigger_config.name,
                        id,
                        trigger_config.require_user_action,
                    );
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

        security_map
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
