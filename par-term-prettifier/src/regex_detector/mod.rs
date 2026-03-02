//! Standard regex-based content detector with weighted confidence scoring.
//!
//! `RegexDetector` is the concrete implementation of `ContentDetector` that all
//! built-in format detectors use. It evaluates a set of `DetectionRule`s against
//! a `ContentBlock`, accumulates weighted confidence scores, and returns a
//! `DetectionResult` when thresholds are met.
//!
//! Inline tests extracted to `tests.rs` (R-41).

#[cfg(test)]
mod tests;

use super::traits::{ConfigurableDetector, ContentDetector};
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
    pub(super) format_id: String,
    /// Human-readable name for settings UI.
    pub(super) display_name: String,
    /// The detection rules to evaluate.
    pub(super) rules: Vec<DetectionRule>,
    /// Minimum confidence score (0.0–1.0) required to return a detection.
    pub(super) confidence_threshold: f32,
    /// Minimum number of rules that must match before returning a detection.
    pub(super) min_matching_rules: usize,
    /// If true, a Definitive rule match immediately returns confidence 1.0.
    pub(super) definitive_rule_shortcircuit: bool,
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
    fn rule_matches(
        &self,
        rule: &DetectionRule,
        content: &ContentBlock,
        cached_full_text: &str,
    ) -> bool {
        match &rule.scope {
            RuleScope::FullBlock => rule.pattern.is_match(cached_full_text),
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

        // Pre-compute full_text once for all FullBlock rule checks.
        let full_text = content.full_text();

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

            if self.rule_matches(rule, content, &full_text) {
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
            crate::debug_log!(
                "PRETTIFIER",
                "detect {}: 0 rules matched (need {}), lines={}",
                self.format_id,
                self.min_matching_rules,
                content.lines.len()
            );
            return None;
        }

        let confidence = total_weight.min(1.0);
        if confidence < self.confidence_threshold {
            // Log all rule results (matched and missed) for diagnosis
            let mut rule_detail = String::new();
            for rule in &self.rules {
                if !rule.enabled {
                    continue;
                }
                let hit = matched_rules.contains(&rule.id);
                rule_detail.push_str(&format!(
                    " {}({:.2})={}",
                    rule.id,
                    rule.weight,
                    if hit { "HIT" } else { "miss" }
                ));
            }
            crate::debug_log!(
                "PRETTIFIER",
                "detect {}: conf={:.2} < thresh={:.2}, rules:{}",
                self.format_id,
                confidence,
                self.confidence_threshold,
                rule_detail
            );
            return None;
        }

        crate::debug_info!(
            "PRETTIFIER",
            "detect {} PASS: conf={:.2}, matched=[{}]",
            self.format_id,
            confidence,
            matched_rules.join(", ")
        );

        Some(DetectionResult {
            format_id: self.format_id.clone(),
            confidence,
            matched_rules,
            source: DetectionSource::AutoDetected,
        })
    }

    fn quick_match(&self, first_lines: &[&str]) -> bool {
        // Check Strong/Definitive rules with AnyLine or FirstLines scope
        // against the first N lines. We sample up to 30 lines because
        // some content (e.g. Claude Code output) has preamble before
        // the actual structured content begins.
        let max_lines = 30;
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

    fn as_configurable_mut(&mut self) -> Option<&mut dyn ConfigurableDetector> {
        Some(self)
    }
}

impl ConfigurableDetector for RegexDetector {
    fn apply_config_overrides(&mut self, overrides: &[crate::config::prettifier::RuleOverride]) {
        // Convert config overrides (no scope field) to detector overrides and delegate to
        // the canonical apply_overrides implementation to keep both paths in sync.
        let converted: Vec<RuleOverride> = overrides
            .iter()
            .map(|ov| RuleOverride {
                id: ov.id.clone(),
                enabled: ov.enabled,
                weight: ov.weight,
                scope: None,
            })
            .collect();
        self.apply_overrides(converted);
    }

    fn merge_config_rules(&mut self, rules: Vec<DetectionRule>) {
        self.merge_user_rules(rules);
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
