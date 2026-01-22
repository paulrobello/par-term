/// URL detection and handling utilities
use regex::Regex;
use std::sync::OnceLock;

/// URL pattern that matches common URL schemes
static URL_REGEX: OnceLock<Regex> = OnceLock::new();

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

/// Detected URL with position information
#[derive(Debug, Clone, PartialEq)]
pub struct DetectedUrl {
    /// The URL text
    pub url: String,
    /// Start column position
    pub start_col: usize,
    /// End column position (exclusive)
    pub end_col: usize,
    /// Row position
    pub row: usize,
    /// OSC 8 hyperlink ID (if this is an OSC 8 hyperlink, None for regex-detected URLs)
    pub hyperlink_id: Option<u32>,
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
        });
    }

    urls
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
        });
    }

    urls
}

/// Check if a specific position is within a URL
pub fn find_url_at_position(urls: &[DetectedUrl], col: usize, row: usize) -> Option<&DetectedUrl> {
    urls.iter()
        .find(|url| url.row == row && col >= url.start_col && col < url.end_col)
}

/// Open a URL in the default browser
pub fn open_url(url: &str) -> Result<(), String> {
    // Add scheme if missing (e.g., www.example.com -> https://www.example.com)
    let url_with_scheme = if !url.contains("://") {
        format!("https://{}", url)
    } else {
        url.to_string()
    };

    open::that(&url_with_scheme).map_err(|e| format!("Failed to open URL: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_http_url() {
        let text = "Visit https://example.com for more info";
        let urls = detect_urls_in_line(text, 0);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].url, "https://example.com");
        assert_eq!(urls[0].start_col, 6);
        assert_eq!(urls[0].end_col, 25); // Exclusive end position
    }

    #[test]
    fn test_detect_www_url() {
        let text = "Check out www.example.com";
        let urls = detect_urls_in_line(text, 0);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].url, "www.example.com");
    }

    #[test]
    fn test_detect_multiple_urls() {
        let text = "See https://example.com and http://test.org";
        let urls = detect_urls_in_line(text, 0);
        assert_eq!(urls.len(), 2);
        assert_eq!(urls[0].url, "https://example.com");
        assert_eq!(urls[1].url, "http://test.org");
    }

    #[test]
    fn test_find_url_at_position() {
        let text = "Visit https://example.com for more";
        let urls = detect_urls_in_line(text, 5);

        // Position within URL
        assert!(find_url_at_position(&urls, 10, 5).is_some());

        // Position outside URL
        assert!(find_url_at_position(&urls, 0, 5).is_none());
        assert!(find_url_at_position(&urls, 30, 5).is_none());

        // Wrong row
        assert!(find_url_at_position(&urls, 10, 6).is_none());
    }

    #[test]
    fn test_no_urls() {
        let text = "This line has no URLs at all";
        let urls = detect_urls_in_line(text, 0);
        assert_eq!(urls.len(), 0);
    }

    #[test]
    fn test_url_schemes() {
        let text = "ftp://files.com ssh://git.com file:///path git://repo.com";
        let urls = detect_urls_in_line(text, 0);
        assert_eq!(urls.len(), 4);
    }
}
