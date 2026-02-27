//! Rendering methods for individual Markdown block and inline elements.
//!
//! This module provides the per-element rendering logic for
//! [`MarkdownRenderer`]: headers, horizontal rules, blockquotes, lists,
//! inline spans, fenced code blocks, and tables.

use super::super::table::{ColumnAlignment, TableRenderer};
use super::config::{HeaderStyle, HorizontalRuleStyle, LinkStyle, MarkdownRendererConfig};
use super::highlight::{get_language_def, highlight_code_line, subtle_bg};
use super::inline::{SpanKind, extract_inline_spans};
use super::regexes::{
    re_blockquote, re_header, re_horizontal_rule, re_ordered_list, re_unordered_list,
};
use crate::prettifier::traits::ThemeColors;
use crate::prettifier::types::{StyledLine, StyledSegment};

// ---------------------------------------------------------------------------
// Color helpers
// ---------------------------------------------------------------------------

/// Get the header color for a given level (1–6) from the theme palette.
pub(super) fn header_color(level: usize, theme: &ThemeColors) -> [u8; 3] {
    match level {
        1 => theme.palette[14],
        2 => theme.palette[10],
        3 => theme.palette[11],
        4 => theme.palette[12],
        5 => theme.palette[13],
        _ => theme.palette[8],
    }
}

/// Get brightness-scaled color for Bold header style.
pub(super) fn header_brightness(level: usize, theme: &ThemeColors) -> [u8; 3] {
    let base = theme.fg;
    let scale = 1.0 - (level as f32 - 1.0) * 0.12;
    [
        (base[0] as f32 * scale) as u8,
        (base[1] as f32 * scale) as u8,
        (base[2] as f32 * scale) as u8,
    ]
}

// ---------------------------------------------------------------------------
// Element renderers (free functions taking config + theme arguments)
// ---------------------------------------------------------------------------

/// Render a single line, classifying it as a block-level element and
/// then applying inline formatting within.
pub(super) fn render_line(
    config: &MarkdownRendererConfig,
    line: &str,
    theme: &ThemeColors,
    width: usize,
    footnote_links: &mut Option<Vec<String>>,
) -> StyledLine {
    // Header
    if let Some(caps) = re_header().captures(line) {
        let level = caps
            .get(1)
            .expect("re_header capture group 1 (hashes) must be present after a match")
            .as_str()
            .len();
        let content = caps
            .get(2)
            .expect("re_header capture group 2 (content) must be present after a match")
            .as_str();
        return render_header(config, level, content, theme, footnote_links);
    }

    // Horizontal rule (check before unordered list since `---` could match list)
    if re_horizontal_rule().is_match(line) {
        return render_horizontal_rule(config, width, theme);
    }

    // Blockquote
    if let Some(caps) = re_blockquote().captures(line) {
        let content = caps
            .get(1)
            .expect("re_blockquote capture group 1 (content) must be present after a match")
            .as_str();
        return render_blockquote(config, content, theme, footnote_links);
    }

    // Unordered list
    if let Some(caps) = re_unordered_list().captures(line) {
        let indent = caps
            .get(1)
            .expect("re_unordered_list capture group 1 (indent) must be present after a match")
            .as_str();
        let content = caps
            .get(3)
            .expect("re_unordered_list capture group 3 (content) must be present after a match")
            .as_str();
        return render_unordered_list(config, indent, content, theme, footnote_links);
    }

    // Ordered list
    if let Some(caps) = re_ordered_list().captures(line) {
        let indent = caps
            .get(1)
            .expect("re_ordered_list capture group 1 (indent) must be present after a match")
            .as_str();
        let number = caps
            .get(2)
            .expect("re_ordered_list capture group 2 (number) must be present after a match")
            .as_str();
        let content = caps
            .get(3)
            .expect("re_ordered_list capture group 3 (content) must be present after a match")
            .as_str();
        return render_ordered_list(config, indent, number, content, theme, footnote_links);
    }

    // Paragraph / plain line: apply inline formatting
    let segments = render_inline(config, line, theme, footnote_links);
    StyledLine::new(segments)
}

/// Render a header (H1–H6) with visual hierarchy.
pub(super) fn render_header(
    config: &MarkdownRendererConfig,
    level: usize,
    content: &str,
    theme: &ThemeColors,
    footnote_links: &mut Option<Vec<String>>,
) -> StyledLine {
    let segments = render_inline(config, content, theme, footnote_links);

    let styled = segments
        .into_iter()
        .map(|mut seg| {
            match config.header_style {
                HeaderStyle::Colored => {
                    seg.fg = Some(header_color(level, theme));
                    seg.bold = level <= 2;
                }
                HeaderStyle::Bold => {
                    seg.bold = true;
                    seg.fg = Some(header_brightness(level, theme));
                }
                HeaderStyle::Underlined => {
                    if level <= 2 {
                        seg.underline = true;
                    }
                    seg.bold = true;
                    seg.fg = Some(header_color(level, theme));
                }
            }
            seg
        })
        .collect();

    StyledLine::new(styled)
}

/// Render a horizontal rule as a full-width line.
pub(super) fn render_horizontal_rule(
    config: &MarkdownRendererConfig,
    width: usize,
    theme: &ThemeColors,
) -> StyledLine {
    let ch = match config.horizontal_rule_style {
        HorizontalRuleStyle::Thin => '─',
        HorizontalRuleStyle::Thick => '━',
        HorizontalRuleStyle::Dashed => '╌',
    };
    let rule_text: String = std::iter::repeat_n(ch, width).collect();
    StyledLine::new(vec![StyledSegment {
        text: rule_text,
        fg: Some(theme.palette[8]),
        ..Default::default()
    }])
}

/// Render a blockquote with left border and dimmed text.
pub(super) fn render_blockquote(
    config: &MarkdownRendererConfig,
    content: &str,
    theme: &ThemeColors,
    footnote_links: &mut Option<Vec<String>>,
) -> StyledLine {
    let mut segments = vec![StyledSegment {
        text: "▎ ".to_string(),
        fg: Some(theme.palette[6]),
        ..Default::default()
    }];

    let inline = render_inline(config, content, theme, footnote_links);
    for mut seg in inline {
        if seg.fg.is_none() {
            seg.fg = Some(theme.palette[7]);
        }
        seg.italic = true;
        segments.push(seg);
    }

    StyledLine::new(segments)
}

/// Render a bullet list item with styled bullet.
pub(super) fn render_unordered_list(
    config: &MarkdownRendererConfig,
    indent: &str,
    content: &str,
    theme: &ThemeColors,
    footnote_links: &mut Option<Vec<String>>,
) -> StyledLine {
    let bullet = match indent.len() / 2 {
        0 => "•",
        1 => "◦",
        _ => "▪",
    };

    let mut segments = vec![StyledSegment {
        text: format!("{indent}{bullet} "),
        fg: Some(theme.palette[6]),
        ..Default::default()
    }];

    segments.extend(render_inline(config, content, theme, footnote_links));
    StyledLine::new(segments)
}

/// Render an ordered list item with styled number.
pub(super) fn render_ordered_list(
    config: &MarkdownRendererConfig,
    indent: &str,
    number: &str,
    content: &str,
    theme: &ThemeColors,
    footnote_links: &mut Option<Vec<String>>,
) -> StyledLine {
    let mut segments = vec![StyledSegment {
        text: format!("{indent}{number} "),
        fg: Some(theme.palette[11]),
        bold: true,
        ..Default::default()
    }];

    segments.extend(render_inline(config, content, theme, footnote_links));
    StyledLine::new(segments)
}

/// Render inline elements within a text span.
///
/// When `footnote_links` is `Some`, links are rendered with footnote-style
/// `[N]` references and URLs are collected into the vector for later display.
pub(super) fn render_inline(
    config: &MarkdownRendererConfig,
    text: &str,
    theme: &ThemeColors,
    footnote_links: &mut Option<Vec<String>>,
) -> Vec<StyledSegment> {
    let spans = extract_inline_spans(text);

    if spans.is_empty() {
        return vec![StyledSegment {
            text: text.to_string(),
            ..Default::default()
        }];
    }

    let mut segments = Vec::new();
    let mut pos = 0;

    for span in &spans {
        if span.start > pos {
            segments.push(StyledSegment {
                text: text[pos..span.start].to_string(),
                ..Default::default()
            });
        }

        match &span.kind {
            SpanKind::Code(content) => {
                segments.push(StyledSegment {
                    text: content.clone(),
                    fg: Some(theme.palette[9]),
                    bg: Some(subtle_bg(theme)),
                    ..Default::default()
                });
            }
            SpanKind::Link { text: lt, url } => match config.link_style {
                LinkStyle::UnderlineColor => {
                    segments.push(StyledSegment {
                        text: lt.clone(),
                        fg: Some(theme.palette[12]),
                        underline: true,
                        link_url: Some(url.clone()),
                        ..Default::default()
                    });
                }
                LinkStyle::InlineUrl => {
                    segments.push(StyledSegment {
                        text: format!("{lt} ({url})"),
                        fg: Some(theme.palette[12]),
                        underline: true,
                        ..Default::default()
                    });
                }
                LinkStyle::Footnote => {
                    // In footnote mode, footnote_links must be Some.
                    // We append the reference number inline and collect
                    // the URL for a footnote section at the end.
                    if let Some(footnotes) = footnote_links {
                        footnotes.push(url.clone());
                        let n = footnotes.len();
                        segments.push(StyledSegment {
                            text: lt.clone(),
                            fg: Some(theme.palette[12]),
                            underline: true,
                            ..Default::default()
                        });
                        segments.push(StyledSegment {
                            text: format!("[{n}]"),
                            fg: Some(theme.palette[8]),
                            ..Default::default()
                        });
                    } else {
                        // Fallback if footnote_links is None (shouldn't happen).
                        segments.push(StyledSegment {
                            text: lt.clone(),
                            fg: Some(theme.palette[12]),
                            underline: true,
                            link_url: Some(url.clone()),
                            ..Default::default()
                        });
                    }
                }
            },
            SpanKind::BoldItalic(content) => {
                segments.push(StyledSegment {
                    text: content.clone(),
                    bold: true,
                    italic: true,
                    ..Default::default()
                });
            }
            SpanKind::Bold(content) => {
                segments.push(StyledSegment {
                    text: content.clone(),
                    bold: true,
                    ..Default::default()
                });
            }
            SpanKind::Italic(content) => {
                segments.push(StyledSegment {
                    text: content.clone(),
                    italic: true,
                    ..Default::default()
                });
            }
        }

        pos = span.end;
    }

    if pos < text.len() {
        segments.push(StyledSegment {
            text: text[pos..].to_string(),
            ..Default::default()
        });
    }

    segments
}

/// Render a fenced code block with optional syntax highlighting and background.
pub(super) fn render_code_block(
    config: &MarkdownRendererConfig,
    language: &Option<String>,
    code_lines: &[String],
    theme: &ThemeColors,
    width: usize,
) -> Vec<StyledLine> {
    let lang_def = language.as_deref().and_then(get_language_def);
    let show_bg = config.code_block_background;
    let code_bg = if show_bg {
        Some(subtle_bg(theme))
    } else {
        None
    };

    let mut lines = Vec::new();

    // Language label line (if language is specified).
    if let Some(lang) = language {
        let label = format!(" {lang} ");
        let padding = width.saturating_sub(label.len());
        let padded = format!("{label}{}", " ".repeat(padding));
        lines.push(StyledLine::new(vec![StyledSegment {
            text: padded,
            fg: Some(theme.palette[8]),
            bg: code_bg,
            bold: true,
            ..Default::default()
        }]));
    }

    // Highlighted code lines.
    for line in code_lines {
        lines.push(highlight_code_line(line, lang_def.as_ref(), theme, show_bg));
    }

    lines
}

/// Render a markdown table using the shared `TableRenderer`.
pub(super) fn render_table(
    config: &MarkdownRendererConfig,
    headers: &[String],
    rows: &[Vec<String>],
    alignments: &[ColumnAlignment],
    theme: &ThemeColors,
    max_width: usize,
) -> Vec<StyledLine> {
    let table_renderer = TableRenderer::new(
        config.table_style.clone(),
        config.table_border_color,
        header_color(3, theme), // use H3 color for table headers
    );
    table_renderer.render_table(headers, rows, alignments, max_width)
}
