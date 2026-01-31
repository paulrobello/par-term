//! Types for terminal search functionality.

/// A single search match in the terminal scrollback.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchMatch {
    /// Line index in scrollback (0 = oldest line)
    pub line: usize,
    /// Column position in the line (0-indexed)
    pub column: usize,
    /// Length of the match in characters
    pub length: usize,
}

impl SearchMatch {
    /// Create a new search match.
    pub fn new(line: usize, column: usize, length: usize) -> Self {
        Self {
            line,
            column,
            length,
        }
    }
}

/// Configuration options for search behavior.
#[derive(Clone, Debug)]
pub struct SearchConfig {
    /// Whether search is case-sensitive.
    pub case_sensitive: bool,
    /// Whether to use regex pattern matching.
    pub use_regex: bool,
    /// Whether to match whole words only.
    pub whole_word: bool,
    /// Whether to wrap around when navigating matches.
    pub wrap_around: bool,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            case_sensitive: false,
            use_regex: false,
            whole_word: false,
            wrap_around: true,
        }
    }
}

/// Actions that can result from search UI interaction.
#[derive(Debug, Clone)]
pub enum SearchAction {
    /// No action needed.
    None,
    /// Scroll to make a match visible at the given scroll offset.
    ScrollToMatch(usize),
    /// Close the search UI.
    Close,
}
