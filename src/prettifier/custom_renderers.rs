//! Custom renderer registration from user config.
//!
//! Supports:
//! - External command renderers that pipe content to a shell command
//! - Custom regex-only detectors created from config patterns
//! - Custom fenced block diagram languages

use std::io::Write;
use std::process::{Command, Stdio};

use crate::config::prettifier::CustomRendererConfig;

use super::regex_detector::RegexDetectorBuilder;
use super::registry::RendererRegistry;
use super::traits::{ContentRenderer, RenderError, RendererConfig};
use super::types::{
    ContentBlock, DetectionRule, RenderedContent, RendererCapability, RuleScope, RuleSource,
    RuleStrength, SourceLineMapping, StyledLine, StyledSegment,
};

// ---------------------------------------------------------------------------
// ExternalCommandRenderer
// ---------------------------------------------------------------------------

/// A user-defined renderer that delegates to an external command.
///
/// Content is piped to the command's stdin and the output is captured as styled text.
pub struct ExternalCommandRenderer {
    format_id: String,
    display_name: String,
    render_command: String,
    render_args: Vec<String>,
}

impl ExternalCommandRenderer {
    /// Create a new external command renderer.
    pub fn new(
        format_id: String,
        display_name: String,
        render_command: String,
        render_args: Vec<String>,
    ) -> Self {
        Self {
            format_id,
            display_name,
            render_command,
            render_args,
        }
    }
}

impl ContentRenderer for ExternalCommandRenderer {
    fn format_id(&self) -> &str {
        &self.format_id
    }

    fn display_name(&self) -> &str {
        &self.display_name
    }

    fn capabilities(&self) -> Vec<RendererCapability> {
        vec![RendererCapability::ExternalCommand]
    }

    fn render(
        &self,
        content: &ContentBlock,
        _config: &RendererConfig,
    ) -> Result<RenderedContent, RenderError> {
        let input = content.full_text();

        let mut child = Command::new(&self.render_command)
            .args(&self.render_args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| RenderError::CommandNotFound(format!("{}: {e}", self.render_command)))?;

        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(input.as_bytes());
        }

        let output = child
            .wait_with_output()
            .map_err(|e| RenderError::RenderFailed(format!("command execution failed: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(RenderError::RenderFailed(format!(
                "{} exited with {}: {}",
                self.render_command,
                output.status,
                stderr.trim()
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<StyledLine> = stdout.lines().map(parse_ansi_line).collect();

        let line_mapping: Vec<SourceLineMapping> = lines
            .iter()
            .enumerate()
            .map(|(i, _)| SourceLineMapping {
                rendered_line: i,
                source_line: if i < content.line_count() {
                    Some(i)
                } else {
                    None
                },
            })
            .collect();

        let badge = self
            .format_id
            .chars()
            .take(3)
            .collect::<String>()
            .to_uppercase();

        Ok(RenderedContent {
            lines,
            line_mapping,
            graphics: vec![],
            format_badge: badge,
        })
    }

    fn format_badge(&self) -> &str {
        // Return a static badge; the dynamic one is in render() output.
        "EXT"
    }
}

/// Current SGR (Select Graphic Rendition) attribute state during ANSI parsing.
#[derive(Clone, Default)]
struct SgrState {
    fg: Option<[u8; 3]>,
    bg: Option<[u8; 3]>,
    bold: bool,
    italic: bool,
    underline: bool,
    strikethrough: bool,
}

/// Standard ANSI 8-color palette (indices 0–7).
const ANSI_COLORS: [[u8; 3]; 8] = [
    [0, 0, 0],       // 0 Black
    [170, 0, 0],     // 1 Red
    [0, 170, 0],     // 2 Green
    [170, 85, 0],    // 3 Yellow
    [0, 0, 170],     // 4 Blue
    [170, 0, 170],   // 5 Magenta
    [0, 170, 170],   // 6 Cyan
    [192, 192, 192], // 7 White
];

/// Bright ANSI color palette (indices 8–15).
const ANSI_BRIGHT: [[u8; 3]; 8] = [
    [85, 85, 85],     // 8  Bright black
    [255, 85, 85],    // 9  Bright red
    [85, 255, 85],    // 10 Bright green
    [255, 255, 85],   // 11 Bright yellow
    [85, 85, 255],    // 12 Bright blue
    [255, 85, 255],   // 13 Bright magenta
    [85, 255, 255],   // 14 Bright cyan
    [255, 255, 255],  // 15 Bright white
];

/// Convert a 256-color index to RGB.
fn color_256_to_rgb(idx: u8) -> [u8; 3] {
    match idx {
        0..=7 => ANSI_COLORS[idx as usize],
        8..=15 => ANSI_BRIGHT[(idx - 8) as usize],
        16..=231 => {
            // 6x6x6 color cube
            let idx = idx - 16;
            let r = (idx / 36) % 6;
            let g = (idx / 6) % 6;
            let b = idx % 6;
            let to_val = |v: u8| if v == 0 { 0 } else { 55 + 40 * v };
            [to_val(r), to_val(g), to_val(b)]
        }
        232..=255 => {
            // Grayscale ramp
            let v = 8 + 10 * (idx - 232);
            [v, v, v]
        }
    }
}

/// Apply a sequence of SGR parameters to the current state.
fn apply_sgr(state: &mut SgrState, params: &[u16]) {
    let mut i = 0;
    while i < params.len() {
        match params[i] {
            0 => *state = SgrState::default(),
            1 => state.bold = true,
            3 => state.italic = true,
            4 => state.underline = true,
            9 => state.strikethrough = true,
            22 => state.bold = false,
            23 => state.italic = false,
            24 => state.underline = false,
            29 => state.strikethrough = false,
            // Standard foreground colors
            30..=37 => state.fg = Some(ANSI_COLORS[(params[i] - 30) as usize]),
            38 => {
                // Extended foreground: 38;5;N or 38;2;R;G;B
                if i + 1 < params.len() {
                    match params[i + 1] {
                        5 if i + 2 < params.len() => {
                            state.fg = Some(color_256_to_rgb(params[i + 2] as u8));
                            i += 2;
                        }
                        2 if i + 4 < params.len() => {
                            state.fg = Some([
                                params[i + 2] as u8,
                                params[i + 3] as u8,
                                params[i + 4] as u8,
                            ]);
                            i += 4;
                        }
                        _ => {}
                    }
                }
            }
            39 => state.fg = None,
            // Standard background colors
            40..=47 => state.bg = Some(ANSI_COLORS[(params[i] - 40) as usize]),
            48 => {
                // Extended background: 48;5;N or 48;2;R;G;B
                if i + 1 < params.len() {
                    match params[i + 1] {
                        5 if i + 2 < params.len() => {
                            state.bg = Some(color_256_to_rgb(params[i + 2] as u8));
                            i += 2;
                        }
                        2 if i + 4 < params.len() => {
                            state.bg = Some([
                                params[i + 2] as u8,
                                params[i + 3] as u8,
                                params[i + 4] as u8,
                            ]);
                            i += 4;
                        }
                        _ => {}
                    }
                }
            }
            49 => state.bg = None,
            // Bright foreground colors
            90..=97 => state.fg = Some(ANSI_BRIGHT[(params[i] - 90) as usize]),
            // Bright background colors
            100..=107 => state.bg = Some(ANSI_BRIGHT[(params[i] - 100) as usize]),
            _ => {} // Ignore unrecognized codes
        }
        i += 1;
    }
}

/// Parse a line that may contain ANSI escape codes into a `StyledLine`.
///
/// Handles CSI SGR sequences (`ESC[...m`) including:
/// - Reset (0), bold (1), italic (3), underline (4), strikethrough (9)
/// - Standard colors (30–37 fg, 40–47 bg)
/// - Bright colors (90–97 fg, 100–107 bg)
/// - 256-color mode (38;5;N / 48;5;N)
/// - RGB true-color (38;2;R;G;B / 48;2;R;G;B)
///
/// Non-SGR escape sequences (cursor movement, etc.) are silently skipped.
fn parse_ansi_line(line: &str) -> StyledLine {
    let mut segments: Vec<StyledSegment> = Vec::new();
    let mut state = SgrState::default();
    let mut text_buf = String::new();
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['

                // Collect the parameter string until a letter terminates the sequence.
                let mut param_str = String::new();
                let mut terminator = ' ';
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if next.is_ascii_alphabetic() {
                        terminator = next;
                        break;
                    }
                    param_str.push(next);
                }

                if terminator == 'm' {
                    // SGR sequence — flush current text and apply new attributes.
                    if !text_buf.is_empty() {
                        segments.push(StyledSegment {
                            text: std::mem::take(&mut text_buf),
                            fg: state.fg,
                            bg: state.bg,
                            bold: state.bold,
                            italic: state.italic,
                            underline: state.underline,
                            strikethrough: state.strikethrough,
                            link_url: None,
                        });
                    }

                    let params: Vec<u16> = if param_str.is_empty() {
                        vec![0] // bare ESC[m means reset
                    } else {
                        param_str
                            .split(';')
                            .filter_map(|p| p.parse().ok())
                            .collect()
                    };
                    apply_sgr(&mut state, &params);
                }
                // Non-SGR sequences (cursor movement, etc.) are silently dropped.
            }
            // Bare ESC without '[' — skip it.
        } else {
            text_buf.push(c);
        }
    }

    // Flush remaining text.
    if !text_buf.is_empty() {
        segments.push(StyledSegment {
            text: text_buf,
            fg: state.fg,
            bg: state.bg,
            bold: state.bold,
            italic: state.italic,
            underline: state.underline,
            strikethrough: state.strikethrough,
            link_url: None,
        });
    }

    if segments.is_empty() {
        StyledLine::plain("")
    } else {
        StyledLine::new(segments)
    }
}

// ---------------------------------------------------------------------------
// Custom detector creation
// ---------------------------------------------------------------------------

/// Create a `RegexDetector` from a `CustomRendererConfig`'s detection patterns.
fn create_custom_detector(config: &CustomRendererConfig) -> super::regex_detector::RegexDetector {
    let mut builder = RegexDetectorBuilder::new(&config.id, &config.name)
        .confidence_threshold(0.6)
        .min_matching_rules(1)
        .definitive_rule_shortcircuit(true);

    for (i, pattern_str) in config.detect_patterns.iter().enumerate() {
        if let Ok(pattern) = regex::Regex::new(pattern_str) {
            builder = builder.rule(DetectionRule {
                id: format!("{}_rule_{}", config.id, i),
                pattern,
                weight: 0.8,
                scope: RuleScope::AnyLine,
                strength: if i == 0 {
                    RuleStrength::Strong
                } else {
                    RuleStrength::Supporting
                },
                source: RuleSource::UserDefined,
                command_context: None,
                description: format!("Custom pattern for {}", config.name),
                enabled: true,
            });
        }
    }

    builder.build()
}

// ---------------------------------------------------------------------------
// Registration entry point
// ---------------------------------------------------------------------------

/// Load and register custom renderers from config.
///
/// For each `CustomRendererConfig`:
/// 1. Creates a regex detector from the config's detection patterns.
/// 2. Creates an `ExternalCommandRenderer` if a render command is specified.
/// 3. Registers both with the registry at the configured priority.
pub fn register_custom_renderers(
    registry: &mut RendererRegistry,
    custom_configs: &[CustomRendererConfig],
) {
    for config in custom_configs {
        // Register the detector if there are detection patterns.
        if !config.detect_patterns.is_empty() {
            let detector = create_custom_detector(config);
            registry.register_detector(config.priority, Box::new(detector));
        }

        // Register the renderer if a render command is specified.
        if let Some(ref command) = config.render_command {
            let renderer = ExternalCommandRenderer::new(
                config.id.clone(),
                config.name.clone(),
                command.clone(),
                config.render_args.clone(),
            );
            registry.register_renderer(&config.id, Box::new(renderer));
        }
    }
}

// ---------------------------------------------------------------------------
// Custom diagram languages
// ---------------------------------------------------------------------------

/// Register custom diagram languages from config.
///
/// Adds user-defined fenced block language tags to the diagram renderer's
/// language registry.
pub fn register_custom_diagram_languages(
    renderer: &mut super::renderers::diagrams::DiagramRenderer,
    languages: &[CustomDiagramLanguageConfig],
) {
    for lang in languages {
        renderer.add_language(super::renderers::diagrams::DiagramLanguage {
            tag: lang.tag.clone(),
            display_name: lang.display_name.clone(),
            kroki_type: lang.kroki_type.clone(),
            local_command: lang.local_command.clone(),
            local_args: lang.local_args.clone().unwrap_or_default(),
        });
    }
}

/// Configuration for a custom diagram language (from YAML config).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct CustomDiagramLanguageConfig {
    /// The fenced code block tag (e.g., "tikz").
    pub tag: String,
    /// Display name (e.g., "TikZ").
    pub display_name: String,
    /// Kroki API type identifier (if supported).
    #[serde(default)]
    pub kroki_type: Option<String>,
    /// Local CLI command.
    #[serde(default)]
    pub local_command: Option<String>,
    /// Arguments for local CLI command.
    #[serde(default)]
    pub local_args: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prettifier::traits::ContentDetector;

    #[test]
    fn test_parse_ansi_plain_text() {
        let line = parse_ansi_line("hello");
        assert_eq!(line.segments.len(), 1);
        assert_eq!(line.segments[0].text, "hello");
        assert!(line.segments[0].fg.is_none());
    }

    #[test]
    fn test_parse_ansi_fg_color() {
        let line = parse_ansi_line("\x1b[31mred\x1b[0m normal");
        assert_eq!(line.segments.len(), 2);
        assert_eq!(line.segments[0].text, "red");
        assert_eq!(line.segments[0].fg, Some([170, 0, 0])); // ANSI red
        assert_eq!(line.segments[1].text, " normal");
        assert!(line.segments[1].fg.is_none());
    }

    #[test]
    fn test_parse_ansi_bold_and_color() {
        let line = parse_ansi_line("\x1b[1;32mbold green\x1b[0m");
        assert_eq!(line.segments.len(), 1);
        assert_eq!(line.segments[0].text, "bold green");
        assert!(line.segments[0].bold);
        assert_eq!(line.segments[0].fg, Some([0, 170, 0])); // ANSI green
    }

    #[test]
    fn test_parse_ansi_bright_colors() {
        let line = parse_ansi_line("\x1b[91mbright red\x1b[0m");
        assert_eq!(line.segments[0].fg, Some([255, 85, 85]));
    }

    #[test]
    fn test_parse_ansi_256_color() {
        let line = parse_ansi_line("\x1b[38;5;196mcolor\x1b[0m");
        assert_eq!(line.segments[0].text, "color");
        assert!(line.segments[0].fg.is_some());
    }

    #[test]
    fn test_parse_ansi_rgb_color() {
        let line = parse_ansi_line("\x1b[38;2;100;200;50mrgb\x1b[0m");
        assert_eq!(line.segments[0].text, "rgb");
        assert_eq!(line.segments[0].fg, Some([100, 200, 50]));
    }

    #[test]
    fn test_parse_ansi_bg_color() {
        let line = parse_ansi_line("\x1b[44mblue bg\x1b[0m");
        assert_eq!(line.segments[0].bg, Some([0, 0, 170])); // ANSI blue
    }

    #[test]
    fn test_parse_ansi_italic_underline_strikethrough() {
        let line = parse_ansi_line("\x1b[3mitalic\x1b[0m \x1b[4munderline\x1b[0m \x1b[9mstrike\x1b[0m");
        // Resets between styled words produce separate segments for the spaces
        let italic_seg = line.segments.iter().find(|s| s.text == "italic").unwrap();
        let underline_seg = line.segments.iter().find(|s| s.text == "underline").unwrap();
        let strike_seg = line.segments.iter().find(|s| s.text == "strike").unwrap();
        assert!(italic_seg.italic);
        assert!(underline_seg.underline);
        assert!(strike_seg.strikethrough);
    }

    #[test]
    fn test_parse_ansi_reset_bare() {
        // ESC[m (no params) means reset
        let line = parse_ansi_line("\x1b[31mred\x1b[mnormal");
        assert_eq!(line.segments.len(), 2);
        assert!(line.segments[0].fg.is_some());
        assert!(line.segments[1].fg.is_none());
    }

    #[test]
    fn test_parse_ansi_empty_line() {
        let line = parse_ansi_line("");
        assert_eq!(line.segments.len(), 1);
        assert_eq!(line.segments[0].text, "");
    }

    #[test]
    fn test_color_256_to_rgb_grayscale() {
        let c = color_256_to_rgb(232); // first grayscale
        assert_eq!(c, [8, 8, 8]);
        let c = color_256_to_rgb(255); // last grayscale
        assert_eq!(c, [238, 238, 238]);
    }

    #[test]
    fn test_create_custom_detector_empty_patterns() {
        let config = CustomRendererConfig {
            id: "test".to_string(),
            name: "Test".to_string(),
            detect_patterns: vec![],
            render_command: None,
            render_args: vec![],
            priority: 50,
        };
        let detector = create_custom_detector(&config);
        assert_eq!(detector.format_id(), "test");
        assert_eq!(detector.display_name(), "Test");
        assert!(detector.detection_rules().is_empty());
    }

    #[test]
    fn test_create_custom_detector_with_patterns() {
        let config = CustomRendererConfig {
            id: "proto".to_string(),
            name: "Protobuf".to_string(),
            detect_patterns: vec![r"^message\s+\w+".to_string(), r"^syntax\s*=".to_string()],
            render_command: None,
            render_args: vec![],
            priority: 30,
        };
        let detector = create_custom_detector(&config);
        assert_eq!(detector.detection_rules().len(), 2);
        assert_eq!(detector.detection_rules()[0].id, "proto_rule_0");
        assert_eq!(detector.detection_rules()[1].id, "proto_rule_1");
    }

    #[test]
    fn test_create_custom_detector_invalid_pattern_skipped() {
        let config = CustomRendererConfig {
            id: "bad".to_string(),
            name: "Bad".to_string(),
            detect_patterns: vec![r"[invalid".to_string(), r"^valid$".to_string()],
            render_command: None,
            render_args: vec![],
            priority: 50,
        };
        let detector = create_custom_detector(&config);
        // Invalid pattern skipped, only the valid one remains.
        assert_eq!(detector.detection_rules().len(), 1);
    }

    #[test]
    fn test_register_custom_renderers() {
        let mut registry = RendererRegistry::new(0.5);
        let configs = vec![CustomRendererConfig {
            id: "custom_test".to_string(),
            name: "Custom Test".to_string(),
            detect_patterns: vec![r"^CUSTOM:".to_string()],
            render_command: None, // No external command
            render_args: vec![],
            priority: 40,
        }];

        register_custom_renderers(&mut registry, &configs);

        // Detector should be registered (1 detector).
        assert_eq!(registry.detector_count(), 1);
        // No renderer since render_command is None.
        assert_eq!(registry.renderer_count(), 0);
    }

    #[test]
    fn test_external_command_renderer_traits() {
        let renderer = ExternalCommandRenderer::new(
            "test".to_string(),
            "Test Renderer".to_string(),
            "echo".to_string(),
            vec!["hello".to_string()],
        );
        assert_eq!(renderer.format_id(), "test");
        assert_eq!(renderer.display_name(), "Test Renderer");
        assert_eq!(renderer.format_badge(), "EXT");
        assert_eq!(
            renderer.capabilities(),
            vec![RendererCapability::ExternalCommand]
        );
    }
}
