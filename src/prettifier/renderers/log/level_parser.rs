//! Log level parsing, line parsing, and renderer implementation.

use std::sync::OnceLock;

use regex::Regex;

use crate::prettifier::registry::RendererRegistry;
use crate::prettifier::renderers::push_line;
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
    pub(super) fn from_str(s: &str) -> Option<Self> {
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
        .expect("re_timestamp_level: pattern is valid and should always compile")
    })
}

fn re_level_prefix() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"^\s*\[?(TRACE|DEBUG|INFO|WARN(?:ING)?|ERROR|ERR|FATAL|CRIT(?:ICAL)?)\]?\s+(.*)",
        )
        .expect("re_level_prefix: pattern is valid and should always compile")
    })
}

fn re_syslog() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"^((?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\s+\d+\s+\d{2}:\d{2}:\d{2})\s+(.*)",
        )
        .expect("re_syslog: pattern is valid and should always compile")
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
        // Capture groups are guaranteed by the regex pattern after a successful match.
        let ts = caps.get(1).map_or("", |m| m.as_str()).to_string();
        let level_str = caps.get(2).map_or("", |m| m.as_str());
        let level = LogLevel::from_str(level_str);
        let msg = caps.get(3).map_or("", |m| m.as_str()).to_string();
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
        // Capture groups are guaranteed by the regex pattern after a successful match.
        let ts = caps.get(1).map_or("", |m| m.as_str()).to_string();
        let rest = caps.get(2).map_or("", |m| m.as_str());
        // Try to extract level from the rest
        if let Some(level_caps) = re_level_prefix().captures(rest) {
            let level_str = level_caps.get(1).map_or("", |m| m.as_str());
            let level = LogLevel::from_str(level_str);
            let msg = level_caps.get(2).map_or("", |m| m.as_str()).to_string();
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
        // Capture groups are guaranteed by the regex pattern after a successful match.
        let level_str = caps.get(1).map_or("", |m| m.as_str());
        let level = LogLevel::from_str(level_str);
        let msg = caps.get(2).map_or("", |m| m.as_str()).to_string();
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
