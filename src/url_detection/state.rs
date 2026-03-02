//! URL detection state types.
//!
//! Defines the core data types for representing detected URLs and file paths,
//! along with helper functions for querying detection state.

/// Type of detected clickable item
#[derive(Debug, Clone, PartialEq)]
pub enum DetectedItemType {
    /// A URL (http, https, etc.)
    Url,
    /// A file path (optionally with line number)
    FilePath {
        /// Line number if specified (e.g., file.rs:42)
        line: Option<usize>,
        /// Column number if specified (e.g., file.rs:42:10)
        column: Option<usize>,
    },
}

/// Detected URL or file path with position information
#[derive(Debug, Clone, PartialEq)]
pub struct DetectedUrl {
    /// The URL or file path text
    pub url: String,
    /// Start column position
    pub start_col: usize,
    /// End column position (exclusive)
    pub end_col: usize,
    /// Row position
    pub row: usize,
    /// OSC 8 hyperlink ID (if this is an OSC 8 hyperlink, None for regex-detected items)
    pub hyperlink_id: Option<u32>,
    /// Type of detected item (URL or FilePath)
    pub item_type: DetectedItemType,
}

/// Check if a specific position is within a URL or file path
pub fn find_url_at_position(urls: &[DetectedUrl], col: usize, row: usize) -> Option<&DetectedUrl> {
    urls.iter()
        .find(|url| url.row == row && col >= url.start_col && col < url.end_col)
}
