//! DiffRenderer struct and ContentRenderer implementation.

use super::super::push_line;
use super::config::{DiffRendererConfig, DiffStyle};
use super::diff_parser::parse_unified_diff;
use super::helpers::{line_num_segment, truncate_str};
use super::inline_render::render_inline;
use super::side_by_side::{SbsCell, build_side_by_side_rows};
use crate::registry::RendererRegistry;
use crate::traits::{ContentRenderer, RenderError, RendererConfig, ThemeColors};
use crate::types::{
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

    /// Render diff in side-by-side mode.
    fn render_side_by_side(
        &self,
        files: &[super::diff_parser::DiffFile],
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
            render_inline(
                &self.config,
                &files,
                &mut lines,
                &mut line_mapping,
                &config.theme_colors,
            );
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
