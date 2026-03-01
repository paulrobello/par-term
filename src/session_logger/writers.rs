//! Format-specific writers and text-processing helpers for session logging.
//!
//! This module contains:
//! - [`strip_ansi_escapes`]: Remove VT/ANSI escape sequences from raw PTY output.
//! - [`html_escape`]: Escape HTML special characters for the HTML log format.
//! - [`contains_password_prompt`]: Heuristic check for password-prompt strings.

use crate::session_logger::core::PASSWORD_PROMPT_PATTERNS;

/// Strip ANSI escape sequences from raw PTY output.
///
/// Handles CSI sequences (`ESC [`), OSC sequences (`ESC ]`), and single-character
/// escape sequences. The result is plain UTF-8 text suitable for plain-text logging
/// and password-prompt detection.
pub(super) fn strip_ansi_escapes(data: &[u8]) -> String {
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

/// Escape HTML special characters for safe embedding in HTML documents.
pub(super) fn html_escape(text: &str) -> String {
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
