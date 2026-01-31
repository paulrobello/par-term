use std::char;
use std::iter::Peekable;
use std::str::Chars;

/// Convert escape sequences in the configured initial text into their literal values.
///
/// Supported sequences:
/// - `\n` => newline
/// - `\r` => carriage return
/// - `\t` => horizontal tab
/// - `\xHH` => byte encoded as two hex digits (e.g., `\x1b` for ESC)
/// - `\e`  => ESC (0x1b)
/// - `\\` => literal backslash
/// - `\0`  => NUL
pub(crate) fn unescape_initial_text(raw: &str) -> String {
    let mut output = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '\\' {
            output.push(ch);
            continue;
        }

        match chars.next() {
            Some('n') => output.push('\n'),
            Some('r') => output.push('\r'),
            Some('t') => output.push('\t'),
            Some('0') => output.push('\0'),
            Some('e') | Some('E') => output.push('\u{1b}'),
            Some('x') | Some('X') => {
                let (hex, parsed) = parse_hex_byte(&mut chars);
                if let Some(value) = parsed {
                    output.push(value);
                } else {
                    // Preserve the original text when parsing fails
                    output.push('\\');
                    output.push('x');
                    output.push_str(&hex);
                }
            }
            Some('\\') => output.push('\\'),
            Some(other) => {
                // Unknown escape, keep it literal
                output.push('\\');
                output.push(other);
            }
            None => output.push('\\'),
        }
    }

    output
}

/// Normalize line endings so they match the Enter key behavior (carriage return).
pub(crate) fn normalize_line_endings(text: &str) -> String {
    let mut normalized = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '\r' => {
                // Collapse CRLF into a single CR
                if matches!(chars.peek(), Some('\n')) {
                    chars.next();
                }
                normalized.push('\r');
            }
            '\n' => normalized.push('\r'),
            _ => normalized.push(ch),
        }
    }

    normalized
}

/// Build the byte payload to send for the configured initial text.
/// Returns `None` when there is nothing to send (empty input after unescaping).
pub(crate) fn build_initial_text_payload(raw: &str, append_newline: bool) -> Option<Vec<u8>> {
    let unescaped = unescape_initial_text(raw);
    if unescaped.is_empty() {
        return None;
    }

    let normalized = normalize_line_endings(&unescaped);
    if normalized.is_empty() {
        return None;
    }

    let mut bytes = normalized.into_bytes();

    if append_newline && !bytes.ends_with(&[b'\r']) && !bytes.ends_with(&[b'\n']) {
        bytes.push(b'\r');
    }

    Some(bytes)
}

fn parse_hex_byte(chars: &mut Peekable<Chars<'_>>) -> (String, Option<char>) {
    let mut hex = String::new();

    for _ in 0..2 {
        if let Some(&next) = chars.peek() {
            if next.is_ascii_hexdigit() {
                hex.push(next);
                chars.next();
            } else {
                break;
            }
        }
    }

    if hex.len() == 2 {
        let parsed = u8::from_str_radix(&hex, 16)
            .ok()
            .and_then(|v| char::from_u32(v as u32));
        (hex, parsed)
    } else {
        (hex, None)
    }
}

#[cfg(test)]
mod tests {
    use super::{build_initial_text_payload, normalize_line_endings, unescape_initial_text};

    #[test]
    fn test_unescape_supported_sequences() {
        let raw = "echo \\x1b[0m\\nready\\tgo\\r";
        let unescaped = unescape_initial_text(raw);
        assert_eq!(unescaped, "echo \u{1b}[0m\nready\tgo\r");
    }

    #[test]
    fn test_normalize_line_endings_collapses_variants() {
        let text = "first\r\nsecond\nthird\rfourth";
        let normalized = normalize_line_endings(text);
        assert_eq!(normalized, "first\rsecond\rthird\rfourth");
    }

    #[test]
    fn test_build_payload_appends_newline_once() {
        let payload = build_initial_text_payload("ssh server", true).unwrap();
        assert_eq!(payload, b"ssh server\r");

        let already_newline = build_initial_text_payload("echo test\n", true).unwrap();
        assert_eq!(already_newline, b"echo test\r");
    }

    #[test]
    fn test_build_payload_handles_multiline_and_hex() {
        let payload = build_initial_text_payload("echo ready\\nrun\\x1b", false).unwrap();
        assert_eq!(payload, b"echo ready\rrun\x1b");
    }

    #[test]
    fn test_build_payload_empty_returns_none() {
        assert!(build_initial_text_payload("", true).is_none());
        // Pure newlines are allowed and normalized to carriage returns
        let payload = build_initial_text_payload("\n\r", false).unwrap();
        assert_eq!(payload, b"\r\r");
    }
}
