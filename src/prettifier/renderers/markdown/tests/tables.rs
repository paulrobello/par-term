//! Table rendering tests for the Markdown renderer.

use super::super::MarkdownRenderer;
use super::super::config::MarkdownRendererConfig;
use crate::prettifier::testing::{make_block, test_renderer_config};
use crate::prettifier::traits::{ContentRenderer, RendererConfig};

fn test_config() -> RendererConfig {
    test_renderer_config()
}

fn renderer() -> MarkdownRenderer {
    MarkdownRenderer::new(MarkdownRendererConfig::default())
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
