//! Standard regex-based content detector with weighted confidence scoring.
//!
//! `RegexDetector` is the concrete implementation of `ContentDetector` that all
//! built-in format detectors use. It evaluates a set of `DetectionRule`s against
//! a `ContentBlock`, accumulates weighted confidence scores, and returns a
//! `DetectionResult` when thresholds are met.

use super::traits::ContentDetector;
use super::types::{
    ContentBlock, DetectionResult, DetectionRule, DetectionSource, RuleScope, RuleStrength,
};

/// Lightweight override for patching existing rules without replacing them.
///
/// Used by `RegexDetector::apply_overrides()` to let users toggle, reweight,
/// or rescope built-in rules from config without replacing the full rule.
#[derive(Debug)]
pub struct RuleOverride {
    /// The ID of the rule to override.
    pub id: String,
    /// If `Some`, override the rule's enabled state.
    pub enabled: Option<bool>,
    /// If `Some`, override the rule's weight.
    pub weight: Option<f32>,
    /// If `Some`, override the rule's scope.
    pub scope: Option<RuleScope>,
}

/// A regex-based content detector with weighted confidence scoring.
///
/// Evaluates `DetectionRule`s against content blocks and accumulates confidence
/// from matching rules. Supports definitive short-circuit, minimum rule counts,
/// and configurable confidence thresholds.
pub struct RegexDetector {
    /// Unique format identifier (e.g., "markdown", "json").
    format_id: String,
    /// Human-readable name for settings UI.
    display_name: String,
    /// The detection rules to evaluate.
    rules: Vec<DetectionRule>,
    /// Minimum confidence score (0.0–1.0) required to return a detection.
    confidence_threshold: f32,
    /// Minimum number of rules that must match before returning a detection.
    min_matching_rules: usize,
    /// If true, a Definitive rule match immediately returns confidence 1.0.
    definitive_rule_shortcircuit: bool,
}

impl RegexDetector {
    /// Merge user-defined rules into this detector.
    ///
    /// Rules with the same ID as existing rules override individual fields
    /// (pattern, weight, scope, strength, command_context, description, enabled).
    /// Rules with new IDs are appended.
    pub fn merge_user_rules(&mut self, user_rules: Vec<DetectionRule>) {
        for user_rule in user_rules {
            if let Some(existing) = self.rules.iter_mut().find(|r| r.id == user_rule.id) {
                // Override fields from the user rule into the existing rule.
                existing.pattern = user_rule.pattern;
                existing.weight = user_rule.weight;
                existing.scope = user_rule.scope;
                existing.strength = user_rule.strength;
                existing.command_context = user_rule.command_context;
                existing.description = user_rule.description;
                existing.enabled = user_rule.enabled;
                existing.source = user_rule.source;
            } else {
                self.rules.push(user_rule);
            }
        }
    }

    /// Apply lightweight overrides to existing rules.
    ///
    /// Only patches the fields that are `Some` in each override. Unknown rule
    /// IDs are silently ignored.
    pub fn apply_overrides(&mut self, overrides: Vec<RuleOverride>) {
        for ov in overrides {
            if let Some(rule) = self.rules.iter_mut().find(|r| r.id == ov.id) {
                if let Some(enabled) = ov.enabled {
                    rule.enabled = enabled;
                }
                if let Some(weight) = ov.weight {
                    rule.weight = weight;
                }
                if let Some(scope) = ov.scope {
                    rule.scope = scope;
                }
            }
        }
    }

    /// Extract the text to match against for a given rule scope.
    ///
    /// Returns `None` for `PrecedingCommand` scope when no command is available.
    fn text_for_scope<'a>(
        &self,
        content: &'a ContentBlock,
        scope: &RuleScope,
    ) -> Option<Vec<&'a str>> {
        match scope {
            RuleScope::AnyLine => Some(content.lines.iter().map(|s| s.as_str()).collect()),
            RuleScope::FirstLines(n) => Some(content.first_lines(*n)),
            RuleScope::LastLines(n) => Some(content.last_lines(*n)),
            RuleScope::FullBlock => None, // Handled specially — match against joined text.
            RuleScope::PrecedingCommand => content
                .preceding_command
                .as_ref()
                .map(|cmd| vec![cmd.as_str()]),
        }
    }

    /// Test a single rule against the content block.
    ///
    /// Returns `true` if the rule's pattern matches the extracted text.
    fn rule_matches(&self, rule: &DetectionRule, content: &ContentBlock) -> bool {
        match &rule.scope {
            RuleScope::FullBlock => {
                let full = content.full_text();
                rule.pattern.is_match(&full)
            }
            RuleScope::PrecedingCommand => match &content.preceding_command {
                Some(cmd) => rule.pattern.is_match(cmd),
                None => false,
            },
            scope => match self.text_for_scope(content, scope) {
                Some(lines) => lines.iter().any(|line| rule.pattern.is_match(line)),
                None => false,
            },
        }
    }
}

impl ContentDetector for RegexDetector {
    fn format_id(&self) -> &str {
        &self.format_id
    }

    fn display_name(&self) -> &str {
        &self.display_name
    }

    fn detect(&self, content: &ContentBlock) -> Option<DetectionResult> {
        let mut total_weight: f32 = 0.0;
        let mut match_count: usize = 0;
        let mut matched_rules: Vec<String> = Vec::new();

        for rule in &self.rules {
            // Skip disabled rules.
            if !rule.enabled {
                continue;
            }

            // Check command_context gate: if the rule requires a specific preceding
            // command, skip it when the command doesn't match.
            if let Some(ctx_pattern) = &rule.command_context {
                match &content.preceding_command {
                    Some(cmd) => {
                        if !ctx_pattern.is_match(cmd) {
                            continue;
                        }
                    }
                    None => continue,
                }
            }

            if self.rule_matches(rule, content) {
                total_weight += rule.weight;
                match_count += 1;
                matched_rules.push(rule.id.clone());

                // Definitive short-circuit: if enabled and a Definitive rule matches,
                // return immediately with confidence 1.0.
                if self.definitive_rule_shortcircuit && rule.strength == RuleStrength::Definitive {
                    return Some(DetectionResult {
                        format_id: self.format_id.clone(),
                        confidence: 1.0,
                        matched_rules: vec![rule.id.clone()],
                        source: DetectionSource::AutoDetected,
                    });
                }
            }
        }

        // Check thresholds.
        if match_count < self.min_matching_rules {
            return None;
        }

        let confidence = total_weight.min(1.0);
        if confidence < self.confidence_threshold {
            return None;
        }

        Some(DetectionResult {
            format_id: self.format_id.clone(),
            confidence,
            matched_rules,
            source: DetectionSource::AutoDetected,
        })
    }

    fn quick_match(&self, first_lines: &[&str]) -> bool {
        // Only check Strong/Definitive rules with AnyLine or FirstLines scope,
        // tested against at most the first 5 lines.
        let max_lines = 5;
        let lines: Vec<&str> = first_lines.iter().take(max_lines).copied().collect();

        for rule in &self.rules {
            if !rule.enabled {
                continue;
            }

            // Only Strong or Definitive rules qualify for quick_match.
            match rule.strength {
                RuleStrength::Strong | RuleStrength::Definitive => {}
                RuleStrength::Supporting => continue,
            }

            // Only AnyLine or FirstLines scopes qualify for quick_match.
            match &rule.scope {
                RuleScope::AnyLine | RuleScope::FirstLines(_) => {}
                _ => continue,
            }

            if lines.iter().any(|line| rule.pattern.is_match(line)) {
                return true;
            }
        }

        false
    }

    fn detection_rules(&self) -> &[DetectionRule] {
        &self.rules
    }
}

/// Builder for constructing `RegexDetector` instances with sensible defaults.
pub struct RegexDetectorBuilder {
    format_id: String,
    display_name: String,
    rules: Vec<DetectionRule>,
    confidence_threshold: f32,
    min_matching_rules: usize,
    definitive_rule_shortcircuit: bool,
}

impl RegexDetectorBuilder {
    /// Create a new builder with the given format ID and display name.
    pub fn new(format_id: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self {
            format_id: format_id.into(),
            display_name: display_name.into(),
            rules: Vec::new(),
            confidence_threshold: 0.6,
            min_matching_rules: 1,
            definitive_rule_shortcircuit: true,
        }
    }

    /// Add a detection rule.
    pub fn rule(mut self, rule: DetectionRule) -> Self {
        self.rules.push(rule);
        self
    }

    /// Add multiple detection rules.
    pub fn rules(mut self, rules: Vec<DetectionRule>) -> Self {
        self.rules.extend(rules);
        self
    }

    /// Set the minimum confidence threshold (default: 0.6).
    pub fn confidence_threshold(mut self, threshold: f32) -> Self {
        self.confidence_threshold = threshold;
        self
    }

    /// Set the minimum number of matching rules (default: 1).
    pub fn min_matching_rules(mut self, min: usize) -> Self {
        self.min_matching_rules = min;
        self
    }

    /// Set whether definitive rules short-circuit detection (default: true).
    pub fn definitive_rule_shortcircuit(mut self, enabled: bool) -> Self {
        self.definitive_rule_shortcircuit = enabled;
        self
    }

    /// Build the `RegexDetector`.
    pub fn build(self) -> RegexDetector {
        RegexDetector {
            format_id: self.format_id,
            display_name: self.display_name,
            rules: self.rules,
            confidence_threshold: self.confidence_threshold,
            min_matching_rules: self.min_matching_rules,
            definitive_rule_shortcircuit: self.definitive_rule_shortcircuit,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prettifier::types::{RuleSource, RuleStrength};
    use std::time::SystemTime;

    /// Helper: create a `ContentBlock` from lines with optional preceding command.
    fn make_block(lines: &[&str], command: Option<&str>) -> ContentBlock {
        ContentBlock {
            lines: lines.iter().map(|s| s.to_string()).collect(),
            preceding_command: command.map(|s| s.to_string()),
            start_row: 0,
            end_row: lines.len(),
            timestamp: SystemTime::now(),
        }
    }

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

        let block = make_block(&["# Hello", "This is **bold** text"], None);
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

        let block = make_block(&["This is **bold** text"], None);
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

        let block = make_block(&["{", "  \"key\": \"value\"", "}"], None);
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
        let block = make_block(&["# Hello", "Plain text"], None);
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

        let block_match = make_block(&["HEADER", "line2", "line3"], None);
        assert!(detector.detect(&block_match).is_some());

        let block_no_match = make_block(&["line1", "line2", "HEADER"], None);
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

        let block_match = make_block(&["line1", "line2", "FOOTER"], None);
        assert!(detector.detect(&block_match).is_some());

        let block_no_match = make_block(&["FOOTER", "line2", "line3"], None);
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

        let block_match = make_block(&["line1", "line2", "line3"], None);
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

        let block_match = make_block(&["diff output"], Some("git diff --cached"));
        assert!(detector.detect(&block_match).is_some());

        let block_no_cmd = make_block(&["diff output"], None);
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
        let block_match = make_block(&["diff --git a/foo b/foo"], Some("git diff"));
        assert!(detector.detect(&block_match).is_some());

        // Without matching command context.
        let block_wrong_cmd = make_block(&["diff --git a/foo b/foo"], Some("svn diff"));
        assert!(detector.detect(&block_wrong_cmd).is_none());

        // Without any command.
        let block_no_cmd = make_block(&["diff --git a/foo b/foo"], None);
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

        let block = make_block(&["# Hello"], None);
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

        let block = make_block(&["{", "  \"key\": \"value\"", "}"], None);
        let result = detector.detect(&block).unwrap();

        // Both rules match, confidence is summed (0.3 + 0.5 = 0.8), not short-circuited to 1.0.
        assert!((result.confidence - 0.8).abs() < f32::EPSILON);
        assert_eq!(result.matched_rules.len(), 2);
    }
}
