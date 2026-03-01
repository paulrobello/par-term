//! Tests for the TOML renderer.

use super::parser::{
    TomlRenderer, TomlRendererConfig, is_toml_datetime, is_toml_number, register_toml_renderer,
};
use crate::prettifier::registry::RendererRegistry;
use crate::prettifier::testing::test_renderer_config;
use crate::prettifier::traits::{ContentRenderer, RendererConfig, ThemeColors};
use crate::prettifier::types::{ContentBlock, RendererCapability, StyledLine};
use std::time::SystemTime;

fn test_config() -> RendererConfig {
    test_renderer_config()
}

fn renderer() -> TomlRenderer {
    TomlRenderer::new(TomlRendererConfig::default())
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
    assert_eq!(r.format_id(), "toml");
    assert_eq!(r.display_name(), "TOML");
    assert_eq!(r.format_badge(), "TOML");
    assert_eq!(r.capabilities(), vec![RendererCapability::TextStyling]);
}

#[test]
fn test_render_section_header() {
    let r = renderer();
    let block = make_block(&["[package]", "name = \"par-term\""]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    assert!(text.contains("[package]"));
    assert!(text.contains("name"));
    assert!(text.contains("\"par-term\""));
    assert_eq!(result.format_badge, "TOML");
}

#[test]
fn test_section_header_bold_blue() {
    let r = renderer();
    let block = make_block(&["[package]"]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();
    let header_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text.contains("[package]"))
        .unwrap();
    assert_eq!(header_seg.fg, Some(theme.palette[4]));
    assert!(header_seg.bold);
}

#[test]
fn test_render_array_table() {
    let r = renderer();
    let block = make_block(&["[[bin]]", "name = \"par-term\""]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    assert!(text.contains("[[bin]]"));
}

#[test]
fn test_array_table_bright_blue() {
    let r = renderer();
    let block = make_block(&["[[bin]]"]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();
    let header_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text.contains("[[bin]]"))
        .unwrap();
    assert_eq!(header_seg.fg, Some(theme.palette[12]));
    assert!(header_seg.bold);
}

#[test]
fn test_render_comment() {
    let r = renderer();
    let block = make_block(&["# This is a comment"]);
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
fn test_string_value_coloring() {
    let r = renderer();
    let block = make_block(&["name = \"par-term\""]);
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

#[test]
fn test_number_value_coloring() {
    let r = renderer();
    let block = make_block(&["port = 8080"]);
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
fn test_boolean_value_coloring() {
    let r = renderer();
    let block = make_block(&["enabled = true"]);
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
fn test_datetime_value_coloring() {
    let r = renderer();
    let block = make_block(&["created = 2024-01-15T10:30:00"]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();
    let date_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text.contains("2024-01-15"))
        .unwrap();
    assert_eq!(date_seg.fg, Some(theme.palette[14]));
}

#[test]
fn test_key_coloring() {
    let r = renderer();
    let block = make_block(&["mykey = \"value\""]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();
    let key_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text == "mykey")
        .unwrap();
    assert_eq!(key_seg.fg, Some(theme.palette[6]));
}

#[test]
fn test_key_value_alignment() {
    let r = renderer();
    let block = make_block(&[
        "[package]",
        "name = \"par-term\"",
        "version = \"0.16.0\"",
        "edition = \"2024\"",
    ]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    assert!(text.contains("name"));
    assert!(text.contains("version"));
    assert!(text.contains("edition"));
}

#[test]
fn test_nested_section_depth() {
    let r = renderer();
    let block = make_block(&[
        "[server]",
        "host = \"localhost\"",
        "",
        "[server.tls]",
        "enabled = true",
    ]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    assert!(text.contains("[server.tls]"));
}

#[test]
fn test_line_mappings_populated() {
    let r = renderer();
    let block = make_block(&["[package]", "name = \"test\""]);
    let result = r.render(&block, &test_config()).unwrap();
    assert_eq!(result.line_mapping.len(), result.lines.len());
}

#[test]
fn test_auto_collapse_deep_section() {
    let r = TomlRenderer::new(TomlRendererConfig {
        max_depth_expanded: 0,
        ..Default::default()
    });
    let block = make_block(&["[package]", "name = \"test\"", "version = \"1.0\""]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    assert!(text.contains("keys"));
}

#[test]
fn test_empty_lines_preserved() {
    let r = renderer();
    let block = make_block(&["[a]", "x = 1", "", "[b]", "y = 2"]);
    let result = r.render(&block, &test_config()).unwrap();
    assert_eq!(result.lines.len(), 5);
}

#[test]
fn test_register_toml_renderer() {
    let mut registry = RendererRegistry::new(0.6);
    register_toml_renderer(&mut registry, &TomlRendererConfig::default());
    assert_eq!(registry.renderer_count(), 1);
    assert!(registry.get_renderer("toml").is_some());
    assert_eq!(
        registry.get_renderer("toml").unwrap().display_name(),
        "TOML"
    );
}

#[test]
fn test_config_defaults() {
    let config = TomlRendererConfig::default();
    assert!(config.align_equals);
    assert_eq!(config.max_depth_expanded, 4);
}

#[test]
fn test_is_toml_number() {
    assert!(is_toml_number("42"));
    assert!(is_toml_number("3.14"));
    assert!(is_toml_number("-17"));
    assert!(is_toml_number("1_000"));
    assert!(is_toml_number("0xFF"));
    assert!(is_toml_number("0o77"));
    assert!(is_toml_number("0b1010"));
    assert!(is_toml_number("inf"));
    assert!(is_toml_number("+inf"));
    assert!(is_toml_number("nan"));
    assert!(!is_toml_number("hello"));
}

#[test]
fn test_is_toml_datetime() {
    assert!(is_toml_datetime("2024-01-15"));
    assert!(is_toml_datetime("2024-01-15T10:30:00"));
    assert!(is_toml_datetime("2024-01-15 10:30"));
    assert!(!is_toml_datetime("hello"));
    assert!(!is_toml_datetime("42"));
}

#[test]
fn test_array_value() {
    let r = renderer();
    let block = make_block(&["tags = [\"rust\", \"terminal\"]"]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();
    let arr_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text.contains('['))
        .unwrap();
    assert_eq!(arr_seg.fg, Some(theme.palette[3]));
}

#[test]
fn test_inline_table_value() {
    let r = renderer();
    let block = make_block(&["point = { x = 1, y = 2 }"]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();
    let table_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text.contains('{'))
        .unwrap();
    assert_eq!(table_seg.fg, Some(theme.palette[3]));
}

#[test]
fn test_no_alignment_when_disabled() {
    let r = TomlRenderer::new(TomlRendererConfig {
        align_equals: false,
        ..Default::default()
    });
    let block = make_block(&["a = 1", "longkey = 2"]);
    let result = r.render(&block, &test_config()).unwrap();
    let first_line_text: String = result.lines[0]
        .segments
        .iter()
        .map(|s| s.text.as_str())
        .collect();
    assert!(first_line_text.contains("a = "));
}
