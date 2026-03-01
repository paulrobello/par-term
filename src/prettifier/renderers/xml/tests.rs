//! Tests for the XML/HTML renderer.

use super::*;
use crate::prettifier::testing::{make_block, test_renderer_config};
use crate::prettifier::traits::RendererConfig;
use crate::prettifier::types::StyledLine;

fn test_config() -> RendererConfig {
    test_renderer_config()
}

fn renderer() -> XmlRenderer {
    XmlRenderer::new(XmlRendererConfig::default())
}

fn all_text(lines: &[StyledLine]) -> String {
    lines
        .iter()
        .map(|l| {
            l.segments
                .iter()
                .map(|s| s.text.as_str())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn test_format_id() {
    let r = renderer();
    assert_eq!(r.format_id(), "xml");
    assert_eq!(r.display_name(), "XML/HTML");
    assert_eq!(r.format_badge(), "XML");
    assert_eq!(r.capabilities(), vec![RendererCapability::TextStyling]);
}

#[test]
fn test_render_simple_xml() {
    let r = renderer();
    let block = make_block(&[
        "<root>",
        "  <child>",
        "    content",
        "  </child>",
        "</root>",
    ]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    assert!(text.contains("root"));
    assert!(text.contains("child"));
    assert!(text.contains("content"));
    assert_eq!(result.format_badge, "XML");
}

#[test]
fn test_render_xml_declaration() {
    let r = renderer();
    let block = make_block(&["<?xml version=\"1.0\" encoding=\"UTF-8\"?>"]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    assert!(text.contains("<?xml"));
}

#[test]
fn test_render_attributes() {
    let r = renderer();
    let block = make_block(&["<item name=\"test\" id=\"1\">value</item>"]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    assert!(text.contains("name"));
    assert!(text.contains("\"test\""));
    assert!(text.contains("id"));
}

#[test]
fn test_attribute_highlighting() {
    let r = renderer();
    let block = make_block(&["<tag attr=\"value\">"]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();

    // Find the attribute name segment
    let attr_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text == "attr")
        .unwrap();
    assert_eq!(attr_seg.fg, Some(theme.key_color())); // Cyan

    // Find the attribute value segment
    let val_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text == "\"value\"")
        .unwrap();
    assert_eq!(val_seg.fg, Some(theme.string_color())); // Green
}

#[test]
fn test_namespace_coloring() {
    let r = renderer();
    let block = make_block(&["<ns:tag>content</ns:tag>"]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();

    // Namespace prefix in magenta
    let ns_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text == "ns")
        .unwrap();
    assert_eq!(ns_seg.fg, Some(theme.palette[5])); // Magenta (no semantic accessor)

    // Tag name in blue
    let tag_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text == "tag" && s.bold)
        .unwrap();
    assert_eq!(tag_seg.fg, Some(theme.palette[4])); // Blue (no semantic accessor)
}

#[test]
fn test_self_closing_tag() {
    let r = renderer();
    let block = make_block(&["<root>", "  <br />", "</root>"]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    assert!(text.contains("br"));
    assert!(text.contains("/>"));
}

#[test]
fn test_comment_styling() {
    let r = renderer();
    let block = make_block(&["<!-- This is a comment -->"]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();

    let comment_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text.contains("<!-- This is a comment -->"))
        .unwrap();
    assert_eq!(comment_seg.fg, Some(theme.dim_color()));
    assert!(comment_seg.italic);
}

#[test]
fn test_cdata_styling() {
    let r = renderer();
    let block = make_block(&["<![CDATA[some data]]>"]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();

    let cdata_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text.contains("CDATA"))
        .unwrap();
    assert_eq!(cdata_seg.fg, Some(theme.palette[3])); // Yellow
}

#[test]
fn test_tree_guides_present() {
    let r = renderer();
    let block = make_block(&["<root>", "  <child>text</child>", "</root>"]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    assert!(text.contains('│'));
}

#[test]
fn test_auto_collapse_deep_elements() {
    let r = XmlRenderer::new(XmlRendererConfig {
        max_depth_expanded: 1,
    });
    let block = make_block(&[
        "<root>",
        "  <level1>",
        "    <deep>content</deep>",
        "  </level1>",
        "</root>",
    ]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    assert!(text.contains("..."));
}

#[test]
fn test_line_mappings_populated() {
    let r = renderer();
    let block = make_block(&["<root>", "  <child />", "</root>"]);
    let result = r.render(&block, &test_config()).unwrap();
    assert_eq!(result.line_mapping.len(), result.lines.len());
}

#[test]
fn test_register_xml_renderer() {
    let mut registry = RendererRegistry::new(0.6);
    register_xml_renderer(&mut registry, &XmlRendererConfig::default());
    assert_eq!(registry.renderer_count(), 1);
    assert!(registry.get_renderer("xml").is_some());
    assert_eq!(
        registry.get_renderer("xml").unwrap().display_name(),
        "XML/HTML"
    );
}

#[test]
fn test_config_defaults() {
    let config = XmlRendererConfig::default();
    assert_eq!(config.max_depth_expanded, 4);
}

#[test]
fn test_empty_content() {
    let r = renderer();
    let block = make_block(&[]);
    let result = r.render(&block, &test_config()).unwrap();
    assert!(result.lines.is_empty());
}

#[test]
fn test_doctype_rendering() {
    let r = renderer();
    let block = make_block(&["<!DOCTYPE html>", "<html>", "</html>"]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    assert!(text.contains("DOCTYPE"));
}
