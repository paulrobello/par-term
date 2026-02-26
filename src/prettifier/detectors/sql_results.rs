//! Built-in SQL result set detection rules.
//!
//! Creates a `RegexDetector` with 4 rules for identifying SQL result set output
//! from tools like psql, mysql, and sqlite3. MySQL-style table borders are
//! definitive; psql-style separators and row count footers provide supporting
//! evidence.

use regex::Regex;

use crate::config::prettifier::RenderersConfig;
use crate::prettifier::regex_detector::RegexDetectorBuilder;
use crate::prettifier::registry::RendererRegistry;
use crate::prettifier::types::{DetectionRule, RuleScope, RuleSource, RuleStrength};

/// Create the built-in SQL results detector with default regex rules.
///
/// Four rules:
/// - `sql_psql_separator`: psql-style `---+---` separator line (Strong)
/// - `sql_mysql_border`: mysql-style `+---+---+` border (Definitive)
/// - `sql_row_count`: Row count footer like `(5 rows)` (Supporting)
/// - `sql_command_context`: Preceding command is a SQL client (Supporting)
pub fn create_sql_results_detector() -> crate::prettifier::regex_detector::RegexDetector {
    RegexDetectorBuilder::new("sql_results", "SQL Results")
        .confidence_threshold(0.6)
        .min_matching_rules(2)
        .definitive_rule_shortcircuit(true)
        .rule(DetectionRule {
            id: "sql_psql_separator".into(),
            pattern: Regex::new(r"^-+\+-[-+]+$").expect("regex pattern is valid and should always compile"),
            weight: 0.4,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Strong,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "psql-style table separator (---+---)".into(),
            enabled: true,
        })
        .rule(DetectionRule {
            id: "sql_mysql_border".into(),
            pattern: Regex::new(r"^\+[-+]+\+$").expect("regex pattern is valid and should always compile"),
            weight: 0.6,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Definitive,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "MySQL-style table border (+---+---+)".into(),
            enabled: true,
        })
        .rule(DetectionRule {
            id: "sql_row_count".into(),
            pattern: Regex::new(r"^\(\d+ rows?\)$").expect("sql_row_count: pattern is valid and should always compile"),
            weight: 0.3,
            scope: RuleScope::LastLines(3),
            strength: RuleStrength::Supporting,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Row count footer (e.g., '(5 rows)')".into(),
            enabled: true,
        })
        .rule(DetectionRule {
            id: "sql_command_context".into(),
            pattern: Regex::new(r"(psql|mysql|sqlite3|pgcli|mycli)").expect("sql_command_context: pattern is valid and should always compile"),
            weight: 0.3,
            scope: RuleScope::PrecedingCommand,
            strength: RuleStrength::Supporting,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Preceding command is a SQL client".into(),
            enabled: true,
        })
        .build()
}

/// Register the SQL results detector with the registry.
pub fn register_sql_results(registry: &mut RendererRegistry, config: &RenderersConfig) {
    if config.sql_results.enabled {
        let detector = create_sql_results_detector();
        registry.register_detector(config.sql_results.priority, Box::new(detector));
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
        let detector = create_sql_results_detector();
        assert_eq!(detector.detection_rules().len(), 4);
    }

    #[test]
    fn test_mysql_style_results() {
        let detector = create_sql_results_detector();
        let block = make_block(
            &[
                "+----+-------+-----+",
                "| id | name  | age |",
                "+----+-------+-----+",
                "|  1 | Alice |  30 |",
                "|  2 | Bob   |  25 |",
                "+----+-------+-----+",
            ],
            None,
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
        let result = result.unwrap();
        assert!(
            result
                .matched_rules
                .contains(&"sql_mysql_border".to_string())
        );
    }

    #[test]
    fn test_psql_style_results() {
        let detector = create_sql_results_detector();
        let block = make_block(
            &[
                " id | name  | age",
                "----+-------+----",
                "  1 | Alice |  30",
                "  2 | Bob   |  25",
                "(2 rows)",
            ],
            None,
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
    }

    #[test]
    fn test_sql_with_command_context() {
        let detector = create_sql_results_detector();
        let block = make_block(
            &["----+-------+----", "  1 | Alice |  30"],
            Some("psql -d mydb -c 'SELECT * FROM users'"),
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
        let result = result.unwrap();
        assert!(
            result
                .matched_rules
                .contains(&"sql_command_context".to_string())
        );
    }

    #[test]
    fn test_not_sql_plain_text() {
        let detector = create_sql_results_detector();
        let block = make_block(&["Hello world", "This is plain text"], None);
        let result = detector.detect(&block);
        assert!(result.is_none());
    }

    #[test]
    fn test_quick_match_mysql() {
        let detector = create_sql_results_detector();
        assert!(detector.quick_match(&["+----+-------+", "| id | name  |"]));
    }

    #[test]
    fn test_quick_match_plain_text() {
        let detector = create_sql_results_detector();
        assert!(!detector.quick_match(&["just plain text"]));
    }

    #[test]
    fn test_registration_enabled() {
        let config = RenderersConfig::default();
        let mut registry = RendererRegistry::new(0.6);
        register_sql_results(&mut registry, &config);
        assert_eq!(registry.detector_count(), 1);
    }

    #[test]
    fn test_registration_disabled() {
        let mut config = RenderersConfig::default();
        config.sql_results.enabled = false;
        let mut registry = RendererRegistry::new(0.6);
        register_sql_results(&mut registry, &config);
        assert_eq!(registry.detector_count(), 0);
    }
}
