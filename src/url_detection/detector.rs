//! URL and file path detection using regex patterns and OSC 8 hyperlink parsing.
//!
//! This module provides the core detection logic:
//! - Regex-based URL detection
//! - Regex-based file path detection (with optional line/column numbers)
//! - OSC 8 hyperlink extraction from terminal cells

use regex::Regex;
use std::sync::OnceLock;

use super::state::{DetectedItemType, DetectedUrl};

/// URL pattern that matches common URL schemes
static URL_REGEX: OnceLock<Regex> = OnceLock::new();

/// File path pattern that matches Unix-style file paths
static FILE_PATH_REGEX: OnceLock<Regex> = OnceLock::new();

/// Get the compiled URL regex pattern
fn url_regex() -> &'static Regex {
    URL_REGEX.get_or_init(|| {
        // Matches URLs with common schemes (http, https, ftp, etc.)
        // Also matches URLs without schemes that start with www.
        Regex::new(
            r"(?x)
            \b(?:
                # URLs with explicit schemes
                (?:https?|ftps?|file|git|ssh)://[^\s<>{}|\\^`\[\]]+
                |
                # URLs starting with www.
                www\.[^\s<>{}|\\^`\[\]]+
            )\b
            ",
        )
        .expect("Failed to compile URL regex")
    })
}

/// Get the compiled file path regex pattern
fn file_path_regex() -> &'static Regex {
    FILE_PATH_REGEX.get_or_init(|| {
        // Matches file paths at the START of a logical token:
        // - Absolute paths starting with / (must follow whitespace or start of line)
        // - Relative paths starting with ./ or ../
        // - Home-relative paths starting with ~/
        //
        // Absolute paths use a lookbehind to require whitespace or start-of-string
        // before the leading /, preventing false matches inside relative paths
        // like ./a/b/c where /b/c would otherwise also match.
        //
        // Optionally followed by :line_number or :line_number:col_number
        // Also supports other line number formats from iTerm2:
        // - [line, col] or [line,col]
        // - (line, col) or (line,col)
        // - (line)
        Regex::new(
            r#"(?x)
            (?:
                # Home-relative paths (~/...)
                ~/[^\s:,;'"<>|)\]}\[\(\x00-\x1f\u{2500}-\u{257F}]+
                |
                # Relative paths starting with ./ or ../
                \.\.?/[^\s:,;'"<>|)\]}\[\(\x00-\x1f\u{2500}-\u{257F}]+
                |
                # Absolute paths: must be at start of string or after whitespace
                # Require at least two path components to reduce false positives
                (?:^|\s)/[^\s:,;'"<>|)\]}\[\(\x00-\x1f\u{2500}-\u{257F}]+/[^\s:,;'"<>|)\]}\[\(\x00-\x1f\u{2500}-\u{257F}]+
            )
            # Optional line/column number in various formats
            (?:
                :\d+(?::\d+)?           # :line or :line:col
                | \[\d+(?:,\s?\d+)?\]   # [line] or [line, col]
                | \(\d+(?:,\s?\d+)?\)   # (line) or (line, col)
            )?
            "#,
        )
        .expect("Failed to compile file path regex")
    })
}

/// Detect URLs in a line of text using regex patterns
pub fn detect_urls_in_line(text: &str, row: usize) -> Vec<DetectedUrl> {
    let regex = url_regex();
    let mut urls = Vec::new();

    for mat in regex.find_iter(text) {
        let url = mat.as_str().to_string();
        let start_col = mat.start();
        let end_col = mat.end();

        urls.push(DetectedUrl {
            url,
            start_col,
            end_col,
            row,
            hyperlink_id: None, // Regex-detected URLs don't have OSC 8 IDs
            item_type: DetectedItemType::Url,
        });
    }

    urls
}

/// Detect file paths in a line of text using regex patterns.
///
/// Detects Unix-style paths like /path/to/file, ./relative, ../parent, ~/home.
/// Also detects line numbers like file.rs:42 and file.rs:42:10.
pub fn detect_file_paths_in_line(text: &str, row: usize) -> Vec<DetectedUrl> {
    let regex = file_path_regex();
    let mut paths = Vec::new();

    for mat in regex.find_iter(text) {
        let full_match = mat.as_str();
        let mut start_col = mat.start();
        let end_col = mat.end();

        // The absolute path branch uses (?:^|\s) which may include a leading
        // whitespace character in the match. Strip it to get the actual path.
        let trimmed_match = if full_match.starts_with(char::is_whitespace) {
            let trimmed = full_match.trim_start();
            start_col += full_match.len() - trimmed.len();
            trimmed
        } else {
            full_match
        };

        // Parse line and column numbers from the path
        let (path, line, column) = parse_path_with_line_number(trimmed_match);

        paths.push(DetectedUrl {
            url: path,
            start_col,
            end_col,
            row,
            hyperlink_id: None,
            item_type: DetectedItemType::FilePath { line, column },
        });
    }

    paths
}

/// Parse a file path that may include line/column suffixes in various formats:
/// - `:line` or `:line:col` (most common)
/// - `[line]` or `[line, col]` (some editors)
/// - `(line)` or `(line, col)` (some error formats)
fn parse_path_with_line_number(path_str: &str) -> (String, Option<usize>, Option<usize>) {
    // Try bracket format: [line] or [line, col]
    if let Some(bracket_start) = path_str.rfind('[')
        && path_str.ends_with(']')
    {
        let path = path_str[..bracket_start].to_string();
        let inner = &path_str[bracket_start + 1..path_str.len() - 1];
        let (line, col) = parse_line_col_pair(inner);
        if line.is_some() {
            return (path, line, col);
        }
    }

    // Try paren format: (line) or (line, col)
    if let Some(paren_start) = path_str.rfind('(')
        && path_str.ends_with(')')
    {
        let path = path_str[..paren_start].to_string();
        let inner = &path_str[paren_start + 1..path_str.len() - 1];
        let (line, col) = parse_line_col_pair(inner);
        if line.is_some() {
            return (path, line, col);
        }
    }

    // Try colon format: :line or :line:col
    let parts: Vec<&str> = path_str.rsplitn(3, ':').collect();

    match parts.len() {
        3 => {
            // file:line:col format
            let col = parts[0].parse::<usize>().ok();
            let line = parts[1].parse::<usize>().ok();
            if line.is_some() {
                let path = parts[2].to_string();
                (path, line, col)
            } else {
                (path_str.to_string(), None, None)
            }
        }
        2 => {
            // file:line format (or just path with colon)
            let line = parts[0].parse::<usize>().ok();
            if line.is_some() {
                let path = parts[1].to_string();
                (path, line, None)
            } else {
                (path_str.to_string(), None, None)
            }
        }
        _ => (path_str.to_string(), None, None),
    }
}

/// Parse "line" or "line, col" or "line,col" into (Option<line>, Option<col>)
fn parse_line_col_pair(s: &str) -> (Option<usize>, Option<usize>) {
    let parts: Vec<&str> = s.split(',').map(|p| p.trim()).collect();
    match parts.len() {
        1 => (parts[0].parse().ok(), None),
        2 => (parts[0].parse().ok(), parts[1].parse().ok()),
        _ => (None, None),
    }
}

/// Detect OSC 8 hyperlinks from terminal cells
///
/// # Arguments
/// * `cells` - Slice of cells from a single row
/// * `row` - Row number
/// * `hyperlink_urls` - Mapping from hyperlink_id to URL string
///
/// # Returns
/// Vector of DetectedUrl objects for OSC 8 hyperlinks in this row
pub fn detect_osc8_hyperlinks(
    cells: &[crate::cell_renderer::Cell],
    row: usize,
    hyperlink_urls: &std::collections::HashMap<u32, String>,
) -> Vec<DetectedUrl> {
    let mut urls = Vec::new();
    let mut current_hyperlink: Option<(u32, usize, String)> = None; // (id, start_col, url)

    for (col, cell) in cells.iter().enumerate() {
        match (cell.hyperlink_id, &current_hyperlink) {
            // Cell has a hyperlink ID
            (Some(id), Some((current_id, _start_col, _url))) if id == *current_id => {
                // Continue existing hyperlink (same ID as previous cell)
                continue;
            }
            (Some(id), _) => {
                // Start of a new hyperlink or different hyperlink
                // First, save the previous hyperlink if there was one
                if let Some((prev_id, start_col, url)) = current_hyperlink.take() {
                    urls.push(DetectedUrl {
                        url,
                        start_col,
                        end_col: col, // Previous hyperlink ends at current position
                        row,
                        hyperlink_id: Some(prev_id),
                        item_type: DetectedItemType::Url,
                    });
                }

                // Start new hyperlink if we have a URL for this ID
                if let Some(url) = hyperlink_urls.get(&id) {
                    current_hyperlink = Some((id, col, url.clone()));
                }
            }
            (None, Some((prev_id, start_col, url))) => {
                // End of current hyperlink
                urls.push(DetectedUrl {
                    url: url.clone(),
                    start_col: *start_col,
                    end_col: col, // Hyperlink ends at current position
                    row,
                    hyperlink_id: Some(*prev_id),
                    item_type: DetectedItemType::Url,
                });
                current_hyperlink = None;
            }
            (None, None) => {
                // No hyperlink in this cell or previous cells
                continue;
            }
        }
    }

    // Save last hyperlink if it extends to the end of the row
    if let Some((id, start_col, url)) = current_hyperlink {
        urls.push(DetectedUrl {
            url,
            start_col,
            end_col: cells.len(), // Extends to end of row
            row,
            hyperlink_id: Some(id),
            item_type: DetectedItemType::Url,
        });
    }

    urls
}
