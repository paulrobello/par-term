//! Built-in YAML detection rules.
//!
//! Creates a `RegexDetector` with 4 rules for identifying YAML content
//! in terminal output. Requires at least 2 matching rules to avoid
//! false positives (e.g., `---` alone could be a Markdown horizontal rule).

use regex::Regex;

use crate::config::prettifier::RenderersConfig;
use crate::prettifier::regex_detector::RegexDetectorBuilder;
use crate::prettifier::registry::RendererRegistry;
use crate::prettifier::types::{DetectionRule, RuleScope, RuleSource, RuleStrength};

/// Create the built-in YAML detector with default regex rules.
///
/// Four rules from spec lines 348–372:
/// - `yaml_doc_start`: `---` document separator in first 3 lines (Definitive)
/// - `yaml_key_value`: top-level `key: value` pattern (Strong)
/// - `yaml_nested`: indented `key: value` pattern (Supporting)
/// - `yaml_list`: YAML list items `- item` (Supporting)
///
/// Requires `min_matching_rules: 2` so that `---` alone (which conflicts
/// with Markdown horizontal rules) is not enough to trigger detection.
pub fn create_yaml_detector() -> crate::prettifier::regex_detector::RegexDetector {
    RegexDetectorBuilder::new("yaml", "YAML")
        .confidence_threshold(0.6)
        .min_matching_rules(2)
        .definitive_rule_shortcircuit(false) // Disabled so min_matching_rules is always respected
        .rule(DetectionRule {
            id: "yaml_doc_start".into(),
            pattern: Regex::new(r"^---\s*$")
                .expect("regex pattern is valid and should always compile"),
            weight: 0.5,
            scope: RuleScope::FirstLines(3),
            strength: RuleStrength::Definitive,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "YAML document start marker (---)".into(),
            enabled: true,
        })
        .rule(DetectionRule {
            id: "yaml_key_value".into(),
            pattern: Regex::new(r"^[a-zA-Z_][\w.\-]*:(\s|$)")
                .expect("yaml_key_value: pattern is valid and should always compile"),
            weight: 0.4,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Strong,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Top-level YAML key-value pair (key: value)".into(),
            enabled: true,
        })
        .rule(DetectionRule {
            id: "yaml_nested".into(),
            pattern: Regex::new(r"^\s{2,}[a-zA-Z_][\w.\-]*:(\s|$)")
                .expect("yaml_nested: pattern is valid and should always compile"),
            weight: 0.25,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Supporting,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Indented YAML key-value pair (nested mapping)".into(),
            enabled: true,
        })
        .rule(DetectionRule {
            id: "yaml_list".into(),
            pattern: Regex::new(r"^\s*-\s+\S")
                .expect("regex pattern is valid and should always compile"),
            weight: 0.2,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Supporting,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "YAML list item (- item)".into(),
            enabled: true,
        })
        .build()
}

/// Register the YAML detector with the registry.
pub fn register_yaml(registry: &mut RendererRegistry, config: &RenderersConfig) {
    if config.yaml.enabled {
        let detector = create_yaml_detector();
        registry.register_detector(config.yaml.priority, Box::new(detector));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prettifier::testing::make_block_with_command;
    use crate::prettifier::traits::ContentDetector;

    #[test]
    fn test_all_rules_compile() {
        let detector = create_yaml_detector();
        assert_eq!(detector.detection_rules().len(), 4);
    }

    #[test]
    fn test_yaml_with_doc_start_and_keys() {
        let detector = create_yaml_detector();
        let block = make_block_with_command(&["---", "name: par-term", "version: 0.16.0"], None);
        let result = detector.detect(&block);
        assert!(result.is_some());
        let result = result.unwrap();
        assert!(result.confidence >= 0.6);
        assert!(result.matched_rules.contains(&"yaml_doc_start".to_string()));
    }

    #[test]
    fn test_yaml_key_value_only() {
        let detector = create_yaml_detector();
        // key_value(0.3) + nested(0.2) = 0.5, plus list = 0.65
        let block = make_block_with_command(
            &[
                "name: my-app",
                "config:",
                "  port: 8080",
                "  hosts:",
                "  - localhost",
            ],
            None,
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
    }

    #[test]
    fn test_yaml_doc_start_alone_not_detected() {
        let detector = create_yaml_detector();
        // Only `---` matches — min_matching_rules=2 prevents detection.
        let block = make_block_with_command(&["---", "plain text here", "nothing yaml"], None);
        let result = detector.detect(&block);
        assert!(result.is_none());
    }

    #[test]
    fn test_not_yaml_plain_text() {
        let detector = create_yaml_detector();
        let block = make_block_with_command(&["Hello world", "This is plain text"], None);
        let result = detector.detect(&block);
        assert!(result.is_none());
    }

    #[test]
    fn test_not_yaml_markdown() {
        let detector = create_yaml_detector();
        let block = make_block_with_command(&["# Title", "Some **bold** text", "- item"], None);
        // The `- item` could match yaml_list but alone it's not enough
        let result = detector.detect(&block);
        assert!(result.is_none());
    }

    #[test]
    fn test_yaml_nested_keys() {
        let detector = create_yaml_detector();
        let block = make_block_with_command(
            &[
                "database:",
                "  host: localhost",
                "  port: 5432",
                "  credentials:",
                "    username: admin",
            ],
            None,
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
    }

    #[test]
    fn test_yaml_list_items() {
        let detector = create_yaml_detector();
        let block = make_block_with_command(
            &["dependencies:", "  - serde", "  - tokio", "  - wgpu"],
            None,
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
    }

    #[test]
    fn test_quick_match_with_key_value() {
        let detector = create_yaml_detector();
        assert!(detector.quick_match(&["---", "name: test"]));
    }

    #[test]
    fn test_quick_match_key_value_only() {
        let detector = create_yaml_detector();
        assert!(detector.quick_match(&["name: my-app", "version: 1.0"]));
    }

    #[test]
    fn test_quick_match_plain_text() {
        let detector = create_yaml_detector();
        assert!(!detector.quick_match(&["just plain text"]));
    }

    #[test]
    fn test_registration_enabled() {
        let config = RenderersConfig::default();
        let mut registry = RendererRegistry::new(0.6);
        register_yaml(&mut registry, &config);
        assert_eq!(registry.detector_count(), 1);
    }

    #[test]
    fn test_registration_disabled() {
        let mut config = RenderersConfig::default();
        config.yaml.enabled = false;
        let mut registry = RendererRegistry::new(0.6);
        register_yaml(&mut registry, &config);
        assert_eq!(registry.detector_count(), 0);
    }
}
