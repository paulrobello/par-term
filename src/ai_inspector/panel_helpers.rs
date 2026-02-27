//! Helper functions for the AI inspector panel rendering.
//!
//! Contains small utility functions used by [`super::panel::AIInspectorPanel`]
//! for formatting and text truncation.

/// Format a duration in milliseconds to a human-readable string.
pub(super) fn format_duration(ms: u64) -> String {
    if ms < 1000 {
        format!("{ms}ms")
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else {
        let minutes = ms / 60_000;
        let seconds = (ms % 60_000) / 1000;
        format!("{minutes}m {seconds}s")
    }
}

/// Truncate a string to at most `max_chars` characters, respecting UTF-8
/// char boundaries (never panics on multi-byte characters like emoji or CJK).
pub(super) fn truncate_chars(s: &str, max_chars: usize) -> &str {
    // Use character count, not byte length, for comparison
    if s.chars().count() <= max_chars {
        return s;
    }
    // Find the byte index where we should truncate at the max_chars-th character
    match s.char_indices().nth(max_chars) {
        Some((byte_idx, _)) => &s[..byte_idx],
        None => s, // Shouldn't happen due to check above, but safe fallback
    }
}

/// Truncate output text to a maximum number of lines.
pub(super) fn truncate_output(output: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = output.lines().take(max_lines + 1).collect();
    if lines.len() > max_lines {
        let mut result: String = lines[..max_lines].join("\n");
        result.push_str("\n... (truncated)");
        result
    } else {
        output.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(500), "500ms");
        assert_eq!(format_duration(1500), "1.5s");
        assert_eq!(format_duration(65_000), "1m 5s");
    }

    #[test]
    fn test_truncate_output() {
        let output = "line1\nline2\nline3\nline4\nline5";
        let truncated = truncate_output(output, 3);
        assert!(truncated.contains("... (truncated)"));
        assert!(truncated.contains("line1"));
    }

    #[test]
    fn test_truncate_chars_ascii() {
        assert_eq!(truncate_chars("hello world", 5), "hello");
        assert_eq!(truncate_chars("abc", 10), "abc");
        assert_eq!(truncate_chars("", 5), "");
        assert_eq!(truncate_chars("test", 0), "");
    }

    #[test]
    fn test_truncate_chars_multibyte() {
        let s = "héllo";
        let result = truncate_chars(s, 3);
        assert_eq!(result, "hél"); // 3 characters, not 3 bytes
        assert!(s.is_char_boundary(result.len()));
    }

    #[test]
    fn test_truncate_chars_cjk() {
        // Each CJK character is 3 bytes in UTF-8
        let s = "你好世界"; // 4 characters, 12 bytes
        let result = truncate_chars(s, 2);
        assert_eq!(result, "你好"); // 2 characters, not 6 bytes
        assert_eq!(result.len(), 6); // 6 bytes (2 chars * 3 bytes each)
        assert!(s.is_char_boundary(result.len()));
    }

    #[test]
    fn test_truncate_chars_emoji() {
        // Emoji are 4 bytes in UTF-8
        let s = "😀😁😂😃"; // 4 emoji, 16 bytes
        let result = truncate_chars(s, 2);
        assert_eq!(result, "😀😁"); // 2 emoji, not 8 bytes
        assert_eq!(result.len(), 8); // 8 bytes (2 chars * 4 bytes each)
        assert!(s.is_char_boundary(result.len()));
    }

    #[test]
    fn test_truncate_chars_mixed() {
        // Mixed ASCII and multi-byte
        // "Hi 你好" = ['H', 'i', ' ', '你', '好'] = 5 characters, 9 bytes
        let s = "Hi 你好";
        let result = truncate_chars(s, 4);
        assert_eq!(result, "Hi 你"); // 4 characters: H, i, space, 你
        assert_eq!(result.len(), 6); // 1+1+1+3 = 6 bytes
        assert!(s.is_char_boundary(result.len()));
    }
}
