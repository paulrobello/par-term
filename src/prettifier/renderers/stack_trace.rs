//! Stack trace renderer with root error highlighting, frame classification,
//! clickable file paths, and collapsible long traces.
//!
//! Parses stack traces from multiple languages (Java, Python, Rust, Go, JS)
//! and renders with:
//!
//! - **Root error highlighting**: error/exception messages bold red
//! - **Frame classification**: application frames bright, framework frames dimmed
//! - **Clickable file paths**: file:line patterns rendered as links
//! - **Collapsible traces**: long traces folded with "... N more frames"

use std::sync::OnceLock;

use regex::Regex;

use crate::prettifier::registry::RendererRegistry;
use crate::prettifier::traits::{ContentRenderer, RenderError, RendererConfig, ThemeColors};
use crate::prettifier::types::{
    ContentBlock, RenderedContent, RendererCapability, SourceLineMapping, StyledLine, StyledSegment,
};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the stack trace renderer.
#[derive(Clone, Debug)]
pub struct StackTraceRendererConfig {
    /// Package name prefixes considered "application" code (bright styling).
    /// Frames not matching these are "framework" (dimmed).
    pub app_packages: Vec<String>,
    /// Maximum frames to show before collapsing (default: 5).
    pub max_visible_frames: usize,
    /// Always keep the last N frames visible (for "Caused by" chains, default: 1).
    pub keep_tail_frames: usize,
}

impl Default for StackTraceRendererConfig {
    fn default() -> Self {
        Self {
            app_packages: Vec::new(),
            max_visible_frames: 5,
            keep_tail_frames: 1,
        }
    }
}

// ---------------------------------------------------------------------------
// Frame types
// ---------------------------------------------------------------------------

/// Classification of a stack frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrameType {
    /// User application code — rendered with bright/normal colors.
    Application,
    /// Framework or library code — rendered dimmed.
    Framework,
}

/// A file path with line number extracted from a stack frame.
#[derive(Debug, Clone)]
struct FilePath {
    /// The file path as it appeared in the frame.
    path: String,
    /// The line number (if found).
    line: Option<usize>,
    /// Optional column number.
    column: Option<usize>,
}

/// Classification of a line in a stack trace.
#[derive(Debug)]
enum TraceLine {
    /// Error/exception header (e.g., "java.lang.NullPointerException: message").
    ErrorHeader(String),
    /// "Caused by:" chain header.
    CausedBy(String),
    /// A stack frame line.
    Frame {
        text: String,
        frame_type: FrameType,
        file_path: Option<FilePath>,
    },
    /// Other text (context, notes, etc.).
    Other(String),
}

// ---------------------------------------------------------------------------
// Regex helpers
// ---------------------------------------------------------------------------

fn re_java_frame() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\s+at\s+([\w.$]+)\(([\w.]+):(\d+)\)").unwrap())
}

fn re_python_frame() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"^\s+File "([^"]+)", line (\d+)"#).unwrap())
}

fn re_js_frame() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\s+at\s+\S+\s+\((.+):(\d+):(\d+)\)").unwrap())
}

fn re_rust_location() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"([\w/\\.-]+\.rs):(\d+)").unwrap())
}

fn re_go_location() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"([\w/\\.-]+\.go):(\d+)").unwrap())
}

fn re_error_header() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^([\w.]+(?:Error|Exception|Panic)):?\s").unwrap())
}

fn re_caused_by() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^Caused by:").unwrap())
}

fn re_python_traceback_header() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^Traceback \(most recent call last\):").unwrap())
}

fn re_rust_panic() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^thread '.*' panicked at").unwrap())
}

fn re_go_panic() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^goroutine \d+ \[").unwrap())
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Classify a frame as application or framework based on app package prefixes.
fn classify_frame(frame_text: &str, app_packages: &[String]) -> FrameType {
    if app_packages.is_empty() {
        // If no app packages configured, consider indented "at" frames as framework
        // and other frames as application by default
        return FrameType::Application;
    }
    if app_packages
        .iter()
        .any(|pkg| frame_text.contains(pkg.as_str()))
    {
        FrameType::Application
    } else {
        FrameType::Framework
    }
}

/// Extract a file path with line number from a stack frame.
fn extract_file_path(line: &str) -> Option<FilePath> {
    // Java: at package.Class(FileName.java:42)
    if let Some(caps) = re_java_frame().captures(line) {
        return Some(FilePath {
            path: caps.get(2).unwrap().as_str().to_string(),
            line: caps.get(3).and_then(|m| m.as_str().parse().ok()),
            column: None,
        });
    }

    // Python: File "path/to/file.py", line 42
    if let Some(caps) = re_python_frame().captures(line) {
        return Some(FilePath {
            path: caps.get(1).unwrap().as_str().to_string(),
            line: caps.get(2).and_then(|m| m.as_str().parse().ok()),
            column: None,
        });
    }

    // JavaScript/Node.js: at Function (file.js:42:10)
    if let Some(caps) = re_js_frame().captures(line) {
        return Some(FilePath {
            path: caps.get(1).unwrap().as_str().to_string(),
            line: caps.get(2).and_then(|m| m.as_str().parse().ok()),
            column: caps.get(3).and_then(|m| m.as_str().parse().ok()),
        });
    }

    // Rust: src/main.rs:42
    if let Some(caps) = re_rust_location().captures(line) {
        return Some(FilePath {
            path: caps.get(1).unwrap().as_str().to_string(),
            line: caps.get(2).and_then(|m| m.as_str().parse().ok()),
            column: None,
        });
    }

    // Go: /home/user/app/main.go:42
    if let Some(caps) = re_go_location().captures(line) {
        return Some(FilePath {
            path: caps.get(1).unwrap().as_str().to_string(),
            line: caps.get(2).and_then(|m| m.as_str().parse().ok()),
            column: None,
        });
    }

    None
}

/// Parse a line of a stack trace into a classified TraceLine.
fn parse_trace_line(line: &str, app_packages: &[String]) -> TraceLine {
    // Check for Caused by:
    if re_caused_by().is_match(line) {
        return TraceLine::CausedBy(line.to_string());
    }

    // Check for error headers
    if re_error_header().is_match(line)
        || re_python_traceback_header().is_match(line)
        || re_rust_panic().is_match(line)
        || re_go_panic().is_match(line)
    {
        return TraceLine::ErrorHeader(line.to_string());
    }

    // Check for stack frames (indented lines with frame patterns)
    let file_path = extract_file_path(line);
    let is_frame = file_path.is_some()
        || line.trim_start().starts_with("at ")
        || (line.starts_with(' ') || line.starts_with('\t'));

    if is_frame {
        let frame_type = classify_frame(line, app_packages);
        return TraceLine::Frame {
            text: line.to_string(),
            frame_type,
            file_path,
        };
    }

    TraceLine::Other(line.to_string())
}

// ---------------------------------------------------------------------------
// StackTraceRenderer
// ---------------------------------------------------------------------------

/// Renders stack traces with error highlighting, frame classification, and folding.
pub struct StackTraceRenderer {
    config: StackTraceRendererConfig,
}

impl StackTraceRenderer {
    /// Create a new stack trace renderer with the given configuration.
    pub fn new(config: StackTraceRendererConfig) -> Self {
        Self { config }
    }

    /// Render an error header line (bold red).
    fn render_error_header(text: &str, theme: &ThemeColors) -> Vec<StyledSegment> {
        vec![StyledSegment {
            text: text.to_string(),
            fg: Some(theme.palette[9]), // Bright red
            bold: true,
            ..Default::default()
        }]
    }

    /// Render a "Caused by:" line (red, bold).
    fn render_caused_by(text: &str, theme: &ThemeColors) -> Vec<StyledSegment> {
        vec![StyledSegment {
            text: text.to_string(),
            fg: Some(theme.palette[1]), // Red
            bold: true,
            ..Default::default()
        }]
    }

    /// Render a stack frame with classification and optional clickable path.
    fn render_frame(
        text: &str,
        frame_type: FrameType,
        file_path: Option<&FilePath>,
        theme: &ThemeColors,
    ) -> Vec<StyledSegment> {
        let fg = match frame_type {
            FrameType::Application => None, // Normal/default color
            FrameType::Framework => Some(theme.palette[8]), // Dimmed
        };

        // If we have a file path, try to make it clickable
        if let Some(fp) = file_path {
            let link_target = if let Some(line) = fp.line {
                if let Some(col) = fp.column {
                    format!("{}:{}:{}", fp.path, line, col)
                } else {
                    format!("{}:{}", fp.path, line)
                }
            } else {
                fp.path.clone()
            };

            // Find the file path portion in the text to make only that clickable
            if let Some(path_idx) = text.find(&fp.path) {
                let mut segments = Vec::new();

                // Text before the path
                if path_idx > 0 {
                    segments.push(StyledSegment {
                        text: text[..path_idx].to_string(),
                        fg,
                        ..Default::default()
                    });
                }

                // The path portion (clickable)
                // Find the end of the file:line:col pattern in the source text
                let actual_end = text[path_idx..]
                    .find([')', ',', ' '])
                    .map(|i| path_idx + i)
                    .unwrap_or(text.len());

                segments.push(StyledSegment {
                    text: text[path_idx..actual_end].to_string(),
                    fg: Some(theme.palette[6]), // Cyan for paths
                    underline: true,
                    link_url: Some(link_target),
                    ..Default::default()
                });

                // Text after the path
                if actual_end < text.len() {
                    segments.push(StyledSegment {
                        text: text[actual_end..].to_string(),
                        fg,
                        ..Default::default()
                    });
                }

                return segments;
            }
        }

        // No clickable path — render as a single segment
        vec![StyledSegment {
            text: text.to_string(),
            fg,
            ..Default::default()
        }]
    }

    /// Render a group of consecutive frames, collapsing if too many.
    fn render_frame_group(
        &self,
        frames: &[TraceLine],
        lines: &mut Vec<StyledLine>,
        line_mapping: &mut Vec<SourceLineMapping>,
        base_source_line: usize,
        theme: &ThemeColors,
    ) {
        let count = frames.len();
        if count <= self.config.max_visible_frames {
            // Show all frames
            for (i, frame) in frames.iter().enumerate() {
                if let TraceLine::Frame {
                    text,
                    frame_type,
                    file_path,
                } = frame
                {
                    let segments = Self::render_frame(text, *frame_type, file_path.as_ref(), theme);
                    push_line(lines, line_mapping, segments, Some(base_source_line + i));
                }
            }
        } else {
            // Show first N frames
            let head = self
                .config
                .max_visible_frames
                .saturating_sub(self.config.keep_tail_frames);
            for (i, frame) in frames.iter().take(head).enumerate() {
                if let TraceLine::Frame {
                    text,
                    frame_type,
                    file_path,
                } = frame
                {
                    let segments = Self::render_frame(text, *frame_type, file_path.as_ref(), theme);
                    push_line(lines, line_mapping, segments, Some(base_source_line + i));
                }
            }

            // Collapse middle
            let hidden = count - head - self.config.keep_tail_frames;
            if hidden > 0 {
                push_line(
                    lines,
                    line_mapping,
                    vec![StyledSegment {
                        text: format!("    ... {hidden} more frames"),
                        fg: Some(theme.palette[8]),
                        italic: true,
                        ..Default::default()
                    }],
                    None,
                );
            }

            // Show tail frames
            let tail_start = count - self.config.keep_tail_frames;
            for (offset, frame) in frames[tail_start..].iter().enumerate() {
                if let TraceLine::Frame {
                    text,
                    frame_type,
                    file_path,
                } = frame
                {
                    let segments = Self::render_frame(text, *frame_type, file_path.as_ref(), theme);
                    push_line(
                        lines,
                        line_mapping,
                        segments,
                        Some(base_source_line + tail_start + offset),
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helper function
// ---------------------------------------------------------------------------

/// Push a styled line and its source mapping.
fn push_line(
    lines: &mut Vec<StyledLine>,
    line_mapping: &mut Vec<SourceLineMapping>,
    segments: Vec<StyledSegment>,
    source_line: Option<usize>,
) {
    line_mapping.push(SourceLineMapping {
        rendered_line: lines.len(),
        source_line,
    });
    lines.push(StyledLine::new(segments));
}

// ---------------------------------------------------------------------------
// ContentRenderer implementation
// ---------------------------------------------------------------------------

impl ContentRenderer for StackTraceRenderer {
    fn format_id(&self) -> &str {
        "stack_trace"
    }

    fn display_name(&self) -> &str {
        "Stack Trace"
    }

    fn capabilities(&self) -> Vec<RendererCapability> {
        vec![RendererCapability::TextStyling]
    }

    fn render(
        &self,
        content: &ContentBlock,
        config: &RendererConfig,
    ) -> Result<RenderedContent, RenderError> {
        let theme = &config.theme_colors;
        let mut lines = Vec::new();
        let mut line_mapping = Vec::new();

        // Parse all lines
        let parsed: Vec<TraceLine> = content
            .lines
            .iter()
            .map(|l| parse_trace_line(l, &self.config.app_packages))
            .collect();

        // Group consecutive frames for collapsing
        let mut i = 0;
        while i < parsed.len() {
            match &parsed[i] {
                TraceLine::ErrorHeader(text) => {
                    let segments = Self::render_error_header(text, theme);
                    push_line(&mut lines, &mut line_mapping, segments, Some(i));
                    i += 1;
                }
                TraceLine::CausedBy(text) => {
                    let segments = Self::render_caused_by(text, theme);
                    push_line(&mut lines, &mut line_mapping, segments, Some(i));
                    i += 1;
                }
                TraceLine::Frame { .. } => {
                    // Collect consecutive frames
                    let frame_start = i;
                    while i < parsed.len() && matches!(&parsed[i], TraceLine::Frame { .. }) {
                        i += 1;
                    }
                    self.render_frame_group(
                        &parsed[frame_start..i],
                        &mut lines,
                        &mut line_mapping,
                        frame_start,
                        theme,
                    );
                }
                TraceLine::Other(text) => {
                    push_line(
                        &mut lines,
                        &mut line_mapping,
                        vec![StyledSegment {
                            text: text.clone(),
                            ..Default::default()
                        }],
                        Some(i),
                    );
                    i += 1;
                }
            }
        }

        Ok(RenderedContent {
            lines,
            line_mapping,
            graphics: vec![],
            format_badge: "TRACE".to_string(),
        })
    }

    fn format_badge(&self) -> &str {
        "TRACE"
    }
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register the stack trace renderer with the registry.
pub fn register_stack_trace_renderer(
    registry: &mut RendererRegistry,
    config: &StackTraceRendererConfig,
) {
    registry.register_renderer(
        "stack_trace",
        Box::new(StackTraceRenderer::new(config.clone())),
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prettifier::traits::RendererConfig;
    use crate::prettifier::types::ContentBlock;
    use std::time::SystemTime;

    fn test_config() -> RendererConfig {
        RendererConfig {
            terminal_width: 80,
            theme_colors: ThemeColors::default(),
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
}
