//! Built-in TOML detection rules.
//!
//! Creates a `RegexDetector` with 5 rules for identifying TOML content
//! in terminal output. Requires at least 2 matching rules for reliable
//! detection.

use regex::Regex;

use crate::config::prettifier::RenderersConfig;
use crate::prettifier::regex_detector::RegexDetectorBuilder;
use crate::prettifier::registry::RendererRegistry;
use crate::prettifier::types::{DetectionRule, RuleScope, RuleSource, RuleStrength};

/// Create the built-in TOML detector with default regex rules.
///
/// Five rules:
/// - `toml_section_header`: `[section]` headers (Strong)
/// - `toml_array_table`: `[[array]]` table headers (Definitive)
/// - `toml_key_value`: `key = value` pairs (Strong)
/// - `toml_string_value`: `= "string"` values (Supporting)
/// - `toml_comment`: `# comment` lines (Supporting)
pub fn create_toml_detector() -> crate::prettifier::regex_detector::RegexDetector {
    RegexDetectorBuilder::new("toml", "TOML")
        .confidence_threshold(0.6)
        .min_matching_rules(2)
        .definitive_rule_shortcircuit(false)
        .rule(DetectionRule {
            id: "toml_section_header".into(),
            pattern: Regex::new(r"^\[[\w.-]+\]\s*$")
                .expect("regex pattern is valid and should always compile"),
            weight: 0.5,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Strong,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "TOML section header [section]".into(),
            enabled: true,
        })
        .rule(DetectionRule {
            id: "toml_array_table".into(),
            pattern: Regex::new(r"^\[\[[\w.-]+\]\]\s*$")
                .expect("regex pattern is valid and should always compile"),
            weight: 0.6,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Definitive,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "TOML array of tables [[array]]".into(),
            enabled: true,
        })
        .rule(DetectionRule {
            id: "toml_key_value".into(),
            pattern: Regex::new(r"^[\w.-]+\s*=\s*")
                .expect("regex pattern is valid and should always compile"),
            weight: 0.3,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Strong,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "TOML key-value pair (key = value)".into(),
            enabled: true,
        })
        .rule(DetectionRule {
            id: "toml_string_value".into(),
            pattern: Regex::new(r#"=\s*"[^"]*"\s*$"#)
                .expect("regex pattern is valid and should always compile"),
            weight: 0.2,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Supporting,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "TOML string value (= \"...\")".into(),
            enabled: true,
        })
        .rule(DetectionRule {
            id: "toml_comment".into(),
            pattern: Regex::new(r"^\s*#")
                .expect("regex pattern is valid and should always compile"),
            weight: 0.1,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Supporting,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "TOML comment line".into(),
            enabled: true,
        })
        .build()
}

/// Register the TOML detector with the registry.
pub fn register_toml(registry: &mut RendererRegistry, config: &RenderersConfig) {
    if config.toml.enabled {
        let detector = create_toml_detector();
        registry.register_detector(config.toml.priority, Box::new(detector));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prettifier::testing::make_block_with_command;
    use crate::prettifier::traits::ContentDetector;

    #[test]
    fn test_all_rules_compile() {
        let detector = create_toml_detector();
        assert_eq!(detector.detection_rules().len(), 5);
    }

    #[test]
    fn test_toml_section_with_key_value() {
        let detector = create_toml_detector();
        let block = make_block_with_command(
            &["[package]", "name = \"par-term\"", "version = \"0.16.0\""],
            None,
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
        let result = result.unwrap();
        assert!(result.confidence >= 0.6);
    }

    #[test]
    fn test_toml_array_table() {
        let detector = create_toml_detector();
        let block = make_block_with_command(
            &["[[bin]]", "name = \"par-term\"", "path = \"src/main.rs\""],
            None,
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
    }

    #[test]
    fn test_toml_with_comments() {
        let detector = create_toml_detector();
        let block = make_block_with_command(
            &[
                "# Configuration file",
                "[server]",
                "host = \"localhost\"",
                "port = 8080",
            ],
            None,
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
    }

    #[test]
    fn test_not_toml_plain_text() {
        let detector = create_toml_detector();
        let block = make_block_with_command(&["Hello world", "This is plain text"], None);
        let result = detector.detect(&block);
        assert!(result.is_none());
    }

    #[test]
    fn test_not_toml_json() {
        let detector = create_toml_detector();
        let block = make_block_with_command(&["{", "  \"name\": \"par-term\"", "}"], None);
        let result = detector.detect(&block);
        assert!(result.is_none());
    }

    #[test]
    fn test_toml_nested_sections() {
        let detector = create_toml_detector();
        let block = make_block_with_command(
            &[
                "[database]",
                "host = \"localhost\"",
                "",
                "[database.pool]",
                "max_size = 10",
            ],
            None,
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
    }

    #[test]
    fn test_quick_match_with_section() {
        let detector = create_toml_detector();
        assert!(detector.quick_match(&["[package]", "name = \"test\""]));
    }

    #[test]
    fn test_quick_match_with_key_value() {
        let detector = create_toml_detector();
        assert!(detector.quick_match(&["name = \"test\"", "version = \"1.0\""]));
    }

    #[test]
    fn test_quick_match_plain_text() {
        let detector = create_toml_detector();
        assert!(!detector.quick_match(&["just plain text"]));
    }

    #[test]
    fn test_registration_enabled() {
        let config = RenderersConfig::default();
        let mut registry = RendererRegistry::new(0.6);
        register_toml(&mut registry, &config);
        assert_eq!(registry.detector_count(), 1);
    }

    #[test]
    fn test_registration_disabled() {
        let mut config = RenderersConfig::default();
        config.toml.enabled = false;
        let mut registry = RendererRegistry::new(0.6);
        register_toml(&mut registry, &config);
        assert_eq!(registry.detector_count(), 0);
    }
}
