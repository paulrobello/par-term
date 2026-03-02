//! Preview generation for collapsed Claude Code output blocks.
//!
//! When Claude Code collapses output (showing "(ctrl+o to expand)"), the
//! prettifier can display a one-line preview in place of the hidden content.
//! This module contains the `RenderedPreview` type and the `generate_preview`
//! helper that constructs it from a `ContentBlock` and a `DetectionResult`.

use crate::types::{ContentBlock, DetectionResult};

// ---------------------------------------------------------------------------
// RenderedPreview
// ---------------------------------------------------------------------------

/// A preview shown for a collapsed Claude Code block.
#[derive(Debug, Clone)]
pub struct RenderedPreview {
    /// Format badge text (e.g., "MD Markdown", "{} JSON").
    pub format_badge: String,
    /// First header extracted from the content (if markdown).
    pub first_header: Option<String>,
    /// Summary of the content (e.g., "42 lines").
    pub content_summary: String,
}

// ---------------------------------------------------------------------------
// Preview generation
// ---------------------------------------------------------------------------

/// Generate a preview for a collapsed block based on its content and detection result.
///
/// # Arguments
///
/// - `content` — The content block that was collapsed.
/// - `detection` — The format detection result for the block.
/// - `show_badges` — Whether to include a format badge in the preview.
pub fn generate_preview(
    content: &ContentBlock,
    detection: &DetectionResult,
    show_badges: bool,
) -> RenderedPreview {
    let format_badge = if show_badges {
        match detection.format_id.as_str() {
            "markdown" => "MD Markdown".to_string(),
            "json" => "{} JSON".to_string(),
            "diagrams" => "Diagram".to_string(),
            "yaml" => "YAML".to_string(),
            "diff" => "± Diff".to_string(),
            other => other.to_string(),
        }
    } else {
        String::new()
    };

    // Extract first header from content (if markdown).
    let first_header = content
        .lines
        .iter()
        .find(|l| l.starts_with('#'))
        .map(|l| l.trim_start_matches('#').trim().to_string());

    RenderedPreview {
        format_badge,
        first_header,
        content_summary: format!("{} lines", content.lines.len()),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DetectionSource;
    use std::time::SystemTime;

    fn make_detection(format_id: &str) -> DetectionResult {
        DetectionResult {
            format_id: format_id.to_string(),
            confidence: 0.9,
            matched_rules: vec![],
            source: DetectionSource::AutoDetected,
        }
    }

    #[test]
    fn test_generate_preview_markdown() {
        let content = ContentBlock {
            lines: vec![
                "# Introduction".to_string(),
                "Some text here.".to_string(),
                "More content.".to_string(),
            ],
            preceding_command: None,
            start_row: 0,
            end_row: 3,
            timestamp: SystemTime::now(),
        };

        let preview = generate_preview(&content, &make_detection("markdown"), true);
        assert_eq!(preview.format_badge, "MD Markdown");
        assert_eq!(preview.first_header.as_deref(), Some("Introduction"));
        assert_eq!(preview.content_summary, "3 lines");
    }

    #[test]
    fn test_generate_preview_json() {
        let content = ContentBlock {
            lines: vec![
                "{".to_string(),
                "  \"key\": \"value\"".to_string(),
                "}".to_string(),
            ],
            preceding_command: None,
            start_row: 0,
            end_row: 3,
            timestamp: SystemTime::now(),
        };

        let preview = generate_preview(&content, &make_detection("json"), true);
        assert_eq!(preview.format_badge, "{} JSON");
        assert!(preview.first_header.is_none());
        assert_eq!(preview.content_summary, "3 lines");
    }

    #[test]
    fn test_generate_preview_diff() {
        let content = ContentBlock {
            lines: vec!["--- a/file".to_string(), "+++ b/file".to_string()],
            preceding_command: None,
            start_row: 0,
            end_row: 2,
            timestamp: SystemTime::now(),
        };

        let preview = generate_preview(&content, &make_detection("diff"), true);
        assert_eq!(preview.format_badge, "± Diff");
        assert!(preview.first_header.is_none());
        assert_eq!(preview.content_summary, "2 lines");
    }

    #[test]
    fn test_generate_preview_badges_disabled() {
        let content = ContentBlock {
            lines: vec!["test".to_string()],
            preceding_command: None,
            start_row: 0,
            end_row: 1,
            timestamp: SystemTime::now(),
        };

        let preview = generate_preview(&content, &make_detection("markdown"), false);
        assert!(preview.format_badge.is_empty());
    }

    #[test]
    fn test_generate_preview_unknown_format() {
        let content = ContentBlock {
            lines: vec!["data".to_string()],
            preceding_command: None,
            start_row: 0,
            end_row: 1,
            timestamp: SystemTime::now(),
        };

        let preview = generate_preview(&content, &make_detection("toml"), true);
        assert_eq!(preview.format_badge, "toml");
    }

    #[test]
    fn test_generate_preview_no_header_in_non_markdown() {
        let content = ContentBlock {
            lines: vec!["key: value".to_string(), "other: data".to_string()],
            preceding_command: None,
            start_row: 0,
            end_row: 2,
            timestamp: SystemTime::now(),
        };

        let preview = generate_preview(&content, &make_detection("yaml"), true);
        assert_eq!(preview.format_badge, "YAML");
        assert!(preview.first_header.is_none());
    }
}
