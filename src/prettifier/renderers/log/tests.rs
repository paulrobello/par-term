//! Tests for the log renderer.

use super::level_parser::{LogLevel, LogRenderer, LogRendererConfig, register_log_renderer};
use crate::prettifier::registry::RendererRegistry;
use crate::prettifier::testing::test_renderer_config;
use crate::prettifier::traits::{ContentRenderer, RendererConfig, ThemeColors};
use crate::prettifier::types::{ContentBlock, RendererCapability, StyledLine};
use std::time::SystemTime;

fn test_config() -> RendererConfig {
    test_renderer_config()
}

fn renderer() -> LogRenderer {
    LogRenderer::new(LogRendererConfig::default())
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
    assert_eq!(r.format_id(), "log");
    assert_eq!(r.display_name(), "Log Output");
    assert_eq!(r.format_badge(), "LOG");
    assert_eq!(r.capabilities(), vec![RendererCapability::TextStyling]);
}

// -- Log level coloring --

#[test]
fn test_info_level_green() {
    let r = renderer();
    let block = make_block(&["2024-01-15T10:30:00Z INFO Server started"]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();

    let level_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text.contains("INFO"))
        .unwrap();
    assert_eq!(level_seg.fg, Some(theme.palette[2])); // Green
}

#[test]
fn test_warn_level_yellow() {
    let r = renderer();
    let block = make_block(&["2024-01-15T10:30:00Z WARN Slow query"]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();

    let level_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text.contains("WARN"))
        .unwrap();
    assert_eq!(level_seg.fg, Some(theme.palette[3])); // Yellow
    assert!(level_seg.bold);
}

#[test]
fn test_error_level_red_bold() {
    let r = renderer();
    let block = make_block(&["2024-01-15T10:30:00Z ERROR Connection failed"]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();

    let level_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text.contains("ERROR"))
        .unwrap();
    assert_eq!(level_seg.fg, Some(theme.palette[9])); // Bright red
    assert!(level_seg.bold);
}

#[test]
fn test_fatal_level_has_bg() {
    let r = renderer();
    let block = make_block(&["2024-01-15T10:30:00Z FATAL System crash"]);
    let result = r.render(&block, &test_config()).unwrap();

    let level_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text.contains("FATAL"))
        .unwrap();
    assert!(level_seg.bg.is_some());
    assert!(level_seg.bold);
}

#[test]
fn test_trace_level_dim() {
    let r = renderer();
    let block = make_block(&["TRACE detailed tracing info"]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();

    let level_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text.contains("TRACE"))
        .unwrap();
    assert_eq!(level_seg.fg, Some(theme.palette[8])); // Dim
}

// -- Timestamp dimming --

#[test]
fn test_timestamp_dimmed() {
    let r = renderer();
    let block = make_block(&["2024-01-15T10:30:00Z INFO test"]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();

    let ts_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text.contains("2024-01-15"))
        .unwrap();
    assert_eq!(ts_seg.fg, Some(theme.palette[8])); // Dim
}

// -- Error message highlighting --

#[test]
fn test_error_message_bold() {
    let r = renderer();
    let block = make_block(&["2024-01-15T10:30:00Z ERROR Connection refused"]);
    let result = r.render(&block, &test_config()).unwrap();

    let msg_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text.contains("Connection refused"))
        .unwrap();
    assert!(msg_seg.bold);
}

// -- Stack trace folding --

#[test]
fn test_stack_trace_folding() {
    let r = LogRenderer::new(LogRendererConfig {
        max_visible_frames: 2,
        ..Default::default()
    });
    let block = make_block(&[
        "2024-01-15T10:30:00Z ERROR NullPointerException",
        "    at com.example.App.main(App.java:42)",
        "    at com.example.Runner.run(Runner.java:10)",
        "    at com.example.Framework.start(Framework.java:5)",
        "    at com.example.Boot.init(Boot.java:3)",
        "2024-01-15T10:30:01Z INFO Recovery started",
    ]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);

    // First 2 frames visible
    assert!(text.contains("App.java:42"));
    assert!(text.contains("Runner.java:10"));
    // Remaining 2 folded
    assert!(text.contains("2 more stack frames"));
    // Next log line still rendered
    assert!(text.contains("Recovery started"));
}

#[test]
fn test_no_folding_when_few_frames() {
    let r = renderer(); // max_visible_frames=3
    let block = make_block(&[
        "2024-01-15T10:30:00Z ERROR NullPointerException",
        "    at com.example.App.main(App.java:42)",
        "    at com.example.Runner.run(Runner.java:10)",
    ]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);

    assert!(text.contains("App.java:42"));
    assert!(text.contains("Runner.java:10"));
    assert!(!text.contains("more stack frames"));
}

// -- JSON-in-log --

#[test]
fn test_json_in_log_highlighted() {
    let r = renderer();
    let block =
        make_block(&[r#"2024-01-15T10:30:00Z INFO Response: {"status":200,"data":{"id":42}}"#]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();

    let json_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text.contains(r#"{"status""#))
        .unwrap();
    assert_eq!(json_seg.fg, Some(theme.palette[6])); // Cyan
}

#[test]
fn test_json_expansion_disabled() {
    let r = LogRenderer::new(LogRendererConfig {
        expand_json: false,
        ..Default::default()
    });
    let block = make_block(&[r#"2024-01-15T10:30:00Z INFO Response: {"status":200}"#]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();

    // JSON should NOT be separately colored
    let has_cyan_json = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .any(|s| s.text.contains(r#"{"status""#) && s.fg == Some(theme.palette[6]));
    assert!(!has_cyan_json);
}

// -- Level prefix format --

#[test]
fn test_level_prefix_without_timestamp() {
    let r = renderer();
    let block = make_block(&[
        "[INFO] Server started on port 8080",
        "[ERROR] Failed to bind",
    ]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    assert!(text.contains("INFO"));
    assert!(text.contains("Server started"));
    assert!(text.contains("ERROR"));
}

// -- Syslog format --

#[test]
fn test_syslog_format() {
    let r = renderer();
    let block = make_block(&["Jan 15 10:30:00 myhost sshd[1234]: accepted connection"]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    assert!(text.contains("Jan 15 10:30:00"));
    assert!(text.contains("accepted connection"));
}

// -- Line mappings --

#[test]
fn test_line_mappings_populated() {
    let r = renderer();
    let block = make_block(&[
        "2024-01-15T10:30:00Z INFO line1",
        "2024-01-15T10:30:01Z DEBUG line2",
    ]);
    let result = r.render(&block, &test_config()).unwrap();
    assert_eq!(result.line_mapping.len(), result.lines.len());
}

// -- Registration --

#[test]
fn test_register_log_renderer() {
    let mut registry = RendererRegistry::new(0.6);
    register_log_renderer(&mut registry, &LogRendererConfig::default());
    assert_eq!(registry.renderer_count(), 1);
    assert!(registry.get_renderer("log").is_some());
    assert_eq!(
        registry.get_renderer("log").unwrap().display_name(),
        "Log Output"
    );
}

// -- Config defaults --

#[test]
fn test_config_defaults() {
    let config = LogRendererConfig::default();
    assert_eq!(config.max_visible_frames, 3);
    assert!(config.expand_json);
}

// -- Edge cases --

#[test]
fn test_empty_log() {
    let r = renderer();
    let block = make_block(&[]);
    let result = r.render(&block, &test_config()).unwrap();
    assert!(result.lines.is_empty());
}

#[test]
fn test_plain_text_passthrough() {
    let r = renderer();
    let block = make_block(&["just some plain text"]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);
    assert!(text.contains("just some plain text"));
}

#[test]
fn test_format_badge() {
    let r = renderer();
    let block = make_block(&["2024-01-15T10:30:00Z INFO test"]);
    let result = r.render(&block, &test_config()).unwrap();
    assert_eq!(result.format_badge, "LOG");
}

#[test]
fn test_log_level_from_str() {
    assert_eq!(LogLevel::from_str("TRACE"), Some(LogLevel::Trace));
    assert_eq!(LogLevel::from_str("DEBUG"), Some(LogLevel::Debug));
    assert_eq!(LogLevel::from_str("INFO"), Some(LogLevel::Info));
    assert_eq!(LogLevel::from_str("WARN"), Some(LogLevel::Warn));
    assert_eq!(LogLevel::from_str("WARNING"), Some(LogLevel::Warn));
    assert_eq!(LogLevel::from_str("ERROR"), Some(LogLevel::Error));
    assert_eq!(LogLevel::from_str("ERR"), Some(LogLevel::Error));
    assert_eq!(LogLevel::from_str("FATAL"), Some(LogLevel::Fatal));
    assert_eq!(LogLevel::from_str("CRITICAL"), Some(LogLevel::Fatal));
    assert_eq!(LogLevel::from_str("UNKNOWN"), None);
}
