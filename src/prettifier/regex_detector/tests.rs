//! Tests for the regex-based content detector.

use super::*;
use crate::prettifier::testing::make_block_with_command;
use crate::prettifier::types::{RuleSource, RuleStrength};

/// Helper: create a `DetectionRule` with sensible defaults.
fn make_rule(
    id: &str,
    pattern: &str,
    weight: f32,
    scope: RuleScope,
    strength: RuleStrength,
) -> DetectionRule {
    DetectionRule {
        id: id.to_string(),
        pattern: regex::Regex::new(pattern).unwrap(),
        weight,
        scope,
        strength,
        source: RuleSource::BuiltIn,
        command_context: None,
        description: format!("Test rule: {id}"),
        enabled: true,
    }
}

#[test]
fn test_basic_detection() {
    let detector = RegexDetectorBuilder::new("markdown", "Markdown")
        .rule(make_rule(
            "md_header",
            r"^#{1,6}\s",
            0.4,
            RuleScope::AnyLine,
            RuleStrength::Strong,
        ))
        .rule(make_rule(
            "md_bold",
            r"\*\*[^*]+\*\*",
            0.3,
            RuleScope::AnyLine,
            RuleStrength::Supporting,
        ))
        .confidence_threshold(0.5)
        .build();

    let block = make_block_with_command(&["# Hello", "This is **bold** text"], None);
    let result = detector.detect(&block);

    assert!(result.is_some());
    let result = result.unwrap();
    assert_eq!(result.format_id, "markdown");
    assert!((result.confidence - 0.7).abs() < f32::EPSILON);
    assert_eq!(result.matched_rules, vec!["md_header", "md_bold"]);
}

#[test]
fn test_confidence_threshold() {
    let detector = RegexDetectorBuilder::new("markdown", "Markdown")
        .rule(make_rule(
            "md_bold",
            r"\*\*[^*]+\*\*",
            0.3,
            RuleScope::AnyLine,
            RuleStrength::Supporting,
        ))
        .confidence_threshold(0.5)
        .build();

    let block = make_block_with_command(&["This is **bold** text"], None);
    let result = detector.detect(&block);

    // 0.3 < 0.5 threshold → None
    assert!(result.is_none());
}

#[test]
fn test_definitive_shortcircuit() {
    let detector = RegexDetectorBuilder::new("json", "JSON")
        .rule(make_rule(
            "json_brace",
            r"^\{",
            0.3,
            RuleScope::FirstLines(1),
            RuleStrength::Supporting,
        ))
        .rule(make_rule(
            "json_parse",
            r#"^\{[\s\S]*\}$"#,
            0.5,
            RuleScope::FullBlock,
            RuleStrength::Definitive,
        ))
        .build();

    let block = make_block_with_command(&["{", "  \"key\": \"value\"", "}"], None);
    let result = detector.detect(&block);

    assert!(result.is_some());
    let result = result.unwrap();
    assert!((result.confidence - 1.0).abs() < f32::EPSILON);
    // Only the definitive rule should be in matched_rules (short-circuited).
    assert_eq!(result.matched_rules, vec!["json_parse"]);
}

#[test]
fn test_min_matching_rules() {
    let detector = RegexDetectorBuilder::new("markdown", "Markdown")
        .rule(make_rule(
            "md_header",
            r"^#{1,6}\s",
            0.8,
            RuleScope::AnyLine,
            RuleStrength::Strong,
        ))
        .rule(make_rule(
            "md_bold",
            r"\*\*[^*]+\*\*",
            0.3,
            RuleScope::AnyLine,
            RuleStrength::Supporting,
        ))
        .confidence_threshold(0.3)
        .min_matching_rules(2)
        .build();

    // Only one rule matches (header), but min is 2.
    let block = make_block_with_command(&["# Hello", "Plain text"], None);
    let result = detector.detect(&block);

    assert!(result.is_none());
}

#[test]
fn test_rule_scoping() {
    // FirstLines: matches only in first N lines.
    let detector = RegexDetectorBuilder::new("test", "Test")
        .rule(make_rule(
            "first",
            r"^HEADER$",
            0.8,
            RuleScope::FirstLines(2),
            RuleStrength::Strong,
        ))
        .confidence_threshold(0.5)
        .build();

    let block_match = make_block_with_command(&["HEADER", "line2", "line3"], None);
    assert!(detector.detect(&block_match).is_some());

    let block_no_match = make_block_with_command(&["line1", "line2", "HEADER"], None);
    assert!(detector.detect(&block_no_match).is_none());

    // LastLines: matches only in last N lines.
    let detector = RegexDetectorBuilder::new("test", "Test")
        .rule(make_rule(
            "last",
            r"^FOOTER$",
            0.8,
            RuleScope::LastLines(1),
            RuleStrength::Strong,
        ))
        .confidence_threshold(0.5)
        .build();

    let block_match = make_block_with_command(&["line1", "line2", "FOOTER"], None);
    assert!(detector.detect(&block_match).is_some());

    let block_no_match = make_block_with_command(&["FOOTER", "line2", "line3"], None);
    assert!(detector.detect(&block_no_match).is_none());

    // FullBlock: matches against joined text.
    let detector = RegexDetectorBuilder::new("test", "Test")
        .rule(make_rule(
            "full",
            r"line1\nline2",
            0.8,
            RuleScope::FullBlock,
            RuleStrength::Strong,
        ))
        .confidence_threshold(0.5)
        .build();

    let block_match = make_block_with_command(&["line1", "line2", "line3"], None);
    assert!(detector.detect(&block_match).is_some());

    // PrecedingCommand: matches against the command.
    let detector = RegexDetectorBuilder::new("test", "Test")
        .rule(make_rule(
            "cmd",
            r"^git\s+diff",
            0.8,
            RuleScope::PrecedingCommand,
            RuleStrength::Strong,
        ))
        .confidence_threshold(0.5)
        .build();

    let block_match = make_block_with_command(&["diff output"], Some("git diff --cached"));
    assert!(detector.detect(&block_match).is_some());

    let block_no_cmd = make_block_with_command(&["diff output"], None);
    assert!(detector.detect(&block_no_cmd).is_none());
}

#[test]
fn test_command_context_filter() {
    let mut rule = make_rule(
        "diff_header",
        r"^diff --git",
        0.8,
        RuleScope::AnyLine,
        RuleStrength::Strong,
    );
    rule.command_context = Some(regex::Regex::new(r"^git\s").unwrap());

    let detector = RegexDetectorBuilder::new("diff", "Diff")
        .rule(rule)
        .confidence_threshold(0.5)
        .build();

    // With matching command context.
    let block_match = make_block_with_command(&["diff --git a/foo b/foo"], Some("git diff"));
    assert!(detector.detect(&block_match).is_some());

    // Without matching command context.
    let block_wrong_cmd = make_block_with_command(&["diff --git a/foo b/foo"], Some("svn diff"));
    assert!(detector.detect(&block_wrong_cmd).is_none());

    // Without any command.
    let block_no_cmd = make_block_with_command(&["diff --git a/foo b/foo"], None);
    assert!(detector.detect(&block_no_cmd).is_none());
}

#[test]
fn test_disabled_rules() {
    let mut rule = make_rule(
        "md_header",
        r"^#{1,6}\s",
        0.8,
        RuleScope::AnyLine,
        RuleStrength::Strong,
    );
    rule.enabled = false;

    let detector = RegexDetectorBuilder::new("markdown", "Markdown")
        .rule(rule)
        .confidence_threshold(0.5)
        .build();

    let block = make_block_with_command(&["# Hello"], None);
    assert!(detector.detect(&block).is_none());
}

#[test]
fn test_user_rule_merging() {
    let mut detector = RegexDetectorBuilder::new("markdown", "Markdown")
        .rule(make_rule(
            "md_header",
            r"^#{1,6}\s",
            0.4,
            RuleScope::AnyLine,
            RuleStrength::Strong,
        ))
        .confidence_threshold(0.3)
        .build();

    // Override existing rule's weight.
    let override_rule = DetectionRule {
        id: "md_header".to_string(),
        pattern: regex::Regex::new(r"^#{1,6}\s").unwrap(),
        weight: 0.9,
        scope: RuleScope::AnyLine,
        strength: RuleStrength::Strong,
        source: RuleSource::UserDefined,
        command_context: None,
        description: "Overridden header rule".to_string(),
        enabled: true,
    };

    // Append a new user rule.
    let new_rule = DetectionRule {
        id: "md_custom".to_string(),
        pattern: regex::Regex::new(r"^---$").unwrap(),
        weight: 0.3,
        scope: RuleScope::AnyLine,
        strength: RuleStrength::Supporting,
        source: RuleSource::UserDefined,
        command_context: None,
        description: "Custom frontmatter rule".to_string(),
        enabled: true,
    };

    detector.merge_user_rules(vec![override_rule, new_rule]);

    // Should now have 2 rules (1 overridden + 1 new).
    assert_eq!(detector.rules.len(), 2);
    assert_eq!(detector.rules[0].id, "md_header");
    assert!((detector.rules[0].weight - 0.9).abs() < f32::EPSILON);
    assert_eq!(detector.rules[0].source, RuleSource::UserDefined);
    assert_eq!(detector.rules[1].id, "md_custom");
}

#[test]
fn test_quick_match() {
    let detector = RegexDetectorBuilder::new("markdown", "Markdown")
        .rule(make_rule(
            "md_header",
            r"^#{1,6}\s",
            0.4,
            RuleScope::AnyLine,
            RuleStrength::Strong,
        ))
        .rule(make_rule(
            "md_bold",
            r"\*\*[^*]+\*\*",
            0.3,
            RuleScope::AnyLine,
            RuleStrength::Supporting,
        ))
        .build();

    // Strong rule matches → true.
    assert!(detector.quick_match(&["# Hello", "world"]));

    // Only Supporting rule matches → false.
    assert!(!detector.quick_match(&["This is **bold**"]));

    // No matches → false.
    assert!(!detector.quick_match(&["plain text"]));
}

#[test]
fn test_apply_overrides() {
    let mut detector = RegexDetectorBuilder::new("markdown", "Markdown")
        .rule(make_rule(
            "md_header",
            r"^#{1,6}\s",
            0.4,
            RuleScope::AnyLine,
            RuleStrength::Strong,
        ))
        .build();

    detector.apply_overrides(vec![
        RuleOverride {
            id: "md_header".to_string(),
            enabled: Some(false),
            weight: Some(0.9),
            scope: None,
        },
        // Unknown ID — should be silently ignored.
        RuleOverride {
            id: "nonexistent".to_string(),
            enabled: Some(true),
            weight: None,
            scope: None,
        },
    ]);

    assert!(!detector.rules[0].enabled);
    assert!((detector.rules[0].weight - 0.9).abs() < f32::EPSILON);
    // Scope was not overridden.
    assert_eq!(detector.rules[0].scope, RuleScope::AnyLine);
}

#[test]
fn test_builder_defaults() {
    let detector = RegexDetectorBuilder::new("test", "Test").build();

    assert_eq!(detector.format_id, "test");
    assert_eq!(detector.display_name, "Test");
    assert!(detector.rules.is_empty());
    assert!((detector.confidence_threshold - 0.6).abs() < f32::EPSILON);
    assert_eq!(detector.min_matching_rules, 1);
    assert!(detector.definitive_rule_shortcircuit);
}

#[test]
fn test_definitive_shortcircuit_disabled() {
    let detector = RegexDetectorBuilder::new("json", "JSON")
        .rule(make_rule(
            "json_brace",
            r"^\{",
            0.3,
            RuleScope::FirstLines(1),
            RuleStrength::Supporting,
        ))
        .rule(make_rule(
            "json_parse",
            r#"^\{[\s\S]*\}$"#,
            0.5,
            RuleScope::FullBlock,
            RuleStrength::Definitive,
        ))
        .definitive_rule_shortcircuit(false)
        .confidence_threshold(0.3)
        .build();

    let block = make_block_with_command(&["{", "  \"key\": \"value\"", "}"], None);
    let result = detector.detect(&block).unwrap();

    // Both rules match, confidence is summed (0.3 + 0.5 = 0.8), not short-circuited to 1.0.
    assert!((result.confidence - 0.8).abs() < f32::EPSILON);
    assert_eq!(result.matched_rules.len(), 2);
}
