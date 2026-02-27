//! Markdown renderer — inline elements, fenced code blocks, and tables.
//!
//! Renders markdown content into styled terminal output using a two-pass parser.
//! **Pass 1** classifies source lines into block-level elements (paragraphs,
//! headers, code blocks, tables, lists, blockquotes, horizontal rules).
//! **Pass 2** renders each block, applying inline formatting within paragraphs
//! and using dedicated renderers for code blocks (with syntax highlighting) and
//! tables (via the shared `TableRenderer`).
//!
//! Sub-modules:
//! - [`blocks`]    — block-level element classification
//! - [`config`]    — renderer configuration types
//! - [`highlight`] — keyword-based syntax highlighting
//! - [`inline`]    — inline span extraction (bold, italic, code, links)
//! - [`regexes`]   — compiled block-level regular expressions
//! - [`render`]    — per-element rendering functions

mod blocks;
pub mod config;
mod highlight;
mod inline;
mod regexes;
mod render;

#[cfg(test)]
mod tests;

pub use config::{HeaderStyle, HorizontalRuleStyle, LinkStyle, MarkdownRendererConfig};

use super::diagrams::DiagramRenderer;
use crate::config::prettifier::DiagramRendererConfig;
use crate::prettifier::registry::RendererRegistry;
use crate::prettifier::traits::{ContentRenderer, RendererConfig};
use crate::prettifier::types::{
    ContentBlock, InlineGraphic, RenderedContent, RendererCapability, SourceLineMapping,
    StyledLine, StyledSegment,
};

use blocks::{BlockElement, classify_blocks};
use render::{render_code_block, render_line as render_line_impl, render_table};

// ---------------------------------------------------------------------------
// MarkdownRenderer
// ---------------------------------------------------------------------------

/// Renders Markdown content into styled terminal output.
pub struct MarkdownRenderer {
    config: MarkdownRendererConfig,
    /// Diagram sub-renderer for fenced code blocks with diagram language tags.
    diagram_renderer: DiagramRenderer,
}

impl MarkdownRenderer {
    /// Create a new `MarkdownRenderer` with the given config.
    pub fn new(config: MarkdownRendererConfig) -> Self {
        Self::with_diagram_config(config, DiagramRendererConfig::default())
    }

    /// Create a new `MarkdownRenderer` with explicit diagram renderer config.
    pub fn with_diagram_config(
        config: MarkdownRendererConfig,
        diagram_config: DiagramRendererConfig,
    ) -> Self {
        Self {
            config,
            diagram_renderer: DiagramRenderer::new(diagram_config),
        }
    }

    /// Check if a fenced code block language should be sub-rendered by another renderer.
    ///
    /// Returns `true` for languages that have dedicated renderers in the registry
    /// (e.g., diagram languages like Mermaid, PlantUML). This enables multi-format
    /// handling where Markdown content embeds code blocks in other supported formats.
    pub fn should_sub_render(language: &str, registry: &RendererRegistry) -> bool {
        registry.get_renderer(language).is_some()
    }

    /// Render a single line, classifying it as a block-level element and
    /// then applying inline formatting within.
    #[cfg(test)]
    pub(super) fn render_line(
        &self,
        line: &str,
        renderer_config: &RendererConfig,
        footnote_links: &mut Option<Vec<String>>,
    ) -> StyledLine {
        let theme = &renderer_config.theme_colors;
        let width = renderer_config.terminal_width;
        render_line_impl(&self.config, line, theme, width, footnote_links)
    }
}

// ---------------------------------------------------------------------------
// ContentRenderer trait implementation
// ---------------------------------------------------------------------------

impl ContentRenderer for MarkdownRenderer {
    fn format_id(&self) -> &str {
        "markdown"
    }

    fn display_name(&self) -> &str {
        "Markdown"
    }

    fn capabilities(&self) -> Vec<RendererCapability> {
        vec![RendererCapability::TextStyling]
    }

    fn render(
        &self,
        content: &ContentBlock,
        config: &RendererConfig,
    ) -> Result<RenderedContent, crate::prettifier::traits::RenderError> {
        let theme = &config.theme_colors;
        let width = config.terminal_width;

        // Initialize footnote collection if using footnote link style.
        let mut footnote_links = match self.config.link_style {
            LinkStyle::Footnote => Some(Vec::new()),
            _ => None,
        };

        // Pass 1: classify lines into block-level elements.
        let blocks = classify_blocks(&content.lines);

        // Pass 2: render each block element.
        let mut lines = Vec::new();
        let mut line_mapping = Vec::new();
        let mut graphics: Vec<InlineGraphic> = Vec::new();

        for block in &blocks {
            match block {
                BlockElement::Line { source_idx } => {
                    let styled = render_line_impl(
                        &self.config,
                        &content.lines[*source_idx],
                        theme,
                        width,
                        &mut footnote_links,
                    );
                    line_mapping.push(SourceLineMapping {
                        rendered_line: lines.len(),
                        source_line: Some(*source_idx),
                    });
                    lines.push(styled);
                }

                BlockElement::CodeBlock {
                    language,
                    lines: code_lines,
                    fence_open_idx,
                    fence_close_idx,
                } => {
                    // Check if this code block is a diagram language — if so,
                    // delegate to the DiagramRenderer for full backend rendering
                    // (local CLI / Kroki API / styled text fallback).
                    let is_diagram = language
                        .as_deref()
                        .is_some_and(|lang| self.diagram_renderer.is_diagram_language(lang));

                    if is_diagram {
                        let lang = language
                            .as_deref()
                            .expect("is_diagram implies language is Some");
                        let source_refs: Vec<&str> =
                            code_lines.iter().map(String::as_str).collect();
                        let (diagram_lines, diagram_mappings, diagram_graphics) = self
                            .diagram_renderer
                            .render_diagram_section(lang, &source_refs, *fence_open_idx, config);

                        // Adjust line mappings to account for current output offset.
                        let offset = lines.len();
                        for mut mapping in diagram_mappings {
                            mapping.rendered_line += offset;
                            line_mapping.push(mapping);
                        }

                        // Adjust graphic row positions for current offset.
                        for mut graphic in diagram_graphics {
                            graphic.row += offset;
                            graphics.push(graphic);
                        }

                        // Closing fence: no rendered line, but record mapping.
                        if let Some(close_idx) = fence_close_idx {
                            line_mapping.push(SourceLineMapping {
                                rendered_line: lines.len() + diagram_lines.len(),
                                source_line: Some(*close_idx),
                            });
                        }

                        lines.extend(diagram_lines);
                    } else {
                        // Standard code block with syntax highlighting.
                        let rendered_code =
                            render_code_block(&self.config, language, code_lines, theme, width);

                        if language.is_some() {
                            // Language label line maps to the opening fence source line.
                            line_mapping.push(SourceLineMapping {
                                rendered_line: lines.len(),
                                source_line: Some(*fence_open_idx),
                            });
                        }

                        // Code content lines map 1:1 to their source lines.
                        let content_start = if language.is_some() { 1 } else { 0 };
                        for (j, _) in rendered_code.iter().enumerate().skip(content_start) {
                            let source_line = fence_open_idx + 1 + (j - content_start);
                            line_mapping.push(SourceLineMapping {
                                rendered_line: lines.len() + j,
                                source_line: Some(source_line),
                            });
                        }

                        // Closing fence: no rendered line, but record mapping.
                        if let Some(close_idx) = fence_close_idx {
                            line_mapping.push(SourceLineMapping {
                                rendered_line: lines.len() + rendered_code.len(),
                                source_line: Some(*close_idx),
                            });
                        }

                        lines.extend(rendered_code);
                    }
                }

                BlockElement::Table {
                    headers,
                    alignments,
                    rows,
                    source_start,
                    source_end,
                } => {
                    let rendered_table =
                        render_table(&self.config, headers, rows, alignments, theme, width);

                    // Map rendered table lines back to source range.
                    // The source has: header (1 line) + separator (1 line) + N data rows.
                    // The rendered has: top border + header + separator + N data rows + bottom border.
                    let source_line_count = source_end - source_start;
                    for (j, _) in rendered_table.iter().enumerate() {
                        // Best-effort mapping: map to nearest source line.
                        let source_line = if source_line_count > 0 {
                            let ratio = j as f64 / rendered_table.len().max(1) as f64;
                            let mapped =
                                *source_start + (ratio * source_line_count as f64) as usize;
                            Some(mapped.min(source_end - 1))
                        } else {
                            Some(*source_start)
                        };
                        line_mapping.push(SourceLineMapping {
                            rendered_line: lines.len() + j,
                            source_line,
                        });
                    }

                    lines.extend(rendered_table);
                }
            }
        }

        // Append footnote references section if any links were collected.
        if let Some(ref footnotes) = footnote_links
            && !footnotes.is_empty()
        {
            // Blank separator line.
            line_mapping.push(SourceLineMapping {
                rendered_line: lines.len(),
                source_line: None,
            });
            lines.push(StyledLine::plain(""));

            // Horizontal rule.
            let rule: String = std::iter::repeat_n('─', width.min(40)).collect();
            line_mapping.push(SourceLineMapping {
                rendered_line: lines.len(),
                source_line: None,
            });
            lines.push(StyledLine::new(vec![StyledSegment {
                text: rule,
                fg: Some(theme.palette[8]),
                ..Default::default()
            }]));

            // Each footnote: [N]: url
            for (i, url) in footnotes.iter().enumerate() {
                line_mapping.push(SourceLineMapping {
                    rendered_line: lines.len(),
                    source_line: None,
                });
                lines.push(StyledLine::new(vec![
                    StyledSegment {
                        text: format!("[{}]", i + 1),
                        fg: Some(theme.palette[8]),
                        bold: true,
                        ..Default::default()
                    },
                    StyledSegment {
                        text: format!(": {url}"),
                        fg: Some(theme.palette[12]),
                        underline: true,
                        link_url: Some(url.clone()),
                        ..Default::default()
                    },
                ]));
            }
        }

        Ok(RenderedContent {
            lines,
            line_mapping,
            graphics,
            format_badge: "\u{1F4DD}".to_string(), // 📝
        })
    }

    fn format_badge(&self) -> &str {
        "MD"
    }
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register the markdown renderer with the registry.
pub fn register_markdown_renderer(
    registry: &mut RendererRegistry,
    config: &MarkdownRendererConfig,
) {
    registry.register_renderer("markdown", Box::new(MarkdownRenderer::new(config.clone())));
}

/// Register the markdown renderer with diagram sub-rendering support.
pub fn register_markdown_renderer_with_diagrams(
    registry: &mut RendererRegistry,
    config: &MarkdownRendererConfig,
    diagram_config: &DiagramRendererConfig,
) {
    registry.register_renderer(
        "markdown",
        Box::new(MarkdownRenderer::with_diagram_config(
            config.clone(),
            diagram_config.clone(),
        )),
    );
}
