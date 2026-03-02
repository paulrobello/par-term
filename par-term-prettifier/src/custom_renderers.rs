//! Custom renderer registration from user config.
//!
//! Supports:
//! - External command renderers that pipe content to a shell command
//! - Custom regex-only detectors created from config patterns
//! - Custom fenced block diagram languages

use std::io::{Read as _, Write};
use std::process::{Command, Stdio};

use super::ansi_parser::parse_ansi_line;
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
///
/// # Security Warning
///
/// This renderer executes **arbitrary commands** specified in the user's configuration
/// file (`render_command` and `render_args`). There is intentionally no validation or
/// allowlisting of the command being executed — users have full control over what runs.
///
/// **Risk**: A malicious configuration file shared with a user (e.g., via a dotfile
/// repository, a project-level config, or social engineering) could include a custom
/// renderer that executes destructive or exfiltrating commands whenever matching
/// terminal output is detected.
///
/// **Trust assumption**: This renderer inherits the full trust of the user's config
/// file. Only load configuration from sources you trust. Do not import or share config
/// files from untrusted parties without auditing all `custom_renderers` entries.
///
/// A 10-second execution timeout and 1 MiB output cap are applied as resource guards,
/// but these do not prevent malicious commands from causing harm within those limits.
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
        config: &RendererConfig,
    ) -> Result<RenderedContent, RenderError> {
        let input = content.full_text();

        // SECURITY: Before executing a user-configured external command, check whether
        // the config has populated an allowlist. If `allowed_commands` is non-empty,
        // the command basename must appear in it; otherwise execution is refused and
        // a warning is logged. When the allowlist is empty (default), the command is
        // allowed but a warning is always emitted so users are aware of the risk.
        //
        // The command and arguments still come directly from the user's config file with
        // no further sanitisation — see the struct-level doc comment for the full risk
        // assessment. Only run configurations from sources you trust.
        let cmd_basename = std::path::Path::new(&self.render_command)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&self.render_command);

        if !config.allowed_commands.is_empty() {
            if !config
                .allowed_commands
                .iter()
                .any(|allowed| allowed == cmd_basename || allowed == &self.render_command)
            {
                // The allowlist is configured and this command is not on it — refuse.
                crate::debug_error!(
                    "PRETTIFIER",
                    "ExternalCommandRenderer: command '{}' (basename '{}') is not in the \
                     allowed_commands list. Skipping execution. Add it to \
                     prettifier.allowed_commands in your config to permit it.",
                    self.render_command,
                    cmd_basename,
                );
                log::warn!(
                    "par-term prettifier: external command '{}' is not in the \
                     allowed_commands allowlist and will not be executed. \
                     Add it to prettifier.allowed_commands in your config to permit it.",
                    self.render_command,
                );
                return Err(RenderError::RenderFailed(format!(
                    "command '{}' is not in the prettifier allowed_commands list",
                    self.render_command,
                )));
            }
        } else {
            // No allowlist configured — allow execution but warn loudly.
            crate::debug_error!(
                "PRETTIFIER",
                "SECURITY WARNING: ExternalCommandRenderer is executing '{}' (format: {}) \
                 with no command allowlist configured. Set prettifier.allowed_commands in \
                 your config to restrict which commands can be run. Only load configs from \
                 trusted sources.",
                self.render_command,
                self.format_id,
            );
            log::warn!(
                "par-term prettifier: executing external command '{}' with no allowlist \
                 configured. Consider setting prettifier.allowed_commands to restrict \
                 which commands can be run by custom renderers.",
                self.render_command,
            );
        }

        crate::debug_info!(
            "PRETTIFIER",
            "ExternalCommandRenderer: invoking user-configured command '{}' (format: {}). \
             Only run configs from trusted sources.",
            self.render_command,
            self.format_id
        );

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

        // Poll with timeout (10s) and output cap (1 MiB) to prevent hangs.
        const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
        const MAX_OUTPUT: usize = 1024 * 1024; // 1 MiB
        let deadline = std::time::Instant::now() + TIMEOUT;

        loop {
            match child.try_wait() {
                Ok(Some(_status)) => break,
                Ok(None) => {
                    if std::time::Instant::now() >= deadline {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(RenderError::RenderFailed(format!(
                            "{} timed out after {}s",
                            self.render_command,
                            TIMEOUT.as_secs()
                        )));
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(e) => {
                    return Err(RenderError::RenderFailed(format!(
                        "command execution failed: {e}"
                    )));
                }
            }
        }

        let exit_status = child
            .wait()
            .map_err(|e| RenderError::RenderFailed(format!("command execution failed: {e}")))?;

        if !exit_status.success() {
            let mut stderr_buf = String::new();
            if let Some(mut stderr) = child.stderr.take() {
                let _ = stderr.read_to_string(&mut stderr_buf);
            }
            return Err(RenderError::RenderFailed(format!(
                "{} exited with {}: {}",
                self.render_command,
                exit_status,
                stderr_buf.trim()
            )));
        }

        let mut stdout_bytes = Vec::new();
        if let Some(mut stdout) = child.stdout.take() {
            stdout_bytes.resize(MAX_OUTPUT, 0);
            let n = stdout.read(&mut stdout_bytes).unwrap_or(0);
            stdout_bytes.truncate(n);
        }
        let stdout = String::from_utf8_lossy(&stdout_bytes);
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
    use crate::traits::ContentDetector;

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
