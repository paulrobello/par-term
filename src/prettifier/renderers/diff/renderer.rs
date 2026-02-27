//! DiffRenderer struct and ContentRenderer implementation.

use super::super::push_line;
use super::config::{DiffLineState, DiffRendererConfig, DiffStyle};
use super::diff_parser::{DiffFile, DiffHunk, DiffLine, parse_unified_diff};
use super::diff_word::word_diff_segments;
use super::helpers::{gutter_segment, line_num_segment, truncate_str};
use super::side_by_side::{SbsCell, build_side_by_side_rows};
use crate::prettifier::registry::RendererRegistry;
use crate::prettifier::traits::{ContentRenderer, RenderError, RendererConfig, ThemeColors};
use crate::prettifier::types::{
    ContentBlock, RenderedContent, RendererCapability, SourceLineMapping, StyledLine, StyledSegment,
};

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
    pub(super) fn use_side_by_side(&self, terminal_width: usize) -> bool {
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
                                        if let DiffLine::Added(a_text) =
                                            &hunk_lines[add_start + j]
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
                                segments
                                    .push(line_num_segment(Some(*ln), gutter_width, theme));
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
                                segments
                                    .push(line_num_segment(Some(*ln), gutter_width, theme));
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
                                segments
                                    .push(line_num_segment(Some(*ln), gutter_width, theme));
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
                                segments
                                    .push(line_num_segment(Some(*ln), gutter_width, theme));
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

/// Register the diff renderer with the registry.
pub fn register_diff_renderer(registry: &mut RendererRegistry, config: &DiffRendererConfig) {
    registry.register_renderer("diff", Box::new(DiffRenderer::new(config.clone())));
}
