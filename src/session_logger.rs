//! Session logging and recording for terminal sessions.
//!
//! This module provides automatic session logging with support for multiple formats:
//! - Plain text: Simple output without escape sequences
//! - HTML: Rendered output with colors preserved
//! - Asciicast: asciinema-compatible format for replay/sharing
//!
//! # Security: Sensitive Data Filtering
//!
//! Session logs capture raw terminal I/O, which may include passwords and other
//! credentials typed at prompts (sudo, ssh, gpg, etc.). To mitigate this risk,
//! the logger supports **password prompt detection** via [`set_redact_passwords`]:
//!
//! When enabled, the logger monitors terminal output for common password prompt
//! patterns. When a prompt is detected, subsequent keyboard input is replaced
//! with `[INPUT REDACTED - echo off]` until the user presses Enter (completing
//! the password entry). This is a heuristic approach and cannot guarantee
//! detection of all sensitive input scenarios.
//!
//! Additionally, callers can explicitly signal that echo is suppressed (e.g.,
//! because the PTY has disabled echo for a password prompt) by calling
//! [`set_echo_suppressed`]. This provides a second layer of protection when
//! terminal mode information is available.
//!
//! **WARNING**: Even with password redaction enabled, session logs may still
//! contain sensitive data (API keys pasted into the terminal, tokens in command
//! arguments, etc.). Users should treat session log files as potentially
//! sensitive and store them accordingly.
//!
//! [`set_redact_passwords`]: SessionLogger::set_redact_passwords
//! [`set_echo_suppressed`]: SessionLogger::set_echo_suppressed

use crate::config::SessionLogFormat;
use anyhow::Result;
use chrono::{Local, Utc};
use par_term_emu_core_rust::terminal::{RecordingEvent, RecordingEventType, RecordingSession};
use parking_lot::Mutex;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Marker text written to the log when input is redacted during a password prompt.
const REDACTION_MARKER: &str = "[INPUT REDACTED - echo off]";

/// Common password prompt patterns (case-insensitive matching).
///
/// These patterns are matched against terminal output (after stripping ANSI
/// escape sequences) to detect when the user is being asked for a password.
/// The match is performed on the last line of each output chunk.
const PASSWORD_PROMPT_PATTERNS: &[&str] = &[
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
/// See the [module-level documentation](self) for information about
/// sensitive data filtering.
pub struct SessionLogger {
    /// Whether logging is currently active
    active: bool,
    /// The log format to use
    format: SessionLogFormat,
    /// Output file path
    output_path: PathBuf,
    /// Buffered writer for the log file
    writer: Option<BufWriter<File>>,
    /// Recording session data (for asciicast format)
    recording: Option<RecordingSession>,
    /// Recording start time (for relative timestamps)
    start_time: std::time::Instant,
    /// Terminal dimensions
    dimensions: (usize, usize),
    /// Session title
    title: Option<String>,
    /// Whether password redaction is enabled (heuristic prompt detection)
    redact_passwords: bool,
    /// Whether the logger has detected a password prompt in recent output
    /// and is currently suppressing input recording.
    password_prompt_active: bool,
    /// Whether echo is externally known to be suppressed (e.g., PTY echo off).
    /// When true, input is always redacted regardless of prompt detection.
    echo_suppressed: bool,
    /// Whether a redaction marker has already been emitted for the current
    /// suppression period (to avoid flooding the log with repeated markers).
    redaction_marker_emitted: bool,
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

        // Create the log file
        let file = File::create(&output_path)?;
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
            writer.flush()?;
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
            writer.flush()?;
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

    fn write_html_header(&mut self) -> Result<()> {
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
            writer.write_all(header.as_bytes())?;
        }
        Ok(())
    }

    fn write_html_footer(&mut self) -> Result<()> {
        let footer = r#"
</pre>
</body>
</html>
"#;
        if let Some(ref mut writer) = self.writer {
            writer.write_all(footer.as_bytes())?;
        }
        Ok(())
    }

    fn write_asciicast(&mut self) -> Result<()> {
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
                writeln!(writer, "{}", header)?;

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

/// Thread-safe wrapper for SessionLogger
pub type SharedSessionLogger = Arc<Mutex<Option<SessionLogger>>>;

/// Create a new shared session logger
pub fn create_shared_logger() -> SharedSessionLogger {
    Arc::new(Mutex::new(None))
}

// === Helper functions ===

/// Strip ANSI escape sequences from text
fn strip_ansi_escapes(data: &[u8]) -> String {
    let text = String::from_utf8_lossy(data);
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // ESC sequence - skip until we hit the terminator
            if let Some(&next) = chars.peek() {
                if next == '[' {
                    // CSI sequence - skip until we hit a letter
                    chars.next(); // consume '['
                    while let Some(&c) = chars.peek() {
                        chars.next();
                        if c.is_ascii_alphabetic() || c == '@' || c == '`' {
                            break;
                        }
                    }
                } else if next == ']' {
                    // OSC sequence - skip until BEL or ST
                    chars.next(); // consume ']'
                    while let Some(c) = chars.next() {
                        if c == '\x07' {
                            break;
                        }
                        if c == '\x1b'
                            && let Some(&'\\') = chars.peek()
                        {
                            chars.next();
                            break;
                        }
                    }
                } else if next == '(' || next == ')' || next == '*' || next == '+' {
                    // Character set designation - skip one more char
                    chars.next();
                    chars.next();
                } else {
                    // Other ESC sequence - skip one char
                    chars.next();
                }
            }
        } else {
            result.push(c);
        }
    }

    result
}

/// Escape HTML special characters
fn html_escape(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '&' => result.push_str("&amp;"),
            '"' => result.push_str("&quot;"),
            '\'' => result.push_str("&#39;"),
            _ => result.push(c),
        }
    }
    result
}

/// Check if the given output text contains a password prompt pattern.
///
/// This is exposed for testing purposes. The check is case-insensitive
/// and matches against the last non-empty line of the output.
pub fn contains_password_prompt(output: &str) -> bool {
    let last_line = output
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("")
        .to_ascii_lowercase();

    if last_line.is_empty() {
        return false;
    }

    PASSWORD_PROMPT_PATTERNS
        .iter()
        .any(|pattern| last_line.contains(pattern))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_strip_ansi_escapes() {
        // Simple text
        assert_eq!(strip_ansi_escapes(b"hello world"), "hello world");

        // CSI sequence (color)
        assert_eq!(strip_ansi_escapes(b"\x1b[32mgreen\x1b[0m"), "green");

        // OSC sequence (title)
        assert_eq!(strip_ansi_escapes(b"\x1b]0;title\x07text"), "text");

        // Multiple sequences
        assert_eq!(
            strip_ansi_escapes(b"\x1b[1;32mBold Green\x1b[0m Normal"),
            "Bold Green Normal"
        );
    }

    #[test]
    fn test_html_escape() {
        assert_eq!(html_escape("<script>"), "&lt;script&gt;");
        assert_eq!(html_escape("a & b"), "a &amp; b");
        assert_eq!(html_escape("\"quoted\""), "&quot;quoted&quot;");
    }

    #[test]
    fn test_session_logger_plain() {
        let temp_dir = TempDir::new().unwrap();
        let mut logger = SessionLogger::new(
            SessionLogFormat::Plain,
            temp_dir.path(),
            (80, 24),
            Some("Test Session".to_string()),
        )
        .unwrap();

        logger.start().unwrap();
        logger.record_output(b"Hello, World!\n");
        logger.record_output(b"\x1b[32mGreen text\x1b[0m\n");
        let path = logger.stop().unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("Hello, World!"));
        assert!(content.contains("Green text"));
        assert!(!content.contains("\x1b")); // No escape sequences
    }

    #[test]
    fn test_session_logger_asciicast() {
        let temp_dir = TempDir::new().unwrap();
        let mut logger = SessionLogger::new(
            SessionLogFormat::Asciicast,
            temp_dir.path(),
            (80, 24),
            Some("Test Session".to_string()),
        )
        .unwrap();

        logger.start().unwrap();
        logger.record_output(b"Hello\n");
        std::thread::sleep(std::time::Duration::from_millis(10));
        logger.record_output(b"World\n");
        let path = logger.stop().unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();

        // First line should be header
        assert!(lines[0].contains("\"version\":2"));
        assert!(lines[0].contains("\"width\":80"));
        assert!(lines[0].contains("\"height\":24"));

        // Should have output events
        assert!(lines.len() >= 3);
    }

    #[test]
    fn test_password_prompt_detection() {
        assert!(contains_password_prompt("Password:"));
        assert!(contains_password_prompt("[sudo] password for user:"));
        assert!(contains_password_prompt("Enter passphrase for key:"));
        assert!(contains_password_prompt("Enter PIN:"));
        assert!(contains_password_prompt("some output\nPassword:"));
        assert!(contains_password_prompt("Verification code:"));
        assert!(contains_password_prompt("(current) UNIX password:"));
        // Case insensitive
        assert!(contains_password_prompt("PASSWORD:"));
        assert!(contains_password_prompt("Enter Password:"));
        // Should not match normal output
        assert!(!contains_password_prompt("user@host:~$"));
        assert!(!contains_password_prompt("Hello, World!"));
        assert!(!contains_password_prompt("ls -la"));
    }

    #[test]
    fn test_password_prompt_with_ansi_escapes() {
        // Password prompt with ANSI color codes
        let data = b"\x1b[1;31m[sudo] password for user:\x1b[0m ";
        let stripped = strip_ansi_escapes(data);
        assert!(contains_password_prompt(&stripped));
    }

    #[test]
    fn test_input_redaction_on_password_prompt() {
        let temp_dir = TempDir::new().unwrap();
        let mut logger = SessionLogger::new(
            SessionLogFormat::Asciicast,
            temp_dir.path(),
            (80, 24),
            Some("Test Session".to_string()),
        )
        .unwrap();

        logger.start().unwrap();

        // Normal output and input (should be recorded)
        logger.record_output(b"user@host:~$ ");
        logger.record_input(b"sudo ls\r");

        // Password prompt output
        logger.record_output(b"[sudo] password for user: ");

        // Password input (should be redacted)
        logger.record_input(b"s");
        logger.record_input(b"e");
        logger.record_input(b"c");
        logger.record_input(b"r");
        logger.record_input(b"e");
        logger.record_input(b"t");
        // Enter completes the password
        logger.record_input(b"\r");

        // Normal output after password
        logger.record_output(b"\nfile1  file2  file3\n");
        logger.record_input(b"echo done\r");

        let path = logger.stop().unwrap();

        let content = std::fs::read_to_string(&path).unwrap();

        // The actual password ("secret") should NOT appear in the log
        // Check that no input event contains the password characters individually
        assert!(
            !content.contains("secret"),
            "Password text 'secret' should not appear in the log"
        );
        // The redaction marker SHOULD appear
        assert!(
            content.contains(REDACTION_MARKER),
            "Redaction marker should appear in the log"
        );
        // Normal input should still appear
        assert!(
            content.contains("sudo ls"),
            "Normal input before prompt should be recorded"
        );
        assert!(
            content.contains("echo done"),
            "Normal input after password should be recorded"
        );
    }

    #[test]
    fn test_input_redaction_via_echo_suppressed() {
        let temp_dir = TempDir::new().unwrap();
        let mut logger = SessionLogger::new(
            SessionLogFormat::Asciicast,
            temp_dir.path(),
            (80, 24),
            Some("Test Session".to_string()),
        )
        .unwrap();

        logger.start().unwrap();

        // Normal input
        logger.record_input(b"ls\r");

        // Externally signal echo off
        logger.set_echo_suppressed(true);
        logger.record_input(b"mysecret");
        logger.record_input(b"\r");

        // Echo back on
        logger.set_echo_suppressed(false);
        logger.record_input(b"whoami\r");

        let path = logger.stop().unwrap();

        let content = std::fs::read_to_string(&path).unwrap();

        assert!(!content.contains("mysecret"), "Secret should not appear");
        assert!(
            content.contains(REDACTION_MARKER),
            "Redaction marker should appear"
        );
        assert!(content.contains("whoami"), "Normal input should appear");
    }

    #[test]
    fn test_redaction_disabled() {
        let temp_dir = TempDir::new().unwrap();
        let mut logger = SessionLogger::new(
            SessionLogFormat::Asciicast,
            temp_dir.path(),
            (80, 24),
            Some("Test Session".to_string()),
        )
        .unwrap();

        logger.set_redact_passwords(false);
        logger.start().unwrap();

        // Even with a password prompt, input should NOT be redacted
        logger.record_output(b"Password: ");
        logger.record_input(b"mysecret\r");

        let path = logger.stop().unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        // With redaction disabled, the password IS recorded (user's choice)
        assert!(
            content.contains("mysecret"),
            "With redaction disabled, input should be recorded"
        );
        assert!(
            !content.contains(REDACTION_MARKER),
            "No redaction marker when disabled"
        );
    }

    #[test]
    fn test_redaction_marker_emitted_once_per_period() {
        let temp_dir = TempDir::new().unwrap();
        let mut logger = SessionLogger::new(
            SessionLogFormat::Asciicast,
            temp_dir.path(),
            (80, 24),
            Some("Test Session".to_string()),
        )
        .unwrap();

        logger.start().unwrap();
        logger.record_output(b"Password: ");

        // Multiple input events during password entry
        logger.record_input(b"a");
        logger.record_input(b"b");
        logger.record_input(b"c");
        logger.record_input(b"\r");

        let path = logger.stop().unwrap();

        let content = std::fs::read_to_string(&path).unwrap();

        // Count occurrences of the redaction marker
        let marker_count = content.matches(REDACTION_MARKER).count();
        assert_eq!(
            marker_count, 1,
            "Redaction marker should appear exactly once per password entry"
        );
    }
}
