//! Whitespace and newline transformations.

// ============================================================================
// Newline transformations
// ============================================================================

/// Strip all newlines and join into a single line, replacing newlines with spaces.
pub(super) fn paste_as_single_line(input: &str) -> String {
    input.lines().collect::<Vec<_>>().join(" ")
}

/// Ensure each line ends with a newline character.
pub(super) fn add_newlines(input: &str) -> String {
    if input.is_empty() {
        return String::new();
    }
    let mut result: String = input.lines().collect::<Vec<_>>().join("\n");
    if !result.ends_with('\n') {
        result.push('\n');
    }
    result
}

/// Remove all newline characters from the text.
pub(super) fn remove_newlines(input: &str) -> String {
    input.replace(['\n', '\r'], "")
}

// ============================================================================
// Whitespace transformations
// ============================================================================

/// Trim whitespace from each line individually.
pub(super) fn trim_lines(input: &str) -> String {
    input
        .lines()
        .map(|line| line.trim())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Collapse multiple consecutive spaces into a single space.
pub(super) fn collapse_spaces(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut prev_was_space = false;

    for c in input.chars() {
        if c == ' ' {
            if !prev_was_space {
                result.push(c);
                prev_was_space = true;
            }
        } else {
            result.push(c);
            prev_was_space = false;
        }
    }
    result
}

/// Remove completely empty or whitespace-only lines.
pub(super) fn remove_empty_lines(input: &str) -> String {
    input
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Convert all line endings (CRLF, CR) to LF.
pub(super) fn normalize_line_endings(input: &str) -> String {
    input.replace("\r\n", "\n").replace('\r', "\n")
}
