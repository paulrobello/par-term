//! Full renderer integration tests for the Markdown renderer.

use super::super::MarkdownRenderer;
use super::super::config::MarkdownRendererConfig;
use crate::prettifier::testing::{make_block, test_renderer_config};
use crate::prettifier::traits::{ContentRenderer, RendererConfig};
use crate::prettifier::types::RendererCapability;

fn test_config() -> RendererConfig {
    test_renderer_config()
}

fn renderer() -> MarkdownRenderer {
    MarkdownRenderer::new(MarkdownRendererConfig::default())
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

// -- Headers integration --

#[test]
fn test_header_h1() {
    let r = renderer();
    let line = r.render_line("# Hello World", &test_config(), &mut None);
    let texts: Vec<&str> = line.segments.iter().map(|s| s.text.as_str()).collect();
    assert_eq!(texts, vec!["Hello World"]);
    assert!(line.segments[0].bold);
    assert!(line.segments[0].fg.is_some());
}

#[test]
fn test_header_h2() {
    let r = renderer();
    let line = r.render_line("## Subtitle", &test_config(), &mut None);
    let texts: Vec<&str> = line.segments.iter().map(|s| s.text.as_str()).collect();
    assert_eq!(texts, vec!["Subtitle"]);
    assert!(line.segments[0].bold);
}

#[test]
fn test_header_h3_through_h6() {
    let r = renderer();
    for (level, prefix) in [(3, "###"), (4, "####"), (5, "#####"), (6, "######")] {
        let line = r.render_line(&format!("{prefix} Title"), &test_config(), &mut None);
        let texts: Vec<&str> = line.segments.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(texts, vec!["Title"], "H{level} text should be 'Title'");
        assert!(line.segments[0].fg.is_some(), "H{level} should have color");
    }
}

// -- Mixed content full round-trip --

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
