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
    RuleStrength, SourceLineMapping, StyledLine,
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

/// Parse a line that may contain ANSI escape codes into a StyledLine.
///
/// This is a simplified parser that handles basic SGR sequences (colors, bold, etc.).
fn parse_ansi_line(line: &str) -> StyledLine {
    // Simple approach: strip ANSI codes and return as plain text.
    // Full ANSI parsing would be complex; this is sufficient for most external tools.
    let stripped = strip_ansi_codes(line);
    StyledLine::plain(&stripped)
}

/// Strip ANSI escape sequences from a string.
fn strip_ansi_codes(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip the escape sequence.
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['
                // Read until a letter (the terminator).
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
        } else {
            result.push(c);
        }
    }

    result
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
                Vec::new(), // Args could be added to CustomRendererConfig if needed.
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
    fn test_strip_ansi_codes() {
        assert_eq!(strip_ansi_codes("hello"), "hello");
        assert_eq!(strip_ansi_codes("\x1b[31mred\x1b[0m"), "red");
        assert_eq!(
            strip_ansi_codes("\x1b[1;32mbold green\x1b[0m"),
            "bold green"
        );
        assert_eq!(strip_ansi_codes("no escape"), "no escape");
    }

    #[test]
    fn test_parse_ansi_line() {
        let line = parse_ansi_line("\x1b[31mhello\x1b[0m world");
        assert_eq!(line.segments.len(), 1);
        assert_eq!(line.segments[0].text, "hello world");
    }

    #[test]
    fn test_create_custom_detector_empty_patterns() {
        let config = CustomRendererConfig {
            id: "test".to_string(),
            name: "Test".to_string(),
            detect_patterns: vec![],
            render_command: None,
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
