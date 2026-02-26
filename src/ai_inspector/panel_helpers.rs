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
    if s.len() <= max_chars {
        return s;
    }
    // Find the last char boundary at or before max_chars bytes
    let mut end = max_chars.min(s.len());
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
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
    }

    #[test]
    fn test_truncate_chars_multibyte() {
        let s = "héllo";
        let result = truncate_chars(s, 3);
        assert!(s.is_char_boundary(result.len()));
    }

    #[test]
    fn test_truncate_chars_cjk() {
        let s = "你好世界";
        let result = truncate_chars(s, 6);
        assert!(s.is_char_boundary(result.len()));
    }
}
