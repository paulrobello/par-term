//! Diff renderer with green/red coloring, word-level highlighting, line number
//! gutter, file/hunk header styling, and optional side-by-side mode.
//!
//! Parses unified diff format into structured hunks, then renders with:
//! - **Line-level coloring**: green for additions, red for removals
//! - **File headers** (`---`/`+++`): bold with distinct color
//! - **Hunk headers** (`@@`): cyan/blue with line range info
//! - **Word-level diff**: highlights changed words within paired +/- lines
//! - **Line number gutter**: old/new line numbers from hunk headers
//! - **Side-by-side mode**: when terminal is wide enough
//!
//! Helper sub-modules:
//! - [`diff_parser`] — unified diff parsing (DiffFile, DiffHunk, DiffLine)
//! - [`diff_word`] — word-level LCS diff and segment building

#[path = "diff_parser.rs"]
mod diff_parser;
#[path = "diff_word.rs"]
mod diff_word;

use super::push_line;
use crate::prettifier::registry::RendererRegistry;
use crate::prettifier::traits::{ContentRenderer, RenderError, RendererConfig, ThemeColors};
use crate::prettifier::types::{
    ContentBlock, RenderedContent, RendererCapability, SourceLineMapping, StyledLine, StyledSegment,
};

use diff_parser::{DiffFile, DiffHunk, DiffLine, parse_unified_diff};
use diff_word::word_diff_segments;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Display style for diff output.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum DiffStyle {
    /// Traditional unified diff (inline).
    Inline,
    /// Side-by-side removed/added columns.
    SideBySide,
    /// Auto-select based on terminal width.
    #[default]
    Auto,
}

/// Configuration for the diff renderer.
#[derive(Clone, Debug)]
pub struct DiffRendererConfig {
    /// Display style (Inline, SideBySide, or Auto).
    pub style: DiffStyle,
    /// Minimum terminal columns for side-by-side mode (default: 160).
    pub side_by_side_min_width: usize,
    /// Enable word-level highlighting within changed lines (default: true).
    pub word_diff: bool,
    /// Show line number gutter (default: true).
    pub show_line_numbers: bool,
}

impl Default for DiffRendererConfig {
    fn default() -> Self {
        Self {
            style: DiffStyle::Auto,
            side_by_side_min_width: 160,
            word_diff: true,
            show_line_numbers: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Line-state tracker
// ---------------------------------------------------------------------------

/// Tracks current line numbers while rendering.
struct DiffLineState {
    old_line: usize,
    new_line: usize,
}

// ---------------------------------------------------------------------------
// DiffRenderer
// ---------------------------------------------------------------------------

/// Renders diff content with syntax coloring and optional side-by-side mode.
pub struct DiffRenderer {
    config: DiffRendererConfig,
}

impl DiffRenderer {
    /// Create a new diff renderer with the given configuration.
    pub fn new(config: DiffRendererConfig) -> Self {
        Self { config }
    }

    /// Determine whether to use side-by-side mode based on config and terminal width.
    fn use_side_by_side(&self, terminal_width: usize) -> bool {
        match self.config.style {
            DiffStyle::SideBySide => true,
            DiffStyle::Inline => false,
            DiffStyle::Auto => terminal_width >= self.config.side_by_side_min_width,
        }
    }

    /// Render a diff in inline mode.
    fn render_inline(
        &self,
        files: &[DiffFile],
        lines: &mut Vec<StyledLine>,
        line_mapping: &mut Vec<SourceLineMapping>,
        theme: &ThemeColors,
    ) {
        for file in files {
            // Render file diff header
            if !file.header_lines.is_empty() {
                for header_line in &file.header_lines {
                    push_line(
                        lines,
                        line_mapping,
                        vec![StyledSegment {
                            text: header_line.clone(),
                            fg: Some(theme.palette[15]), // Bright white
                            bold: true,
                            ..Default::default()
                        }],
                        None,
                    );
                }
            }

            // Render --- / +++ file headers
            if !file.old_path.is_empty() {
                push_line(
                    lines,
                    line_mapping,
                    vec![StyledSegment {
                        text: format!("--- {}", file.old_path),
                        fg: Some(theme.palette[1]), // Red
                        bold: true,
                        ..Default::default()
                    }],
                    None,
                );
                push_line(
                    lines,
                    line_mapping,
                    vec![StyledSegment {
                        text: format!("+++ {}", file.new_path),
                        fg: Some(theme.palette[2]), // Green
                        bold: true,
                        ..Default::default()
                    }],
                    None,
                );
            }

            for hunk in &file.hunks {
                self.render_hunk_inline(hunk, lines, line_mapping, theme);
            }
        }
    }

    /// Render a single hunk in inline mode.
    fn render_hunk_inline(
        &self,
        hunk: &DiffHunk,
        lines: &mut Vec<StyledLine>,
        line_mapping: &mut Vec<SourceLineMapping>,
        theme: &ThemeColors,
    ) {
        // Hunk header
        let hunk_header = format!(
            "@@ -{},{} +{},{} @@{}",
            hunk.old_start,
            hunk.old_count,
            hunk.new_start,
            hunk.new_count,
            if hunk.header_text.is_empty() {
                String::new()
            } else {
                format!(" {}", hunk.header_text)
            }
        );
        push_line(
            lines,
            line_mapping,
            vec![StyledSegment {
                text: hunk_header,
                fg: Some(theme.palette[6]), // Cyan
                ..Default::default()
            }],
            None,
        );

        let mut state = DiffLineState {
            old_line: hunk.old_start,
            new_line: hunk.new_start,
        };

        // Collect lines for word-diff pairing
        let hunk_lines = &hunk.lines;
        let mut i = 0;

        while i < hunk_lines.len() {
            match &hunk_lines[i] {
                DiffLine::Context(text) => {
                    let mut segments = Vec::new();
                    if self.config.show_line_numbers {
                        segments.push(gutter_segment(
                            Some(state.old_line),
                            Some(state.new_line),
                            theme,
                        ));
                    }
                    segments.push(StyledSegment {
                        text: format!(" {text}"),
                        ..Default::default()
                    });
                    push_line(lines, line_mapping, segments, None);
                    state.old_line += 1;
                    state.new_line += 1;
                    i += 1;
                }
                DiffLine::Removed(removed_text) => {
                    // Check if this is a paired remove/add for word-level diff
                    if self.config.word_diff {
                        // Collect consecutive removed lines
                        let remove_start = i;
                        let mut remove_end = i;
                        while remove_end < hunk_lines.len() {
                            if matches!(&hunk_lines[remove_end], DiffLine::Removed(_)) {
                                remove_end += 1;
                            } else {
                                break;
                            }
                        }
                        // Collect consecutive added lines
                        let add_start = remove_end;
                        let mut add_end = remove_end;
                        while add_end < hunk_lines.len() {
                            if matches!(&hunk_lines[add_end], DiffLine::Added(_)) {
                                add_end += 1;
                            } else {
                                break;
                            }
                        }

                        let removed_count = remove_end - remove_start;
                        let added_count = add_end - add_start;

                        if added_count > 0 && removed_count > 0 {
                            // Pair up remove/add lines for word-level diff
                            let pair_count = removed_count.min(added_count);

                            for j in 0..removed_count {
                                if let DiffLine::Removed(r_text) = &hunk_lines[remove_start + j] {
                                    let mut segments = Vec::new();
                                    if self.config.show_line_numbers {
                                        segments.push(gutter_segment(
                                            Some(state.old_line),
                                            None,
                                            theme,
                                        ));
                                    }
                                    segments.push(StyledSegment {
                                        text: "-".to_string(),
                                        fg: Some(theme.palette[1]),
                                        ..Default::default()
                                    });
                                    if j < pair_count {
                                        if let DiffLine::Added(a_text) = &hunk_lines[add_start + j]
                                        {
                                            segments.extend(word_diff_segments(
                                                r_text,
                                                a_text,
                                                theme.palette[1],
                                                [100, 0, 0],
                                            ));
                                        }
                                    } else {
                                        segments.push(StyledSegment {
                                            text: r_text.clone(),
                                            fg: Some(theme.palette[1]),
                                            ..Default::default()
                                        });
                                    }
                                    push_line(lines, line_mapping, segments, None);
                                    state.old_line += 1;
                                }
                            }
                            for j in 0..added_count {
                                if let DiffLine::Added(a_text) = &hunk_lines[add_start + j] {
                                    let mut segments = Vec::new();
                                    if self.config.show_line_numbers {
                                        segments.push(gutter_segment(
                                            None,
                                            Some(state.new_line),
                                            theme,
                                        ));
                                    }
                                    segments.push(StyledSegment {
                                        text: "+".to_string(),
                                        fg: Some(theme.palette[2]),
                                        ..Default::default()
                                    });
                                    if j < pair_count {
                                        if let DiffLine::Removed(r_text) =
                                            &hunk_lines[remove_start + j]
                                        {
                                            segments.extend(word_diff_segments(
                                                a_text,
                                                r_text,
                                                theme.palette[2],
                                                [0, 80, 0],
                                            ));
                                        }
                                    } else {
                                        segments.push(StyledSegment {
                                            text: a_text.clone(),
                                            fg: Some(theme.palette[2]),
                                            ..Default::default()
                                        });
                                    }
                                    push_line(lines, line_mapping, segments, None);
                                    state.new_line += 1;
                                }
                            }
                            i = add_end;
                        } else {
                            // No paired add — plain removed line
                            let mut segments = Vec::new();
                            if self.config.show_line_numbers {
                                segments.push(gutter_segment(Some(state.old_line), None, theme));
                            }
                            segments.push(StyledSegment {
                                text: format!("-{removed_text}"),
                                fg: Some(theme.palette[1]), // Red
                                ..Default::default()
                            });
                            push_line(lines, line_mapping, segments, None);
                            state.old_line += 1;
                            i += 1;
                        }
                    } else {
                        // No word diff — plain removed line
                        let mut segments = Vec::new();
                        if self.config.show_line_numbers {
                            segments.push(gutter_segment(Some(state.old_line), None, theme));
                        }
                        segments.push(StyledSegment {
                            text: format!("-{removed_text}"),
                            fg: Some(theme.palette[1]), // Red
                            ..Default::default()
                        });
                        push_line(lines, line_mapping, segments, None);
                        state.old_line += 1;
                        i += 1;
                    }
                }
                DiffLine::Added(added_text) => {
                    // Standalone added line (not paired with a removed line)
                    let mut segments = Vec::new();
                    if self.config.show_line_numbers {
                        segments.push(gutter_segment(None, Some(state.new_line), theme));
                    }
                    segments.push(StyledSegment {
                        text: format!("+{added_text}"),
                        fg: Some(theme.palette[2]), // Green
                        ..Default::default()
                    });
                    push_line(lines, line_mapping, segments, None);
                    state.new_line += 1;
                    i += 1;
                }
            }
        }
    }

    /// Render diff in side-by-side mode.
    fn render_side_by_side(
        &self,
        files: &[DiffFile],
        terminal_width: usize,
        lines: &mut Vec<StyledLine>,
        line_mapping: &mut Vec<SourceLineMapping>,
        theme: &ThemeColors,
    ) {
        // Each side gets half the terminal width minus the divider
        let half_width = (terminal_width.saturating_sub(3)) / 2;
        let gutter_width = if self.config.show_line_numbers { 6 } else { 0 };
        let content_width = half_width.saturating_sub(gutter_width + 1); // +1 for +/- prefix

        for file in files {
            // File header spanning full width
            if !file.header_lines.is_empty() {
                for header_line in &file.header_lines {
                    push_line(
                        lines,
                        line_mapping,
                        vec![StyledSegment {
                            text: header_line.clone(),
                            fg: Some(theme.palette[15]),
                            bold: true,
                            ..Default::default()
                        }],
                        None,
                    );
                }
            }

            // --- / +++ headers
            if !file.old_path.is_empty() {
                push_line(
                    lines,
                    line_mapping,
                    vec![
                        StyledSegment {
                            text: format!("--- {}", file.old_path),
                            fg: Some(theme.palette[1]),
                            bold: true,
                            ..Default::default()
                        },
                        StyledSegment {
                            text: " | ".to_string(),
                            fg: Some(theme.palette[8]),
                            ..Default::default()
                        },
                        StyledSegment {
                            text: format!("+++ {}", file.new_path),
                            fg: Some(theme.palette[2]),
                            bold: true,
                            ..Default::default()
                        },
                    ],
                    None,
                );
            }

            for hunk in &file.hunks {
                // Hunk header
                let hunk_header = format!(
                    "@@ -{},{} +{},{} @@{}",
                    hunk.old_start,
                    hunk.old_count,
                    hunk.new_start,
                    hunk.new_count,
                    if hunk.header_text.is_empty() {
                        String::new()
                    } else {
                        format!(" {}", hunk.header_text)
                    }
                );
                push_line(
                    lines,
                    line_mapping,
                    vec![StyledSegment {
                        text: hunk_header,
                        fg: Some(theme.palette[6]),
                        ..Default::default()
                    }],
                    None,
                );

                // Build side-by-side rows
                let rows = build_side_by_side_rows(&hunk.lines, hunk.old_start, hunk.new_start);

                for row in &rows {
                    let mut segments = Vec::new();

                    // Left side (old/removed)
                    match &row.left {
                        SbsCell::Context(ln, text) => {
                            if self.config.show_line_numbers {
                                segments.push(line_num_segment(Some(*ln), gutter_width, theme));
                            }
                            let truncated = truncate_str(text, content_width);
                            let padded = format!(" {truncated:<width$}", width = content_width);
                            segments.push(StyledSegment {
                                text: padded,
                                ..Default::default()
                            });
                        }
                        SbsCell::Removed(ln, text) => {
                            if self.config.show_line_numbers {
                                segments.push(line_num_segment(Some(*ln), gutter_width, theme));
                            }
                            let truncated = truncate_str(text, content_width);
                            let padded = format!("-{truncated:<width$}", width = content_width);
                            segments.push(StyledSegment {
                                text: padded,
                                fg: Some(theme.palette[1]),
                                ..Default::default()
                            });
                        }
                        SbsCell::Empty => {
                            if self.config.show_line_numbers {
                                segments.push(line_num_segment(None, gutter_width, theme));
                            }
                            segments.push(StyledSegment {
                                text: " ".repeat(content_width + 1),
                                ..Default::default()
                            });
                        }
                    }

                    // Divider
                    segments.push(StyledSegment {
                        text: " | ".to_string(),
                        fg: Some(theme.palette[8]),
                        ..Default::default()
                    });

                    // Right side (new/added)
                    match &row.right {
                        SbsCell::Context(ln, text) => {
                            if self.config.show_line_numbers {
                                segments.push(line_num_segment(Some(*ln), gutter_width, theme));
                            }
                            let truncated = truncate_str(text, content_width);
                            segments.push(StyledSegment {
                                text: format!(" {truncated}"),
                                ..Default::default()
                            });
                        }
                        SbsCell::Removed(ln, text) => {
                            // This shouldn't happen on right side but handle gracefully
                            if self.config.show_line_numbers {
                                segments.push(line_num_segment(Some(*ln), gutter_width, theme));
                            }
                            let truncated = truncate_str(text, content_width);
                            segments.push(StyledSegment {
                                text: format!("+{truncated}"),
                                fg: Some(theme.palette[2]),
                                ..Default::default()
                            });
                        }
                        SbsCell::Empty => {
                            if self.config.show_line_numbers {
                                segments.push(line_num_segment(None, gutter_width, theme));
                            }
                        }
                    }

                    push_line(lines, line_mapping, segments, None);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Side-by-side helpers
// ---------------------------------------------------------------------------

/// A row in side-by-side mode with left (old) and right (new) columns.
struct SbsRow {
    left: SbsCell,
    right: SbsCell,
}

/// A cell in a side-by-side row.
enum SbsCell {
    Context(usize, String),
    Removed(usize, String),
    Empty,
}

/// Build side-by-side rows from hunk lines.
fn build_side_by_side_rows(
    hunk_lines: &[DiffLine],
    old_start: usize,
    new_start: usize,
) -> Vec<SbsRow> {
    let mut rows = Vec::new();
    let mut old_line = old_start;
    let mut new_line = new_start;
    let mut i = 0;

    while i < hunk_lines.len() {
        match &hunk_lines[i] {
            DiffLine::Context(text) => {
                rows.push(SbsRow {
                    left: SbsCell::Context(old_line, text.clone()),
                    right: SbsCell::Context(new_line, text.clone()),
                });
                old_line += 1;
                new_line += 1;
                i += 1;
            }
            DiffLine::Removed(_) => {
                // Collect consecutive removed/added for pairing
                let remove_start = i;
                while i < hunk_lines.len() && matches!(&hunk_lines[i], DiffLine::Removed(_)) {
                    i += 1;
                }
                let add_start = i;
                while i < hunk_lines.len() && matches!(&hunk_lines[i], DiffLine::Added(_)) {
                    i += 1;
                }

                let removed: Vec<_> = hunk_lines[remove_start..add_start]
                    .iter()
                    .map(|l| match l {
                        DiffLine::Removed(t) => t.clone(),
                        _ => String::new(),
                    })
                    .collect();
                let added: Vec<_> = hunk_lines[add_start..i]
                    .iter()
                    .map(|l| match l {
                        DiffLine::Added(t) => t.clone(),
                        _ => String::new(),
                    })
                    .collect();

                let max_len = removed.len().max(added.len());
                for j in 0..max_len {
                    let left = if j < removed.len() {
                        let ln = old_line;
                        old_line += 1;
                        SbsCell::Removed(ln, removed[j].clone())
                    } else {
                        SbsCell::Empty
                    };
                    let right = if j < added.len() {
                        let ln = new_line;
                        new_line += 1;
                        // Reuse Removed variant for added (displayed with + on right side)
                        SbsCell::Removed(ln, added[j].clone())
                    } else {
                        SbsCell::Empty
                    };
                    rows.push(SbsRow { left, right });
                }
            }
            DiffLine::Added(text) => {
                rows.push(SbsRow {
                    left: SbsCell::Empty,
                    right: SbsCell::Removed(new_line, text.clone()),
                });
                new_line += 1;
                i += 1;
            }
        }
    }

    rows
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Create a line number gutter segment for inline mode.
fn gutter_segment(old: Option<usize>, new: Option<usize>, theme: &ThemeColors) -> StyledSegment {
    let old_str = old
        .map(|n| format!("{n:>4}"))
        .unwrap_or_else(|| "    ".to_string());
    let new_str = new
        .map(|n| format!("{n:>4}"))
        .unwrap_or_else(|| "    ".to_string());
    StyledSegment {
        text: format!("{old_str} {new_str} "),
        fg: Some(theme.palette[8]), // Dim grey
        ..Default::default()
    }
}

/// Create a line number segment for side-by-side mode.
fn line_num_segment(num: Option<usize>, width: usize, theme: &ThemeColors) -> StyledSegment {
    let text = num
        .map(|n| format!("{n:>width$} ", width = width - 1))
        .unwrap_or_else(|| format!("{:>width$} ", "", width = width - 1));
    StyledSegment {
        text,
        fg: Some(theme.palette[8]),
        ..Default::default()
    }
}

/// Truncate a string to fit within a given width.
fn truncate_str(s: &str, max_width: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_width {
        s.to_string()
    } else if max_width > 1 {
        let truncated: String = s.chars().take(max_width - 1).collect();
        format!("{truncated}~")
    } else {
        "~".to_string()
    }
}

// ---------------------------------------------------------------------------
// ContentRenderer implementation
// ---------------------------------------------------------------------------

impl ContentRenderer for DiffRenderer {
    fn format_id(&self) -> &str {
        "diff"
    }

    fn display_name(&self) -> &str {
        "Diff"
    }

    fn capabilities(&self) -> Vec<RendererCapability> {
        vec![RendererCapability::TextStyling]
    }

    fn render(
        &self,
        content: &ContentBlock,
        config: &RendererConfig,
    ) -> Result<RenderedContent, RenderError> {
        let files = parse_unified_diff(&content.lines);

        if files.is_empty() {
            return Err(RenderError::RenderFailed(
                "No diff content found".to_string(),
            ));
        }

        let mut lines = Vec::new();
        let mut line_mapping = Vec::new();

        if self.use_side_by_side(config.terminal_width) {
            self.render_side_by_side(
                &files,
                config.terminal_width,
                &mut lines,
                &mut line_mapping,
                &config.theme_colors,
            );
        } else {
            self.render_inline(&files, &mut lines, &mut line_mapping, &config.theme_colors);
        }

        Ok(RenderedContent {
            lines,
            line_mapping,
            graphics: vec![],
            format_badge: "DIFF".to_string(),
        })
    }

    fn format_badge(&self) -> &str {
        "DIFF"
    }
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register the diff renderer with the registry.
pub fn register_diff_renderer(registry: &mut RendererRegistry, config: &DiffRendererConfig) {
    registry.register_renderer("diff", Box::new(DiffRenderer::new(config.clone())));
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prettifier::traits::RendererConfig;
    use crate::prettifier::types::ContentBlock;
    use diff_parser::parse_hunk_header;
    use diff_word::split_into_words;
    use std::time::SystemTime;

    fn test_config() -> RendererConfig {
        RendererConfig {
            terminal_width: 80,
            ..Default::default()
        }
    }

    fn wide_config() -> RendererConfig {
        RendererConfig {
            terminal_width: 200,
            ..Default::default()
        }
    }

    fn renderer() -> DiffRenderer {
        DiffRenderer::new(DiffRendererConfig::default())
    }

    fn inline_renderer() -> DiffRenderer {
        DiffRenderer::new(DiffRendererConfig {
            style: DiffStyle::Inline,
            ..Default::default()
        })
    }

    fn make_block(lines: &[&str]) -> ContentBlock {
        ContentBlock {
            lines: lines.iter().map(|s| s.to_string()).collect(),
            preceding_command: None,
            start_row: 0,
            end_row: lines.len(),
            timestamp: SystemTime::now(),
        }
    }

    fn all_text(lines: &[StyledLine]) -> String {
        lines
            .iter()
            .map(|l| {
                l.segments
                    .iter()
                    .map(|s| s.text.as_str())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    // -- Trait methods --

    #[test]
    fn test_format_id() {
        let r = renderer();
        assert_eq!(r.format_id(), "diff");
        assert_eq!(r.display_name(), "Diff");
        assert_eq!(r.format_badge(), "DIFF");
        assert_eq!(r.capabilities(), vec![RendererCapability::TextStyling]);
    }

    // -- Parsing --

    #[test]
    fn test_parse_git_diff() {
        let lines: Vec<String> = vec![
            "diff --git a/src/main.rs b/src/main.rs",
            "index abc1234..def5678 100644",
            "--- a/src/main.rs",
            "+++ b/src/main.rs",
            "@@ -1,3 +1,4 @@",
            " line1",
            "+added",
            " line2",
            " line3",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        let files = parse_unified_diff(&lines);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].old_path, "a/src/main.rs");
        assert_eq!(files[0].new_path, "b/src/main.rs");
        assert_eq!(files[0].hunks.len(), 1);
        assert_eq!(files[0].hunks[0].old_start, 1);
        assert_eq!(files[0].hunks[0].old_count, 3);
        assert_eq!(files[0].hunks[0].new_start, 1);
        assert_eq!(files[0].hunks[0].new_count, 4);
        assert_eq!(files[0].hunks[0].lines.len(), 4);
    }

    #[test]
    fn test_parse_multiple_files() {
        let lines: Vec<String> = vec![
            "diff --git a/file1.rs b/file1.rs",
            "--- a/file1.rs",
            "+++ b/file1.rs",
            "@@ -1,2 +1,2 @@",
            "-old1",
            "+new1",
            "diff --git a/file2.rs b/file2.rs",
            "--- a/file2.rs",
            "+++ b/file2.rs",
            "@@ -1,2 +1,2 @@",
            "-old2",
            "+new2",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        let files = parse_unified_diff(&lines);
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn test_parse_multiple_hunks() {
        let lines: Vec<String> = vec![
            "diff --git a/file.rs b/file.rs",
            "--- a/file.rs",
            "+++ b/file.rs",
            "@@ -1,3 +1,3 @@",
            " context",
            "-old",
            "+new",
            "@@ -10,3 +10,3 @@",
            " another",
            "-old2",
            "+new2",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        let files = parse_unified_diff(&lines);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].hunks.len(), 2);
        assert_eq!(files[0].hunks[0].old_start, 1);
        assert_eq!(files[0].hunks[1].old_start, 10);
    }

    #[test]
    fn test_parse_non_git_diff() {
        let lines: Vec<String> = vec![
            "--- file.txt.orig",
            "+++ file.txt",
            "@@ -1,3 +1,3 @@",
            " line1",
            "-old",
            "+new",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        let files = parse_unified_diff(&lines);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].old_path, "file.txt.orig");
        assert_eq!(files[0].new_path, "file.txt");
    }

    // -- Line coloring --

    #[test]
    fn test_added_lines_green() {
        let r = inline_renderer();
        let block = make_block(&[
            "diff --git a/f b/f",
            "--- a/f",
            "+++ b/f",
            "@@ -1,2 +1,3 @@",
            " ctx",
            "+added line",
        ]);
        let result = r.render(&block, &test_config()).unwrap();
        let theme = ThemeColors::default();

        let added_seg = result
            .lines
            .iter()
            .flat_map(|l| &l.segments)
            .find(|s| s.text.contains("+added"))
            .unwrap();
        assert_eq!(added_seg.fg, Some(theme.palette[2])); // Green
    }

    #[test]
    fn test_removed_lines_red() {
        let r = DiffRenderer::new(DiffRendererConfig {
            style: DiffStyle::Inline,
            word_diff: false,
            ..Default::default()
        });
        let block = make_block(&[
            "diff --git a/f b/f",
            "--- a/f",
            "+++ b/f",
            "@@ -1,2 +1,1 @@",
            "-removed line",
            " ctx",
        ]);
        let result = r.render(&block, &test_config()).unwrap();
        let theme = ThemeColors::default();

        let removed_seg = result
            .lines
            .iter()
            .flat_map(|l| &l.segments)
            .find(|s| s.text.contains("-removed"))
            .unwrap();
        assert_eq!(removed_seg.fg, Some(theme.palette[1])); // Red
    }

    #[test]
    fn test_file_headers_bold() {
        let r = inline_renderer();
        let block = make_block(&[
            "diff --git a/f b/f",
            "--- a/f",
            "+++ b/f",
            "@@ -1,1 +1,1 @@",
            "-old",
            "+new",
        ]);
        let result = r.render(&block, &test_config()).unwrap();

        let old_header = result
            .lines
            .iter()
            .flat_map(|l| &l.segments)
            .find(|s| s.text.starts_with("--- "))
            .unwrap();
        assert!(old_header.bold);

        let new_header = result
            .lines
            .iter()
            .flat_map(|l| &l.segments)
            .find(|s| s.text.starts_with("+++ "))
            .unwrap();
        assert!(new_header.bold);
    }

    #[test]
    fn test_hunk_headers_cyan() {
        let r = inline_renderer();
        let block = make_block(&[
            "diff --git a/f b/f",
            "--- a/f",
            "+++ b/f",
            "@@ -1,3 +1,3 @@ fn main()",
            " context",
        ]);
        let result = r.render(&block, &test_config()).unwrap();
        let theme = ThemeColors::default();

        let hunk = result
            .lines
            .iter()
            .flat_map(|l| &l.segments)
            .find(|s| s.text.contains("@@"))
            .unwrap();
        assert_eq!(hunk.fg, Some(theme.palette[6])); // Cyan
    }

    #[test]
    fn test_context_lines_default_color() {
        let r = inline_renderer();
        let block = make_block(&[
            "diff --git a/f b/f",
            "--- a/f",
            "+++ b/f",
            "@@ -1,3 +1,3 @@",
            " context line",
        ]);
        let result = r.render(&block, &test_config()).unwrap();

        let ctx = result
            .lines
            .iter()
            .flat_map(|l| &l.segments)
            .find(|s| s.text.contains("context line"))
            .unwrap();
        assert!(ctx.fg.is_none()); // Default foreground
    }

    // -- Word-level diff --

    #[test]
    fn test_word_diff_highlighting() {
        let r = inline_renderer();
        let block = make_block(&[
            "diff --git a/f b/f",
            "--- a/f",
            "+++ b/f",
            "@@ -1,1 +1,1 @@",
            "-the old word here",
            "+the new word here",
        ]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        assert!(text.contains("old"));
        assert!(text.contains("new"));
    }

    #[test]
    fn test_word_diff_disabled() {
        let r = DiffRenderer::new(DiffRendererConfig {
            style: DiffStyle::Inline,
            word_diff: false,
            ..Default::default()
        });
        let block = make_block(&[
            "diff --git a/f b/f",
            "--- a/f",
            "+++ b/f",
            "@@ -1,1 +1,1 @@",
            "-old line",
            "+new line",
        ]);
        let result = r.render(&block, &test_config()).unwrap();
        // Should still render, just without word-level highlighting
        let text = all_text(&result.lines);
        assert!(text.contains("-old line"));
        assert!(text.contains("+new line"));
    }

    // -- Line numbers --

    #[test]
    fn test_line_numbers_shown() {
        let r = inline_renderer();
        let block = make_block(&[
            "diff --git a/f b/f",
            "--- a/f",
            "+++ b/f",
            "@@ -10,3 +10,3 @@",
            " context",
            "-old",
            "+new",
        ]);
        let result = r.render(&block, &test_config()).unwrap();
        let text = all_text(&result.lines);
        assert!(text.contains("10"));
    }

    #[test]
    fn test_line_numbers_hidden() {
        let r = DiffRenderer::new(DiffRendererConfig {
            style: DiffStyle::Inline,
            show_line_numbers: false,
            ..Default::default()
        });
        let block = make_block(&[
            "diff --git a/f b/f",
            "--- a/f",
            "+++ b/f",
            "@@ -10,3 +10,3 @@",
            " context",
        ]);
        let result = r.render(&block, &test_config()).unwrap();
        // Check that there is no gutter segment (the first segment should not be a line number)
        let first_ctx_line = result
            .lines
            .iter()
            .find(|l| l.segments.iter().any(|s| s.text.contains("context")));
        assert!(first_ctx_line.is_some());
        // With show_line_numbers off, first segment should be the content itself
        let segments = &first_ctx_line.unwrap().segments;
        assert!(segments[0].text.contains("context"));
    }

    // -- Side-by-side mode --

    #[test]
    fn test_auto_style_inline_narrow() {
        let r = renderer();
        assert!(!r.use_side_by_side(80));
    }

    #[test]
    fn test_auto_style_side_by_side_wide() {
        let r = renderer();
        assert!(r.use_side_by_side(200));
    }

    #[test]
    fn test_forced_inline() {
        let r = DiffRenderer::new(DiffRendererConfig {
            style: DiffStyle::Inline,
            ..Default::default()
        });
        assert!(!r.use_side_by_side(200));
    }

    #[test]
    fn test_forced_side_by_side() {
        let r = DiffRenderer::new(DiffRendererConfig {
            style: DiffStyle::SideBySide,
            ..Default::default()
        });
        assert!(r.use_side_by_side(80));
    }

    #[test]
    fn test_side_by_side_render() {
        let r = DiffRenderer::new(DiffRendererConfig {
            style: DiffStyle::SideBySide,
            ..Default::default()
        });
        let block = make_block(&[
            "diff --git a/f b/f",
            "--- a/f",
            "+++ b/f",
            "@@ -1,3 +1,3 @@",
            " context",
            "-old line",
            "+new line",
        ]);
        let result = r.render(&block, &wide_config()).unwrap();
        let text = all_text(&result.lines);
        // Side-by-side should have the divider
        assert!(text.contains(" | "));
    }

    // -- Error cases --

    #[test]
    fn test_empty_diff_error() {
        let r = renderer();
        let block = make_block(&["not a diff at all"]);
        let result = r.render(&block, &test_config());
        assert!(result.is_err());
    }

    // -- Line mappings --

    #[test]
    fn test_line_mappings_populated() {
        let r = inline_renderer();
        let block = make_block(&[
            "diff --git a/f b/f",
            "--- a/f",
            "+++ b/f",
            "@@ -1,2 +1,2 @@",
            "-old",
            "+new",
        ]);
        let result = r.render(&block, &test_config()).unwrap();
        assert_eq!(result.line_mapping.len(), result.lines.len());
    }

    // -- Registration --

    #[test]
    fn test_register_diff_renderer() {
        let mut registry = RendererRegistry::new(0.6);
        register_diff_renderer(&mut registry, &DiffRendererConfig::default());
        assert_eq!(registry.renderer_count(), 1);
        assert!(registry.get_renderer("diff").is_some());
        assert_eq!(
            registry.get_renderer("diff").unwrap().display_name(),
            "Diff"
        );
    }

    // -- Config defaults --

    #[test]
    fn test_config_defaults() {
        let config = DiffRendererConfig::default();
        assert_eq!(config.style, DiffStyle::Auto);
        assert_eq!(config.side_by_side_min_width, 160);
        assert!(config.word_diff);
        assert!(config.show_line_numbers);
    }

    // -- Hunk header parsing --

    #[test]
    fn test_hunk_header_parsing() {
        let (old_s, old_c, new_s, new_c, text) = parse_hunk_header("@@ -10,5 +20,7 @@ fn main()");
        assert_eq!(old_s, 10);
        assert_eq!(old_c, 5);
        assert_eq!(new_s, 20);
        assert_eq!(new_c, 7);
        assert_eq!(text, "fn main()");
    }

    #[test]
    fn test_hunk_header_no_count() {
        let (old_s, old_c, new_s, new_c, _) = parse_hunk_header("@@ -1 +1 @@");
        assert_eq!(old_s, 1);
        assert_eq!(old_c, 1);
        assert_eq!(new_s, 1);
        assert_eq!(new_c, 1);
    }

    // -- Word splitting --

    #[test]
    fn test_split_into_words() {
        let words = split_into_words("hello world");
        assert_eq!(words, vec!["hello", " ", "world"]);
    }

    #[test]
    fn test_split_into_words_punctuation() {
        let words = split_into_words("fn(a, b)");
        assert_eq!(words, vec!["fn", "(", "a", ",", " ", "b", ")"]);
    }

    // -- Format badge --

    #[test]
    fn test_format_badge() {
        let r = renderer();
        let block = make_block(&[
            "diff --git a/f b/f",
            "--- a/f",
            "+++ b/f",
            "@@ -1,1 +1,1 @@",
            "-old",
            "+new",
        ]);
        let result = r.render(&block, &test_config()).unwrap();
        assert_eq!(result.format_badge, "DIFF");
    }
}
