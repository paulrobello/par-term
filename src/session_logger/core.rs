//! Core [`SessionLogger`] struct, state management, and recording methods.
//!
//! See the [module-level documentation](super) for information about security
//! considerations and password redaction.

use crate::config::SessionLogFormat;
use crate::session_logger::writers::{html_escape, strip_ansi_escapes};
use anyhow::{Context, Result};
use chrono::{Local, Utc};
use par_term_emu_core_rust::terminal::{RecordingEvent, RecordingEventType, RecordingSession};
use parking_lot::Mutex;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Marker text written to the log when input is redacted during a password prompt.
pub(super) const REDACTION_MARKER: &str = "[INPUT REDACTED - echo off]";

/// Common password prompt patterns (case-insensitive matching).
///
/// These patterns are matched against terminal output (after stripping ANSI
/// escape sequences) to detect when the user is being asked for a password.
/// The match is performed on the last line of each output chunk.
pub(super) const PASSWORD_PROMPT_PATTERNS: &[&str] = &[
    "password:",
    "password for",
    "passwd:",
    "[sudo]",
    "passphrase:",
    "passphrase for",
    "enter pin",
    "enter passphrase",
    "enter password",
    "old password:",
    "new password:",
    "retype password:",
    "confirm password:",
    "current password:",
    "verification code:",
    "login password:",
    "ldap password:",
    "key password:",
    "decryption password:",
    "encryption password:",
    "(current) unix password:",
    "token:",
];

/// Session logger that records terminal output to files.
///
/// The logger captures PTY output with timestamps and can export
/// to multiple formats. It uses buffered writes for performance.
///
/// See the [module-level documentation](super) for information about
/// sensitive data filtering.
pub struct SessionLogger {
    /// Whether logging is currently active
    pub(super) active: bool,
    /// The log format to use
    pub(super) format: SessionLogFormat,
    /// Output file path
    pub(super) output_path: PathBuf,
    /// Buffered writer for the log file
    pub(super) writer: Option<BufWriter<File>>,
    /// Recording session data (for asciicast format)
    pub(super) recording: Option<RecordingSession>,
    /// Recording start time (for relative timestamps)
    pub(super) start_time: std::time::Instant,
    /// Terminal dimensions
    pub(super) dimensions: (usize, usize),
    /// Session title
    pub(super) title: Option<String>,
    /// Whether password redaction is enabled (heuristic prompt detection)
    pub(super) redact_passwords: bool,
    /// Whether the logger has detected a password prompt in recent output
    /// and is currently suppressing input recording.
    pub(super) password_prompt_active: bool,
    /// Whether echo is externally known to be suppressed (e.g., PTY echo off).
    /// When true, input is always redacted regardless of prompt detection.
    pub(super) echo_suppressed: bool,
    /// Whether a redaction marker has already been emitted for the current
    /// suppression period (to avoid flooding the log with repeated markers).
    pub(super) redaction_marker_emitted: bool,
}

impl SessionLogger {
    /// Create a new session logger.
    ///
    /// # Arguments
    /// * `format` - The output format to use
    /// * `log_dir` - Directory where log files are stored
    /// * `dimensions` - Terminal dimensions (cols, rows)
    /// * `title` - Optional session title
    pub fn new(
        format: SessionLogFormat,
        log_dir: &Path,
        dimensions: (usize, usize),
        title: Option<String>,
    ) -> Result<Self> {
        // Generate filename with timestamp
        let timestamp = Local::now().format("%Y%m%d_%H%M%S");
        let filename = format!("session_{}.{}", timestamp, format.extension());
        let output_path = log_dir.join(filename);

        log::info!(
            "Creating session logger: {:?} (format: {:?})",
            output_path,
            format
        );

        // Create the log file with restrictive permissions (owner read/write only)
        // On Unix, use mode 0o600 to prevent world-readable session logs
        // On Windows, file permissions work differently but this is still safe
        let mut opts = OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let file = opts
            .open(&output_path)
            .with_context(|| format!("Failed to create session log file: {:?}", output_path))?;
        let writer = BufWriter::with_capacity(8192, file); // 8KB buffer

        // Initialize recording session for asciicast format
        let recording = if format == SessionLogFormat::Asciicast {
            let mut env = std::collections::HashMap::new();
            env.insert("TERM".to_string(), "xterm-256color".to_string());
            env.insert("COLS".to_string(), dimensions.0.to_string());
            env.insert("ROWS".to_string(), dimensions.1.to_string());

            Some(RecordingSession {
                id: uuid::Uuid::new_v4().to_string(),
                created_at: Utc::now().timestamp_millis() as u64,
                initial_size: dimensions,
                env,
                events: Vec::new(),
                duration: 0,
                title: title
                    .clone()
                    .unwrap_or_else(|| "Terminal Recording".to_string()),
            })
        } else {
            None
        };

        Ok(Self {
            active: false,
            format,
            output_path,
            writer: Some(writer),
            recording,
            start_time: std::time::Instant::now(),
            dimensions,
            title,
            redact_passwords: true, // Enabled by default for safety
            password_prompt_active: false,
            echo_suppressed: false,
            redaction_marker_emitted: false,
        })
    }

    /// Start logging.
    pub fn start(&mut self) -> Result<()> {
        if self.active {
            return Ok(());
        }

        self.active = true;
        self.start_time = std::time::Instant::now();

        // Write header for HTML format
        if self.format == SessionLogFormat::Html {
            self.write_html_header()?;
        }

        log::info!("Session logging started: {:?}", self.output_path);
        Ok(())
    }

    /// Stop logging and finalize the log file.
    pub fn stop(&mut self) -> Result<PathBuf> {
        if !self.active {
            return Ok(self.output_path.clone());
        }

        self.active = false;

        // Finalize based on format
        match self.format {
            SessionLogFormat::Plain => {
                // Nothing special needed
            }
            SessionLogFormat::Html => {
                self.write_html_footer()?;
            }
            SessionLogFormat::Asciicast => {
                self.write_asciicast()?;
            }
        }

        // Flush and close the writer
        if let Some(mut writer) = self.writer.take() {
            writer
                .flush()
                .with_context(|| format!("Failed to flush session log: {:?}", self.output_path))?;
        }

        log::info!("Session logging stopped: {:?}", self.output_path);
        Ok(self.output_path.clone())
    }

    /// Enable or disable password prompt detection and input redaction.
    ///
    /// When enabled, the logger monitors terminal output for common password
    /// prompt patterns and suppresses input recording during password entry.
    /// Enabled by default.
    pub fn set_redact_passwords(&mut self, enabled: bool) {
        self.redact_passwords = enabled;
    }

    /// Check whether password redaction is enabled.
    pub fn redact_passwords(&self) -> bool {
        self.redact_passwords
    }

    /// Externally signal that echo is suppressed (e.g., PTY echo off).
    ///
    /// When set to `true`, all input recording is suppressed regardless of
    /// prompt detection. This is useful when the caller has access to the
    /// terminal's echo mode state.
    ///
    /// Call with `false` when echo is re-enabled.
    pub fn set_echo_suppressed(&mut self, suppressed: bool) {
        if self.echo_suppressed != suppressed {
            self.echo_suppressed = suppressed;
            if !suppressed {
                // Echo re-enabled; reset the marker flag so a new redaction
                // period will emit a fresh marker.
                self.redaction_marker_emitted = false;
            }
        }
    }

    /// Check whether echo is externally marked as suppressed.
    pub fn echo_suppressed(&self) -> bool {
        self.echo_suppressed
    }

    /// Record output data from the terminal.
    pub fn record_output(&mut self, data: &[u8]) {
        if !self.active {
            return;
        }

        // Check for password prompts in the output if redaction is enabled
        if self.redact_passwords {
            self.check_for_password_prompt(data);
        }

        let elapsed = self.start_time.elapsed().as_millis() as u64;

        match self.format {
            SessionLogFormat::Plain => {
                // Strip ANSI escape sequences and write plain text
                let text = strip_ansi_escapes(data);
                if let Some(ref mut writer) = self.writer {
                    let _ = writer.write_all(text.as_bytes());
                }
            }
            SessionLogFormat::Html => {
                // Convert to HTML (basic escaping for now)
                let text = String::from_utf8_lossy(data);
                let escaped = html_escape(&text);
                if let Some(ref mut writer) = self.writer {
                    let _ = writer.write_all(escaped.as_bytes());
                }
            }
            SessionLogFormat::Asciicast => {
                // Add event to recording
                if let Some(ref mut recording) = self.recording {
                    recording.events.push(RecordingEvent {
                        timestamp: elapsed,
                        event_type: RecordingEventType::Output,
                        data: data.to_vec(),
                        metadata: None,
                    });
                    recording.duration = elapsed;
                }
            }
        }
    }

    /// Record input data (keyboard input).
    ///
    /// When password redaction is active (either via prompt detection or
    /// explicit echo suppression), input data is replaced with a redaction
    /// marker. A newline or carriage return in the input data signals the
    /// end of a password entry, clearing the prompt-detected suppression.
    pub fn record_input(&mut self, data: &[u8]) {
        if !self.active {
            return;
        }

        let is_suppressed = self.echo_suppressed || self.password_prompt_active;

        if is_suppressed && self.redact_passwords {
            // Check if input contains a newline (password entry complete)
            let has_newline = data.iter().any(|&b| b == b'\n' || b == b'\r');

            // Emit a single redaction marker per suppression period
            if !self.redaction_marker_emitted {
                self.emit_redaction_marker();
                self.redaction_marker_emitted = true;
            }

            if has_newline {
                // Password entry complete; clear prompt-based suppression.
                // (echo_suppressed is managed externally and not cleared here.)
                self.password_prompt_active = false;
                self.redaction_marker_emitted = false;
            }
            return;
        }

        // Only asciicast records input
        if self.format == SessionLogFormat::Asciicast {
            let elapsed = self.start_time.elapsed().as_millis() as u64;
            if let Some(ref mut recording) = self.recording {
                recording.events.push(RecordingEvent {
                    timestamp: elapsed,
                    event_type: RecordingEventType::Input,
                    data: data.to_vec(),
                    metadata: None,
                });
                recording.duration = elapsed;
            }
        }
    }

    /// Record a terminal resize event.
    pub fn record_resize(&mut self, cols: usize, rows: usize) {
        if !self.active {
            return;
        }

        self.dimensions = (cols, rows);

        // Only asciicast records resize events
        if self.format == SessionLogFormat::Asciicast {
            let elapsed = self.start_time.elapsed().as_millis() as u64;
            if let Some(ref mut recording) = self.recording {
                recording.events.push(RecordingEvent {
                    timestamp: elapsed,
                    event_type: RecordingEventType::Resize,
                    data: Vec::new(),
                    metadata: Some((cols, rows)),
                });
                recording.duration = elapsed;
            }
        }
    }

    /// Check if logging is active.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Get the output path.
    pub fn output_path(&self) -> &PathBuf {
        &self.output_path
    }

    /// Flush buffered data to disk.
    pub fn flush(&mut self) -> Result<()> {
        if let Some(ref mut writer) = self.writer {
            writer
                .flush()
                .with_context(|| format!("Failed to flush session log: {:?}", self.output_path))?;
        }
        Ok(())
    }

    // === Private helper methods ===

    /// Check terminal output for password prompt patterns.
    ///
    /// Examines the last line of the output data (after stripping ANSI escape
    /// sequences) for common password prompt patterns. If a match is found,
    /// sets `password_prompt_active` to suppress input recording.
    fn check_for_password_prompt(&mut self, data: &[u8]) {
        let text = strip_ansi_escapes(data);
        // Get the last non-empty line (prompts are typically the last thing printed)
        let last_line = text
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("")
            .to_ascii_lowercase();

        if last_line.is_empty() {
            return;
        }

        if PASSWORD_PROMPT_PATTERNS
            .iter()
            .any(|pattern| last_line.contains(pattern))
            && !self.password_prompt_active
        {
            self.password_prompt_active = true;
            self.redaction_marker_emitted = false;
        }
    }

    /// Emit a redaction marker into the recording/log.
    fn emit_redaction_marker(&mut self) {
        if self.format == SessionLogFormat::Asciicast {
            let elapsed = self.start_time.elapsed().as_millis() as u64;
            if let Some(ref mut recording) = self.recording {
                recording.events.push(RecordingEvent {
                    timestamp: elapsed,
                    event_type: RecordingEventType::Input,
                    data: REDACTION_MARKER.as_bytes().to_vec(),
                    metadata: None,
                });
                recording.duration = elapsed;
            }
        }
    }

    pub(super) fn write_html_header(&mut self) -> Result<()> {
        let header = format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>{}</title>
    <style>
        body {{
            background-color: #1e1e1e;
            color: #d4d4d4;
            font-family: 'JetBrains Mono', 'Fira Code', 'Consolas', monospace;
            font-size: 14px;
            padding: 20px;
            white-space: pre-wrap;
            word-wrap: break-word;
        }}
        .timestamp {{
            color: #808080;
            font-size: 10px;
        }}
    </style>
</head>
<body>
<pre>
"#,
            self.title.as_deref().unwrap_or("Terminal Session")
        );

        if let Some(ref mut writer) = self.writer {
            writer.write_all(header.as_bytes()).with_context(|| {
                format!("Failed to write HTML header to {:?}", self.output_path)
            })?;
        }
        Ok(())
    }

    pub(super) fn write_html_footer(&mut self) -> Result<()> {
        let footer = r#"
</pre>
</body>
</html>
"#;
        if let Some(ref mut writer) = self.writer {
            writer.write_all(footer.as_bytes()).with_context(|| {
                format!("Failed to write HTML footer to {:?}", self.output_path)
            })?;
        }
        Ok(())
    }

    pub(super) fn write_asciicast(&mut self) -> Result<()> {
        if let Some(ref recording) = self.recording {
            // Write asciicast v2 format
            // Header line (JSON object)
            let header = serde_json::json!({
                "version": 2,
                "width": recording.initial_size.0,
                "height": recording.initial_size.1,
                "timestamp": recording.created_at / 1000, // Convert to seconds
                "title": &recording.title,
                "env": recording.env,
            });

            if let Some(ref mut writer) = self.writer {
                writeln!(writer, "{}", header).with_context(|| {
                    format!("Failed to write asciicast header to {:?}", self.output_path)
                })?;

                // Event lines (JSON arrays)
                for event in &recording.events {
                    let time_seconds = event.timestamp as f64 / 1000.0;

                    match event.event_type {
                        RecordingEventType::Output => {
                            let data_str = String::from_utf8_lossy(&event.data);
                            let line = serde_json::json!([time_seconds, "o", data_str]);
                            writeln!(writer, "{}", line)?;
                        }
                        RecordingEventType::Input => {
                            let data_str = String::from_utf8_lossy(&event.data);
                            let line = serde_json::json!([time_seconds, "i", data_str]);
                            writeln!(writer, "{}", line)?;
                        }
                        RecordingEventType::Resize => {
                            if let Some((cols, rows)) = event.metadata {
                                let line = serde_json::json!([
                                    time_seconds,
                                    "r",
                                    format!("{}x{}", cols, rows)
                                ]);
                                writeln!(writer, "{}", line)?;
                            }
                        }
                        RecordingEventType::Marker => {
                            let label = String::from_utf8_lossy(&event.data);
                            let line = serde_json::json!([time_seconds, "m", label]);
                            writeln!(writer, "{}", line)?;
                        }
                        RecordingEventType::Metadata => {
                            // Metadata events store key-value pairs; emit as asciicast marker
                            let data_str = String::from_utf8_lossy(&event.data);
                            let line = serde_json::json!([time_seconds, "m", data_str]);
                            writeln!(writer, "{}", line)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

impl Drop for SessionLogger {
    fn drop(&mut self) {
        if self.active {
            let _ = self.stop();
        }
    }
}

/// Thread-safe wrapper for SessionLogger.
///
/// Uses `parking_lot::Mutex` because all access is from sync contexts (the winit
/// event loop and background std threads). The non-async, non-poisoning API of
/// `parking_lot` is sufficient and avoids the overhead of `tokio::sync::Mutex`.
pub type SharedSessionLogger = Arc<Mutex<Option<SessionLogger>>>;

/// Create a new shared session logger.
pub fn create_shared_logger() -> SharedSessionLogger {
    Arc::new(Mutex::new(None))
}
