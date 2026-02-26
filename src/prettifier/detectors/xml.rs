//! Built-in XML/HTML detection rules.
//!
//! Creates a `RegexDetector` with 5 rules for identifying XML/HTML content
//! in terminal output. An `<?xml` declaration or `<!DOCTYPE` is definitive;
//! opening/closing tags and self-closing tags provide supporting evidence.

use regex::Regex;

use crate::config::prettifier::RenderersConfig;
use crate::prettifier::regex_detector::RegexDetectorBuilder;
use crate::prettifier::registry::RendererRegistry;
use crate::prettifier::types::{DetectionRule, RuleScope, RuleSource, RuleStrength};

/// Create the built-in XML/HTML detector with default regex rules.
///
/// Five rules:
/// - `xml_declaration`: `<?xml ...` in the first 3 lines (Definitive)
/// - `xml_doctype`: `<!DOCTYPE ...` in the first 5 lines (Definitive)
/// - `xml_opening_tag`: XML/HTML opening tag with optional attributes (Strong)
/// - `xml_closing_tag`: XML/HTML closing tag (Supporting)
/// - `xml_self_closing`: Self-closing tag ending with `/>` (Supporting)
pub fn create_xml_detector() -> crate::prettifier::regex_detector::RegexDetector {
    RegexDetectorBuilder::new("xml", "XML/HTML")
        .confidence_threshold(0.6)
        .min_matching_rules(1)
        .definitive_rule_shortcircuit(true)
        .rule(DetectionRule {
            id: "xml_declaration".into(),
            pattern: Regex::new(r"^<\?xml\s+")
                .expect("regex pattern is valid and should always compile"),
            weight: 0.9,
            scope: RuleScope::FirstLines(3),
            strength: RuleStrength::Definitive,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "XML declaration (<?xml ...)".into(),
            enabled: true,
        })
        .rule(DetectionRule {
            id: "xml_doctype".into(),
            pattern: Regex::new(r"^<!DOCTYPE\s+")
                .expect("regex pattern is valid and should always compile"),
            weight: 0.8,
            scope: RuleScope::FirstLines(5),
            strength: RuleStrength::Definitive,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "DOCTYPE declaration".into(),
            enabled: true,
        })
        .rule(DetectionRule {
            id: "xml_opening_tag".into(),
            pattern: Regex::new(r"^\s*<[a-zA-Z][\w:-]*(\s+[\w:-]+=)?")
                .expect("xml_opening_tag: pattern is valid and should always compile"),
            weight: 0.3,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Strong,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "XML/HTML opening tag with optional attributes".into(),
            enabled: true,
        })
        .rule(DetectionRule {
            id: "xml_closing_tag".into(),
            pattern: Regex::new(r"^\s*</[a-zA-Z][\w:-]*>")
                .expect("regex pattern is valid and should always compile"),
            weight: 0.2,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Supporting,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "XML/HTML closing tag".into(),
            enabled: true,
        })
        .rule(DetectionRule {
            id: "xml_self_closing".into(),
            pattern: Regex::new(r"/>\s*$")
                .expect("regex pattern is valid and should always compile"),
            weight: 0.15,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Supporting,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Self-closing tag".into(),
            enabled: true,
        })
        .build()
}

/// Register the XML detector with the registry.
pub fn register_xml(registry: &mut RendererRegistry, config: &RenderersConfig) {
    if config.xml.enabled {
        let detector = create_xml_detector();
        registry.register_detector(config.xml.priority, Box::new(detector));
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
        let detector = create_xml_detector();
        assert_eq!(detector.detection_rules().len(), 5);
    }

    #[test]
    fn test_xml_declaration_detected() {
        let detector = create_xml_detector();
        let block = make_block(
            &[
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>",
                "<root>",
                "  <child>text</child>",
                "</root>",
            ],
            None,
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
        let result = result.unwrap();
        assert!(result.confidence >= 0.6);
        assert!(
            result
                .matched_rules
                .contains(&"xml_declaration".to_string())
        );
    }

    #[test]
    fn test_doctype_detected() {
        let detector = create_xml_detector();
        let block = make_block(
            &[
                "<!DOCTYPE html>",
                "<html>",
                "<head><title>Test</title></head>",
                "</html>",
            ],
            None,
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
        let result = result.unwrap();
        assert!(result.matched_rules.contains(&"xml_doctype".to_string()));
    }

    #[test]
    fn test_xml_tags_detection() {
        let detector = create_xml_detector();
        let block = make_block(
            &[
                "<root>",
                "  <item name=\"test\">value</item>",
                "  <self-closing />",
                "</root>",
            ],
            None,
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
    }

    #[test]
    fn test_not_xml_plain_text() {
        let detector = create_xml_detector();
        let block = make_block(&["Hello world", "This is plain text"], None);
        let result = detector.detect(&block);
        assert!(result.is_none());
    }

    #[test]
    fn test_quick_match_with_xml_declaration() {
        let detector = create_xml_detector();
        assert!(detector.quick_match(&["<?xml version=\"1.0\"?>"]));
    }

    #[test]
    fn test_quick_match_with_tag() {
        let detector = create_xml_detector();
        assert!(detector.quick_match(&["<root>", "  <child>text</child>"]));
    }

    #[test]
    fn test_quick_match_plain_text() {
        let detector = create_xml_detector();
        assert!(!detector.quick_match(&["just plain text"]));
    }

    #[test]
    fn test_registration_enabled() {
        let config = RenderersConfig::default();
        let mut registry = RendererRegistry::new(0.6);
        register_xml(&mut registry, &config);
        assert_eq!(registry.detector_count(), 1);
    }

    #[test]
    fn test_registration_disabled() {
        let mut config = RenderersConfig::default();
        config.xml.enabled = false;
        let mut registry = RendererRegistry::new(0.6);
        register_xml(&mut registry, &config);
        assert_eq!(registry.detector_count(), 0);
    }
}
