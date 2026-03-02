//! Built-in CSV/TSV detection rules.
//!
//! Creates a `RegexDetector` with 4 rules for identifying CSV/TSV content
//! in terminal output. Requires at least 2 matching rules to avoid false
//! positives since comma-separated text is common in many contexts.

use regex::Regex;

use crate::config::prettifier::RenderersConfig;
use crate::prettifier::regex_detector::RegexDetectorBuilder;
use crate::prettifier::registry::RendererRegistry;
use crate::prettifier::types::{DetectionRule, RuleScope, RuleSource, RuleStrength};

/// Create the built-in CSV/TSV detector with default regex rules.
///
/// Four rules:
/// - `csv_comma_consistent`: Multiple comma-separated fields (Supporting)
/// - `csv_tab_consistent`: Multiple tab-separated fields (Supporting)
/// - `csv_header_row`: Header-like first line with word,word,word pattern (Strong)
/// - `csv_command_context`: Preceding command is a CSV tool (Supporting)
pub fn create_csv_detector() -> crate::prettifier::regex_detector::RegexDetector {
    RegexDetectorBuilder::new("csv", "CSV/TSV")
        .confidence_threshold(0.6)
        .min_matching_rules(2)
        .definitive_rule_shortcircuit(false)
        .rule(DetectionRule {
            id: "csv_comma_consistent".into(),
            pattern: Regex::new(r"^[^,]+,[^,]+,")
                .expect("regex pattern is valid and should always compile"),
            weight: 0.3,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Supporting,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Multiple comma-separated fields".into(),
            enabled: true,
        })
        .rule(DetectionRule {
            id: "csv_tab_consistent".into(),
            pattern: Regex::new(r"^[^\t]+\t[^\t]+\t")
                .expect("regex pattern is valid and should always compile"),
            weight: 0.4,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Supporting,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Multiple tab-separated fields".into(),
            enabled: true,
        })
        .rule(DetectionRule {
            id: "csv_header_row".into(),
            pattern: Regex::new(r"^[a-zA-Z_]\w*(,[a-zA-Z_]\w*)+\s*$")
                .expect("csv_header_row: pattern is valid and should always compile"),
            weight: 0.4,
            scope: RuleScope::FirstLines(1),
            strength: RuleStrength::Strong,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Header row with word-like column names".into(),
            enabled: true,
        })
        .rule(DetectionRule {
            id: "csv_command_context".into(),
            pattern: Regex::new(r"(csvtool|csvkit|cut|awk)")
                .expect("csv_command_context: pattern is valid and should always compile"),
            weight: 0.2,
            scope: RuleScope::PrecedingCommand,
            strength: RuleStrength::Supporting,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Preceding command is a CSV tool".into(),
            enabled: true,
        })
        .build()
}

/// Register the CSV/TSV detector with the registry.
pub fn register_csv(registry: &mut RendererRegistry, config: &RenderersConfig) {
    if config.csv.enabled {
        let detector = create_csv_detector();
        registry.register_detector(config.csv.priority, Box::new(detector));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prettifier::testing::make_block_with_command;
    use crate::prettifier::traits::ContentDetector;

    #[test]
    fn test_all_rules_compile() {
        let detector = create_csv_detector();
        assert_eq!(detector.detection_rules().len(), 4);
    }

    #[test]
    fn test_csv_with_header_and_data() {
        let detector = create_csv_detector();
        let block =
            make_block_with_command(&["name,age,city", "Alice,30,NYC", "Bob,25,London"], None);
        let result = detector.detect(&block);
        assert!(result.is_some());
        let result = result.unwrap();
        assert!(result.confidence >= 0.6);
        assert!(result.matched_rules.contains(&"csv_header_row".to_string()));
    }

    #[test]
    fn test_tsv_detection() {
        let detector = create_csv_detector();
        // TSV needs command context to reach min_matching_rules=2
        let block = make_block_with_command(
            &["name\tage\tcity", "Alice\t30\tNYC", "Bob\t25\tLondon"],
            Some("cut -f1-3 data.tsv"),
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
    }

    #[test]
    fn test_csv_with_command_context() {
        let detector = create_csv_detector();
        // Need header row + command context for min_matching_rules=2
        let block = make_block_with_command(
            &["name,age,city", "Alice,30,NYC", "Bob,25,London"],
            Some("csvtool col 1-3 data.csv"),
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
        let result = result.unwrap();
        assert!(
            result
                .matched_rules
                .contains(&"csv_command_context".to_string())
        );
    }

    #[test]
    fn test_not_csv_plain_text() {
        let detector = create_csv_detector();
        let block = make_block_with_command(&["Hello world", "This is plain text"], None);
        let result = detector.detect(&block);
        assert!(result.is_none());
    }

    #[test]
    fn test_single_comma_line_not_enough() {
        let detector = create_csv_detector();
        // Only one rule matches (csv_comma_consistent), but min_matching_rules=2
        let block = make_block_with_command(&["just, some, text"], None);
        let result = detector.detect(&block);
        assert!(result.is_none());
    }

    #[test]
    fn test_quick_match_csv() {
        let detector = create_csv_detector();
        assert!(detector.quick_match(&["name,age,city", "Alice,30,NYC"]));
    }

    #[test]
    fn test_quick_match_plain_text() {
        let detector = create_csv_detector();
        assert!(!detector.quick_match(&["just plain text"]));
    }

    #[test]
    fn test_registration_enabled() {
        let config = RenderersConfig::default();
        let mut registry = RendererRegistry::new(0.6);
        register_csv(&mut registry, &config);
        assert_eq!(registry.detector_count(), 1);
    }

    #[test]
    fn test_registration_disabled() {
        let mut config = RenderersConfig::default();
        config.csv.enabled = false;
        let mut registry = RendererRegistry::new(0.6);
        register_csv(&mut registry, &config);
        assert_eq!(registry.detector_count(), 0);
    }
}
