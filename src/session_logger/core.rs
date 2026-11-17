//! Core [`SessionLogger`] struct, state management, and recording methods.
//!
//! See the [module-level documentation](super) for information about security
//! considerations and password redaction.
//!
//! Format-specific finalization (HTML headers/footers, asciicast serialization)
//! lives in `super::format_writers`.

use crate::config::SessionLogFormat;
use crate::session_logger::writers::{html_escape, strip_ansi_escapes};
use anyhow::{Context, Result};
use jiff::Timestamp;
use par_term_emu_core_rust::terminal::{RecordingEvent, RecordingEventType, RecordingSession};
use parking_lot::Mutex;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Serializes a recording through the terminal's graphics-aware v3 export;
/// `None` when the terminal is unavailable, which finalization treats as
/// text-only v3.
pub(super) type V3Exporter = Box<dyn Fn(&RecordingSession) -> Option<String> + Send>;

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
    /// First write error seen since logging started, if any.
    ///
    /// A transcript that stops growing mid-session (disk full, log file deleted)
    /// is indistinguishable from an idle one, so the failure is latched here and
    /// reported through [`SessionLogger::is_active`] rather than discarded.
    pub(super) write_error: Option<String>,
    /// Whether the logger has detected a password prompt in recent output
    /// and is currently suppressing input recording.
    pub(super) password_prompt_active: bool,
    /// Whether echo is externally known to be suppressed (e.g., PTY echo off).
    /// When true, input is always redacted regardless of prompt detection.
    pub(super) echo_suppressed: bool,
    /// Whether a redaction marker has already been emitted for the current
    /// suppression period (to avoid flooding the log with repeated markers).
    pub(super) redaction_marker_emitted: bool,
    /// Graphics-aware v3 exporter wired by the owning Tab: serializes the
    /// recording through the terminal (whose graphics store is crate-internal
    /// to emu-core); returns None when the terminal is unavailable, which
    /// finalization treats as text-only v3.
    pub(super) v3_exporter: Option<V3Exporter>,
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
        let timestamp = jiff::ZonedDateTime::now()
            .strftime("%Y%m%d_%H%M%S")
            .to_string();
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
        let file = opts.open(&output_path).with_context(|| {
            format!("Failed to create output recording file: {:?}", output_path)
        })?;
        let writer = BufWriter::with_capacity(8192, file); // 8KB buffer

        // Initialize recording session for asciicast formats
        let recording = if matches!(
            format,
            SessionLogFormat::Asciicast | SessionLogFormat::AsciicastV3
        ) {
            let mut env = std::collections::HashMap::new();
            env.insert("TERM".to_string(), "xterm-256color".to_string());
            env.insert("COLS".to_string(), dimensions.0.to_string());
            env.insert("ROWS".to_string(), dimensions.1.to_string());

            Some(RecordingSession {
                id: uuid::Uuid::new_v4().to_string(),
                created_at: Timestamp::now().as_millisecond() as u64,
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
            write_error: None,
            password_prompt_active: false,
            echo_suppressed: false,
            redaction_marker_emitted: false,
            v3_exporter: None,
        })
    }

    /// Start logging.
    pub fn start(&mut self) -> Result<()> {
        if self.active {
            return Ok(());
        }

        // A logger stopped by a write failure keeps the same unwritable file
        // handle, so restarting it would fail again on the next byte. Surface the
        // original error instead of pretending the restart worked.
        if let Some(ref e) = self.write_error {
            return Err(anyhow::anyhow!(
                "output recording file is not writable: {}",
                e
            ));
        }

        self.active = true;
        self.start_time = std::time::Instant::now();

        // Re-anchor the recording's epoch start to the same moment the
        // monotonic event timeline starts: the v3 exporter derives graphic
        // times as added_at - created_at, so keeping the construction stamp
        // would offset every graphic by the construct-to-start gap.
        if let Some(ref mut recording) = self.recording {
            recording.created_at = Timestamp::now().as_millisecond() as u64;
        }

        // Write format-specific header / startup comment.
        match self.format {
            SessionLogFormat::Html => {
                self.write_html_header()?;
            }
            SessionLogFormat::Plain => {
                // SEC-004: Write a startup warning so readers of the log file are
                // aware of the redaction limitations. This comment appears at the
                // top of every plain-text session log.
                self.write_plain_redaction_warning()?;
            }
            SessionLogFormat::Asciicast => {
                // Streaming v2: the header lands on disk at start so a crash,
                // a kill, or a stop-less teardown (shutdown_fast transfer
                // drop) still leaves a parseable transcript instead of a
                // 0-byte file. A logger restarted after a stop (Cmd+Shift+R
                // toggle) lost its writer to stop()'s take(); reopen it.
                if self.writer.is_none() {
                    self.reopen_writer()?;
                }
                self.write_asciicast_header()?;
            }
            SessionLogFormat::AsciicastV3 => {
                // v3 is a single JSON document assembled at finalization: the
                // graphics exporter serializes the whole session in one pass.
            }
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

        // Finalize based on format (delegates to format_writers module)
        super::format_writers::finalize_format(self)?;

        // Flush and close the writer
        if let Some(mut writer) = self.writer.take() {
            writer.flush().with_context(|| {
                format!("Failed to flush output recording: {:?}", self.output_path)
            })?;
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

    /// Wire the graphics-aware v3 exporter used at finalization (see
    /// `write_asciicast_v3`). Harmless to set on non-v3 formats: the exporter
    /// is only consulted when a v3 recording is serialized. Returning None
    /// from the exporter falls back to text-only v3.
    pub fn set_v3_exporter(&mut self, exporter: V3Exporter) {
        self.v3_exporter = Some(exporter);
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

        // Check for password prompts in the output if redaction is enabled.
        // This must run before filtering so that subsequent input is suppressed.
        if self.redact_passwords {
            self.check_for_password_prompt(data);
        }

        // SEC-006: Check for sensitive credential output patterns line-by-line.
        // Lines that match known sensitive-data heuristics are replaced with a
        // redaction annotation before writing to the log.
        let data_to_write: Vec<u8> = if self.redact_passwords {
            self.filter_sensitive_output(data)
        } else {
            data.to_vec()
        };
        let data = data_to_write.as_slice();

        let elapsed = self.start_time.elapsed().as_millis() as u64;

        match self.format {
            SessionLogFormat::Plain => {
                // Strip ANSI escape sequences and write plain text
                let text = strip_ansi_escapes(data);
                self.write_bytes(text.as_bytes());
            }
            SessionLogFormat::Html => {
                // Convert to HTML (basic escaping for now)
                let text = String::from_utf8_lossy(data);
                let escaped = html_escape(&text);
                self.write_bytes(escaped.as_bytes());
            }
            SessionLogFormat::Asciicast => {
                // Streaming v2: append the event line now instead of
                // buffering it for finalization, so the transcript survives
                // an unclean end.
                self.record_asciicast_event(RecordingEvent {
                    timestamp: elapsed,
                    event_type: RecordingEventType::Output,
                    data: data.to_vec(),
                    metadata: None,
                });
            }
            SessionLogFormat::AsciicastV3 => {
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
        if matches!(
            self.format,
            SessionLogFormat::Asciicast | SessionLogFormat::AsciicastV3
        ) {
            let elapsed = self.start_time.elapsed().as_millis() as u64;
            if self.format == SessionLogFormat::Asciicast {
                self.record_asciicast_event(RecordingEvent {
                    timestamp: elapsed,
                    event_type: RecordingEventType::Input,
                    data: data.to_vec(),
                    metadata: None,
                });
            } else if let Some(ref mut recording) = self.recording {
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
        if matches!(
            self.format,
            SessionLogFormat::Asciicast | SessionLogFormat::AsciicastV3
        ) {
            let elapsed = self.start_time.elapsed().as_millis() as u64;
            if self.format == SessionLogFormat::Asciicast {
                self.record_asciicast_event(RecordingEvent {
                    timestamp: elapsed,
                    event_type: RecordingEventType::Resize,
                    data: Vec::new(),
                    metadata: Some((cols, rows)),
                });
            } else if let Some(ref mut recording) = self.recording {
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
    ///
    /// A write failure clears `active`, so this reflects writer health and not
    /// just the user's last toggle: a transcript that stopped growing mid-session
    /// must not keep reporting itself as being recorded.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// The write failure that stopped logging, if any.
    pub fn write_error(&self) -> Option<&str> {
        self.write_error.as_deref()
    }

    /// Reopen the transcript file in append mode after `stop()` released the
    /// writer (toggle off→on, Cmd+Shift+R twice). Without this a restarted
    /// logger has no file handle at all: events land in a recording that no
    /// finalization can ever flush, and the file stays at its first-stop size.
    fn reopen_writer(&mut self) -> Result<()> {
        let mut opts = OpenOptions::new();
        opts.append(true).create(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let file = opts.open(&self.output_path).with_context(|| {
            format!(
                "Failed to reopen output recording file: {:?}",
                self.output_path
            )
        })?;
        self.writer = Some(BufWriter::with_capacity(8192, file));
        Ok(())
    }

    /// Append an event to the streaming v2 transcript, latching a write
    /// failure the way `write_bytes` does so a broken file stops the logger
    /// instead of silently dropping the rest of the session.
    pub(super) fn record_asciicast_event(&mut self, event: RecordingEvent) {
        if let Err(e) = self.write_asciicast_event(&event) {
            let msg = e.to_string();
            log::error!(
                "Session log write failed for {:?}: {} — logging stopped",
                self.output_path,
                msg
            );
            self.write_error = Some(msg);
            self.active = false;
        }
    }

    /// Write to the log file, stopping logging on the first failure.
    ///
    /// Logging is not retried: once the file is gone or the disk is full every
    /// subsequent write fails too, and the transcript already has a hole in it.
    fn write_bytes(&mut self, bytes: &[u8]) {
        let Some(ref mut writer) = self.writer else {
            return;
        };
        if let Err(e) = writer.write_all(bytes) {
            let msg = e.to_string();
            log::error!(
                "Session log write failed for {:?}: {} — logging stopped",
                self.output_path,
                msg
            );
            self.write_error = Some(msg);
            self.active = false;
        }
    }

    /// Get the output path.
    pub fn output_path(&self) -> &PathBuf {
        &self.output_path
    }

    /// Flush buffered data to disk.
    pub fn flush(&mut self) -> Result<()> {
        if let Some(ref mut writer) = self.writer {
            writer.flush().with_context(|| {
                format!("Failed to flush output recording: {:?}", self.output_path)
            })?;
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
