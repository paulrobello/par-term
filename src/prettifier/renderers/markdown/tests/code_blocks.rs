//! Code block and syntax highlighting tests for the Markdown renderer.

use super::super::MarkdownRenderer;
use super::super::config::MarkdownRendererConfig;
use super::super::highlight::{get_language_def, highlight_code_line};
use crate::prettifier::testing::{make_block, test_renderer_config};
use crate::prettifier::traits::{ContentRenderer, RendererConfig, ThemeColors};

fn test_config() -> RendererConfig {
    test_renderer_config()
}

fn renderer() -> MarkdownRenderer {
    MarkdownRenderer::new(MarkdownRendererConfig::default())
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
