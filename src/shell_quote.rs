//! Shell quoting utilities for safe filename handling.
//!
//! Provides functions to properly quote or escape file paths for use in shell commands.

use crate::config::DroppedFileQuoteStyle;
use std::path::Path;

/// Characters that require quoting/escaping in shell contexts.
/// This covers POSIX shells (bash, zsh, sh) and most common special characters.
const SHELL_SPECIAL_CHARS: &[char] = &[
    ' ', '\t', '\n', '\r', // Whitespace
    '\'', '"', '`', // Quotes and backticks
    '$', '!', '&', '|', // Variable expansion and control operators
    ';', '(', ')', '{', '}', '[', ']', // Grouping and subshell
    '<', '>', // Redirection
    '*', '?', // Glob patterns
    '\\', '#', '~', '^', // Escape, comments, home, history
];

/// Check if a path contains any characters that need quoting.
fn needs_quoting(path: &str) -> bool {
    path.chars().any(|c| SHELL_SPECIAL_CHARS.contains(&c))
}

/// Quote a file path using single quotes.
///
/// Single quotes are the safest option for most shells - everything inside
/// is treated literally except for single quotes themselves.
///
/// Single quotes inside the path are handled by ending the quoted section,
/// adding an escaped single quote, and starting a new quoted section:
/// `it's` becomes `'it'\''s'`
fn quote_single(path: &str) -> String {
    // Always quote for consistency (like iTerm2)
    // Handle single quotes by ending quote, escaping, and starting new quote
    let escaped = path.replace('\'', "'\\''");
    format!("'{}'", escaped)
}

/// Quote a file path using double quotes.
///
/// Double quotes allow variable expansion but protect most special characters.
/// We need to escape: $, `, \, ", and !
fn quote_double(path: &str) -> String {
    // Always quote for consistency (like iTerm2)
    let mut result = String::with_capacity(path.len() + 10);
    result.push('"');

    for c in path.chars() {
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

/// Escape a file path using backslashes.
///
/// Each special character is preceded by a backslash.
/// Only escapes when necessary (no wrapping quotes).
fn quote_backslash(path: &str) -> String {
    if !needs_quoting(path) {
        return path.to_string();
    }

    let mut result = String::with_capacity(path.len() * 2);

    for c in path.chars() {
        if SHELL_SPECIAL_CHARS.contains(&c) {
            result.push('\\');
        }
        result.push(c);
    }

    result
}

/// Quote a file path according to the specified style.
pub fn quote_path(path: &Path, style: DroppedFileQuoteStyle) -> String {
    let path_str = path.to_string_lossy();

    match style {
        DroppedFileQuoteStyle::SingleQuotes => quote_single(&path_str),
        DroppedFileQuoteStyle::DoubleQuotes => quote_double(&path_str),
        DroppedFileQuoteStyle::Backslash => quote_backslash(&path_str),
        DroppedFileQuoteStyle::None => path_str.into_owned(),
    }
}

/// Quote multiple file paths, returning them space-separated.
pub fn quote_paths(paths: &[&Path], style: DroppedFileQuoteStyle) -> String {
    paths
        .iter()
        .map(|p| quote_path(p, style))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_simple_path_always_quoted() {
        let path = Path::new("/usr/local/bin/program");
        // Single and double quotes always wrap for consistency (like iTerm2)
        assert_eq!(
            quote_path(path, DroppedFileQuoteStyle::SingleQuotes),
            "'/usr/local/bin/program'"
        );
        assert_eq!(
            quote_path(path, DroppedFileQuoteStyle::DoubleQuotes),
            "\"/usr/local/bin/program\""
        );
        // Backslash only escapes when needed
        assert_eq!(
            quote_path(path, DroppedFileQuoteStyle::Backslash),
            "/usr/local/bin/program"
        );
        // None never quotes
        assert_eq!(
            quote_path(path, DroppedFileQuoteStyle::None),
            "/usr/local/bin/program"
        );
    }

    #[test]
    fn test_path_with_spaces() {
        let path = Path::new("/path/to/file with spaces.txt");
        assert_eq!(
            quote_path(path, DroppedFileQuoteStyle::SingleQuotes),
            "'/path/to/file with spaces.txt'"
        );
        assert_eq!(
            quote_path(path, DroppedFileQuoteStyle::DoubleQuotes),
            "\"/path/to/file with spaces.txt\""
        );
        assert_eq!(
            quote_path(path, DroppedFileQuoteStyle::Backslash),
            "/path/to/file\\ with\\ spaces.txt"
        );
        assert_eq!(
            quote_path(path, DroppedFileQuoteStyle::None),
            "/path/to/file with spaces.txt"
        );
    }

    #[test]
    fn test_path_with_single_quote() {
        let path = Path::new("/path/to/it's a file.txt");
        assert_eq!(
            quote_path(path, DroppedFileQuoteStyle::SingleQuotes),
            "'/path/to/it'\\''s a file.txt'"
        );
    }

    #[test]
    fn test_path_with_dollar_sign() {
        let path = Path::new("/path/to/$HOME/file.txt");
        assert_eq!(
            quote_path(path, DroppedFileQuoteStyle::SingleQuotes),
            "'/path/to/$HOME/file.txt'"
        );
        assert_eq!(
            quote_path(path, DroppedFileQuoteStyle::DoubleQuotes),
            "\"/path/to/\\$HOME/file.txt\""
        );
        assert_eq!(
            quote_path(path, DroppedFileQuoteStyle::Backslash),
            "/path/to/\\$HOME/file.txt"
        );
    }

    #[test]
    fn test_path_with_glob_chars() {
        let path = Path::new("/path/to/*.txt");
        assert_eq!(
            quote_path(path, DroppedFileQuoteStyle::SingleQuotes),
            "'/path/to/*.txt'"
        );
        assert_eq!(
            quote_path(path, DroppedFileQuoteStyle::Backslash),
            "/path/to/\\*.txt"
        );
    }

    #[test]
    fn test_multiple_paths() {
        let paths: Vec<&Path> = vec![
            Path::new("/simple/path"),
            Path::new("/path with spaces"),
            Path::new("/path/with$dollar"),
        ];
        let result = quote_paths(&paths, DroppedFileQuoteStyle::SingleQuotes);
        // All paths are quoted for consistency
        assert_eq!(
            result,
            "'/simple/path' '/path with spaces' '/path/with$dollar'"
        );
    }
}
