//! Built-in diagram detection rules.
//!
//! Creates a `RegexDetector` that identifies fenced code blocks tagged with
//! diagram language identifiers (mermaid, plantuml, graphviz, d2, etc.).

use regex::Regex;

use crate::config::prettifier::DiagramRendererConfig;
use crate::prettifier::regex_detector::RegexDetectorBuilder;
use crate::prettifier::registry::RendererRegistry;
use crate::prettifier::types::{DetectionRule, RuleScope, RuleSource, RuleStrength};

/// All supported diagram language tags for detection.
const DIAGRAM_TAGS: &[&str] = &[
    "mermaid",
    "plantuml",
    "graphviz",
    "dot",
    "d2",
    "ditaa",
    "svgbob",
    "erd",
    "vegalite",
    "wavedrom",
    "excalidraw",
];

/// Create the built-in diagram detector with a single definitive regex rule
/// that matches fenced code blocks tagged with any known diagram language.
pub fn create_diagram_detector() -> crate::prettifier::regex_detector::RegexDetector {
    let tags_pattern = DIAGRAM_TAGS.join("|");
    let pattern = format!(r"^```({tags_pattern})\s*$");

    RegexDetectorBuilder::new("diagrams", "Diagrams")
        .confidence_threshold(0.8)
        .min_matching_rules(1)
        .definitive_rule_shortcircuit(true)
        .rule(DetectionRule {
            id: "diagram_fenced_block".into(),
            pattern: Regex::new(&pattern)
                .expect("regex pattern is valid and should always compile"),
            weight: 1.0,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Definitive,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Fenced code block with diagram language tag".into(),
            enabled: true,
        })
        .build()
}

/// Register the diagram detector with the registry.
pub fn register_diagrams(registry: &mut RendererRegistry, config: &DiagramRendererConfig) {
    if config.enabled {
        let detector = create_diagram_detector();
        registry.register_detector(config.priority, Box::new(detector));
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
    fn test_rule_compiles() {
        let detector = create_diagram_detector();
        assert_eq!(detector.detection_rules().len(), 1);
    }

    #[test]
    fn test_mermaid_detection() {
        let detector = create_diagram_detector();
        let block = make_block(&["```mermaid", "graph TD", "  A-->B", "```"], None);
        let result = detector.detect(&block);
        assert!(result.is_some());
        let result = result.unwrap();
        assert!((result.confidence - 1.0).abs() < f32::EPSILON);
        assert!(
            result
                .matched_rules
                .contains(&"diagram_fenced_block".to_string())
        );
    }

    #[test]
    fn test_plantuml_detection() {
        let detector = create_diagram_detector();
        let block = make_block(
            &["```plantuml", "@startuml", "Alice -> Bob", "@enduml", "```"],
            None,
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
        assert!((result.unwrap().confidence - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_graphviz_detection() {
        let detector = create_diagram_detector();
        let block = make_block(&["```graphviz", "digraph G { A -> B }", "```"], None);
        assert!(detector.detect(&block).is_some());
    }

    #[test]
    fn test_dot_detection() {
        let detector = create_diagram_detector();
        let block = make_block(&["```dot", "digraph { }", "```"], None);
        assert!(detector.detect(&block).is_some());
    }

    #[test]
    fn test_d2_detection() {
        let detector = create_diagram_detector();
        let block = make_block(&["```d2", "x -> y", "```"], None);
        assert!(detector.detect(&block).is_some());
    }

    #[test]
    fn test_ditaa_detection() {
        let detector = create_diagram_detector();
        let block = make_block(&["```ditaa", "+--+", "|  |", "+--+", "```"], None);
        assert!(detector.detect(&block).is_some());
    }

    #[test]
    fn test_svgbob_detection() {
        let detector = create_diagram_detector();
        let block = make_block(&["```svgbob", ".--.", "| |", "'--'", "```"], None);
        assert!(detector.detect(&block).is_some());
    }

    #[test]
    fn test_erd_detection() {
        let detector = create_diagram_detector();
        let block = make_block(&["```erd", "[Person]", "```"], None);
        assert!(detector.detect(&block).is_some());
    }

    #[test]
    fn test_vegalite_detection() {
        let detector = create_diagram_detector();
        let block = make_block(&["```vegalite", "{}", "```"], None);
        assert!(detector.detect(&block).is_some());
    }

    #[test]
    fn test_wavedrom_detection() {
        let detector = create_diagram_detector();
        let block = make_block(&["```wavedrom", "{}", "```"], None);
        assert!(detector.detect(&block).is_some());
    }

    #[test]
    fn test_excalidraw_detection() {
        let detector = create_diagram_detector();
        let block = make_block(&["```excalidraw", "{}", "```"], None);
        assert!(detector.detect(&block).is_some());
    }

    #[test]
    fn test_non_diagram_not_detected() {
        let detector = create_diagram_detector();
        let block = make_block(&["```rust", "fn main() {}", "```"], None);
        assert!(detector.detect(&block).is_none());
    }

    #[test]
    fn test_plain_text_not_detected() {
        let detector = create_diagram_detector();
        let block = make_block(&["Hello world", "Just some text"], None);
        assert!(detector.detect(&block).is_none());
    }

    #[test]
    fn test_quick_match_with_diagram_tag() {
        let detector = create_diagram_detector();
        assert!(detector.quick_match(&["```mermaid", "graph TD"]));
    }

    #[test]
    fn test_quick_match_plain_text() {
        let detector = create_diagram_detector();
        assert!(!detector.quick_match(&["just plain text"]));
    }

    #[test]
    fn test_registration_enabled() {
        let config = DiagramRendererConfig::default();
        let mut registry = RendererRegistry::new(0.6);
        register_diagrams(&mut registry, &config);
        assert_eq!(registry.detector_count(), 1);
    }

    #[test]
    fn test_registration_disabled() {
        let config = DiagramRendererConfig {
            enabled: false,
            ..Default::default()
        };
        let mut registry = RendererRegistry::new(0.6);
        register_diagrams(&mut registry, &config);
        assert_eq!(registry.detector_count(), 0);
    }

    #[test]
    fn test_all_ten_default_languages() {
        // Verify that all 10 unique diagram languages are detectable.
        // (graphviz and dot both map to GraphViz, so there are 11 tags but 10 languages)
        let detector = create_diagram_detector();
        for tag in DIAGRAM_TAGS {
            let block = make_block(&[&format!("```{tag}"), "content", "```"], None);
            assert!(
                detector.detect(&block).is_some(),
                "Failed to detect diagram tag: {tag}"
            );
        }
    }
}
