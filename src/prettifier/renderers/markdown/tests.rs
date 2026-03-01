//! Tests for the Markdown renderer.

use super::blocks::{
    BlockElement, classify_blocks, is_separator_row, is_table_row, parse_alignment,
    parse_table_cells,
};
use super::config::{HeaderStyle, HorizontalRuleStyle, LinkStyle, MarkdownRendererConfig};
use super::highlight::{get_language_def, highlight_code_line};
use super::render::{header_brightness, header_color};
use super::{MarkdownRenderer, register_markdown_renderer};
use crate::prettifier::registry::RendererRegistry;
use crate::prettifier::renderers::table::ColumnAlignment;
use crate::prettifier::testing::test_renderer_config;
use crate::prettifier::traits::{ContentRenderer, RendererConfig, ThemeColors};
use crate::prettifier::types::{ContentBlock, RendererCapability, StyledLine};
use std::time::SystemTime;

fn test_config() -> RendererConfig {
    test_renderer_config()
}

fn renderer() -> MarkdownRenderer {
    MarkdownRenderer::new(MarkdownRendererConfig::default())
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

fn render_line(line: &str) -> StyledLine {
    renderer().render_line(line, &test_config(), &mut None)
}

fn segment_texts(line: &StyledLine) -> Vec<&str> {
    line.segments.iter().map(|s| s.text.as_str()).collect()
}

// -- ContentRenderer trait --

#[test]
fn test_format_id() {
    let r = renderer();
    assert_eq!(r.format_id(), "markdown");
    assert_eq!(r.display_name(), "Markdown");
    assert_eq!(r.format_badge(), "MD");
    assert_eq!(r.capabilities(), vec![RendererCapability::TextStyling]);
}

#[test]
fn test_render_produces_correct_line_count() {
    let r = renderer();
    let block = make_block(&["# Hello", "World", "---"]);
    let result = r.render(&block, &test_config()).unwrap();
    assert_eq!(result.lines.len(), 3);
    assert_eq!(result.format_badge, "\u{1F4DD}");
}

#[test]
fn test_line_mapping_correctness() {
    let r = renderer();
    let block = make_block(&["line0", "line1", "line2"]);
    let result = r.render(&block, &test_config()).unwrap();
    for (i, mapping) in result.line_mapping.iter().enumerate() {
        assert_eq!(mapping.rendered_line, i);
        assert_eq!(mapping.source_line, Some(i));
    }
}

// -- Headers --

#[test]
fn test_header_h1() {
    let line = render_line("# Hello World");
    let texts = segment_texts(&line);
    assert_eq!(texts, vec!["Hello World"]);
    assert!(line.segments[0].bold);
    assert!(line.segments[0].fg.is_some());
}

#[test]
fn test_header_h2() {
    let line = render_line("## Subtitle");
    let texts = segment_texts(&line);
    assert_eq!(texts, vec!["Subtitle"]);
    assert!(line.segments[0].bold);
}

#[test]
fn test_header_h3_through_h6() {
    for (level, prefix) in [(3, "###"), (4, "####"), (5, "#####"), (6, "######")] {
        let line = render_line(&format!("{prefix} Title"));
        let texts = segment_texts(&line);
        assert_eq!(texts, vec!["Title"], "H{level} text should be 'Title'");
        assert!(line.segments[0].fg.is_some(), "H{level} should have color");
    }
}

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
    use super::highlight::subtle_bg;
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

// =====================================================================
// Fenced code block tests
// =====================================================================

#[test]
fn test_code_block_fence_markers_stripped() {
    let r = renderer();
    let block = make_block(&["```rust", "let x = 42;", "```"]);
    let result = r.render(&block, &test_config()).unwrap();
    // Fence markers should not appear in rendered output.
    for line in &result.lines {
        for seg in &line.segments {
            assert!(
                !seg.text.contains("```"),
                "Fence markers should be stripped, got: {:?}",
                seg.text
            );
        }
    }
}

#[test]
fn test_code_block_language_label() {
    let r = renderer();
    let block = make_block(&["```python", "print('hello')", "```"]);
    let result = r.render(&block, &test_config()).unwrap();
    // First rendered line should contain the language label.
    let first_text: String = result.lines[0]
        .segments
        .iter()
        .map(|s| s.text.as_str())
        .collect();
    assert!(
        first_text.contains("python"),
        "Language label should be displayed"
    );
}

#[test]
fn test_code_block_no_language() {
    let r = renderer();
    let block = make_block(&["```", "plain code", "```"]);
    let result = r.render(&block, &test_config()).unwrap();
    // Without a language, no label line is added.
    // Should have just the code line.
    assert_eq!(result.lines.len(), 1);
    let text: String = result.lines[0]
        .segments
        .iter()
        .map(|s| s.text.as_str())
        .collect();
    assert!(text.contains("plain code"));
}

#[test]
fn test_code_block_background_shading() {
    let r = renderer();
    let block = make_block(&["```", "code line", "```"]);
    let result = r.render(&block, &test_config()).unwrap();
    // Code lines should have background set.
    assert!(result.lines[0].segments[0].bg.is_some());
}

#[test]
fn test_code_block_background_disabled() {
    let r = MarkdownRenderer::new(MarkdownRendererConfig {
        code_block_background: false,
        ..Default::default()
    });
    let block = make_block(&["```", "code line", "```"]);
    let result = r.render(&block, &test_config()).unwrap();
    // Background should be None when disabled.
    assert!(result.lines[0].segments[0].bg.is_none());
}

#[test]
fn test_code_block_preserves_whitespace() {
    let r = renderer();
    let block = make_block(&["```", "  indented", "    more indent", "```"]);
    let result = r.render(&block, &test_config()).unwrap();
    let line0_text: String = result.lines[0]
        .segments
        .iter()
        .map(|s| s.text.as_str())
        .collect();
    assert!(line0_text.contains("  indented"));
    let line1_text: String = result.lines[1]
        .segments
        .iter()
        .map(|s| s.text.as_str())
        .collect();
    assert!(line1_text.contains("    more indent"));
}

#[test]
fn test_code_block_tilde_fences() {
    let r = renderer();
    let block = make_block(&["~~~", "tilde code", "~~~"]);
    let result = r.render(&block, &test_config()).unwrap();
    let text: String = result.lines[0]
        .segments
        .iter()
        .map(|s| s.text.as_str())
        .collect();
    assert!(text.contains("tilde code"));
}

#[test]
fn test_code_block_line_mapping() {
    let r = renderer();
    // Source: 0=fence, 1=code, 2=code, 3=fence
    let block = make_block(&["```rust", "line1", "line2", "```"]);
    let result = r.render(&block, &test_config()).unwrap();
    // Rendered: 0=label, 1=line1, 2=line2
    assert_eq!(result.lines.len(), 3);
    // Verify mappings exist and point to valid source lines.
    for mapping in &result.line_mapping {
        assert!(mapping.source_line.is_some());
    }
}

// -- Syntax highlighting --

#[test]
fn test_rust_keyword_highlighting() {
    let theme = ThemeColors::default();
    let lang_def = get_language_def("rust").unwrap();
    let line = highlight_code_line("let x = 42;", Some(&lang_def), &theme, false);
    // "let" should be highlighted as a keyword (bright magenta).
    let let_seg = line.segments.iter().find(|s| s.text == "let").unwrap();
    assert_eq!(let_seg.fg, Some(theme.palette[13]));
}

#[test]
fn test_python_keyword_highlighting() {
    let theme = ThemeColors::default();
    let lang_def = get_language_def("python").unwrap();
    let line = highlight_code_line("def hello():", Some(&lang_def), &theme, false);
    let def_seg = line.segments.iter().find(|s| s.text == "def").unwrap();
    assert_eq!(def_seg.fg, Some(theme.palette[13]));
}

#[test]
fn test_comment_highlighting() {
    let theme = ThemeColors::default();
    let lang_def = get_language_def("rust").unwrap();
    let line = highlight_code_line("// this is a comment", Some(&lang_def), &theme, false);
    // Entire line should be comment-colored (dim grey, italic).
    assert_eq!(line.segments.len(), 1);
    assert_eq!(line.segments[0].fg, Some(theme.palette[8]));
    assert!(line.segments[0].italic);
}

#[test]
fn test_string_highlighting() {
    let theme = ThemeColors::default();
    let lang_def = get_language_def("rust").unwrap();
    let line = highlight_code_line(r#"let s = "hello";"#, Some(&lang_def), &theme, false);
    let str_seg = line
        .segments
        .iter()
        .find(|s| s.text.contains("hello"))
        .unwrap();
    assert_eq!(str_seg.fg, Some(theme.palette[10])); // bright green
}

#[test]
fn test_json_highlighting() {
    assert!(get_language_def("json").is_some());
}

#[test]
fn test_unknown_language() {
    assert!(get_language_def("brainfuck").is_none());
}

#[test]
fn test_highlight_no_language_def() {
    let theme = ThemeColors::default();
    let line = highlight_code_line("just text", None, &theme, true);
    assert_eq!(line.segments.len(), 1);
    assert_eq!(line.segments[0].text, "just text");
    assert!(line.segments[0].bg.is_some());
}

// =====================================================================
// Table rendering tests
// =====================================================================

#[test]
fn test_table_renders_with_box_drawing() {
    let r = renderer();
    let block = make_block(&[
        "| Name  | Age |",
        "|-------|-----|",
        "| Alice | 30  |",
        "| Bob   | 25  |",
    ]);
    let result = r.render(&block, &test_config()).unwrap();
    // Should produce: top border + header + separator + 2 data rows + bottom border = 6 lines.
    assert_eq!(result.lines.len(), 6);

    // Top border should use box-drawing.
    let top_text: String = result.lines[0]
        .segments
        .iter()
        .map(|s| s.text.as_str())
        .collect();
    assert!(top_text.contains('┌') || top_text.contains('+'));
}

#[test]
fn test_table_header_is_bold() {
    let r = renderer();
    let block = make_block(&["| Name  | Age |", "|-------|-----|", "| Alice | 30  |"]);
    let result = r.render(&block, &test_config()).unwrap();
    // Header row (index 1): cell segments should be bold.
    let header_row = &result.lines[1];
    let name_seg = header_row.segments.iter().find(|s| s.text.contains("Name"));
    assert!(name_seg.is_some());
    assert!(name_seg.unwrap().bold);
}

#[test]
fn test_table_column_alignment() {
    let r = renderer();
    let block = make_block(&[
        "| Left | Center | Right |",
        "|:-----|:------:|------:|",
        "| a    | b      | c     |",
    ]);
    let result = r.render(&block, &test_config()).unwrap();
    // Should render without errors and produce table lines.
    assert!(result.lines.len() >= 5);
}

#[test]
fn test_table_border_color() {
    let r = MarkdownRenderer::new(MarkdownRendererConfig {
        table_border_color: [200, 100, 50],
        ..Default::default()
    });
    let block = make_block(&["| A |", "|---|", "| B |"]);
    let result = r.render(&block, &test_config()).unwrap();
    // Top border should use the configured color.
    assert_eq!(result.lines[0].segments[0].fg, Some([200, 100, 50]));
}

#[test]
fn test_table_line_mapping() {
    let r = renderer();
    let block = make_block(&["| A |", "|---|", "| 1 |"]);
    let result = r.render(&block, &test_config()).unwrap();
    // All rendered lines should have source line mappings.
    for mapping in &result.line_mapping {
        assert!(mapping.source_line.is_some());
    }
}

#[test]
fn test_table_followed_by_text() {
    let r = renderer();
    let block = make_block(&[
        "| A | B |",
        "|---|---|",
        "| 1 | 2 |",
        "",
        "Some text after table",
    ]);
    let result = r.render(&block, &test_config()).unwrap();
    // Last line should be the plain text.
    let last_text: String = result
        .lines
        .last()
        .unwrap()
        .segments
        .iter()
        .map(|s| s.text.as_str())
        .collect();
    assert!(last_text.contains("Some text after table"));
}

#[test]
fn test_mixed_content() {
    let r = renderer();
    let block = make_block(&[
        "# Title",
        "",
        "```rust",
        "let x = 1;",
        "```",
        "",
        "| A |",
        "|---|",
        "| 1 |",
        "",
        "End",
    ]);
    let result = r.render(&block, &test_config()).unwrap();
    // Should render without panics.
    assert!(result.lines.len() >= 8);
}

// -- Table helper tests --

#[test]
fn test_is_table_row() {
    assert!(is_table_row("| A | B |"));
    assert!(is_table_row("| A |"));
    assert!(!is_table_row("no pipes here"));
    assert!(!is_table_row(""));
}

#[test]
fn test_is_separator_row() {
    assert!(is_separator_row("|---|---|"));
    assert!(is_separator_row("| --- | --- |"));
    assert!(is_separator_row("|:---|:---:|---:|"));
    assert!(!is_separator_row("| A | B |"));
    assert!(!is_separator_row("plain text"));
}

#[test]
fn test_parse_table_cells() {
    let cells = parse_table_cells("| A | B | C |");
    assert_eq!(cells, vec!["A", "B", "C"]);
}

#[test]
fn test_parse_alignment() {
    assert_eq!(parse_alignment(":---"), ColumnAlignment::Left);
    assert_eq!(parse_alignment(":---:"), ColumnAlignment::Center);
    assert_eq!(parse_alignment("---:"), ColumnAlignment::Right);
    assert_eq!(parse_alignment("---"), ColumnAlignment::Left);
}

// -- Block classification tests --

#[test]
fn test_classify_code_block() {
    let lines: Vec<String> = vec![
        "```rust".to_string(),
        "let x = 1;".to_string(),
        "```".to_string(),
    ];
    let blocks = classify_blocks(&lines);
    assert_eq!(blocks.len(), 1);
    assert!(
        matches!(&blocks[0], BlockElement::CodeBlock { language: Some(lang), .. } if lang == "rust")
    );
}

#[test]
fn test_classify_table() {
    let lines: Vec<String> = vec![
        "| A | B |".to_string(),
        "|---|---|".to_string(),
        "| 1 | 2 |".to_string(),
    ];
    let blocks = classify_blocks(&lines);
    assert_eq!(blocks.len(), 1);
    assert!(matches!(&blocks[0], BlockElement::Table { .. }));
}

#[test]
fn test_classify_mixed() {
    let lines: Vec<String> = vec![
        "Hello".to_string(),
        "```".to_string(),
        "code".to_string(),
        "```".to_string(),
        "World".to_string(),
    ];
    let blocks = classify_blocks(&lines);
    assert_eq!(blocks.len(), 3); // Line, CodeBlock, Line
}

#[test]
fn test_unclosed_code_block() {
    let lines: Vec<String> = vec![
        "```rust".to_string(),
        "let x = 1;".to_string(),
        // No closing fence.
    ];
    let blocks = classify_blocks(&lines);
    assert_eq!(blocks.len(), 1);
    match &blocks[0] {
        BlockElement::CodeBlock {
            fence_close_idx, ..
        } => {
            assert!(fence_close_idx.is_none());
        }
        _ => panic!("Expected CodeBlock"),
    }
}
