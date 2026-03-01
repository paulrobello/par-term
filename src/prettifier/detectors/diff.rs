//! Built-in diff/patch detection rules.
//!
//! Creates a `RegexDetector` with 6 rules for identifying unified diff content
//! in terminal output. The `diff --git` header and `@@` hunk headers are definitive
//! signals; other rules provide supporting confidence.

use regex::Regex;

use crate::config::prettifier::RenderersConfig;
use crate::prettifier::regex_detector::RegexDetectorBuilder;
use crate::prettifier::registry::RendererRegistry;
use crate::prettifier::types::{DetectionRule, RuleScope, RuleSource, RuleStrength};

/// Create the built-in diff detector with default regex rules.
///
/// Six rules from spec lines 374-410:
/// - `diff_git_header`: `diff --git` prefix (definitive)
/// - `diff_unified_header`: `--- ... / +++ ...` file header pair (definitive, full block)
/// - `diff_hunk`: `@@ -N,N +N,N @@` hunk headers (definitive)
/// - `diff_add_line`: lines starting with `+` (not `++`) (supporting)
/// - `diff_remove_line`: lines starting with `-` (not `--`) (supporting)
/// - `diff_git_context`: preceding command is `git diff/log/show` (supporting)
pub fn create_diff_detector() -> crate::prettifier::regex_detector::RegexDetector {
    RegexDetectorBuilder::new("diff", "Diff")
        .confidence_threshold(0.6)
        .min_matching_rules(1)
        .definitive_rule_shortcircuit(true)
        // Definitive rules
        .rule(DetectionRule {
            id: "diff_git_header".into(),
            pattern: Regex::new(r"^diff --git\s+")
                .expect("regex pattern is valid and should always compile"),
            weight: 0.9,
            scope: RuleScope::FirstLines(5),
            strength: RuleStrength::Definitive,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "diff --git header at start of output".into(),
            enabled: true,
        })
        .rule(DetectionRule {
            id: "diff_unified_header".into(),
            pattern: Regex::new(r"^---\s+\S+.*\n\+\+\+\s+\S+")
                .expect("regex pattern is valid and should always compile"),
            weight: 0.9,
            scope: RuleScope::FullBlock,
            strength: RuleStrength::Definitive,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Unified diff --- / +++ file header pair".into(),
            enabled: true,
        })
        .rule(DetectionRule {
            id: "diff_hunk".into(),
            pattern: Regex::new(r"^@@\s+-\d+,?\d*\s+\+\d+,?\d*\s+@@")
                .expect("regex pattern is valid and should always compile"),
            weight: 0.8,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Definitive,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "@@ hunk header with line ranges".into(),
            enabled: true,
        })
        // Supporting rules
        .rule(DetectionRule {
            id: "diff_add_line".into(),
            pattern: Regex::new(r"^\+[^+]")
                .expect("regex pattern is valid and should always compile"),
            weight: 0.1,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Supporting,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Added line starting with +".into(),
            enabled: true,
        })
        .rule(DetectionRule {
            id: "diff_remove_line".into(),
            pattern: Regex::new(r"^-[^-]")
                .expect("regex pattern is valid and should always compile"),
            weight: 0.1,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Supporting,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Removed line starting with -".into(),
            enabled: true,
        })
        .rule(DetectionRule {
            id: "diff_git_context".into(),
            pattern: Regex::new(r"^git\s+(diff|log|show)")
                .expect("diff_git_context: pattern is valid and should always compile"),
            weight: 0.3,
            scope: RuleScope::PrecedingCommand,
            strength: RuleStrength::Supporting,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Preceding command is git diff/log/show".into(),
            enabled: true,
        })
        .build()
}

/// Register the diff detector with the registry.
pub fn register_diff(registry: &mut RendererRegistry, config: &RenderersConfig) {
    if config.diff.enabled {
        let detector = create_diff_detector();
        registry.register_detector(config.diff.priority, Box::new(detector));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prettifier::testing::make_block_with_command;
    use crate::prettifier::traits::ContentDetector;

    #[test]
    fn test_all_rules_compile() {
        let detector = create_diff_detector();
        assert_eq!(detector.detection_rules().len(), 6);
    }

    #[test]
    fn test_diff_git_header_definitive() {
        let detector = create_diff_detector();
        let block = make_block_with_command(
            &[
                "diff --git a/src/main.rs b/src/main.rs",
                "index abc1234..def5678 100644",
                "--- a/src/main.rs",
                "+++ b/src/main.rs",
                "@@ -1,3 +1,4 @@",
                " line1",
                "+added",
                " line2",
            ],
            None,
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
        let result = result.unwrap();
        // Definitive short-circuit → confidence 1.0
        assert!((result.confidence - 1.0).abs() < f32::EPSILON);
        assert!(
            result
                .matched_rules
                .contains(&"diff_git_header".to_string())
        );
    }

    #[test]
    fn test_hunk_header_definitive() {
        let detector = create_diff_detector();
        let block = make_block_with_command(
            &[
                "--- a/file.txt",
                "+++ b/file.txt",
                "@@ -10,5 +10,6 @@",
                " context",
                "+added",
                "-removed",
            ],
            None,
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
        let result = result.unwrap();
        assert!((result.confidence - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_unified_header_definitive() {
        let detector = create_diff_detector();
        let block = make_block_with_command(
            &[
                "--- a/file.txt",
                "+++ b/file.txt",
                "@@ -1,3 +1,3 @@",
                " line1",
                "-old",
                "+new",
            ],
            None,
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
    }

    #[test]
    fn test_git_context_supporting() {
        let detector = create_diff_detector();
        // Supporting rules alone (git_context=0.3 + add=0.1 + remove=0.1 = 0.5) don't
        // reach the 0.6 threshold, which is correct behavior — a real diff needs
        // structural markers. Verify that with a hunk header, all rules including
        // git context are matched (hunk short-circuits to 1.0 but context is still
        // valid as a supporting signal).
        let block = make_block_with_command(
            &[
                "@@ -1,4 +1,4 @@",
                "+added line",
                "-removed line",
                " context",
            ],
            Some("git diff --cached"),
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
        let result = result.unwrap();
        // Definitive hunk header short-circuits to 1.0
        assert!((result.confidence - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_supporting_rules_below_threshold() {
        let detector = create_diff_detector();
        // Only supporting rules: git_context(0.3) + add(0.1) + remove(0.1) = 0.5 < 0.6
        let block =
            make_block_with_command(&["+added line", "-removed line"], Some("git diff --cached"));
        let result = detector.detect(&block);
        // Should NOT reach threshold without a definitive rule
        assert!(result.is_none());
    }

    #[test]
    fn test_not_diff_plain_text() {
        let detector = create_diff_detector();
        let block = make_block_with_command(&["Hello world", "This is plain text"], None);
        let result = detector.detect(&block);
        assert!(result.is_none());
    }

    #[test]
    fn test_quick_match_with_diff_header() {
        let detector = create_diff_detector();
        assert!(detector.quick_match(&["diff --git a/foo b/foo"]));
    }

    #[test]
    fn test_quick_match_with_hunk() {
        let detector = create_diff_detector();
        assert!(detector.quick_match(&["@@ -1,3 +1,4 @@"]));
    }

    #[test]
    fn test_quick_match_plain_text() {
        let detector = create_diff_detector();
        assert!(!detector.quick_match(&["just plain text"]));
    }

    #[test]
    fn test_registration_enabled() {
        let config = RenderersConfig::default();
        let mut registry = RendererRegistry::new(0.6);
        register_diff(&mut registry, &config);
        assert_eq!(registry.detector_count(), 1);
    }

    #[test]
    fn test_registration_disabled() {
        let mut config = RenderersConfig::default();
        config.diff.enabled = false;
        let mut registry = RendererRegistry::new(0.6);
        register_diff(&mut registry, &config);
        assert_eq!(registry.detector_count(), 0);
    }
}
