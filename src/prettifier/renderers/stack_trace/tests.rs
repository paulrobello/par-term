//! Tests for the stack trace renderer.

use std::time::SystemTime;

use crate::prettifier::registry::RendererRegistry;
use crate::prettifier::traits::{ContentRenderer, RendererConfig, ThemeColors};
use crate::prettifier::types::{ContentBlock, RendererCapability, StyledLine};

use super::config::StackTraceRendererConfig;
use super::parse::{classify_frame, extract_file_path};
use super::renderer::{StackTraceRenderer, register_stack_trace_renderer};
use super::types::FrameType;

fn test_config() -> RendererConfig {
    RendererConfig {
        terminal_width: 80,
        ..Default::default()
    }
}

fn renderer() -> StackTraceRenderer {
    StackTraceRenderer::new(StackTraceRendererConfig::default())
}

fn renderer_with_packages(packages: Vec<String>) -> StackTraceRenderer {
    StackTraceRenderer::new(StackTraceRendererConfig {
        app_packages: packages,
        ..Default::default()
    })
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
    assert_eq!(r.format_id(), "stack_trace");
    assert_eq!(r.display_name(), "Stack Trace");
    assert_eq!(r.format_badge(), "TRACE");
    assert_eq!(r.capabilities(), vec![RendererCapability::TextStyling]);
}

// -- Error header highlighting --

#[test]
fn test_java_error_header_red_bold() {
    let r = renderer();
    let block = make_block(&[
        "java.lang.NullPointerException: Cannot invoke method on null",
        "    at com.example.App.main(App.java:42)",
    ]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();

    // The error header line should be the first line and be bold red
    let first_line = &result.lines[0];
    let header_seg = &first_line.segments[0];
    assert_eq!(header_seg.fg, Some(theme.palette[9])); // Bright red
    assert!(header_seg.bold);
}

#[test]
fn test_python_traceback_header() {
    let r = renderer();
    let block = make_block(&[
        "Traceback (most recent call last):",
        "  File \"app.py\", line 42, in main",
        "TypeError: unsupported operand",
    ]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();

    let header_seg = &result.lines[0].segments[0];
    assert_eq!(header_seg.fg, Some(theme.palette[9]));
    assert!(header_seg.bold);
}

#[test]
fn test_rust_panic_header() {
    let r = renderer();
    let block = make_block(&[
        "thread 'main' panicked at 'index out of bounds'",
        "note: run with RUST_BACKTRACE=1",
    ]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();

    let header_seg = &result.lines[0].segments[0];
    assert_eq!(header_seg.fg, Some(theme.palette[9]));
    assert!(header_seg.bold);
}

#[test]
fn test_go_panic_header() {
    let r = renderer();
    let block = make_block(&["goroutine 1 [running]:", "main.main()"]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();

    let header_seg = &result.lines[0].segments[0];
    assert_eq!(header_seg.fg, Some(theme.palette[9]));
    assert!(header_seg.bold);
}

// -- Caused by highlighting --

#[test]
fn test_caused_by_red_bold() {
    let r = renderer();
    let block = make_block(&[
        "RuntimeException: Failed",
        "Caused by: java.net.ConnectException: refused",
        "    at java.net.Socket.connect(Socket.java:591)",
    ]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();

    let caused = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.text.contains("Caused by"))
        .unwrap();
    assert_eq!(caused.fg, Some(theme.palette[1])); // Red
    assert!(caused.bold);
}

// -- Frame classification --

#[test]
fn test_frame_classification_with_packages() {
    let r = renderer_with_packages(vec!["com.example".to_string()]);
    let block = make_block(&[
        "NullPointerException: test",
        "    at com.example.App.main(App.java:42)",
        "    at org.framework.Runner.run(Runner.java:10)",
    ]);
    let result = r.render(&block, &test_config()).unwrap();
    let theme = ThemeColors::default();

    // App frame should NOT be dimmed (fg = None for Application)
    let app_frame = result
        .lines
        .iter()
        .find(|l| l.segments.iter().any(|s| s.text.contains("com.example")));
    assert!(app_frame.is_some());

    // Framework frame should be dimmed
    let fw_frame = result
        .lines
        .iter()
        .find(|l| l.segments.iter().any(|s| s.text.contains("org.framework")));
    assert!(fw_frame.is_some());
    let fw_segments = &fw_frame.unwrap().segments;
    let dim_seg = fw_segments
        .iter()
        .find(|s| s.text.contains("org.framework"))
        .unwrap();
    assert_eq!(dim_seg.fg, Some(theme.palette[8])); // Dimmed
}

// -- Clickable file paths --

#[test]
fn test_java_file_path_clickable() {
    let r = renderer();
    let block = make_block(&[
        "NullPointerException: test",
        "    at com.example.App.main(App.java:42)",
    ]);
    let result = r.render(&block, &test_config()).unwrap();

    let has_link = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .any(|s| s.link_url.is_some() && s.underline);
    assert!(has_link);
}

#[test]
fn test_python_file_path_clickable() {
    let r = renderer();
    let block = make_block(&[
        "Traceback (most recent call last):",
        "  File \"app.py\", line 42, in main",
    ]);
    let result = r.render(&block, &test_config()).unwrap();

    let link_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.link_url.is_some());
    assert!(link_seg.is_some());
    let link = link_seg.unwrap();
    assert!(link.link_url.as_ref().unwrap().contains("app.py"));
}

#[test]
fn test_js_file_path_clickable() {
    let r = renderer();
    let block = make_block(&[
        "TypeError: Cannot read property",
        "    at Object.main (/app/index.js:42:10)",
    ]);
    let result = r.render(&block, &test_config()).unwrap();

    let link_seg = result
        .lines
        .iter()
        .flat_map(|l| &l.segments)
        .find(|s| s.link_url.is_some());
    assert!(link_seg.is_some());
}

// -- Collapsible traces --

#[test]
fn test_long_trace_collapsed() {
    let r = StackTraceRenderer::new(StackTraceRendererConfig {
        max_visible_frames: 3,
        keep_tail_frames: 1,
        ..Default::default()
    });
    let block = make_block(&[
        "NullPointerException: test",
        "    at com.example.A.method(A.java:1)",
        "    at com.example.B.method(B.java:2)",
        "    at com.example.C.method(C.java:3)",
        "    at com.example.D.method(D.java:4)",
        "    at com.example.E.method(E.java:5)",
        "    at com.example.F.method(F.java:6)",
        "    at com.example.G.method(G.java:7)",
    ]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);

    // First 2 frames visible (max_visible=3 - keep_tail=1)
    assert!(text.contains("A.java:1"));
    assert!(text.contains("B.java:2"));
    // Middle collapsed
    assert!(text.contains("more frames"));
    // Last frame visible (keep_tail=1)
    assert!(text.contains("G.java:7"));
}

#[test]
fn test_short_trace_not_collapsed() {
    let r = renderer(); // max_visible_frames=5
    let block = make_block(&[
        "NullPointerException: test",
        "    at com.example.A.method(A.java:1)",
        "    at com.example.B.method(B.java:2)",
        "    at com.example.C.method(C.java:3)",
    ]);
    let result = r.render(&block, &test_config()).unwrap();
    let text = all_text(&result.lines);

    assert!(text.contains("A.java:1"));
    assert!(text.contains("B.java:2"));
    assert!(text.contains("C.java:3"));
    assert!(!text.contains("more frames"));
}

// -- File path extraction --

#[test]
fn test_extract_java_path() {
    let fp = extract_file_path("    at com.example.App.main(App.java:42)");
    assert!(fp.is_some());
    let fp = fp.unwrap();
    assert_eq!(fp.path, "App.java");
    assert_eq!(fp.line, Some(42));
}

#[test]
fn test_extract_python_path() {
    let fp = extract_file_path("  File \"app.py\", line 42, in main");
    assert!(fp.is_some());
    let fp = fp.unwrap();
    assert_eq!(fp.path, "app.py");
    assert_eq!(fp.line, Some(42));
}

#[test]
fn test_extract_js_path() {
    let fp = extract_file_path("    at Object.main (/app/index.js:42:10)");
    assert!(fp.is_some());
    let fp = fp.unwrap();
    assert_eq!(fp.path, "/app/index.js");
    assert_eq!(fp.line, Some(42));
    assert_eq!(fp.column, Some(10));
}

#[test]
fn test_extract_rust_path() {
    let fp = extract_file_path("   src/main.rs:42");
    assert!(fp.is_some());
    let fp = fp.unwrap();
    assert_eq!(fp.path, "src/main.rs");
    assert_eq!(fp.line, Some(42));
}

#[test]
fn test_extract_go_path() {
    let fp = extract_file_path("    /home/user/app/main.go:42 +0x1a2");
    assert!(fp.is_some());
    let fp = fp.unwrap();
    assert!(fp.path.contains("main.go"));
    assert_eq!(fp.line, Some(42));
}

#[test]
fn test_extract_no_path() {
    let fp = extract_file_path("just some text");
    assert!(fp.is_none());
}

// -- Frame classification function --

#[test]
fn test_classify_app_frame() {
    let ft = classify_frame(
        "    at com.example.App.main(App.java:42)",
        &["com.example".to_string()],
    );
    assert_eq!(ft, FrameType::Application);
}

#[test]
fn test_classify_framework_frame() {
    let ft = classify_frame(
        "    at org.framework.Runner.run(Runner.java:10)",
        &["com.example".to_string()],
    );
    assert_eq!(ft, FrameType::Framework);
}

#[test]
fn test_classify_no_packages_is_app() {
    let ft = classify_frame("    at org.framework.Runner.run(Runner.java:10)", &[]);
    assert_eq!(ft, FrameType::Application);
}

// -- Line mappings --

#[test]
fn test_line_mappings_populated() {
    let r = renderer();
    let block = make_block(&[
        "NullPointerException: test",
        "    at com.example.App.main(App.java:42)",
    ]);
    let result = r.render(&block, &test_config()).unwrap();
    assert_eq!(result.line_mapping.len(), result.lines.len());
}

// -- Registration --

#[test]
fn test_register_stack_trace_renderer() {
    let mut registry = RendererRegistry::new(0.6);
    register_stack_trace_renderer(&mut registry, &StackTraceRendererConfig::default());
    assert_eq!(registry.renderer_count(), 1);
    assert!(registry.get_renderer("stack_trace").is_some());
    assert_eq!(
        registry.get_renderer("stack_trace").unwrap().display_name(),
        "Stack Trace"
    );
}

// -- Config defaults --

#[test]
fn test_config_defaults() {
    let config = StackTraceRendererConfig::default();
    assert!(config.app_packages.is_empty());
    assert_eq!(config.max_visible_frames, 5);
    assert_eq!(config.keep_tail_frames, 1);
}

// -- Edge cases --

#[test]
fn test_empty_trace() {
    let r = renderer();
    let block = make_block(&[]);
    let result = r.render(&block, &test_config()).unwrap();
    assert!(result.lines.is_empty());
}

#[test]
fn test_format_badge() {
    let r = renderer();
    let block = make_block(&["NullPointerException: test"]);
    let result = r.render(&block, &test_config()).unwrap();
    assert_eq!(result.format_badge, "TRACE");
}
