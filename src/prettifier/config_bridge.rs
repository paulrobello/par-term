//! Bridges between YAML configuration types and runtime prettifier types.
//!
//! Converts `ResolvedPrettifierConfig` into the `PrettifierConfig` consumed by
//! `PrettifierPipeline`, and loads/merges detection rules from config.

use std::collections::HashMap;

use crate::config::Config;
use crate::config::prettifier::resolve_prettifier_config;
use crate::config::prettifier::{
    FormatDetectionRulesConfig, ResolvedPrettifierConfig, RuleOverride, UserDetectionRule,
};

use super::boundary::DetectionScope;
use super::pipeline::{PrettifierConfig, PrettifierPipeline};
use super::registry::RendererRegistry;
use super::traits::RendererConfig;
use super::types::{DetectionRule, RuleScope, RuleSource, RuleStrength};

/// Convert a `ResolvedPrettifierConfig` into the runtime `PrettifierConfig`
/// consumed by `PrettifierPipeline`.
pub fn to_pipeline_config(resolved: &ResolvedPrettifierConfig) -> PrettifierConfig {
    PrettifierConfig {
        enabled: resolved.enabled,
        respect_alternate_screen: resolved.respect_alternate_screen,
        confidence_threshold: resolved.detection.confidence_threshold,
        max_scan_lines: resolved.detection.max_scan_lines,
        debounce_ms: resolved.detection.debounce_ms,
        detection_scope: parse_detection_scope(&resolved.detection.scope),
    }
}

/// Build a [`RendererRegistry`] populated with all built-in detectors and renderers,
/// configured from the resolved prettifier settings.
pub fn build_default_registry(resolved: &ResolvedPrettifierConfig) -> RendererRegistry {
    use super::detectors;
    use super::renderers;

    let mut registry = RendererRegistry::new(resolved.detection.confidence_threshold);

    // Register built-in detectors (each checks its own enabled flag)
    detectors::markdown::register_markdown(&mut registry, &resolved.renderers);
    detectors::json::register_json(&mut registry, &resolved.renderers);
    detectors::yaml::register_yaml(&mut registry, &resolved.renderers);
    detectors::toml::register_toml(&mut registry, &resolved.renderers);
    detectors::xml::register_xml(&mut registry, &resolved.renderers);
    detectors::csv::register_csv(&mut registry, &resolved.renderers);
    detectors::diff::register_diff(&mut registry, &resolved.renderers);
    detectors::log::register_log(&mut registry, &resolved.renderers);
    detectors::diagrams::register_diagrams(&mut registry, &resolved.renderers.diagrams);
    detectors::stack_trace::register_stack_trace(&mut registry, &resolved.renderers);
    detectors::sql_results::register_sql_results(&mut registry, &resolved.renderers);

    // Register built-in renderers
    renderers::markdown::register_markdown_renderer_with_diagrams(
        &mut registry,
        &Default::default(),
        &resolved.renderers.diagrams,
    );
    renderers::json::register_json_renderer(&mut registry, &Default::default());
    renderers::yaml::register_yaml_renderer(&mut registry, &Default::default());
    renderers::toml::register_toml_renderer(&mut registry, &Default::default());
    renderers::xml::register_xml_renderer(&mut registry, &Default::default());
    renderers::csv::register_csv_renderer(&mut registry, &Default::default());
    renderers::diff::register_diff_renderer(&mut registry, &Default::default());
    renderers::log::register_log_renderer(&mut registry, &Default::default());
    renderers::diagrams::register_diagram_renderer(&mut registry, &resolved.renderers.diagrams);
    renderers::stack_trace::register_stack_trace_renderer(&mut registry, &Default::default());
    renderers::sql_results::register_sql_results_renderer(&mut registry, &Default::default());

    // Register user-defined custom renderers.
    super::custom_renderers::register_custom_renderers(&mut registry, &resolved.custom_renderers);

    // Apply user detection rule overrides.
    apply_detection_rules(&mut registry, &resolved.detection_rules);

    registry
}

/// Create a [`PrettifierPipeline`] from the application [`Config`], or `None` if
/// the prettifier is disabled.
///
/// Optional `cell_dims` provides `(cell_width_px, cell_height_px)` for sizing
/// inline graphics (diagrams). When `None`, graphics sizing falls back to
/// estimated values.
pub fn create_pipeline_from_config(
    config: &Config,
    terminal_width: usize,
    cell_dims: Option<(f32, f32)>,
) -> Option<PrettifierPipeline> {
    if !config.enable_prettifier {
        return None;
    }
    let resolved = resolve_prettifier_config(
        config.enable_prettifier,
        &config.content_prettifier,
        None,
        None,
    );
    let pipeline_config = to_pipeline_config(&resolved);
    let registry = build_default_registry(&resolved);
    let renderer_config = RendererConfig {
        terminal_width,
        cell_width_px: cell_dims.map(|(w, _)| w),
        cell_height_px: cell_dims.map(|(_, h)| h),
        allowed_commands: resolved.allowed_commands.clone(),
        ..Default::default()
    };
    Some(PrettifierPipeline::new(
        pipeline_config,
        registry,
        renderer_config,
    ))
}

/// Result of running detection on sample content.
#[derive(Debug, Clone)]
pub struct DetectionTestResult {
    /// Detected format ID (e.g., "markdown", "json"), or empty if no match.
    pub format_id: String,
    /// Confidence score (0.0–1.0).
    pub confidence: f32,
    /// IDs of rules that matched.
    pub matched_rules: Vec<String>,
    /// The confidence threshold from config.
    pub threshold: f32,
}

/// Test detection against sample content using the current config.
///
/// Builds a temporary registry from the provided config, constructs a
/// `ContentBlock` from the sample text, and runs `registry.detect()`.
pub fn test_detection(
    config: &Config,
    sample_text: &str,
    preceding_command: Option<&str>,
) -> DetectionTestResult {
    let resolved = resolve_prettifier_config(
        config.enable_prettifier,
        &config.content_prettifier,
        None,
        None,
    );
    let registry = build_default_registry(&resolved);
    let threshold = resolved.detection.confidence_threshold;

    let lines: Vec<String> = sample_text.lines().map(|l| l.to_string()).collect();
    let line_count = lines.len();
    let content = super::types::ContentBlock {
        lines,
        preceding_command: preceding_command.map(|s| s.to_string()),
        start_row: 0,
        end_row: line_count,
        timestamp: std::time::SystemTime::now(),
    };

    match registry.detect(&content) {
        Some(result) => DetectionTestResult {
            format_id: result.format_id.clone(),
            confidence: result.confidence,
            matched_rules: result.matched_rules.clone(),
            threshold,
        },
        None => DetectionTestResult {
            format_id: String::new(),
            confidence: 0.0,
            matched_rules: Vec::new(),
            threshold,
        },
    }
}

/// Apply user detection rule overrides and additional rules to the registry.
fn apply_detection_rules(
    registry: &mut RendererRegistry,
    config_rules: &HashMap<String, FormatDetectionRulesConfig>,
) {
    for (format_id, format_config) in config_rules {
        let additional: Vec<_> = format_config
            .additional
            .iter()
            .filter_map(parse_user_rule)
            .collect();
        registry.apply_rules_for_format(format_id, &format_config.overrides, additional);
    }
}

/// Parse a scope string from config into the runtime enum.
fn parse_detection_scope(scope: &str) -> DetectionScope {
    match scope {
        "all" => DetectionScope::All,
        "command_output" => DetectionScope::CommandOutput,
        "manual_only" => DetectionScope::ManualOnly,
        _ => DetectionScope::All, // default matches declared default
    }
}

/// Load detection rules from config, merging built-in rules with user-defined ones.
///
/// For each format:
/// 1. Start with the provided `built_in_rules`.
/// 2. Apply any overrides from `config_rules` (enable/disable, weight changes).
/// 3. Append any additional user-defined rules from `config_rules`.
///
/// Returns a map from format_id to the merged rule set.
pub fn load_detection_rules(
    built_in_rules: HashMap<String, Vec<DetectionRule>>,
    config_rules: &HashMap<String, FormatDetectionRulesConfig>,
) -> HashMap<String, Vec<DetectionRule>> {
    let mut result = built_in_rules;

    for (format_id, format_rules) in config_rules {
        let rules = result.entry(format_id.clone()).or_default();

        // Apply overrides to existing built-in rules.
        for override_rule in &format_rules.overrides {
            apply_rule_override(rules, override_rule);
        }

        // Append additional user-defined rules.
        for user_rule in &format_rules.additional {
            if let Some(rule) = parse_user_rule(user_rule) {
                rules.push(rule);
            }
        }
    }

    result
}

/// Apply a rule override (enable/disable, weight change) to matching rules.
fn apply_rule_override(rules: &mut [DetectionRule], override_rule: &RuleOverride) {
    for rule in rules.iter_mut() {
        if rule.id == override_rule.id {
            if let Some(enabled) = override_rule.enabled {
                rule.enabled = enabled;
            }
            if let Some(weight) = override_rule.weight {
                rule.weight = weight;
            }
        }
    }
}

/// Parse a user-defined detection rule from config into a runtime `DetectionRule`.
///
/// Returns `None` if the regex pattern fails to compile.
fn parse_user_rule(user_rule: &UserDetectionRule) -> Option<DetectionRule> {
    let pattern = regex::Regex::new(&user_rule.pattern).ok()?;
    let scope = parse_rule_scope(&user_rule.scope);

    Some(DetectionRule {
        id: user_rule.id.clone(),
        pattern,
        weight: user_rule.weight,
        scope,
        strength: RuleStrength::Supporting,
        source: RuleSource::UserDefined,
        command_context: None,
        description: user_rule.description.clone(),
        enabled: user_rule.enabled,
    })
}

/// Parse a rule scope string from config into the runtime enum.
fn parse_rule_scope(scope: &str) -> RuleScope {
    if let Some(n_str) = scope.strip_prefix("first_lines:") {
        let n = n_str.parse().unwrap_or(5);
        return RuleScope::FirstLines(n);
    }
    if let Some(n_str) = scope.strip_prefix("last_lines:") {
        let n = n_str.parse().unwrap_or(3);
        return RuleScope::LastLines(n);
    }
    match scope {
        "full_block" => RuleScope::FullBlock,
        "preceding_command" => RuleScope::PrecedingCommand,
        _ => RuleScope::AnyLine, // default
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::prettifier::*;

    #[test]
    fn test_to_pipeline_config() {
        let resolved = ResolvedPrettifierConfig {
            enabled: true,
            respect_alternate_screen: false,
            global_toggle_key: "Ctrl+Shift+P".to_string(),
            per_block_toggle: true,
            detection: DetectionConfig {
                scope: "all".to_string(),
                confidence_threshold: 0.8,
                max_scan_lines: 200,
                debounce_ms: 50,
            },
            clipboard: ClipboardConfig::default(),
            renderers: RenderersConfig::default(),
            custom_renderers: Vec::new(),
            claude_code_integration: ClaudeCodeConfig::default(),
            detection_rules: HashMap::new(),
            cache: CacheConfig::default(),
            allowed_commands: Vec::new(),
        };

        let config = to_pipeline_config(&resolved);
        assert!(config.enabled);
        assert!(!config.respect_alternate_screen);
        assert!((config.confidence_threshold - 0.8).abs() < f32::EPSILON);
        assert_eq!(config.max_scan_lines, 200);
        assert_eq!(config.debounce_ms, 50);
        assert_eq!(config.detection_scope, DetectionScope::All);
    }

    #[test]
    fn test_parse_detection_scope() {
        assert_eq!(parse_detection_scope("all"), DetectionScope::All);
        assert_eq!(
            parse_detection_scope("command_output"),
            DetectionScope::CommandOutput
        );
        assert_eq!(
            parse_detection_scope("manual_only"),
            DetectionScope::ManualOnly
        );
        assert_eq!(parse_detection_scope("unknown"), DetectionScope::All);
    }

    #[test]
    fn test_parse_rule_scope() {
        assert_eq!(parse_rule_scope("any_line"), RuleScope::AnyLine);
        assert_eq!(parse_rule_scope("full_block"), RuleScope::FullBlock);
        assert_eq!(
            parse_rule_scope("preceding_command"),
            RuleScope::PrecedingCommand
        );
        assert_eq!(
            parse_rule_scope("first_lines:10"),
            RuleScope::FirstLines(10)
        );
        assert_eq!(parse_rule_scope("last_lines:3"), RuleScope::LastLines(3));
        assert_eq!(parse_rule_scope("unknown"), RuleScope::AnyLine);
    }

    #[test]
    fn test_parse_user_rule_valid() {
        let user_rule = UserDetectionRule {
            id: "custom_md".to_string(),
            pattern: r"^#\s+".to_string(),
            weight: 0.5,
            scope: "first_lines:5".to_string(),
            enabled: true,
            description: "Match ATX headers".to_string(),
        };

        let rule = parse_user_rule(&user_rule).unwrap();
        assert_eq!(rule.id, "custom_md");
        assert!((rule.weight - 0.5).abs() < f32::EPSILON);
        assert_eq!(rule.scope, RuleScope::FirstLines(5));
        assert_eq!(rule.source, RuleSource::UserDefined);
        assert_eq!(rule.strength, RuleStrength::Supporting);
        assert!(rule.enabled);
    }

    #[test]
    fn test_parse_user_rule_invalid_regex() {
        let user_rule = UserDetectionRule {
            id: "bad".to_string(),
            pattern: r"[invalid".to_string(),
            weight: 0.5,
            scope: "any_line".to_string(),
            enabled: true,
            description: String::new(),
        };

        assert!(parse_user_rule(&user_rule).is_none());
    }

    #[test]
    fn test_load_detection_rules_empty() {
        let built_in: HashMap<String, Vec<DetectionRule>> = HashMap::new();
        let config_rules: HashMap<String, FormatDetectionRulesConfig> = HashMap::new();
        let result = load_detection_rules(built_in, &config_rules);
        assert!(result.is_empty());
    }

    #[test]
    fn test_load_detection_rules_adds_user_rules() {
        let built_in: HashMap<String, Vec<DetectionRule>> = HashMap::new();
        let mut config_rules = HashMap::new();
        config_rules.insert(
            "markdown".to_string(),
            FormatDetectionRulesConfig {
                additional: vec![UserDetectionRule {
                    id: "user_md1".to_string(),
                    pattern: r"^##\s+".to_string(),
                    weight: 0.4,
                    scope: "any_line".to_string(),
                    enabled: true,
                    description: "H2 headers".to_string(),
                }],
                overrides: vec![],
            },
        );

        let result = load_detection_rules(built_in, &config_rules);
        assert_eq!(result["markdown"].len(), 1);
        assert_eq!(result["markdown"][0].id, "user_md1");
    }

    #[test]
    fn test_load_detection_rules_applies_overrides() {
        let mut built_in: HashMap<String, Vec<DetectionRule>> = HashMap::new();
        built_in.insert(
            "json".to_string(),
            vec![DetectionRule {
                id: "json_brace".to_string(),
                pattern: regex::Regex::new(r"^\{").unwrap(),
                weight: 0.5,
                scope: RuleScope::FirstLines(1),
                strength: RuleStrength::Strong,
                source: RuleSource::BuiltIn,
                command_context: None,
                description: "Opens with brace".to_string(),
                enabled: true,
            }],
        );

        let mut config_rules = HashMap::new();
        config_rules.insert(
            "json".to_string(),
            FormatDetectionRulesConfig {
                additional: vec![],
                overrides: vec![RuleOverride {
                    id: "json_brace".to_string(),
                    enabled: Some(false),
                    weight: Some(0.8),
                }],
            },
        );

        let result = load_detection_rules(built_in, &config_rules);
        let json_rules = &result["json"];
        assert_eq!(json_rules.len(), 1);
        assert!(!json_rules[0].enabled);
        assert!((json_rules[0].weight - 0.8).abs() < f32::EPSILON);
    }

    // -----------------------------------------------------------------------
    // End-to-end detection integration tests
    // -----------------------------------------------------------------------

    fn default_config() -> Config {
        Config::default()
    }

    #[test]
    fn test_detection_markdown_headers_and_emphasis() {
        let config = default_config();
        let sample = "# Hello World\n\nThis is **bold** and *italic* text.\n\n## Sub-header\n\n- Item 1\n- Item 2\n";
        let result = test_detection(&config, sample, None);
        assert_eq!(
            result.format_id, "markdown",
            "Expected markdown detection, got {:?}",
            result.format_id
        );
        assert!(
            result.confidence >= result.threshold,
            "confidence {:.2} < threshold {:.2}",
            result.confidence,
            result.threshold
        );
    }

    #[test]
    fn test_detection_markdown_fenced_code() {
        let config = default_config();
        let sample = "Here is some code:\n\n```python\ndef hello():\n    print(\"hello\")\n```\n\nAnd more text.";
        let result = test_detection(&config, sample, None);
        assert_eq!(
            result.format_id, "markdown",
            "Expected markdown detection for fenced code block"
        );
        assert!(result.confidence >= result.threshold);
    }

    #[test]
    fn test_detection_markdown_table() {
        let config = default_config();
        let sample =
            "# Results\n\n| Name | Score |\n|------|-------|\n| Alice | 95 |\n| Bob | 87 |\n";
        let result = test_detection(&config, sample, None);
        assert_eq!(
            result.format_id, "markdown",
            "Expected markdown detection for tables"
        );
        assert!(result.confidence >= result.threshold);
    }

    #[test]
    fn test_detection_json_object() {
        let config = default_config();
        let sample = "{\n  \"name\": \"par-term\",\n  \"version\": \"0.21.0\",\n  \"features\": [\"prettifier\", \"sixel\"]\n}";
        let result = test_detection(&config, sample, None);
        assert_eq!(
            result.format_id, "json",
            "Expected json detection, got {:?}",
            result.format_id
        );
        assert!(result.confidence >= result.threshold);
    }

    #[test]
    fn test_detection_json_with_curl_context() {
        let config = default_config();
        let sample = "{\n  \"status\": 200,\n  \"data\": {\n    \"id\": 42\n  }\n}";
        let result = test_detection(&config, sample, Some("curl https://api.example.com"));
        assert_eq!(
            result.format_id, "json",
            "Expected json detection with curl context"
        );
        assert!(result.confidence >= result.threshold);
    }

    #[test]
    fn test_detection_yaml_document() {
        let config = default_config();
        let sample = "---\nname: par-term\nversion: 0.21.0\nfeatures:\n  - prettifier\n  - sixel\n";
        let result = test_detection(&config, sample, None);
        assert_eq!(
            result.format_id, "yaml",
            "Expected yaml detection, got {:?}",
            result.format_id
        );
        assert!(result.confidence >= result.threshold);
    }

    #[test]
    fn test_detection_diff_git() {
        let config = default_config();
        let sample = "diff --git a/src/main.rs b/src/main.rs\nindex abc1234..def5678 100644\n--- a/src/main.rs\n+++ b/src/main.rs\n@@ -10,3 +10,4 @@\n fn main() {\n     println!(\"hello\");\n+    println!(\"world\");\n }\n";
        let result = test_detection(&config, sample, None);
        assert_eq!(
            result.format_id, "diff",
            "Expected diff detection, got {:?}",
            result.format_id
        );
        assert!(result.confidence >= result.threshold);
    }

    #[test]
    fn test_detection_xml() {
        let config = default_config();
        let sample = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<root>\n  <item id=\"1\">Hello</item>\n  <item id=\"2\">World</item>\n</root>";
        let result = test_detection(&config, sample, None);
        assert_eq!(
            result.format_id, "xml",
            "Expected xml detection, got {:?}",
            result.format_id
        );
        assert!(result.confidence >= result.threshold);
    }

    #[test]
    fn test_detection_toml() {
        let config = default_config();
        let sample = "[package]\nname = \"par-term\"\nversion = \"0.21.0\"\nedition = \"2024\"\n\n[dependencies]\nwgpu = \"0.20\"\n";
        let result = test_detection(&config, sample, None);
        assert_eq!(
            result.format_id, "toml",
            "Expected toml detection, got {:?}",
            result.format_id
        );
        assert!(result.confidence >= result.threshold);
    }

    #[test]
    fn test_detection_log_output() {
        let config = default_config();
        let sample = "2024-01-15T10:30:00.000Z INFO  server started on port 8080\n2024-01-15T10:30:01.000Z DEBUG handling request GET /api/data\n2024-01-15T10:30:02.000Z WARN  slow query detected (2.5s)\n2024-01-15T10:30:03.000Z ERROR connection refused: database not reachable\n";
        let result = test_detection(&config, sample, None);
        assert_eq!(
            result.format_id, "log",
            "Expected log detection, got {:?}",
            result.format_id
        );
        assert!(result.confidence >= result.threshold);
    }

    #[test]
    fn test_detection_csv() {
        let config = default_config();
        let sample = "name,age,city\nAlice,30,NYC\nBob,25,SF\nCharlie,35,LA\n";
        let result = test_detection(&config, sample, None);
        assert_eq!(
            result.format_id, "csv",
            "Expected csv detection, got {:?}",
            result.format_id
        );
        assert!(result.confidence >= result.threshold);
    }

    #[test]
    fn test_detection_plain_text_no_match() {
        let config = default_config();
        let sample =
            "This is just plain text.\nNothing special about it.\nJust regular terminal output.";
        let result = test_detection(&config, sample, None);
        // Should NOT match any format with sufficient confidence
        assert!(
            result.format_id.is_empty() || result.confidence < result.threshold,
            "Plain text should not be detected as {:?} (confidence={:.2})",
            result.format_id,
            result.confidence
        );
    }

    #[test]
    fn test_detection_full_pipeline_markdown_rendering() {
        // Test the complete flow: config → pipeline → detect → render
        let config = default_config();
        let resolved = resolve_prettifier_config(
            config.enable_prettifier,
            &config.content_prettifier,
            None,
            None,
        );
        let pipeline_config = to_pipeline_config(&resolved);
        let registry = build_default_registry(&resolved);
        let renderer_config = super::super::traits::RendererConfig::default();
        let mut pipeline = super::super::pipeline::PrettifierPipeline::new(
            super::super::pipeline::PrettifierConfig {
                detection_scope: super::super::boundary::DetectionScope::All,
                enabled: true,
                ..pipeline_config
            },
            registry,
            renderer_config,
        );

        // Feed markdown lines
        pipeline.process_output("# Hello World", 0);
        pipeline.process_output("", 1);
        pipeline.process_output("This is **bold** text.", 2);
        pipeline.process_output("", 3);
        pipeline.process_output("", 4); // Two blank lines trigger boundary

        let blocks = pipeline.active_blocks();
        assert!(
            !blocks.is_empty(),
            "Expected at least one detected block after feeding markdown"
        );
        assert_eq!(blocks[0].detection.format_id, "markdown");
        assert!(
            blocks[0].has_rendered(),
            "Block should have rendered content"
        );

        // Verify rendered content has styled lines
        let display = blocks[0].buffer.display_lines();
        assert!(
            !display.is_empty(),
            "Rendered content should have display lines"
        );
    }

    #[test]
    fn test_detection_full_pipeline_command_output_scope() {
        // Test with CommandOutput scope — requires OSC 133 markers
        let config = default_config();
        let resolved = resolve_prettifier_config(
            config.enable_prettifier,
            &config.content_prettifier,
            None,
            None,
        );
        let pipeline_config = to_pipeline_config(&resolved);
        let registry = build_default_registry(&resolved);
        let renderer_config = super::super::traits::RendererConfig::default();
        let mut pipeline = super::super::pipeline::PrettifierPipeline::new(
            super::super::pipeline::PrettifierConfig {
                detection_scope: super::super::boundary::DetectionScope::CommandOutput,
                enabled: true,
                ..pipeline_config
            },
            registry,
            renderer_config,
        );

        // Simulate OSC 133 command flow
        pipeline.on_command_start("cat README.md");
        pipeline.process_output("# par-term", 10);
        pipeline.process_output("", 11);
        pipeline.process_output("A GPU-accelerated terminal.", 12);
        pipeline.process_output("", 13);
        pipeline.process_output("## Features", 14);
        pipeline.process_output("", 15);
        pipeline.process_output("- **Fast rendering**", 16);
        pipeline.process_output("- Inline graphics", 17);
        pipeline.on_command_end(); // OSC 133 D

        let blocks = pipeline.active_blocks();
        assert!(
            !blocks.is_empty(),
            "Expected at least one detected block in CommandOutput scope"
        );
        assert_eq!(blocks[0].detection.format_id, "markdown");
        assert!(blocks[0].has_rendered());
        assert_eq!(
            blocks[0].content().preceding_command.as_deref(),
            Some("cat README.md")
        );
    }
}
