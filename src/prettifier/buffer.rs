//! Dual-view buffer for the Content Prettifier framework.
//!
//! `DualViewBuffer` manages source text and rendered output for a single content
//! block, supporting toggling between views without re-rendering.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use super::types::{ContentBlock, InlineGraphic, RenderedContent, StyledLine, ViewMode};

/// Threshold above which virtual rendering is used (only render the visible portion).
const VIRTUAL_RENDER_THRESHOLD: usize = 10_000;

/// Manages source text + rendered output for a single content block.
/// Supports toggling between views without re-rendering.
#[derive(Debug)]
pub struct DualViewBuffer {
    /// The original source text (never modified).
    source: ContentBlock,
    /// The rendered output (computed lazily, cached).
    rendered: Option<RenderedContent>,
    /// Current view mode.
    view_mode: ViewMode,
    /// Content hash for cache invalidation.
    content_hash: u64,
    /// Terminal width at render time (re-render if width changes).
    rendered_width: Option<usize>,
}

impl DualViewBuffer {
    /// Create a new dual-view buffer from a source content block.
    pub fn new(source: ContentBlock) -> Self {
        let content_hash = compute_content_hash(&source.lines);
        Self {
            source,
            rendered: None,
            view_mode: ViewMode::Rendered,
            content_hash,
            rendered_width: None,
        }
    }

    /// Get the content to display based on current view mode.
    ///
    /// Returns rendered lines if in `Rendered` mode and rendered content exists,
    /// otherwise falls back to plain source lines.
    pub fn display_lines(&self) -> Vec<StyledLine> {
        match self.view_mode {
            ViewMode::Rendered => {
                if let Some(ref rendered) = self.rendered {
                    rendered.lines.clone()
                } else {
                    self.source_as_styled_lines()
                }
            }
            ViewMode::Source => self.source_as_styled_lines(),
        }
    }

    /// Set rendered content.
    pub fn set_rendered(&mut self, rendered: RenderedContent, terminal_width: usize) {
        self.rendered = Some(rendered);
        self.rendered_width = Some(terminal_width);
    }

    /// Check if re-rendering is needed (width changed or no cached render).
    pub fn needs_render(&self, terminal_width: usize) -> bool {
        match self.rendered_width {
            None => true,
            Some(w) => w != terminal_width,
        }
    }

    /// Toggle between source and rendered view.
    pub fn toggle_view(&mut self) {
        self.view_mode = match self.view_mode {
            ViewMode::Rendered => ViewMode::Source,
            ViewMode::Source => ViewMode::Rendered,
        };
    }

    /// Get current view mode.
    pub fn view_mode(&self) -> &ViewMode {
        &self.view_mode
    }

    /// Get source text for copy operations.
    pub fn source_text(&self) -> String {
        self.source.full_text()
    }

    /// Get rendered text for copy operations (plain text extracted from styled lines).
    pub fn rendered_text(&self) -> Option<String> {
        self.rendered.as_ref().map(|r| {
            r.lines
                .iter()
                .map(|line| {
                    line.segments
                        .iter()
                        .map(|seg| seg.text.as_str())
                        .collect::<String>()
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
    }

    /// Map a rendered line number to the corresponding source line number.
    pub fn rendered_to_source_line(&self, rendered_line: usize) -> Option<usize> {
        self.rendered.as_ref().and_then(|r| {
            r.line_mapping
                .iter()
                .find(|m| m.rendered_line == rendered_line)
                .and_then(|m| m.source_line)
        })
    }

    /// Map a source line number to rendered line number(s).
    pub fn source_to_rendered_lines(&self, source_line: usize) -> Vec<usize> {
        match &self.rendered {
            Some(r) => r
                .line_mapping
                .iter()
                .filter(|m| m.source_line == Some(source_line))
                .map(|m| m.rendered_line)
                .collect(),
            None => vec![],
        }
    }

    /// Get the content hash (for cache keying).
    pub fn content_hash(&self) -> u64 {
        self.content_hash
    }

    /// Number of display lines in the current view mode.
    pub fn display_line_count(&self) -> usize {
        match self.view_mode {
            ViewMode::Rendered => self
                .rendered
                .as_ref()
                .map_or(self.source.line_count(), |r| r.lines.len()),
            ViewMode::Source => self.source.line_count(),
        }
    }

    /// For very large blocks, only render the visible portion.
    /// Returns styled lines for the visible range only.
    pub fn display_lines_range(&self, start: usize, count: usize) -> Vec<StyledLine> {
        match self.view_mode {
            ViewMode::Rendered => {
                if let Some(ref rendered) = self.rendered {
                    let end = (start + count).min(rendered.lines.len());
                    if start >= rendered.lines.len() {
                        return vec![];
                    }
                    rendered.lines[start..end].to_vec()
                } else {
                    self.source_lines_range(start, count)
                }
            }
            ViewMode::Source => self.source_lines_range(start, count),
        }
    }

    /// Slice source lines into `StyledLine`s without cloning everything.
    fn source_lines_range(&self, start: usize, count: usize) -> Vec<StyledLine> {
        let total = self.source.lines.len();
        if start >= total {
            return vec![];
        }
        let end = (start + count).min(total);
        self.source.lines[start..end]
            .iter()
            .map(|l| StyledLine::plain(l))
            .collect()
    }

    /// Whether this block uses virtual rendering (>10K lines).
    pub fn is_virtual(&self) -> bool {
        self.source.line_count() > VIRTUAL_RENDER_THRESHOLD
    }

    /// Get a reference to the source content block.
    pub fn source(&self) -> &ContentBlock {
        &self.source
    }

    /// Get a reference to the rendered content, if available.
    pub fn rendered(&self) -> Option<&RenderedContent> {
        self.rendered.as_ref()
    }

    /// Get inline graphics for the current view mode.
    ///
    /// Returns rendered graphics when in `Rendered` mode with available content,
    /// otherwise returns an empty slice.
    pub fn rendered_graphics(&self) -> &[InlineGraphic] {
        match (&self.view_mode, &self.rendered) {
            (ViewMode::Rendered, Some(r)) => &r.graphics,
            _ => &[],
        }
    }

    /// Convert source lines to plain `StyledLine`s for display.
    fn source_as_styled_lines(&self) -> Vec<StyledLine> {
        self.source
            .lines
            .iter()
            .map(|l| StyledLine::plain(l))
            .collect()
    }
}

/// Compute a fast content hash for cache keying.
///
/// Combines the number of lines, first and last line content, total character
/// count, and a hash of the full content for a collision-resistant key.
pub fn compute_content_hash(lines: &[String]) -> u64 {
    let mut hasher = DefaultHasher::new();

    // Number of lines.
    lines.len().hash(&mut hasher);

    // First line content.
    if let Some(first) = lines.first() {
        first.hash(&mut hasher);
    }

    // Last line content.
    if let Some(last) = lines.last() {
        last.hash(&mut hasher);
    }

    // Total character count.
    let total_chars: usize = lines.iter().map(|l| l.len()).sum();
    total_chars.hash(&mut hasher);

    // Hash of full content.
    for line in lines {
        line.hash(&mut hasher);
    }

    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prettifier::types::*;
    use std::time::SystemTime;

    fn make_source(lines: &[&str]) -> ContentBlock {
        ContentBlock {
            lines: lines.iter().map(|s| s.to_string()).collect(),
            preceding_command: None,
            start_row: 0,
            end_row: lines.len(),
            timestamp: SystemTime::now(),
        }
    }

    fn make_rendered(text_lines: &[&str]) -> RenderedContent {
        RenderedContent {
            lines: text_lines.iter().map(|t| StyledLine::plain(t)).collect(),
            line_mapping: text_lines
                .iter()
                .enumerate()
                .map(|(i, _)| SourceLineMapping {
                    rendered_line: i,
                    source_line: Some(i),
                })
                .collect(),
            graphics: vec![],
            format_badge: "TEST".to_string(),
        }
    }

    #[test]
    fn test_new_buffer_defaults() {
        let buf = DualViewBuffer::new(make_source(&["hello", "world"]));
        assert_eq!(*buf.view_mode(), ViewMode::Rendered);
        assert!(buf.rendered().is_none());
        assert!(buf.needs_render(80));
    }

    #[test]
    fn test_toggle_view() {
        let mut buf = DualViewBuffer::new(make_source(&["hello"]));
        assert_eq!(*buf.view_mode(), ViewMode::Rendered);

        buf.toggle_view();
        assert_eq!(*buf.view_mode(), ViewMode::Source);

        buf.toggle_view();
        assert_eq!(*buf.view_mode(), ViewMode::Rendered);
    }

    #[test]
    fn test_source_text_always_available() {
        let buf = DualViewBuffer::new(make_source(&["line1", "line2"]));
        assert_eq!(buf.source_text(), "line1\nline2");
    }

    #[test]
    fn test_rendered_text_none_when_no_render() {
        let buf = DualViewBuffer::new(make_source(&["hello"]));
        assert!(buf.rendered_text().is_none());
    }

    #[test]
    fn test_rendered_text_after_set() {
        let mut buf = DualViewBuffer::new(make_source(&["hello"]));
        buf.set_rendered(make_rendered(&["HELLO"]), 80);
        assert_eq!(buf.rendered_text().unwrap(), "HELLO");
    }

    #[test]
    fn test_display_lines_source_fallback() {
        let buf = DualViewBuffer::new(make_source(&["raw"]));
        // Rendered mode but no rendered content — falls back to source.
        let lines = buf.display_lines();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].segments[0].text, "raw");
    }

    #[test]
    fn test_display_lines_rendered() {
        let mut buf = DualViewBuffer::new(make_source(&["raw"]));
        buf.set_rendered(make_rendered(&["RENDERED"]), 80);

        let lines = buf.display_lines();
        assert_eq!(lines[0].segments[0].text, "RENDERED");
    }

    #[test]
    fn test_display_lines_source_mode() {
        let mut buf = DualViewBuffer::new(make_source(&["raw"]));
        buf.set_rendered(make_rendered(&["RENDERED"]), 80);
        buf.toggle_view(); // Switch to Source

        let lines = buf.display_lines();
        assert_eq!(lines[0].segments[0].text, "raw");
    }

    #[test]
    fn test_needs_render_changes_with_width() {
        let mut buf = DualViewBuffer::new(make_source(&["hello"]));
        assert!(buf.needs_render(80));

        buf.set_rendered(make_rendered(&["hello"]), 80);
        assert!(!buf.needs_render(80));
        assert!(buf.needs_render(120)); // Width changed.
    }

    #[test]
    fn test_rendered_to_source_line() {
        let mut buf = DualViewBuffer::new(make_source(&["a", "b", "c"]));
        buf.set_rendered(make_rendered(&["A", "B", "C"]), 80);

        assert_eq!(buf.rendered_to_source_line(0), Some(0));
        assert_eq!(buf.rendered_to_source_line(2), Some(2));
        assert_eq!(buf.rendered_to_source_line(5), None);
    }

    #[test]
    fn test_source_to_rendered_lines() {
        let mut buf = DualViewBuffer::new(make_source(&["a", "b"]));
        let rendered = RenderedContent {
            lines: vec![
                StyledLine::plain("A-1"),
                StyledLine::plain("A-2"),
                StyledLine::plain("B"),
            ],
            line_mapping: vec![
                SourceLineMapping {
                    rendered_line: 0,
                    source_line: Some(0),
                },
                SourceLineMapping {
                    rendered_line: 1,
                    source_line: Some(0),
                },
                SourceLineMapping {
                    rendered_line: 2,
                    source_line: Some(1),
                },
            ],
            graphics: vec![],
            format_badge: "TEST".to_string(),
        };
        buf.set_rendered(rendered, 80);

        assert_eq!(buf.source_to_rendered_lines(0), vec![0, 1]);
        assert_eq!(buf.source_to_rendered_lines(1), vec![2]);
        assert_eq!(buf.source_to_rendered_lines(5), Vec::<usize>::new());
    }

    #[test]
    fn test_display_line_count() {
        let mut buf = DualViewBuffer::new(make_source(&["a", "b"]));
        assert_eq!(buf.display_line_count(), 2); // Source count (no render yet).

        buf.set_rendered(make_rendered(&["A", "B", "C"]), 80);
        assert_eq!(buf.display_line_count(), 3); // Rendered count.

        buf.toggle_view();
        assert_eq!(buf.display_line_count(), 2); // Source count.
    }

    #[test]
    fn test_display_lines_range() {
        let mut buf = DualViewBuffer::new(make_source(&["a", "b", "c", "d"]));
        buf.set_rendered(make_rendered(&["A", "B", "C", "D"]), 80);

        let range = buf.display_lines_range(1, 2);
        assert_eq!(range.len(), 2);
        assert_eq!(range[0].segments[0].text, "B");
        assert_eq!(range[1].segments[0].text, "C");

        // Out of range.
        let range = buf.display_lines_range(10, 5);
        assert!(range.is_empty());
    }

    #[test]
    fn test_is_virtual() {
        let small = DualViewBuffer::new(make_source(&["a", "b"]));
        assert!(!small.is_virtual());

        // Create a large block.
        let many_lines: Vec<String> = (0..10_001).map(|i| format!("line {i}")).collect();
        let large_source = ContentBlock {
            lines: many_lines,
            preceding_command: None,
            start_row: 0,
            end_row: 10_001,
            timestamp: SystemTime::now(),
        };
        let large = DualViewBuffer::new(large_source);
        assert!(large.is_virtual());
    }

    #[test]
    fn test_content_hash_same_for_identical() {
        let h1 = compute_content_hash(&["hello".to_string(), "world".to_string()]);
        let h2 = compute_content_hash(&["hello".to_string(), "world".to_string()]);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_content_hash_different_for_different() {
        let h1 = compute_content_hash(&["hello".to_string()]);
        let h2 = compute_content_hash(&["world".to_string()]);
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_content_hash_empty() {
        let h1 = compute_content_hash(&[]);
        let h2 = compute_content_hash(&[]);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_content_hash_order_matters() {
        let h1 = compute_content_hash(&["a".to_string(), "b".to_string()]);
        let h2 = compute_content_hash(&["b".to_string(), "a".to_string()]);
        assert_ne!(h1, h2);
    }
}
