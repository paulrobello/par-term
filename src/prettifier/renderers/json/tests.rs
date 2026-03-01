//! Tests for the JSON renderer.

use super::parser::{JsonRenderer, JsonRendererConfig, register_json_renderer};
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

fn renderer() -> JsonRenderer {
    JsonRenderer::new(JsonRendererConfig::default())
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

// -- Trait methods --

#[test]
fn test_format_id() {
    let r = renderer();
    assert_eq!(r.format_id(), "json");
    assert_eq!(r.display_name(), "JSON");
    assert_eq!(r.format_badge(), "{}");
    assert_eq!(r.capabilities(), vec![RendererCapability::TextStyling]);
}

// -- Basic rendering --

#[test]
fn test_render_simple_object() {
    let r = renderer();
    let block = make_block(&[r#"{"name": "par-term", "version": 1}"#]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    assert!(text.contains("\"name\""));
    assert!(text.contains("\"par-term\""));
    assert!(text.contains("1"));
    assert_eq!(result.format_badge, "{}");
}

#[test]
fn test_render_simple_array() {
    let r = renderer();
    let block = make_block(&[r#"["a", "b", "c"]"#]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    assert!(text.contains("\"a\""));
    assert!(text.contains("\"b\""));
    assert!(text.contains("\"c\""));
}

#[test]
fn test_render_nested_object() {
    let r = renderer();
    let json = r#"{"config": {"fps": 60, "vsync": true}}"#;
    let block = make_block(&[json]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    assert!(text.contains("\"config\""));
    assert!(text.contains("\"fps\""));
    assert!(text.contains("60"));
    assert!(text.contains("true"));
}

// -- Syntax highlighting --

#[test]
fn test_string_color() {
    let r = renderer();
    let block = make_block(&[r#"{"key": "value"}"#]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();

    // Find a segment containing "value" text
    let str_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text.contains("\"value\""))
        .unwrap();
    assert_eq!(str_seg.fg, Some(theme.palette[2])); // Green
}

#[test]
fn test_number_color() {
    let r = renderer();
    let block = make_block(&[r#"{"count": 42}"#]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();

    let num_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text == "42")
        .unwrap();
    assert_eq!(num_seg.fg, Some(theme.palette[11])); // Bright yellow
}

#[test]
fn test_boolean_color() {
    let r = renderer();
    let block = make_block(&[r#"{"flag": true}"#]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();

    let bool_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text == "true")
        .unwrap();
    assert_eq!(bool_seg.fg, Some(theme.palette[5])); // Magenta
}

#[test]
fn test_null_highlighted() {
    let r = renderer();
    let block = make_block(&[r#"{"empty": null}"#]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();

    let null_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text == "null")
        .unwrap();
    assert_eq!(null_seg.fg, Some(theme.palette[8])); // Dim grey
    assert!(null_seg.italic);
}

#[test]
fn test_null_not_highlighted() {
    let r = JsonRenderer::new(JsonRendererConfig {
        highlight_nulls: false,
        ..Default::default()
    });
    let block = make_block(&[r#"{"empty": null}"#]);
    let result = r.render(&block, &test_config()).unwrap();

    let null_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text == "null")
        .unwrap();
    assert!(null_seg.fg.is_none());
    assert!(!null_seg.italic);
}

#[test]
fn test_key_color() {
    let r = renderer();
    let block = make_block(&[r#"{"mykey": 1}"#]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();

    let key_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text.contains("\"mykey\""))
        .unwrap();
    assert_eq!(key_seg.fg, Some(theme.palette[6])); // Cyan
}

// -- Tree guides --

#[test]
fn test_tree_guides_present() {
    let r = renderer();
    let json = r#"{"a": {"b": 1}}"#;
    let block = make_block(&[json]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    assert!(text.contains('│'));
}

// -- Collapsible nodes --

#[test]
fn test_auto_collapse_deep_object() {
    let r = JsonRenderer::new(JsonRendererConfig {
        max_depth_expanded: 1,
        ..Default::default()
    });
    let json = r#"{"level1": {"level2": {"deep": true}}}"#;
    let block = make_block(&[json]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    // The deep object should be collapsed
    assert!(text.contains("keys"));
}

#[test]
fn test_auto_collapse_deep_array() {
    let r = JsonRenderer::new(JsonRendererConfig {
        max_depth_expanded: 1,
        ..Default::default()
    });
    let json = r#"{"items": [1, 2, 3]}"#;
    let block = make_block(&[json]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    assert!(text.contains("items"));
}

// -- Type indicators --

#[test]
fn test_show_types() {
    let r = JsonRenderer::new(JsonRendererConfig {
        show_types: true,
        ..Default::default()
    });
    let block = make_block(&[r#"{"name": "test", "count": 5, "active": true, "data": null}"#]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    assert!(text.contains("(string)"));
    assert!(text.contains("(number)"));
    assert!(text.contains("(bool)"));
    assert!(text.contains("(null)"));
}

#[test]
fn test_types_hidden_by_default() {
    let r = renderer();
    let block = make_block(&[r#"{"name": "test"}"#]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    assert!(!text.contains("(string)"));
}

// -- Large array truncation --

#[test]
fn test_large_array_truncation() {
    let r = JsonRenderer::new(JsonRendererConfig {
        max_array_display: 3,
        ..Default::default()
    });
    let block = make_block(&["[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]"]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    assert!(text.contains("... and 7 more items"));
}

// -- String truncation --

#[test]
fn test_long_string_truncation() {
    let r = JsonRenderer::new(JsonRendererConfig {
        max_string_length: 10,
        ..Default::default()
    });
    let long_str = "a".repeat(50);
    let json = format!(r#"{{"text": "{long_str}"}}"#);
    let block = make_block(&[&json]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    assert!(text.contains("...\""));
}

// -- URL detection --

#[test]
fn test_url_becomes_hyperlink() {
    let r = renderer();
    let block = make_block(&[r#"{"url": "https://example.com/api"}"#]);
    let result = r.render(&block, &test_config()).unwrap();

    let url_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.link_url.is_some())
        .unwrap();
    assert_eq!(url_seg.link_url.as_deref(), Some("https://example.com/api"));
    assert!(url_seg.underline);
}

#[test]
fn test_url_detection_disabled() {
    let r = JsonRenderer::new(JsonRendererConfig {
        clickable_urls: false,
        ..Default::default()
    });
    let block = make_block(&[r#"{"url": "https://example.com"}"#]);
    let result = r.render(&block, &test_config()).unwrap();

    let has_link = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .any(|s| s.link_url.is_some());
    assert!(!has_link);
}

// -- Sort keys --

#[test]
fn test_sort_keys() {
    let r = JsonRenderer::new(JsonRendererConfig {
        sort_keys: true,
        ..Default::default()
    });
    let block = make_block(&[r#"{"zebra": 1, "alpha": 2, "middle": 3}"#]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    let alpha_pos = text.find("\"alpha\"").unwrap();
    let middle_pos = text.find("\"middle\"").unwrap();
    let zebra_pos = text.find("\"zebra\"").unwrap();
    assert!(alpha_pos < middle_pos);
    assert!(middle_pos < zebra_pos);
}

// -- Invalid JSON --

#[test]
fn test_invalid_json_produces_error() {
    let r = renderer();
    let block = make_block(&["not valid json {"]);
    let result = r.render(&block, &test_config());
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("Invalid JSON"));
}

// -- Line mappings --

#[test]
fn test_line_mappings_populated() {
    let r = renderer();
    let block = make_block(&[r#"{"a": 1, "b": 2}"#]);
    let result = r.render(&block, &test_config()).unwrap();
    assert_eq!(result.line_mapping.len(), result.lines.len());
}

// -- Registration --

#[test]
fn test_register_json_renderer() {
    let mut registry = RendererRegistry::new(0.6);
    register_json_renderer(&mut registry, &JsonRendererConfig::default());
    assert_eq!(registry.renderer_count(), 1);
    assert!(registry.get_renderer("json").is_some());
    assert_eq!(
        registry.get_renderer("json").unwrap().display_name(),
        "JSON"
    );
}

// -- Edge cases --

#[test]
fn test_empty_object() {
    let r = renderer();
    let block = make_block(&["{}"]);
    let result = r.render(&block, &test_config()).unwrap();
    assert!(!result.lines.is_empty());
}

#[test]
fn test_empty_array() {
    let r = renderer();
    let block = make_block(&["[]"]);
    let result = r.render(&block, &test_config()).unwrap();
    assert!(!result.lines.is_empty());
}

#[test]
fn test_scalar_top_level_string() {
    let r = renderer();
    let block = make_block(&[r#""just a string""#]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    assert!(text.contains("\"just a string\""));
}

#[test]
fn test_scalar_top_level_number() {
    let r = renderer();
    let block = make_block(&["42"]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    assert!(text.contains("42"));
}

#[test]
fn test_multiline_json() {
    let r = renderer();
    let block = make_block(&["{", "  \"key\": \"value\"", "}"]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    assert!(text.contains("\"key\""));
    assert!(text.contains("\"value\""));
}

#[test]
fn test_deeply_nested_auto_collapses() {
    let r = JsonRenderer::new(JsonRendererConfig {
        max_depth_expanded: 2,
        ..Default::default()
    });
    let json = r#"{"a": {"b": {"c": {"d": 1}}}}"#;
    let block = make_block(&[json]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    // depth 2 = c's object should be collapsed
    assert!(text.contains("keys"));
}

#[test]
fn test_array_length_annotation() {
    let r = renderer();
    let block = make_block(&[r#"{"items": [1, 2, 3]}"#]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    assert!(text.contains("3 items"));
}

#[test]
fn test_config_defaults() {
    let config = JsonRendererConfig::default();
    assert_eq!(config.max_depth_expanded, 3);
    assert_eq!(config.max_string_length, 200);
    assert!(config.show_array_length);
    assert!(!config.show_types);
    assert!(!config.sort_keys);
    assert!(config.highlight_nulls);
    assert!(config.clickable_urls);
    assert_eq!(config.max_array_display, 50);
}
