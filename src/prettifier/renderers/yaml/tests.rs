//! Tests for the YAML renderer.

use super::parser::{YamlRenderer, YamlRendererConfig, register_yaml_renderer};
use crate::prettifier::registry::RendererRegistry;
use crate::prettifier::traits::{ContentRenderer, RendererConfig, ThemeColors};
use crate::prettifier::types::{ContentBlock, RendererCapability, StyledLine};
use std::time::SystemTime;

fn test_config() -> RendererConfig {
    RendererConfig {
        terminal_width: 80,
        ..Default::default()
    }
}

fn renderer() -> YamlRenderer {
    YamlRenderer::new(YamlRendererConfig::default())
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
    assert_eq!(r.format_id(), "yaml");
    assert_eq!(r.display_name(), "YAML");
    assert_eq!(r.format_badge(), "YAML");
    assert_eq!(r.capabilities(), vec![RendererCapability::TextStyling]);
}

#[test]
fn test_render_simple_key_value() {
    let r = renderer();
    let block = make_block(&["name: par-term", "version: 0.16.0"]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    assert!(text.contains("name"));
    assert!(text.contains("par-term"));
    assert!(text.contains("version"));
    assert_eq!(result.format_badge, "YAML");
}

#[test]
fn test_render_document_start() {
    let r = renderer();
    let block = make_block(&["---", "key: value"]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    assert!(text.contains("---"));
}

#[test]
fn test_render_document_end() {
    let r = renderer();
    let block = make_block(&["key: value", "..."]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    assert!(text.contains("..."));
}

#[test]
fn test_render_comment() {
    let r = renderer();
    let block = make_block(&["# This is a comment", "key: value"]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();
    let comment_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text.contains("# This is a comment"))
        .unwrap();
    assert_eq!(comment_seg.fg, Some(theme.palette[8]));
    assert!(comment_seg.italic);
}

#[test]
fn test_render_nested_yaml() {
    let r = renderer();
    let block = make_block(&["database:", "  host: localhost", "  port: 5432"]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    assert!(text.contains("database"));
    assert!(text.contains("host"));
    assert!(text.contains("localhost"));
    assert!(text.contains('│'));
}

#[test]
fn test_render_list_items() {
    let r = renderer();
    let block = make_block(&["items:", "  - serde", "  - tokio"]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    assert!(text.contains("- "));
    assert!(text.contains("serde"));
    assert!(text.contains("tokio"));
}

#[test]
fn test_boolean_coloring() {
    let r = renderer();
    let block = make_block(&["enabled: true", "debug: false"]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();
    let bool_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text == "true")
        .unwrap();
    assert_eq!(bool_seg.fg, Some(theme.palette[5]));
}

#[test]
fn test_number_coloring() {
    let r = renderer();
    let block = make_block(&["port: 8080"]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();
    let num_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text == "8080")
        .unwrap();
    assert_eq!(num_seg.fg, Some(theme.palette[11]));
}

#[test]
fn test_null_coloring() {
    let r = renderer();
    let block = make_block(&["data: null"]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();
    let null_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text == "null")
        .unwrap();
    assert_eq!(null_seg.fg, Some(theme.palette[8]));
    assert!(null_seg.italic);
}

#[test]
fn test_anchor_highlighting() {
    let r = renderer();
    let block = make_block(&["defaults: &defaults", "  adapter: postgres"]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();
    let anchor_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text.contains("&defaults"))
        .unwrap();
    assert_eq!(anchor_seg.fg, Some(theme.palette[13]));
    assert!(anchor_seg.bold);
}

#[test]
fn test_alias_highlighting() {
    let r = renderer();
    let block = make_block(&["production: *defaults"]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();
    let alias_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text.contains("*defaults"))
        .unwrap();
    assert_eq!(alias_seg.fg, Some(theme.palette[13]));
    assert!(alias_seg.italic);
}

#[test]
fn test_tag_highlighting() {
    let r = renderer();
    let block = make_block(&["count: !!int 42"]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();
    let tag_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text.contains("!!int"))
        .unwrap();
    assert_eq!(tag_seg.fg, Some(theme.palette[8]));
    assert!(tag_seg.italic);
}

#[test]
fn test_key_coloring() {
    let r = renderer();
    let block = make_block(&["mykey: value"]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();
    let key_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text == "mykey")
        .unwrap();
    assert_eq!(key_seg.fg, Some(theme.palette[6]));
    assert!(key_seg.bold);
}

#[test]
fn test_line_mappings_populated() {
    let r = renderer();
    let block = make_block(&["name: test", "version: 1"]);
    let result = r.render(&block, &test_config()).unwrap();
    assert_eq!(result.line_mapping.len(), result.lines.len());
}

#[test]
fn test_auto_collapse_deep_mapping() {
    let r = YamlRenderer::new(YamlRendererConfig {
        max_depth_expanded: 1,
        ..Default::default()
    });
    let block = make_block(&[
        "level1:",
        "  level2:",
        "    key1: value1",
        "    key2: value2",
    ]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    assert!(text.contains("keys"));
}

#[test]
fn test_empty_lines_preserved() {
    let r = renderer();
    let block = make_block(&["key: value", "", "other: data"]);
    let result = r.render(&block, &test_config()).unwrap();
    assert_eq!(result.lines.len(), 3);
}

#[test]
fn test_register_yaml_renderer() {
    let mut registry = RendererRegistry::new(0.6);
    register_yaml_renderer(&mut registry, &YamlRendererConfig::default());
    assert_eq!(registry.renderer_count(), 1);
    assert!(registry.get_renderer("yaml").is_some());
    assert_eq!(
        registry.get_renderer("yaml").unwrap().display_name(),
        "YAML"
    );
}

#[test]
fn test_config_defaults() {
    let config = YamlRendererConfig::default();
    assert_eq!(config.indent_width, 2);
    assert_eq!(config.max_depth_expanded, 4);
}

#[test]
fn test_quoted_string_value() {
    let r = renderer();
    let block = make_block(&["name: \"par-term\""]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();
    let str_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text.contains("\"par-term\""))
        .unwrap();
    assert_eq!(str_seg.fg, Some(theme.palette[2]));
}
