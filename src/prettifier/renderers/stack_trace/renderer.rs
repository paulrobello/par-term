//! `StackTraceRenderer` struct, rendering methods, `ContentRenderer` impl, and registration.

use super::config::StackTraceRendererConfig;
use super::parse::parse_trace_line;
use super::types::{FilePath, FrameType, TraceLine};
use crate::prettifier::registry::RendererRegistry;
use crate::prettifier::renderers::push_line;
use crate::prettifier::traits::{ContentRenderer, RenderError, RendererConfig, ThemeColors};
use crate::prettifier::types::{
    ContentBlock, RenderedContent, RendererCapability, SourceLineMapping, StyledLine, StyledSegment,
};

/// Renders stack traces with error highlighting, frame classification, and folding.
pub struct StackTraceRenderer {
    config: StackTraceRendererConfig,
}

impl StackTraceRenderer {
    /// Create a new stack trace renderer with the given configuration.
    pub fn new(config: StackTraceRendererConfig) -> Self {
        Self { config }
    }

    /// Render an error header line (bold red).
    fn render_error_header(text: &str, theme: &ThemeColors) -> Vec<StyledSegment> {
        vec![StyledSegment {
            text: text.to_string(),
            fg: Some(theme.palette[9]), // Bright red
            bold: true,
            ..Default::default()
        }]
    }

    /// Render a "Caused by:" line (red, bold).
    fn render_caused_by(text: &str, theme: &ThemeColors) -> Vec<StyledSegment> {
        vec![StyledSegment {
            text: text.to_string(),
            fg: Some(theme.palette[1]), // Red
            bold: true,
            ..Default::default()
        }]
    }

    /// Render a stack frame with classification and optional clickable path.
    fn render_frame(
        text: &str,
        frame_type: FrameType,
        file_path: Option<&FilePath>,
        theme: &ThemeColors,
    ) -> Vec<StyledSegment> {
        let fg = match frame_type {
            FrameType::Application => None, // Normal/default color
            FrameType::Framework => Some(theme.palette[8]), // Dimmed
        };

        // If we have a file path, try to make it clickable
        if let Some(fp) = file_path {
            let link_target = if let Some(line) = fp.line {
                if let Some(col) = fp.column {
                    format!("{}:{}:{}", fp.path, line, col)
                } else {
                    format!("{}:{}", fp.path, line)
                }
            } else {
                fp.path.clone()
            };

            // Find the file path portion in the text to make only that clickable
            if let Some(path_idx) = text.find(&fp.path) {
                let mut segments = Vec::new();

                // Text before the path
                if path_idx > 0 {
                    segments.push(StyledSegment {
                        text: text[..path_idx].to_string(),
                        fg,
                        ..Default::default()
                    });
                }

                // The path portion (clickable)
                // Find the end of the file:line:col pattern in the source text
                let actual_end = text[path_idx..]
                    .find([')', ',', ' '])
                    .map(|i| path_idx + i)
                    .unwrap_or(text.len());

                segments.push(StyledSegment {
                    text: text[path_idx..actual_end].to_string(),
                    fg: Some(theme.palette[6]), // Cyan for paths
                    underline: true,
                    link_url: Some(link_target),
                    ..Default::default()
                });

                // Text after the path
                if actual_end < text.len() {
                    segments.push(StyledSegment {
                        text: text[actual_end..].to_string(),
                        fg,
                        ..Default::default()
                    });
                }

                return segments;
            }
        }

        // No clickable path — render as a single segment
        vec![StyledSegment {
            text: text.to_string(),
            fg,
            ..Default::default()
        }]
    }

    /// Render a group of consecutive frames, collapsing if too many.
    fn render_frame_group(
        &self,
        frames: &[TraceLine],
        lines: &mut Vec<StyledLine>,
        line_mapping: &mut Vec<SourceLineMapping>,
        base_source_line: usize,
        theme: &ThemeColors,
    ) {
        let count = frames.len();
        if count <= self.config.max_visible_frames {
            // Show all frames
            for (i, frame) in frames.iter().enumerate() {
                if let TraceLine::Frame {
                    text,
                    frame_type,
                    file_path,
                } = frame
                {
                    let segments = Self::render_frame(text, *frame_type, file_path.as_ref(), theme);
                    push_line(lines, line_mapping, segments, Some(base_source_line + i));
                }
            }
        } else {
            // Show first N frames
            let head = self
                .config
                .max_visible_frames
                .saturating_sub(self.config.keep_tail_frames);
            for (i, frame) in frames.iter().take(head).enumerate() {
                if let TraceLine::Frame {
                    text,
                    frame_type,
                    file_path,
                } = frame
                {
                    let segments = Self::render_frame(text, *frame_type, file_path.as_ref(), theme);
                    push_line(lines, line_mapping, segments, Some(base_source_line + i));
                }
            }

            // Collapse middle
            let hidden = count - head - self.config.keep_tail_frames;
            if hidden > 0 {
                push_line(
                    lines,
                    line_mapping,
                    vec![StyledSegment {
                        text: format!("    ... {hidden} more frames"),
                        fg: Some(theme.palette[8]),
                        italic: true,
                        ..Default::default()
                    }],
                    None,
                );
            }

            // Show tail frames
            let tail_start = count - self.config.keep_tail_frames;
            for (offset, frame) in frames[tail_start..].iter().enumerate() {
                if let TraceLine::Frame {
                    text,
                    frame_type,
                    file_path,
                } = frame
                {
                    let segments = Self::render_frame(text, *frame_type, file_path.as_ref(), theme);
                    push_line(
                        lines,
                        line_mapping,
                        segments,
                        Some(base_source_line + tail_start + offset),
                    );
                }
            }
        }
    }
}

impl ContentRenderer for StackTraceRenderer {
    fn format_id(&self) -> &str {
        "stack_trace"
    }

    fn display_name(&self) -> &str {
        "Stack Trace"
    }

    fn capabilities(&self) -> Vec<RendererCapability> {
        vec![RendererCapability::TextStyling]
    }

    fn render(
        &self,
        content: &ContentBlock,
        config: &RendererConfig,
    ) -> Result<RenderedContent, RenderError> {
        let theme = &config.theme_colors;
        let mut lines = Vec::new();
        let mut line_mapping = Vec::new();

        // Parse all lines
        let parsed: Vec<TraceLine> = content
            .lines
            .iter()
            .map(|l| parse_trace_line(l, &self.config.app_packages))
            .collect();

        // Group consecutive frames for collapsing
        let mut i = 0;
        while i < parsed.len() {
            match &parsed[i] {
                TraceLine::ErrorHeader(text) => {
                    let segments = Self::render_error_header(text, theme);
                    push_line(&mut lines, &mut line_mapping, segments, Some(i));
                    i += 1;
                }
                TraceLine::CausedBy(text) => {
                    let segments = Self::render_caused_by(text, theme);
                    push_line(&mut lines, &mut line_mapping, segments, Some(i));
                    i += 1;
                }
                TraceLine::Frame { .. } => {
                    // Collect consecutive frames
                    let frame_start = i;
                    while i < parsed.len() && matches!(&parsed[i], TraceLine::Frame { .. }) {
                        i += 1;
                    }
                    self.render_frame_group(
                        &parsed[frame_start..i],
                        &mut lines,
                        &mut line_mapping,
                        frame_start,
                        theme,
                    );
                }
                TraceLine::Other(text) => {
                    push_line(
                        &mut lines,
                        &mut line_mapping,
                        vec![StyledSegment {
                            text: text.clone(),
                            ..Default::default()
                        }],
                        Some(i),
                    );
                    i += 1;
                }
            }
        }

        Ok(RenderedContent {
            lines,
            line_mapping,
            graphics: vec![],
            format_badge: "TRACE".to_string(),
        })
    }

    fn format_badge(&self) -> &str {
        "TRACE"
    }
}

/// Register the stack trace renderer with the registry.
pub fn register_stack_trace_renderer(
    registry: &mut RendererRegistry,
    config: &StackTraceRendererConfig,
) {
    registry.register_renderer(
        "stack_trace",
        Box::new(StackTraceRenderer::new(config.clone())),
    );
}
