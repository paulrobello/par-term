//! Log output renderer with level coloring, timestamp dimming, error
//! highlighting, stack trace folding, and JSON-in-log expansion.
//!
//! Parses each line of log output to extract timestamps, log levels,
//! source info, message text, and embedded JSON payloads, then renders
//! with appropriate styling:
//!
//! - **Log level coloring**: TRACE/DEBUG dim, INFO green, WARN yellow, ERROR/FATAL red
//! - **Timestamp dimming**: timestamps rendered but visually de-emphasized
//! - **Error highlighting**: ERROR/FATAL lines bold/bright
//! - **Stack trace folding**: consecutive indented lines after ERROR collapsed
//! - **JSON-in-log expansion**: embedded JSON payloads detected and highlighted

use std::sync::OnceLock;

use regex::Regex;

use super::push_line;
use crate::prettifier::registry::RendererRegistry;
use crate::prettifier::traits::{ContentRenderer, RenderError, RendererConfig, ThemeColors};
use crate::prettifier::types::{ContentBlock, RenderedContent, RendererCapability, StyledSegment};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the log renderer.
#[derive(Clone, Debug)]
pub struct LogRendererConfig {
    /// Maximum consecutive stack-trace frames shown before folding (default: 3).
    pub max_visible_frames: usize,
    /// Whether to detect and highlight JSON payloads in log lines (default: true).
    pub expand_json: bool,
}

impl Default for LogRendererConfig {
    fn default() -> Self {
        Self {
            max_visible_frames: 3,
            expand_json: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Log level enum
// ---------------------------------------------------------------------------

/// Recognized log levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

impl LogLevel {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "TRACE" => Some(Self::Trace),
            "DEBUG" => Some(Self::Debug),
            "INFO" => Some(Self::Info),
            "WARN" | "WARNING" => Some(Self::Warn),
            "ERROR" | "ERR" => Some(Self::Error),
            "FATAL" | "CRITICAL" | "CRIT" => Some(Self::Fatal),
            _ => None,
        }
    }

    fn is_error_or_fatal(self) -> bool {
        matches!(self, Self::Error | Self::Fatal)
    }
}

// ---------------------------------------------------------------------------
// Parsed log line
// ---------------------------------------------------------------------------

/// A parsed log line broken into semantic components.
#[derive(Debug)]
struct LogLine {
    /// The timestamp portion (if any).
    timestamp: Option<String>,
    /// The detected log level (if any).
    level: Option<LogLevel>,
    /// The log level text as it appeared in the source.
    level_text: Option<String>,
    /// Everything after the level keyword.
    message: String,
    /// Start index of an embedded JSON payload in the message (if any).
    json_start: Option<usize>,
}

// ---------------------------------------------------------------------------
// Regex helpers
// ---------------------------------------------------------------------------

fn re_timestamp_level() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"^(\d{4}[-/]\d{2}[-/]\d{2}[T ]\d{2}:\d{2}:\d{2}[^\s]*)\s+\[?(TRACE|DEBUG|INFO|WARN(?:ING)?|ERROR|ERR|FATAL|CRIT(?:ICAL)?)\]?\s*(.*)",
        )
        .unwrap()
    })
}

fn re_level_prefix() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"^\s*\[?(TRACE|DEBUG|INFO|WARN(?:ING)?|ERROR|ERR|FATAL|CRIT(?:ICAL)?)\]?\s+(.*)",
        )
        .unwrap()
    })
}

fn re_syslog() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"^((?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\s+\d+\s+\d{2}:\d{2}:\d{2})\s+(.*)",
        )
        .unwrap()
    })
}

/// Find the start of a JSON object in a string.
fn find_json_start(s: &str) -> Option<usize> {
    let idx = s.find('{')?;
    // Quick validation: must have at least a closing brace somewhere after
    if s[idx..].contains('}') {
        Some(idx)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Log line parsing
// ---------------------------------------------------------------------------

fn parse_log_line(line: &str) -> LogLine {
    // Try timestamp + level pattern first
    if let Some(caps) = re_timestamp_level().captures(line) {
        let ts = caps.get(1).unwrap().as_str().to_string();
        let level_str = caps.get(2).unwrap().as_str();
        let level = LogLevel::from_str(level_str);
        let msg = caps.get(3).map(|m| m.as_str()).unwrap_or("").to_string();
        let json_start = find_json_start(&msg);
        return LogLine {
            timestamp: Some(ts),
            level,
            level_text: Some(level_str.to_string()),
            message: msg,
            json_start,
        };
    }

    // Try syslog format
    if let Some(caps) = re_syslog().captures(line) {
        let ts = caps.get(1).unwrap().as_str().to_string();
        let rest = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        // Try to extract level from the rest
        if let Some(level_caps) = re_level_prefix().captures(rest) {
            let level_str = level_caps.get(1).unwrap().as_str();
            let level = LogLevel::from_str(level_str);
            let msg = level_caps
                .get(2)
                .map(|m| m.as_str())
                .unwrap_or("")
                .to_string();
            let json_start = find_json_start(&msg);
            return LogLine {
                timestamp: Some(ts),
                level,
                level_text: Some(level_str.to_string()),
                message: msg,
                json_start,
            };
        }
        let json_start = find_json_start(rest);
        return LogLine {
            timestamp: Some(ts),
            level: None,
            level_text: None,
            message: rest.to_string(),
            json_start,
        };
    }

    // Try level prefix only
    if let Some(caps) = re_level_prefix().captures(line) {
        let level_str = caps.get(1).unwrap().as_str();
        let level = LogLevel::from_str(level_str);
        let msg = caps.get(2).map(|m| m.as_str()).unwrap_or("").to_string();
        let json_start = find_json_start(&msg);
        return LogLine {
            timestamp: None,
            level,
            level_text: Some(level_str.to_string()),
            message: msg,
            json_start,
        };
    }

    // Unparseable — treat as plain message
    LogLine {
        timestamp: None,
        level: None,
        level_text: None,
        message: line.to_string(),
        json_start: None,
    }
}

// ---------------------------------------------------------------------------
// Styling helpers
// ---------------------------------------------------------------------------

fn level_fg(level: LogLevel, theme: &ThemeColors) -> [u8; 3] {
    match level {
        LogLevel::Trace => theme.palette[8], // dim grey
        LogLevel::Debug => theme.palette[8], // dim grey
        LogLevel::Info => theme.palette[2],  // green
        LogLevel::Warn => theme.palette[3],  // yellow
        LogLevel::Error => theme.palette[9], // bright red
        LogLevel::Fatal => theme.palette[9], // bright red
    }
}

fn level_bold(level: LogLevel) -> bool {
    matches!(level, LogLevel::Warn | LogLevel::Error | LogLevel::Fatal)
}

fn level_bg(level: LogLevel, theme: &ThemeColors) -> Option<[u8; 3]> {
    if level == LogLevel::Fatal {
        // Subtle dark red background for FATAL
        Some([
            theme.palette[1][0] / 3,
            theme.palette[1][1] / 3,
            theme.palette[1][2] / 3,
        ])
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// LogRenderer
// ---------------------------------------------------------------------------

/// Renders log output with level coloring, timestamp dimming, and error highlighting.
pub struct LogRenderer {
    config: LogRendererConfig,
}

impl LogRenderer {
    /// Create a new log renderer with the given configuration.
    pub fn new(config: LogRendererConfig) -> Self {
        Self { config }
    }

    /// Render a single parsed log line into styled segments.
    fn render_log_line(&self, parsed: &LogLine, theme: &ThemeColors) -> Vec<StyledSegment> {
        let mut segments = Vec::new();
        let level = parsed.level.unwrap_or(LogLevel::Info);

        // Timestamp (dimmed)
        if let Some(ts) = &parsed.timestamp {
            segments.push(StyledSegment {
                text: format!("{ts} "),
                fg: Some(theme.palette[8]),
                ..Default::default()
            });
        }

        // Level badge
        if let Some(level_text) = &parsed.level_text {
            segments.push(StyledSegment {
                text: format!("{level_text} "),
                fg: Some(level_fg(level, theme)),
                bg: level_bg(level, theme),
                bold: level_bold(level),
                ..Default::default()
            });
        }

        // Message (with optional JSON highlighting)
        if self.config.expand_json
            && let Some(json_idx) = parsed.json_start
        {
            // Text before JSON
            if json_idx > 0 {
                let fg = if level.is_error_or_fatal() {
                    Some(level_fg(level, theme))
                } else {
                    None
                };
                segments.push(StyledSegment {
                    text: parsed.message[..json_idx].to_string(),
                    fg,
                    bold: level.is_error_or_fatal(),
                    ..Default::default()
                });
            }
            // JSON payload (cyan)
            segments.push(StyledSegment {
                text: parsed.message[json_idx..].to_string(),
                fg: Some(theme.palette[6]),
                ..Default::default()
            });
        } else {
            let fg = if level.is_error_or_fatal() {
                Some(level_fg(level, theme))
            } else {
                None
            };
            segments.push(StyledSegment {
                text: parsed.message.clone(),
                fg,
                bold: level.is_error_or_fatal(),
                ..Default::default()
            });
        }

        segments
    }

    /// Check if a line looks like a stack trace frame (indented `at ...` etc).
    fn is_stack_frame(line: &str) -> bool {
        let trimmed = line.trim_start();
        trimmed.starts_with("at ")
            || trimmed.starts_with("Caused by:")
            || (line.starts_with(' ') || line.starts_with('\t'))
                && (trimmed.contains(".java:")
                    || trimmed.contains(".py:")
                    || trimmed.contains(".rs:")
                    || trimmed.contains(".js:")
                    || trimmed.contains(".ts:")
                    || trimmed.contains(".go:"))
    }
}

// ---------------------------------------------------------------------------
// ContentRenderer implementation
// ---------------------------------------------------------------------------

impl ContentRenderer for LogRenderer {
    fn format_id(&self) -> &str {
        "log"
    }

    fn display_name(&self) -> &str {
        "Log Output"
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

        let mut i = 0;
        while i < content.lines.len() {
            let line = &content.lines[i];
            let parsed = parse_log_line(line);
            let segments = self.render_log_line(&parsed, theme);
            push_line(&mut lines, &mut line_mapping, segments, Some(i));

            // Check for stack trace frames following an error line
            if parsed.level.is_some_and(|l| l.is_error_or_fatal()) {
                // Collect consecutive stack frames
                let frame_start = i + 1;
                let mut frame_end = frame_start;
                while frame_end < content.lines.len()
                    && Self::is_stack_frame(&content.lines[frame_end])
                {
                    frame_end += 1;
                }

                let frame_count = frame_end - frame_start;
                if frame_count > 0 {
                    let visible = frame_count.min(self.config.max_visible_frames);

                    // Show first N frames
                    for j in frame_start..frame_start + visible {
                        push_line(
                            &mut lines,
                            &mut line_mapping,
                            vec![StyledSegment {
                                text: content.lines[j].clone(),
                                fg: Some(theme.palette[8]),
                                ..Default::default()
                            }],
                            Some(j),
                        );
                    }

                    // Fold remaining frames
                    let hidden = frame_count - visible;
                    if hidden > 0 {
                        push_line(
                            &mut lines,
                            &mut line_mapping,
                            vec![StyledSegment {
                                text: format!("    ... {hidden} more stack frames"),
                                fg: Some(theme.palette[8]),
                                italic: true,
                                ..Default::default()
                            }],
                            None,
                        );
                    }

                    i = frame_end;
                    continue;
                }
            }

            i += 1;
        }

        Ok(RenderedContent {
            lines,
            line_mapping,
            graphics: vec![],
            format_badge: "LOG".to_string(),
        })
    }

    fn format_badge(&self) -> &str {
        "LOG"
    }
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register the log renderer with the registry.
pub fn register_log_renderer(registry: &mut RendererRegistry, config: &LogRendererConfig) {
    registry.register_renderer("log", Box::new(LogRenderer::new(config.clone())));
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prettifier::traits::RendererConfig;
    use crate::prettifier::types::{ContentBlock, StyledLine};
    use std::time::SystemTime;

    fn test_config() -> RendererConfig {
        RendererConfig {
            terminal_width: 80,
            theme_colors: ThemeColors::default(),
        }
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
}
