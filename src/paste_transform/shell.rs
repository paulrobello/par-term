//! Shell quoting and escaping transformations.

/// Characters that require quoting/escaping in shell contexts.
pub(super) const SHELL_SPECIAL_CHARS: &[char] = &[
    ' ', '\t', '\n', '\r', // Whitespace
    '\'', '"', '`', // Quotes and backticks
    '$', '!', '&', '|', // Variable expansion and control operators
    ';', '(', ')', '{', '}', '[', ']', // Grouping and subshell
    '<', '>', // Redirection
    '*', '?', // Glob patterns
    '\\', '#', '~', '^', // Escape, comments, home, history
];

/// Wrap input in single quotes, escaping internal single quotes.
pub(super) fn shell_single_quote(input: &str) -> String {
    // Single quotes: escape internal ' as '\''
    let escaped = input.replace('\'', "'\\''");
    format!("'{}'", escaped)
}

/// Wrap input in double quotes, escaping special shell characters.
pub(super) fn shell_double_quote(input: &str) -> String {
    // Double quotes: escape $, `, \, ", !
    let mut result = String::with_capacity(input.len() + 10);
    result.push('"');
    for c in input.chars() {
        match c {
            '$' | '`' | '\\' | '"' | '!' => {
                result.push('\\');
                result.push(c);
            }
            _ => result.push(c),
        }
    }
    result.push('"');
    result
}

/// Escape special shell characters with backslashes.
pub(super) fn shell_backslash_escape(input: &str) -> String {
    let mut result = String::with_capacity(input.len() * 2);
    for c in input.chars() {
        if SHELL_SPECIAL_CHARS.contains(&c) {
            result.push('\\');
        }
        result.push(c);
    }
    result
}
