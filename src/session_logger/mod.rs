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
//! # Heuristic Redaction Limitations
//!
//! The password-prompt detection is **heuristic** and relies on a fixed list of known
//! prompt strings (see `PASSWORD_PROMPT_PATTERNS` in `core`). It **cannot** guarantee
//! that all sensitive input is redacted. Scenarios where credentials may still be
//! captured include:
//!
//! - Custom or localised password prompts not in the pattern list
//! - Credentials pasted into the terminal (no echo-suppress signal)
//! - API keys or tokens typed as command arguments (e.g. `curl -H "Authorization: Bearer <token>"`)
//! - Multi-factor authentication codes entered outside of recognised prompt patterns
//! - Applications that suppress echo without emitting a matching prompt string
//!
//! **Recommendation**: If you regularly work with sensitive credentials in the terminal
//! (vault access, production API keys, SSH passphrases, etc.), **disable session logging**
//! for those sessions. Session logging can be toggled per-profile or globally via the
//! par-term settings. Do not rely solely on redaction as a security control.
//!
//! [`set_redact_passwords`]: core::SessionLogger::set_redact_passwords
//! [`set_echo_suppressed`]: core::SessionLogger::set_echo_suppressed

pub mod core;
#[cfg(test)]
mod tests;
mod writers;

pub use core::{SessionLogger, SharedSessionLogger, create_shared_logger};
pub use writers::contains_password_prompt;
