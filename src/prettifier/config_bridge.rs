//! Bridges between YAML configuration types and runtime prettifier types.
//!
//! Converts `ResolvedPrettifierConfig` into the `PrettifierConfig` consumed by
//! `PrettifierPipeline`, and loads/merges detection rules from config.

use std::collections::HashMap;

use crate::config::prettifier::{
    FormatDetectionRulesConfig, ResolvedPrettifierConfig, RuleOverride, UserDetectionRule,
};

use super::boundary::DetectionScope;
use super::pipeline::PrettifierConfig;
use super::types::{DetectionRule, RuleScope, RuleSource, RuleStrength};

/// Convert a `ResolvedPrettifierConfig` into the runtime `PrettifierConfig`
/// consumed by `PrettifierPipeline`.
pub fn to_pipeline_config(resolved: &ResolvedPrettifierConfig) -> PrettifierConfig {
    PrettifierConfig {
        enabled: resolved.enabled,
        respect_alternate_screen: resolved.respect_alternate_screen,
        confidence_threshold: resolved.detection.confidence_threshold,
        max_scan_lines: resolved.detection.max_scan_lines,
        debounce_ms: resolved.detection.debounce_ms,
        detection_scope: parse_detection_scope(&resolved.detection.scope),
    }
}

/// Parse a scope string from config into the runtime enum.
fn parse_detection_scope(scope: &str) -> DetectionScope {
    match scope {
        "all" => DetectionScope::All,
        "manual_only" => DetectionScope::ManualOnly,
        _ => DetectionScope::CommandOutput, // default
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
fn parse_user_rule(user_rule: &UserDetectionRule) -> Option<DetectionRule> {
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
fn parse_rule_scope(scope: &str) -> RuleScope {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::prettifier::*;

    #[test]
    fn test_to_pipeline_config() {
        let resolved = ResolvedPrettifierConfig {
            enabled: true,
            respect_alternate_screen: false,
            global_toggle_key: "Ctrl+Shift+P".to_string(),
            per_block_toggle: true,
            detection: DetectionConfig {
                scope: "all".to_string(),
                confidence_threshold: 0.8,
                max_scan_lines: 200,
                debounce_ms: 50,
            },
            clipboard: ClipboardConfig::default(),
            renderers: RenderersConfig::default(),
            custom_renderers: Vec::new(),
            claude_code_integration: ClaudeCodeConfig::default(),
            detection_rules: HashMap::new(),
            cache: CacheConfig::default(),
        };

        let config = to_pipeline_config(&resolved);
        assert!(config.enabled);
        assert!(!config.respect_alternate_screen);
        assert!((config.confidence_threshold - 0.8).abs() < f32::EPSILON);
        assert_eq!(config.max_scan_lines, 200);
        assert_eq!(config.debounce_ms, 50);
        assert_eq!(config.detection_scope, DetectionScope::All);
    }

    #[test]
    fn test_parse_detection_scope() {
        assert_eq!(parse_detection_scope("all"), DetectionScope::All);
        assert_eq!(
            parse_detection_scope("command_output"),
            DetectionScope::CommandOutput
        );
        assert_eq!(
            parse_detection_scope("manual_only"),
            DetectionScope::ManualOnly
        );
        assert_eq!(
            parse_detection_scope("unknown"),
            DetectionScope::CommandOutput
        );
    }

    #[test]
    fn test_parse_rule_scope() {
        assert_eq!(parse_rule_scope("any_line"), RuleScope::AnyLine);
        assert_eq!(parse_rule_scope("full_block"), RuleScope::FullBlock);
        assert_eq!(
            parse_rule_scope("preceding_command"),
            RuleScope::PrecedingCommand
        );
        assert_eq!(
            parse_rule_scope("first_lines:10"),
            RuleScope::FirstLines(10)
        );
        assert_eq!(parse_rule_scope("last_lines:3"), RuleScope::LastLines(3));
        assert_eq!(parse_rule_scope("unknown"), RuleScope::AnyLine);
    }

    #[test]
    fn test_parse_user_rule_valid() {
        let user_rule = UserDetectionRule {
            id: "custom_md".to_string(),
            pattern: r"^#\s+".to_string(),
            weight: 0.5,
            scope: "first_lines:5".to_string(),
            enabled: true,
            description: "Match ATX headers".to_string(),
        };

        let rule = parse_user_rule(&user_rule).unwrap();
        assert_eq!(rule.id, "custom_md");
        assert!((rule.weight - 0.5).abs() < f32::EPSILON);
        assert_eq!(rule.scope, RuleScope::FirstLines(5));
        assert_eq!(rule.source, RuleSource::UserDefined);
        assert_eq!(rule.strength, RuleStrength::Supporting);
        assert!(rule.enabled);
    }

    #[test]
    fn test_parse_user_rule_invalid_regex() {
        let user_rule = UserDetectionRule {
            id: "bad".to_string(),
            pattern: r"[invalid".to_string(),
            weight: 0.5,
            scope: "any_line".to_string(),
            enabled: true,
            description: String::new(),
        };

        assert!(parse_user_rule(&user_rule).is_none());
    }

    #[test]
    fn test_load_detection_rules_empty() {
        let built_in: HashMap<String, Vec<DetectionRule>> = HashMap::new();
        let config_rules: HashMap<String, FormatDetectionRulesConfig> = HashMap::new();
        let result = load_detection_rules(built_in, &config_rules);
        assert!(result.is_empty());
    }

    #[test]
    fn test_load_detection_rules_adds_user_rules() {
        let built_in: HashMap<String, Vec<DetectionRule>> = HashMap::new();
        let mut config_rules = HashMap::new();
        config_rules.insert(
            "markdown".to_string(),
            FormatDetectionRulesConfig {
                additional: vec![UserDetectionRule {
                    id: "user_md1".to_string(),
                    pattern: r"^##\s+".to_string(),
                    weight: 0.4,
                    scope: "any_line".to_string(),
                    enabled: true,
                    description: "H2 headers".to_string(),
                }],
                overrides: vec![],
            },
        );

        let result = load_detection_rules(built_in, &config_rules);
        assert_eq!(result["markdown"].len(), 1);
        assert_eq!(result["markdown"][0].id, "user_md1");
    }

    #[test]
    fn test_load_detection_rules_applies_overrides() {
        let mut built_in: HashMap<String, Vec<DetectionRule>> = HashMap::new();
        built_in.insert(
            "json".to_string(),
            vec![DetectionRule {
                id: "json_brace".to_string(),
                pattern: regex::Regex::new(r"^\{").unwrap(),
                weight: 0.5,
                scope: RuleScope::FirstLines(1),
                strength: RuleStrength::Strong,
                source: RuleSource::BuiltIn,
                command_context: None,
                description: "Opens with brace".to_string(),
                enabled: true,
            }],
        );

        let mut config_rules = HashMap::new();
        config_rules.insert(
            "json".to_string(),
            FormatDetectionRulesConfig {
                additional: vec![],
                overrides: vec![RuleOverride {
                    id: "json_brace".to_string(),
                    enabled: Some(false),
                    weight: Some(0.8),
                }],
            },
        );

        let result = load_detection_rules(built_in, &config_rules);
        let json_rules = &result["json"];
        assert_eq!(json_rules.len(), 1);
        assert!(!json_rules[0].enabled);
        assert!((json_rules[0].weight - 0.8).abs() < f32::EPSILON);
    }
}
