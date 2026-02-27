//! Built-in Markdown detection rules.
//!
//! Creates a `RegexDetector` with all the markdown-specific regex rules
//! for identifying markdown content in terminal output.

use regex::Regex;

use crate::config::prettifier::RenderersConfig;
use crate::prettifier::regex_detector::RegexDetectorBuilder;
use crate::prettifier::registry::RendererRegistry;
use crate::prettifier::types::{DetectionRule, RuleScope, RuleSource, RuleStrength};

/// Create the built-in Markdown detector with default regex rules.
pub fn create_markdown_detector() -> crate::prettifier::regex_detector::RegexDetector {
    RegexDetectorBuilder::new("markdown", "Markdown")
        .confidence_threshold(0.6)
        .min_matching_rules(1)
        .definitive_rule_shortcircuit(true)
        // Definitive rules
        .rule(DetectionRule {
            id: "md_fenced_code".into(),
            pattern: Regex::new(r"^```\w*\s*$")
                .expect("regex pattern is valid and should always compile"),
            weight: 0.8,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Definitive,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Fenced code block opening (``` or ```language)".into(),
            enabled: true,
        })
        .rule(DetectionRule {
            id: "md_fenced_tilde".into(),
            pattern: Regex::new(r"^~~~\w*\s*$")
                .expect("regex pattern is valid and should always compile"),
            weight: 0.8,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Definitive,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Tilde-style fenced code block".into(),
            enabled: true,
        })
        // Strong rules
        .rule(DetectionRule {
            id: "md_atx_header".into(),
            pattern: Regex::new(r"^#{1,6}\s+\S")
                .expect("regex pattern is valid and should always compile"),
            weight: 0.5,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Strong,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "ATX-style header (# through ######)".into(),
            enabled: true,
        })
        .rule(DetectionRule {
            id: "md_table".into(),
            pattern: Regex::new(r"^\|.*\|.*\|")
                .expect("regex pattern is valid and should always compile"),
            weight: 0.4,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Strong,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Markdown table row with pipe delimiters".into(),
            enabled: true,
        })
        // Supporting rules
        .rule(DetectionRule {
            id: "md_table_separator".into(),
            pattern: Regex::new(r"^\|[\s\-:\|]+\|")
                .expect("regex pattern is valid and should always compile"),
            weight: 0.3,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Supporting,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Markdown table separator row".into(),
            enabled: true,
        })
        .rule(DetectionRule {
            id: "md_bold".into(),
            pattern: Regex::new(r"\*\*[^*]+\*\*")
                .expect("regex pattern is valid and should always compile"),
            weight: 0.2,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Supporting,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Bold text (**text**)".into(),
            enabled: true,
        })
        .rule(DetectionRule {
            id: "md_italic".into(),
            pattern: Regex::new(r"(?:^|[^*])\*[^*]+\*(?:[^*]|$)")
                .expect("md_italic: pattern is valid and should always compile"),
            weight: 0.15,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Supporting,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Italic text (*text*)".into(),
            enabled: true,
        })
        .rule(DetectionRule {
            id: "md_link".into(),
            pattern: Regex::new(r"\[([^\]]+)\]\(([^)]+)\)")
                .expect("md_link: pattern is valid and should always compile"),
            weight: 0.2,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Supporting,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Markdown link [text](url)".into(),
            enabled: true,
        })
        .rule(DetectionRule {
            id: "md_list_bullet".into(),
            pattern: Regex::new(r"^\s*[-*+]\s+\S")
                .expect("regex pattern is valid and should always compile"),
            weight: 0.15,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Supporting,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Bullet list item (-, *, +)".into(),
            enabled: true,
        })
        .rule(DetectionRule {
            id: "md_list_ordered".into(),
            pattern: Regex::new(r"^\s*\d+[.)]\s+\S")
                .expect("md_list_ordered: pattern is valid and should always compile"),
            weight: 0.15,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Supporting,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Ordered list item (1. or 1))".into(),
            enabled: true,
        })
        .rule(DetectionRule {
            id: "md_blockquote".into(),
            pattern: Regex::new(r"^>\s+")
                .expect("regex pattern is valid and should always compile"),
            weight: 0.15,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Supporting,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Blockquote (> text)".into(),
            enabled: true,
        })
        .rule(DetectionRule {
            id: "md_inline_code".into(),
            pattern: Regex::new(r"`[^`]+`")
                .expect("regex pattern is valid and should always compile"),
            weight: 0.1,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Supporting,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Inline code (`code`)".into(),
            enabled: true,
        })
        .rule(DetectionRule {
            id: "md_horizontal_rule".into(),
            pattern: Regex::new(r"^[-*_]\s*[-*_]\s*[-*_][\s*_-]*$")
                .expect("regex pattern is valid and should always compile"),
            weight: 0.15,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Supporting,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Horizontal rule (---, ***, ___)".into(),
            enabled: true,
        })
        // Command context rule
        .rule(DetectionRule {
            id: "md_claude_code_context".into(),
            pattern: Regex::new(r"\b(claude|claude-code)\b|(?:^|\s)cc(?:\s|$)")
                .expect("md_claude_code_context: pattern is valid and should always compile"),
            weight: 0.2,
            scope: RuleScope::PrecedingCommand,
            strength: RuleStrength::Supporting,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Output follows a Claude Code command".into(),
            enabled: true,
        })
        .build()
}

/// Register the markdown detector with the registry.
pub fn register_markdown(registry: &mut RendererRegistry, config: &RenderersConfig) {
    if config.markdown.enabled {
        let detector = create_markdown_detector();
        registry.register_detector(config.markdown.priority, Box::new(detector));
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
        let detector = create_markdown_detector();
        assert_eq!(detector.detection_rules().len(), 14);
    }

    #[test]
    fn test_fenced_code_block_definitive() {
        let detector = create_markdown_detector();
        let block = make_block(&["```rust", "fn main() {}", "```"], None);
        let result = detector.detect(&block);
        assert!(result.is_some());
        let result = result.unwrap();
        assert!((result.confidence - 1.0).abs() < f32::EPSILON);
        assert!(result.matched_rules.contains(&"md_fenced_code".to_string()));
    }

    #[test]
    fn test_tilde_fenced_code_block_definitive() {
        let detector = create_markdown_detector();
        let block = make_block(&["~~~python", "print('hello')", "~~~"], None);
        let result = detector.detect(&block);
        assert!(result.is_some());
        let result = result.unwrap();
        assert!((result.confidence - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_headers_only_strong() {
        // Use a low threshold to verify the confidence value produced by headers.
        // The default threshold is 0.6, but the ATX header rule alone produces 0.5.
        let detector = RegexDetectorBuilder::new("markdown", "Markdown")
            .confidence_threshold(0.3)
            .min_matching_rules(1)
            .rule(DetectionRule {
                id: "md_atx_header".into(),
                pattern: Regex::new(r"^#{1,6}\s+\S").unwrap(),
                weight: 0.5,
                scope: RuleScope::AnyLine,
                strength: RuleStrength::Strong,
                source: RuleSource::BuiltIn,
                command_context: None,
                description: "ATX-style header".into(),
                enabled: true,
            })
            .build();

        let block = make_block(&["# Title", "Some body text"], None);
        let result = detector.detect(&block).unwrap();
        assert!(result.confidence >= 0.5);

        // With the default 0.6 threshold, headers alone don't pass detection.
        let full_detector = create_markdown_detector();
        let block = make_block(&["# Title", "Some body text"], None);
        assert!(full_detector.detect(&block).is_none());
    }

    #[test]
    fn test_mixed_signals_exceed_threshold() {
        let detector = create_markdown_detector();
        // bold(0.2) + link(0.2) + list_bullet(0.15) + blockquote(0.15) = 0.7
        let block = make_block(
            &[
                "This is **bold** text",
                "A [link](https://example.com) here",
                "- list item one",
                "> a blockquote",
            ],
            None,
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
        let result = result.unwrap();
        assert!(result.confidence >= 0.6);
    }

    #[test]
    fn test_below_threshold_single_weak_signal() {
        let detector = create_markdown_detector();
        let block = make_block(&["This has `inline code` only"], None);
        let result = detector.detect(&block);
        // inline_code weight = 0.1, far below 0.6 threshold.
        assert!(result.is_none());
    }

    #[test]
    fn test_claude_code_context_boost() {
        let detector = create_markdown_detector();
        let block = make_block(
            &["# Response", "Here is some **bold** and a [link](url)"],
            Some("claude"),
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
        let result = result.unwrap();
        // atx_header(0.5) + bold(0.2) + link(0.2) + claude_context(0.2) = 1.1, capped at 1.0
        assert!(
            result
                .matched_rules
                .contains(&"md_claude_code_context".to_string())
        );
    }

    #[test]
    fn test_table_detection() {
        let detector = create_markdown_detector();
        let block = make_block(
            &[
                "| Name | Age | City |",
                "|------|-----|------|",
                "| Alice | 30 | NYC |",
            ],
            None,
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
        let result = result.unwrap();
        // table(0.4) + table_separator(0.3) = 0.7 >= 0.6
        assert!(result.confidence >= 0.6);
    }

    #[test]
    fn test_false_positive_shell_comments() {
        let detector = create_markdown_detector();
        let block = make_block(
            &[
                "#!/bin/bash",
                "# This is a shell comment",
                "echo 'hello world'",
                "# Another comment",
            ],
            None,
        );
        // Shell comments start with `# ` which would match md_atx_header `^#{1,6}\s+\S`.
        // `#!/bin/bash` matches md_atx_header because `#!` is `#{1}` followed by `!`.
        // Wait: the pattern is `^#{1,6}\s+\S`. `#!/bin/bash` has `#` then `!` (not whitespace),
        // so it does NOT match. `# This is a shell comment` has `# T` which is
        // `#{1}\s+\S` - that DOES match.
        // So md_atx_header matches (0.5). Below 0.6 threshold with no other signals.
        // Additionally md_inline_code might match `'hello world'` - no, that uses
        // single quotes not backticks.
        // md_list_bullet: no match (no `- ` or `* ` at start).
        // So total = 0.5, below 0.6. Not detected.
        let result = detector.detect(&block);
        assert!(result.is_none());
    }

    #[test]
    fn test_not_markdown_json() {
        let detector = create_markdown_detector();
        let block = make_block(
            &["{", "  \"name\": \"test\",", "  \"value\": 42", "}"],
            None,
        );
        let result = detector.detect(&block);
        assert!(result.is_none());
    }

    #[test]
    fn test_quick_match_with_header() {
        let detector = create_markdown_detector();
        assert!(detector.quick_match(&["# Hello", "world"]));
    }

    #[test]
    fn test_quick_match_with_fenced_code() {
        let detector = create_markdown_detector();
        assert!(detector.quick_match(&["```rust", "fn main() {}"]));
    }

    #[test]
    fn test_quick_match_plain_text() {
        let detector = create_markdown_detector();
        assert!(!detector.quick_match(&["just plain text", "nothing special"]));
    }

    #[test]
    fn test_registration_enabled() {
        let config = RenderersConfig::default();
        let mut registry = RendererRegistry::new(0.6);
        register_markdown(&mut registry, &config);
        assert_eq!(registry.detector_count(), 1);
    }

    #[test]
    fn test_registration_disabled() {
        let mut config = RenderersConfig::default();
        config.markdown.enabled = false;
        let mut registry = RendererRegistry::new(0.6);
        register_markdown(&mut registry, &config);
        assert_eq!(registry.detector_count(), 0);
    }

    #[test]
    fn test_registration_custom_priority() {
        let mut config = RenderersConfig::default();
        config.markdown.priority = 100;
        let mut registry = RendererRegistry::new(0.6);
        register_markdown(&mut registry, &config);
        assert_eq!(registry.detector_count(), 1);
    }

    #[test]
    fn test_horizontal_rule() {
        let detector = create_markdown_detector();
        // Horizontal rule alone (0.15) is below threshold.
        let block = make_block(&["---"], None);
        let result = detector.detect(&block);
        assert!(result.is_none());

        // With enough other signals to pass threshold.
        let block = make_block(&["# Title", "---", "**bold** and [link](url)"], None);
        let result = detector.detect(&block);
        assert!(result.is_some());
    }

    #[test]
    fn test_blockquote() {
        let detector = create_markdown_detector();
        let block = make_block(
            &[
                "# Quote Section",
                "> This is a blockquote",
                "> with multiple lines",
                "And **bold** text too",
            ],
            None,
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
        let result = result.unwrap();
        // atx_header(0.5) + blockquote(0.15) + bold(0.2) = 0.85
        assert!(result.confidence >= 0.6);
    }

    #[test]
    fn test_ordered_list() {
        let detector = create_markdown_detector();
        let block = make_block(
            &[
                "# Steps",
                "1. First step",
                "2. Second step",
                "3. Third step",
            ],
            None,
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
        let result = result.unwrap();
        // atx_header(0.5) + ordered_list(0.15) = 0.65
        assert!(result.confidence >= 0.6);
    }
}
