//! Core [`SessionLogger`] struct, state management, and recording methods.
//!
//! See the [module-level documentation](super) for information about security
//! considerations and password redaction.
//!
//! Format-specific finalization (HTML headers/footers, asciicast serialization)
//! lives in [`super::format_writers`].

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
///
/// SEC-009: Includes patterns for common non-English password prompts to reduce
/// redaction gaps for internationalized systems and multilingual users.
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
    // Two-factor / MFA prompts
    "authenticator code:",
    "2fa code:",
    "otp:",
    "one-time password:",
    "one time password:",
    "security code:",
    "totp:",
    // API key / secret prompts (interactive tools that prompt for credentials)
    "api key:",
    "api secret:",
    "secret key:",
    "access key:",
    "access token:",
    "secret token:",
    "private key:",
    "client secret:",
    "auth token:",
    "bearer token:",
    // SSH / GPG passphrases
    "enter passphrase for key",
    "key passphrase:",
    "gpg passphrase:",
    // Database / service credential prompts
    "db password:",
    "database password:",
    "mysql password:",
    "postgres password:",
    "redis password:",
    // Cloud / vault prompts
    "vault token:",
    "vault password:",
    "aws secret",
    "azure secret",
    // SEC-009: Non-English password prompt patterns.
    // Covers Portuguese, Spanish, French, Russian, German, Japanese, Korean,
    // Chinese, Italian, Dutch, Polish, Turkish, Hindi, Arabic, and Hebrew.
    // All matched case-insensitively.
    // Portuguese
    "senha:",
    "digite a senha",
    "informe a senha",
    "senha atual:",
    "nova senha:",
    "confirme a senha",
    // Spanish
    "contrase\u{00f1}a:",      // contraseña:
    "contrasena:",              // contrasena: (ASCII fallback)
    "introduzca la contrase\u{00f1}a",
    "contrase\u{00f1}a actual:",
    "nueva contrase\u{00f1}a:",
    "confirme la contrase\u{00f1}a",
    // French
    "mot de passe:",
    "entrez le mot de passe",
    "mot de passe actuel:",
    "nouveau mot de passe:",
    "confirmez le mot de passe",
    // Russian
    "\u{043f}\u{0430}\u{0440}\u{043e}\u{043b}\u{044c}:",                 // пароль:
    "\u{0432}\u{0432}\u{0435}\u{0434}\u{0438}\u{0442}\u{0435} \u{043f}\u{0430}\u{0440}\u{043e}\u{043b}\u{044c}", // введите пароль
    "\u{043d}\u{043e}\u{0432}\u{044b}\u{0439} \u{043f}\u{0430}\u{0440}\u{043e}\u{043b}\u{044c}:",               // новый пароль:
    // German
    "passwort:",
    "geben sie das passwort",
    "passwort eingeben",
    "neues passwort:",
    "passwort best\u{00e4}tigen", // passwort bestätigen
    // Japanese
    "\u{30d1}\u{30b9}\u{30ef}\u{30fc}\u{30c9}:",     // パスワード:
    "\u{30d1}\u{30b9}\u{30ef}\u{30fc}\u{30c9}\u{5165}\u{529b}", // パスワード入力
    // Korean
    "암호:",       // 암호:
    "비밀번호:", // 비밀번호:
    // Chinese (Simplified)
    "密码:",       // 密码:
    "请输入密码", // 请输入密码
    // Italian
    "password:",           // (already covered by English "password:" — case-insensitive)
    "inserire la password",
    "nuova password:",
    // Dutch
    "wachtwoord:",
    "voer het wachtwoord",
    "nieuw wachtwoord:",
    // Polish
    "has\u{0142}o:",       // hasło:
    "wprowadź has\u{0142}o", // wprowadź hasło
    "nowe has\u{0142}o:",  // nowe hasło:
    // Turkish
    "ş}ifre:",        // şifre: (note: also covered by lowercase match of "Şifre:")
    "parola:",
    // Hindi
    "प}ासवर्द}:", // पासवर्ड: (Hindi often uses English loanword)
    // Arabic
    "ك}لمة ا}لمرور}:", // كلمة المرور:
    // Hebrew
    "ס}יסמא}:", // סיסמא:
];

/// Sensitive output line heuristics (case-insensitive substring matching).
///
/// SEC-006: These patterns detect terminal *output* lines that are likely to
/// contain sensitive values being printed (e.g. `export API_KEY=abc123`,
/// `aws_secret_access_key = ...`). When a line matches, the logger emits a
/// warning comment in the log rather than the raw content.
///
/// Unlike password prompt detection (which suppresses subsequent *input*),
/// these patterns trigger on *output* data — they redact or annotate the
/// output line itself.
///
/// # Limitations
///
/// This is a heuristic. It cannot catch all forms of sensitive output (e.g.
/// values printed without a recognisable key name, or base64-encoded secrets).
/// Users who regularly work with credentials in the terminal should disable
/// session logging for those sessions.
pub(super) const SENSITIVE_OUTPUT_PATTERNS: &[&str] = &[
    // Shell variable export of credential-like names
    "export aws_access_key",
    "export aws_secret",
    "export api_key",
    "export api_secret",
    "export auth_token",
    "export access_token",
    "export secret_key",
    "export private_key",
    "export client_secret",
    "export database_url",
    "export db_password",
    "export gh_token",
    "export github_token",
    "export gitlab_token",
    "export npm_token",
    "export pypi_token",
    // AWS credential file / env output patterns
    "aws_access_key_id",
    "aws_secret_access_key",
    "aws_session_token",
    // Generic key=value patterns for common secret variable names
    "api_key=",
    "apikey=",
    "api_secret=",
    "access_token=",
    "auth_token=",
    "secret_key=",
    "private_key=",
    "client_secret=",
    // CI/CD and hosting service tokens
    "github_token=",
    "gh_token=",
    "heroku_api_key=",
    "heroku_api_token=",
    "npm_token=",
    "pypi_token=",
    "gitlab_token=",
    "circleci_token=",
    // HTTP Authorization Bearer tokens (e.g. from curl -v output or httpie)
    "bearer ",
    // .env file content echoed to terminal
    "dotenv",
    // Token/key output from CLI tools
    "-----begin rsa private key-----",
    "-----begin ec private key-----",
    "-----begin openssh private key-----",
    "-----begin pgp private key block-----",
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
                // Asciicast format: the header is written during finalization.
                // No startup banner is added here; warnings are in the log file
                // at the application level via log::warn!.
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

    /// SEC-006: Check whether a single stripped line matches any sensitive
    /// output heuristic.
    ///
    /// Exposed as `pub(super)` for unit-test access from `tests.rs`.
    pub(super) fn line_is_sensitive(line: &str) -> bool {
        let lower = line.to_ascii_lowercase();
        SENSITIVE_OUTPUT_PATTERNS.iter().any(|p| lower.contains(p))
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

    /// SEC-006: Filter sensitive credential patterns from raw PTY output.
    ///
    /// Each line in `data` is stripped of ANSI escapes and checked against
    /// [`SENSITIVE_OUTPUT_PATTERNS`]. Lines that match are replaced with a
    /// `[OUTPUT REDACTED - sensitive data heuristic]` marker in the returned
    /// byte vector. Non-matching lines are passed through unchanged (using the
    /// *original* bytes, including ANSI codes, so that HTML/asciicast formats
    /// preserve colour).
    ///
    /// The check operates on ANSI-stripped content (case-insensitive) to
    /// avoid coloured prompt text defeating the heuristic.
    fn filter_sensitive_output(&self, data: &[u8]) -> Vec<u8> {
        const SENSITIVE_OUTPUT_MARKER: &str =
            "[OUTPUT REDACTED - sensitive data heuristic matched]\n";

        // Fast path: strip ANSI from the whole chunk and check if any line
        // could be sensitive. If not, return the original bytes unchanged.
        let stripped = strip_ansi_escapes(data);
        let any_sensitive = stripped.lines().any(Self::line_is_sensitive);
        if !any_sensitive {
            return data.to_vec();
        }

        // Slow path: reconstruct the output, replacing sensitive lines.
        // We work on the ANSI-stripped text for the per-line decision but
        // need to write back sanitised content; since the raw bytes may have
        // multi-byte ANSI sequences that don't align 1:1 with stripped lines,
        // we rebuild the output from the stripped text (forgoing colour for
        // the filtered chunk — acceptable given the security trade-off).
        let mut result = Vec::with_capacity(data.len());
        for line in stripped.lines() {
            if Self::line_is_sensitive(line) {
                result.extend_from_slice(SENSITIVE_OUTPUT_MARKER.as_bytes());
            } else {
                result.extend_from_slice(line.as_bytes());
                result.push(b'\n');
            }
        }
        // Preserve trailing newline status from original stripped text.
        if !stripped.ends_with('\n') && result.last() == Some(&b'\n') {
            result.pop();
        }
        result
    }

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
