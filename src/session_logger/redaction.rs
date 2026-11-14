//! Credential redaction for [`SessionLogger`] (SEC-006/SEC-009).
//!
//! Password-prompt detection, input redaction markers, and the
//! sensitive-output heuristics, split from `core.rs` to keep every file under
//! the 800-line gate. The constants are `pub(super)`; `tests.rs` asserts
//! against `REDACTION_MARKER` directly.

use crate::config::SessionLogFormat;
use crate::session_logger::writers::strip_ansi_escapes;
use par_term_emu_core_rust::terminal::{RecordingEvent, RecordingEventType};

use super::core::SessionLogger;

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
    "contrase\u{00f1}a:", // contraseña:
    "contrasena:",        // contrasena: (ASCII fallback)
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
    "\u{043f}\u{0430}\u{0440}\u{043e}\u{043b}\u{044c}:", // пароль:
    "\u{0432}\u{0432}\u{0435}\u{0434}\u{0438}\u{0442}\u{0435} \u{043f}\u{0430}\u{0440}\u{043e}\u{043b}\u{044c}", // введите пароль
    "\u{043d}\u{043e}\u{0432}\u{044b}\u{0439} \u{043f}\u{0430}\u{0440}\u{043e}\u{043b}\u{044c}:", // новый пароль:
    // German
    "passwort:",
    "geben sie das passwort",
    "passwort eingeben",
    "neues passwort:",
    "passwort best\u{00e4}tigen", // passwort bestätigen
    // Japanese
    "\u{30d1}\u{30b9}\u{30ef}\u{30fc}\u{30c9}:", // パスワード:
    "\u{30d1}\u{30b9}\u{30ef}\u{30fc}\u{30c9}\u{5165}\u{529b}", // パスワード入力
    // Korean
    "암호:",     // 암호:
    "비밀번호:", // 비밀번호:
    // Chinese (Simplified)
    "密码:",      // 密码:
    "请输入密码", // 请输入密码
    // Italian
    "password:", // (already covered by English "password:" — case-insensitive)
    "inserire la password",
    "nuova password:",
    // Dutch
    "wachtwoord:",
    "voer het wachtwoord",
    "nieuw wachtwoord:",
    // Polish
    "has\u{0142}o:",         // hasło:
    "wprowadź has\u{0142}o", // wprowadź hasło
    "nowe has\u{0142}o:",    // nowe hasło:
    // Turkish
    "ş}ifre:", // şifre: (note: also covered by lowercase match of "Şifre:")
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

impl SessionLogger {
    /// SEC-006: Check whether a single stripped line matches any sensitive
    /// output heuristic.
    ///
    /// Exposed as `pub(super)` for unit-test access from `tests.rs`.
    pub(super) fn line_is_sensitive(line: &str) -> bool {
        let lower = line.to_ascii_lowercase();
        SENSITIVE_OUTPUT_PATTERNS.iter().any(|p| lower.contains(p))
    }

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
    pub(super) fn filter_sensitive_output(&self, data: &[u8]) -> Vec<u8> {
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
    pub(super) fn check_for_password_prompt(&mut self, data: &[u8]) {
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
    pub(super) fn emit_redaction_marker(&mut self) {
        if matches!(
            self.format,
            SessionLogFormat::Asciicast | SessionLogFormat::AsciicastV3
        ) {
            let elapsed = self.start_time.elapsed().as_millis() as u64;
            if self.format == SessionLogFormat::Asciicast {
                self.record_asciicast_event(RecordingEvent {
                    timestamp: elapsed,
                    event_type: RecordingEventType::Input,
                    data: REDACTION_MARKER.as_bytes().to_vec(),
                    metadata: None,
                });
            } else if let Some(ref mut recording) = self.recording {
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
