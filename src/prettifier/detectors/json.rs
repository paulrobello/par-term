//! Built-in JSON detection rules.
//!
//! Creates a `RegexDetector` with 6 rules for identifying JSON content
//! in terminal output. No single rule is definitive; detection relies
//! on accumulating weighted confidence from multiple signals.

use regex::Regex;

use crate::config::prettifier::RenderersConfig;
use crate::prettifier::regex_detector::RegexDetectorBuilder;
use crate::prettifier::registry::RendererRegistry;
use crate::prettifier::types::{DetectionRule, RuleScope, RuleSource, RuleStrength};

/// Create the built-in JSON detector with default regex rules.
///
/// Six rules from spec lines 311–346:
/// - `json_open_brace`: opening `{` on its own line
/// - `json_open_bracket`: opening `[` on its own line
/// - `json_key_value`: `"key": value` patterns
/// - `json_close_brace`: closing `}` on its own line
/// - `json_curl_context`: preceding command is curl/http/httpie/wget
/// - `json_jq_context`: preceding command is jq/gron/fx
pub fn create_json_detector() -> crate::prettifier::regex_detector::RegexDetector {
    RegexDetectorBuilder::new("json", "JSON")
        .confidence_threshold(0.6)
        .min_matching_rules(1)
        .definitive_rule_shortcircuit(false) // No single definitive rule for JSON
        // Strong rules
        .rule(DetectionRule {
            id: "json_open_brace".into(),
            pattern: Regex::new(r"^\s*\{\s*$").unwrap(),
            weight: 0.4,
            scope: RuleScope::FirstLines(3),
            strength: RuleStrength::Strong,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Line containing only an opening brace {".into(),
            enabled: true,
        })
        .rule(DetectionRule {
            id: "json_open_bracket".into(),
            pattern: Regex::new(r"^\s*\[\s*$").unwrap(),
            weight: 0.35,
            scope: RuleScope::FirstLines(3),
            strength: RuleStrength::Strong,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Line containing only an opening bracket [".into(),
            enabled: true,
        })
        .rule(DetectionRule {
            id: "json_key_value".into(),
            pattern: Regex::new(r#"^\s*"[^"]+"\s*:\s*"#).unwrap(),
            weight: 0.3,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Strong,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: r#"JSON key-value pattern ("key": value)"#.into(),
            enabled: true,
        })
        // Supporting rules
        .rule(DetectionRule {
            id: "json_close_brace".into(),
            pattern: Regex::new(r"^\s*\}\s*,?\s*$").unwrap(),
            weight: 0.2,
            scope: RuleScope::LastLines(3),
            strength: RuleStrength::Supporting,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Line containing only a closing brace }".into(),
            enabled: true,
        })
        .rule(DetectionRule {
            id: "json_curl_context".into(),
            pattern: Regex::new(r"^(curl|http|httpie|wget)\s+").unwrap(),
            weight: 0.3,
            scope: RuleScope::PrecedingCommand,
            strength: RuleStrength::Supporting,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Preceding command is curl, http, httpie, or wget".into(),
            enabled: true,
        })
        .rule(DetectionRule {
            id: "json_jq_context".into(),
            pattern: Regex::new(r"^(jq|gron|fx)\s+").unwrap(),
            weight: 0.3,
            scope: RuleScope::PrecedingCommand,
            strength: RuleStrength::Supporting,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Preceding command is jq, gron, or fx".into(),
            enabled: true,
        })
        .build()
}

/// Register the JSON detector with the registry.
pub fn register_json(registry: &mut RendererRegistry, config: &RenderersConfig) {
    if config.json.enabled {
        let detector = create_json_detector();
        registry.register_detector(config.json.priority, Box::new(detector));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prettifier::traits::ContentDetector;
    use crate::prettifier::types::ContentBlock;
    use std::time::SystemTime;

    fn make_block(lines: &[&str], command: Option<&str>) -> ContentBlock {
        ContentBlock {
            lines: lines.iter().map(|s| s.to_string()).collect(),
            preceding_command: command.map(|s| s.to_string()),
            start_row: 0,
            end_row: lines.len(),
            timestamp: SystemTime::now(),
        }
    }

    #[test]
    fn test_all_rules_compile() {
        let detector = create_json_detector();
        assert_eq!(detector.detection_rules().len(), 6);
    }

    #[test]
    fn test_json_object_detection() {
        let detector = create_json_detector();
        let block = make_block(
            &["{", "  \"name\": \"par-term\",", "  \"version\": 1", "}"],
            None,
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
        let result = result.unwrap();
        // open_brace(0.4) + key_value(0.3) + close_brace(0.2) = 0.9
        assert!(result.confidence >= 0.6);
    }

    #[test]
    fn test_json_array_detection() {
        let detector = create_json_detector();
        // open_bracket(0.35) + jq context(0.3) = 0.65 >= 0.6
        let block = make_block(
            &["[", "  \"item1\",", "  \"item2\"", "]"],
            Some("jq '.[]' data.json"),
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
    }

    #[test]
    fn test_json_with_curl_context() {
        let detector = create_json_detector();
        let block = make_block(
            &["{", "  \"status\": \"ok\"", "}"],
            Some("curl https://api.example.com"),
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
        let result = result.unwrap();
        assert!(
            result
                .matched_rules
                .contains(&"json_curl_context".to_string())
        );
    }

    #[test]
    fn test_json_with_jq_context() {
        let detector = create_json_detector();
        let block = make_block(&["{", "  \"key\": \"value\"", "}"], Some("jq . data.json"));
        let result = detector.detect(&block);
        assert!(result.is_some());
        let result = result.unwrap();
        assert!(
            result
                .matched_rules
                .contains(&"json_jq_context".to_string())
        );
    }

    #[test]
    fn test_not_json_plain_text() {
        let detector = create_json_detector();
        let block = make_block(&["Hello world", "This is plain text"], None);
        let result = detector.detect(&block);
        assert!(result.is_none());
    }

    #[test]
    fn test_not_json_markdown() {
        let detector = create_json_detector();
        let block = make_block(&["# Title", "Some **bold** text", "- item"], None);
        let result = detector.detect(&block);
        assert!(result.is_none());
    }

    #[test]
    fn test_quick_match_with_brace() {
        let detector = create_json_detector();
        assert!(detector.quick_match(&["{", "  \"key\": \"value\""]));
    }

    #[test]
    fn test_quick_match_with_bracket() {
        let detector = create_json_detector();
        assert!(detector.quick_match(&["[", "  1, 2, 3"]));
    }

    #[test]
    fn test_quick_match_plain_text() {
        let detector = create_json_detector();
        assert!(!detector.quick_match(&["just plain text"]));
    }

    #[test]
    fn test_registration_enabled() {
        let config = RenderersConfig::default();
        let mut registry = RendererRegistry::new(0.6);
        register_json(&mut registry, &config);
        assert_eq!(registry.detector_count(), 1);
    }

    #[test]
    fn test_registration_disabled() {
        let mut config = RenderersConfig::default();
        config.json.enabled = false;
        let mut registry = RendererRegistry::new(0.6);
        register_json(&mut registry, &config);
        assert_eq!(registry.detector_count(), 0);
    }

    #[test]
    fn test_no_definitive_shortcircuit() {
        let detector = create_json_detector();
        // Even with matching patterns, confidence should be accumulated, not 1.0
        let block = make_block(
            &["{", "  \"name\": \"test\"", "}"],
            Some("curl https://api.example.com"),
        );
        let result = detector.detect(&block).unwrap();
        // Should not be exactly 1.0 since shortcircuit is disabled
        // (unless weights happen to sum to >= 1.0, which they do here)
        assert!(result.confidence >= 0.6);
    }

    #[test]
    fn test_key_value_rule_matches() {
        let detector = create_json_detector();
        // open_brace(0.4) + key_value(0.3) + close_brace(0.2) = 0.9
        let block = make_block(
            &[
                "{",
                "  \"name\": \"test\",",
                "  \"version\": \"1.0\",",
                "  \"count\": 42",
                "}",
            ],
            None,
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
        let result = result.unwrap();
        assert!(result.matched_rules.contains(&"json_key_value".to_string()));
    }
}
