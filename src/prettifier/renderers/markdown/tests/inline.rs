//! Inline element rendering tests for the Markdown renderer.

use super::super::config::{HeaderStyle, HorizontalRuleStyle, LinkStyle, MarkdownRendererConfig};
use super::super::highlight::subtle_bg;
use super::super::render::{header_brightness, header_color};
use super::super::{MarkdownRenderer, register_markdown_renderer};
use crate::prettifier::registry::RendererRegistry;
use crate::prettifier::testing::{make_block, test_renderer_config};
use crate::prettifier::traits::{ContentRenderer, RendererConfig, ThemeColors};

fn test_config() -> RendererConfig {
    test_renderer_config()
}

fn renderer() -> MarkdownRenderer {
    MarkdownRenderer::new(MarkdownRendererConfig::default())
}

fn render_line(line: &str) -> crate::prettifier::types::StyledLine {
    renderer().render_line(line, &test_config(), &mut None)
}

fn segment_texts(line: &crate::prettifier::types::StyledLine) -> Vec<&str> {
    line.segments.iter().map(|s| s.text.as_str()).collect()
}

// -- Bold --

#[test]
fn test_bold_asterisks() {
    let line = render_line("This is **bold** text");
    assert_eq!(line.segments.len(), 3);
    assert_eq!(line.segments[0].text, "This is ");
    assert!(!line.segments[0].bold);
    assert_eq!(line.segments[1].text, "bold");
    assert!(line.segments[1].bold);
    assert_eq!(line.segments[2].text, " text");
}

#[test]
fn test_bold_underscores() {
    let line = render_line("This is __bold__ text");
    let bold_seg = line.segments.iter().find(|s| s.text == "bold").unwrap();
    assert!(bold_seg.bold);
}

// -- Italic --

#[test]
fn test_italic_asterisks() {
    let line = render_line("This is *italic* text");
    let italic_seg = line.segments.iter().find(|s| s.text == "italic").unwrap();
    assert!(italic_seg.italic);
    assert!(!italic_seg.bold);
}

#[test]
fn test_italic_underscores() {
    let line = render_line("This is _italic_ text");
    let italic_seg = line.segments.iter().find(|s| s.text == "italic").unwrap();
    assert!(italic_seg.italic);
}

// -- Bold + Italic --

#[test]
fn test_bold_italic() {
    let line = render_line("This is ***bold italic*** text");
    let bi_seg = line
        .segments
        .iter()
        .find(|s| s.text == "bold italic")
        .unwrap();
    assert!(bi_seg.bold);
    assert!(bi_seg.italic);
}

#[test]
fn test_bold_italic_underscores() {
    let line = render_line("This is ___bold italic___ text");
    let bi_seg = line
        .segments
        .iter()
        .find(|s| s.text == "bold italic")
        .unwrap();
    assert!(bi_seg.bold);
    assert!(bi_seg.italic);
}

// -- Inline code --

#[test]
fn test_inline_code() {
    let line = render_line("Use `cargo build` to compile");
    let code_seg = line
        .segments
        .iter()
        .find(|s| s.text == "cargo build")
        .unwrap();
    assert!(code_seg.bg.is_some(), "Inline code should have background");
    assert!(
        code_seg.fg.is_some(),
        "Inline code should have foreground color"
    );
}

#[test]
fn test_inline_code_is_opaque() {
    let line = render_line("Use `**not bold**` here");
    let code_seg = line
        .segments
        .iter()
        .find(|s| s.text == "**not bold**")
        .unwrap();
    assert!(code_seg.bg.is_some());
    assert!(!code_seg.bold);
}

// -- Links --

#[test]
fn test_link_underline_color() {
    let line = render_line("Visit [Example](https://example.com) now");
    let link_seg = line.segments.iter().find(|s| s.text == "Example").unwrap();
    assert!(link_seg.underline);
    assert!(link_seg.fg.is_some());
    assert_eq!(link_seg.link_url.as_deref(), Some("https://example.com"));
}

#[test]
fn test_link_inline_url_style() {
    let r = MarkdownRenderer::new(MarkdownRendererConfig {
        link_style: LinkStyle::InlineUrl,
        ..Default::default()
    });
    let line = r.render_line("See [Docs](https://docs.rs)", &test_config(), &mut None);
    let link_seg = line
        .segments
        .iter()
        .find(|s| s.text.contains("Docs"))
        .unwrap();
    assert!(link_seg.text.contains("https://docs.rs"));
}

// -- Blockquotes --

#[test]
fn test_blockquote() {
    let line = render_line("> This is a quote");
    assert!(line.segments[0].text.contains('▎'));
    let quote_seg = line
        .segments
        .iter()
        .find(|s| s.text.contains("This is a quote"))
        .unwrap();
    assert!(quote_seg.italic);
}

#[test]
fn test_blockquote_with_inline() {
    let line = render_line("> This has **bold** text");
    let bold_seg = line.segments.iter().find(|s| s.text == "bold").unwrap();
    assert!(bold_seg.bold);
    assert!(bold_seg.italic);
}

// -- Lists --

#[test]
fn test_unordered_list_dash() {
    let line = render_line("- First item");
    assert!(line.segments[0].text.contains('•'));
    assert!(line.segments[0].fg.is_some());
}

#[test]
fn test_unordered_list_asterisk() {
    let line = render_line("* Second item");
    assert!(line.segments[0].text.contains('•'));
}

#[test]
fn test_unordered_list_plus() {
    let line = render_line("+ Third item");
    assert!(line.segments[0].text.contains('•'));
}

#[test]
fn test_nested_unordered_list() {
    let line = render_line("  - Nested item");
    assert!(line.segments[0].text.contains('◦'));
}

#[test]
fn test_deeply_nested_list() {
    let line = render_line("    - Deep item");
    assert!(line.segments[0].text.contains('▪'));
}

#[test]
fn test_ordered_list() {
    let line = render_line("1. First step");
    assert!(line.segments[0].text.contains("1."));
    assert!(line.segments[0].bold);
}

#[test]
fn test_ordered_list_paren() {
    let line = render_line("2) Second step");
    assert!(line.segments[0].text.contains("2)"));
}

#[test]
fn test_list_with_inline_formatting() {
    let line = render_line("- This is **important**");
    let bold_seg = line
        .segments
        .iter()
        .find(|s| s.text == "important")
        .unwrap();
    assert!(bold_seg.bold);
}

// -- Horizontal rules --

#[test]
fn test_horizontal_rule_dashes() {
    let line = render_line("---");
    assert_eq!(line.segments.len(), 1);
    assert!(line.segments[0].text.contains('─'));
    assert_eq!(line.segments[0].text.chars().count(), 80);
}

#[test]
fn test_horizontal_rule_asterisks() {
    let line = render_line("***");
    assert!(line.segments[0].text.contains('─'));
}

#[test]
fn test_horizontal_rule_underscores() {
    let line = render_line("___");
    assert!(line.segments[0].text.contains('─'));
}

#[test]
fn test_horizontal_rule_thick_style() {
    let r = MarkdownRenderer::new(MarkdownRendererConfig {
        horizontal_rule_style: HorizontalRuleStyle::Thick,
        ..Default::default()
    });
    let line = r.render_line("---", &test_config(), &mut None);
    assert!(line.segments[0].text.contains('━'));
}

#[test]
fn test_horizontal_rule_dashed_style() {
    let r = MarkdownRenderer::new(MarkdownRendererConfig {
        horizontal_rule_style: HorizontalRuleStyle::Dashed,
        ..Default::default()
    });
    let line = r.render_line("---", &test_config(), &mut None);
    assert!(line.segments[0].text.contains('╌'));
}

// -- Plain text --

#[test]
fn test_plain_text_passthrough() {
    let line = render_line("Just plain text");
    assert_eq!(line.segments.len(), 1);
    assert_eq!(line.segments[0].text, "Just plain text");
    assert!(!line.segments[0].bold);
    assert!(!line.segments[0].italic);
}

#[test]
fn test_empty_line() {
    let line = render_line("");
    assert_eq!(line.segments.len(), 1);
    assert_eq!(line.segments[0].text, "");
}

// -- Multiple inline elements --

#[test]
fn test_multiple_inline_elements() {
    let line = render_line("**Bold** and *italic* and `code`");
    assert!(line.segments.iter().any(|s| s.text == "Bold" && s.bold));
    assert!(line.segments.iter().any(|s| s.text == "italic" && s.italic));
    assert!(
        line.segments
            .iter()
            .any(|s| s.text == "code" && s.bg.is_some())
    );
}

// -- Registration --

#[test]
fn test_register_markdown_renderer() {
    let mut registry = RendererRegistry::new(0.6);
    register_markdown_renderer(&mut registry, &MarkdownRendererConfig::default());
    assert_eq!(registry.renderer_count(), 1);
    assert!(registry.get_renderer("markdown").is_some());
    assert_eq!(
        registry.get_renderer("markdown").unwrap().display_name(),
        "Markdown"
    );
}

// -- Header styles --

#[test]
fn test_header_bold_style() {
    let r = MarkdownRenderer::new(MarkdownRendererConfig {
        header_style: HeaderStyle::Bold,
        ..Default::default()
    });
    let line = r.render_line("# Title", &test_config(), &mut None);
    assert!(line.segments[0].bold);
}

#[test]
fn test_header_underlined_style() {
    let r = MarkdownRenderer::new(MarkdownRendererConfig {
        header_style: HeaderStyle::Underlined,
        ..Default::default()
    });
    let line = r.render_line("# Title", &test_config(), &mut None);
    assert!(line.segments[0].underline);
    assert!(line.segments[0].bold);

    let line = r.render_line("### Title", &test_config(), &mut None);
    assert!(line.segments[0].bold);
    assert!(!line.segments[0].underline);
}

// -- Footnote link style --

#[test]
fn test_link_footnote_style_inline_ref() {
    let r = MarkdownRenderer::new(MarkdownRendererConfig {
        link_style: LinkStyle::Footnote,
        ..Default::default()
    });
    let block = make_block(&["See [Example](https://example.com) and [Docs](https://docs.rs)"]);
    let result = r.render(&block, &test_config()).unwrap();
    // Should have: content line + blank + rule + 2 footnote lines = 5 lines
    assert!(result.lines.len() >= 4);
    // The content line should have [1] and [2] references.
    let content_text: String = result.lines[0]
        .segments
        .iter()
        .map(|s| s.text.as_str())
        .collect();
    assert!(
        content_text.contains("[1]"),
        "Should have [1] reference: {content_text}"
    );
    assert!(
        content_text.contains("[2]"),
        "Should have [2] reference: {content_text}"
    );
    // Last two lines should be footnote references.
    let last = &result.lines[result.lines.len() - 1];
    let last_text: String = last.segments.iter().map(|s| s.text.as_str()).collect();
    assert!(last_text.contains("docs.rs"));
}

#[test]
fn test_link_footnote_style_no_links() {
    let r = MarkdownRenderer::new(MarkdownRendererConfig {
        link_style: LinkStyle::Footnote,
        ..Default::default()
    });
    let block = make_block(&["No links here"]);
    let result = r.render(&block, &test_config()).unwrap();
    // No footnotes should be appended.
    assert_eq!(result.lines.len(), 1);
}

// -- Edge cases --

#[test]
fn test_code_span_prevents_bold_parsing() {
    let line = render_line("Check `**this**` out");
    assert!(line.segments.iter().any(|s| s.text == "**this**"));
    assert!(!line.segments.iter().any(|s| s.text == "this" && s.bold));
}

#[test]
fn test_adjacent_formatting() {
    let line = render_line("**bold***italic*");
    assert!(line.segments.iter().any(|s| s.text == "bold" && s.bold));
    assert!(line.segments.iter().any(|s| s.text == "italic" && s.italic));
}

#[test]
fn test_link_with_special_chars_in_url() {
    let line = render_line("[API](https://api.example.com/v1?key=val&foo=bar)");
    let link_seg = line.segments.iter().find(|s| s.link_url.is_some()).unwrap();
    assert_eq!(
        link_seg.link_url.as_deref(),
        Some("https://api.example.com/v1?key=val&foo=bar")
    );
}

// -- Helper function tests --

#[test]
fn test_subtle_bg() {
    let theme = ThemeColors::default();
    let bg = subtle_bg(&theme);
    assert_eq!(bg, [55, 55, 71]);
}

#[test]
fn test_header_brightness_scaling() {
    let theme = ThemeColors::default();
    let h1 = header_brightness(1, &theme);
    let h6 = header_brightness(6, &theme);
    assert!(h1[0] >= h6[0]);
    assert!(h1[1] >= h6[1]);
    assert!(h1[2] >= h6[2]);
}

// -- Header visual hierarchy --

#[test]
fn test_header_visual_hierarchy() {
    let theme = ThemeColors::default();
    let h1_color = header_color(1, &theme);
    let h6_color = header_color(6, &theme);
    assert_eq!(h1_color, theme.palette[14]);
    assert_eq!(h6_color, theme.palette[8]);
}

#[test]
fn test_header_strips_prefix() {
    let line = render_line("## Keep this text");
    for seg in &line.segments {
        assert!(!seg.text.contains("##"), "Header prefix should be stripped");
    }
}

#[test]
fn test_header_with_inline_bold() {
    let line = render_line("# Hello **World**");
    assert!(line.segments.len() >= 2);
    let bold_seg = line.segments.iter().find(|s| s.text == "World").unwrap();
    assert!(bold_seg.bold);
}
