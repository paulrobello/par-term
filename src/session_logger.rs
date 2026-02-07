//! Session logging and recording for terminal sessions.
//!
//! This module provides automatic session logging with support for multiple formats:
//! - Plain text: Simple output without escape sequences
//! - HTML: Rendered output with colors preserved
//! - Asciicast: asciinema-compatible format for replay/sharing

use crate::config::SessionLogFormat;
use anyhow::Result;
use chrono::{Local, Utc};
use par_term_emu_core_rust::terminal::{RecordingEvent, RecordingEventType, RecordingSession};
use parking_lot::Mutex;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Session logger that records terminal output to files.
///
/// The logger captures PTY output with timestamps and can export
/// to multiple formats. It uses buffered writes for performance.
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
                start_time: Utc::now().timestamp_millis() as u64,
                initial_size: dimensions,
                env,
                events: Vec::new(),
                duration: 0,
                title: title.clone(),
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

    /// Record output data from the terminal.
    pub fn record_output(&mut self, data: &[u8]) {
        if !self.active {
            return;
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
    pub fn record_input(&mut self, data: &[u8]) {
        if !self.active {
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
                "timestamp": recording.start_time / 1000, // Convert to seconds
                "title": recording.title.as_deref().unwrap_or("Terminal Recording"),
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
}
