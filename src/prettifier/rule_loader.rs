//! Detection rule loading and merging for the prettifier pipeline.
//!
//! Converts user-facing YAML rule definitions (`UserDetectionRule`, `RuleOverride`)
//! into runtime `DetectionRule` instances, applies overrides to built-in rules, and
//! loads format-specific rule sets into the `RendererRegistry`.
//!
//! Extracted from `config_bridge.rs` (R-43) to keep conversion/orchestration logic
//! separate from rule-loading detail.

use std::collections::HashMap;

use crate::config::prettifier::{FormatDetectionRulesConfig, RuleOverride, UserDetectionRule};

use super::boundary::DetectionScope;
use super::registry::RendererRegistry;
use super::types::{DetectionRule, RuleScope, RuleSource, RuleStrength};

/// Apply user detection rule overrides and additional rules to the registry.
pub(super) fn apply_detection_rules(
    registry: &mut RendererRegistry,
    config_rules: &HashMap<String, FormatDetectionRulesConfig>,
) {
    for (format_id, format_config) in config_rules {
        let additional: Vec<_> = format_config
            .additional
            .iter()
            .filter_map(parse_user_rule)
            .collect();
        registry.apply_rules_for_format(format_id, &format_config.overrides, additional);
    }
}

/// Parse a detection scope string from config into the runtime enum.
pub(super) fn parse_detection_scope(scope: &str) -> DetectionScope {
    match scope {
        "all" => DetectionScope::All,
        "command_output" => DetectionScope::CommandOutput,
        "manual_only" => DetectionScope::ManualOnly,
        _ => DetectionScope::All, // default matches declared default
    }
}

/// Load detection rules from config, merging built-in rules with user-defined ones.
///
/// For each format:
/// 1. Start with the provided `built_in_rules`.
/// 2. Apply any overrides from `config_rules` (enable/disable, weight changes).
/// 3. Append any additional user-defined rules from `config_rules`.
///
/// Returns a map from format_id to the merged rule set.
pub fn load_detection_rules(
    built_in_rules: HashMap<String, Vec<DetectionRule>>,
    config_rules: &HashMap<String, FormatDetectionRulesConfig>,
) -> HashMap<String, Vec<DetectionRule>> {
    let mut result = built_in_rules;

    for (format_id, format_rules) in config_rules {
        let rules = result.entry(format_id.clone()).or_default();

        // Apply overrides to existing built-in rules.
        for override_rule in &format_rules.overrides {
            apply_rule_override(rules, override_rule);
        }

        // Append additional user-defined rules.
        for user_rule in &format_rules.additional {
            if let Some(rule) = parse_user_rule(user_rule) {
                rules.push(rule);
            }
        }
    }

    result
}

/// Apply a rule override (enable/disable, weight change) to matching rules.
fn apply_rule_override(rules: &mut [DetectionRule], override_rule: &RuleOverride) {
    for rule in rules.iter_mut() {
        if rule.id == override_rule.id {
            if let Some(enabled) = override_rule.enabled {
                rule.enabled = enabled;
            }
            if let Some(weight) = override_rule.weight {
                rule.weight = weight;
            }
        }
    }
}

/// Parse a user-defined detection rule from config into a runtime `DetectionRule`.
///
/// Returns `None` if the regex pattern fails to compile.
pub(super) fn parse_user_rule(user_rule: &UserDetectionRule) -> Option<DetectionRule> {
    let pattern = regex::Regex::new(&user_rule.pattern).ok()?;
    let scope = parse_rule_scope(&user_rule.scope);

    Some(DetectionRule {
        id: user_rule.id.clone(),
        pattern,
        weight: user_rule.weight,
        scope,
        strength: RuleStrength::Supporting,
        source: RuleSource::UserDefined,
        command_context: None,
        description: user_rule.description.clone(),
        enabled: user_rule.enabled,
    })
}

/// Parse a rule scope string from config into the runtime enum.
pub(super) fn parse_rule_scope(scope: &str) -> RuleScope {
    if let Some(n_str) = scope.strip_prefix("first_lines:") {
        let n = n_str.parse().unwrap_or(5);
        return RuleScope::FirstLines(n);
    }
    if let Some(n_str) = scope.strip_prefix("last_lines:") {
        let n = n_str.parse().unwrap_or(3);
        return RuleScope::LastLines(n);
    }
    match scope {
        "full_block" => RuleScope::FullBlock,
        "preceding_command" => RuleScope::PrecedingCommand,
        _ => RuleScope::AnyLine, // default
    }
}
