//! Built-in log output detection rules.
//!
//! Creates a `RegexDetector` with 5 rules for identifying log output
//! in terminal output. Detection relies on timestamp patterns, log level
//! keywords, syslog format, and JSON structured log lines.

use regex::Regex;

use crate::config::prettifier::RenderersConfig;
use crate::prettifier::regex_detector::RegexDetectorBuilder;
use crate::prettifier::registry::RendererRegistry;
use crate::prettifier::types::{DetectionRule, RuleScope, RuleSource, RuleStrength};

/// Create the built-in log detector with default regex rules.
///
/// Five rules for identifying log output:
/// - `log_timestamp_level`: timestamp + log level on same line (strongest signal)
/// - `log_level_prefix`: log level keyword at start of line
/// - `log_iso_timestamp`: ISO 8601 timestamp at start of line
/// - `log_syslog`: syslog-format timestamp (e.g. `Jan 15 10:30:00`)
/// - `log_json_line`: JSON structured log line with known keys
pub fn create_log_detector() -> crate::prettifier::regex_detector::RegexDetector {
    RegexDetectorBuilder::new("log", "Log Output")
        .confidence_threshold(0.5)
        .min_matching_rules(2)
        .definitive_rule_shortcircuit(false)
        // Timestamp + log level is the strongest signal
        .rule(DetectionRule {
            id: "log_timestamp_level".into(),
            pattern: Regex::new(
                r"^\d{4}[-/]\d{2}[-/]\d{2}[T ]\d{2}:\d{2}:\d{2}.*?(TRACE|DEBUG|INFO|WARN|ERROR|FATAL)",
            )
            .expect("log_timestamp_level: pattern is valid and should always compile"),
            weight: 0.7,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Strong,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Timestamp followed by log level keyword".into(),
            enabled: true,
        })
        // Log level at start of line
        .rule(DetectionRule {
            id: "log_level_prefix".into(),
            pattern: Regex::new(r"^\s*\[?(TRACE|DEBUG|INFO|WARN|ERROR|FATAL)\]?\s").expect("log_level_prefix: pattern is valid and should always compile"),
            weight: 0.5,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Strong,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Log level keyword at start of line".into(),
            enabled: true,
        })
        // ISO timestamp
        .rule(DetectionRule {
            id: "log_iso_timestamp".into(),
            pattern: Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}").expect("regex pattern is valid and should always compile"),
            weight: 0.3,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Supporting,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "ISO 8601 timestamp at start of line".into(),
            enabled: true,
        })
        // Syslog format
        .rule(DetectionRule {
            id: "log_syslog".into(),
            pattern: Regex::new(
                r"^(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\s+\d+\s+\d{2}:\d{2}:\d{2}",
            )
            .expect("log_syslog: pattern is valid and should always compile"),
            weight: 0.4,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Strong,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Syslog-format timestamp (e.g. Jan 15 10:30:00)".into(),
            enabled: true,
        })
        // JSON log lines (structured logging)
        .rule(DetectionRule {
            id: "log_json_line".into(),
            pattern: Regex::new(r#"^\{"(timestamp|time|ts|level|msg|message)":"#).expect("log_json_line: pattern is valid and should always compile"),
            weight: 0.6,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Strong,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "JSON structured log line with known keys".into(),
            enabled: true,
        })
        .build()
}

/// Register the log detector with the registry.
pub fn register_log(registry: &mut RendererRegistry, config: &RenderersConfig) {
    if config.log.enabled {
        let detector = create_log_detector();
        registry.register_detector(config.log.priority, Box::new(detector));
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
        let detector = create_log_detector();
        assert_eq!(detector.detection_rules().len(), 5);
    }

    #[test]
    fn test_timestamp_with_level() {
        let detector = create_log_detector();
        let block = make_block(
            &[
                "2024-01-15T10:30:00Z INFO Starting server",
                "2024-01-15T10:30:01Z DEBUG Loaded config",
            ],
            None,
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
        let result = result.unwrap();
        assert!(result.confidence >= 0.5);
        assert!(
            result
                .matched_rules
                .contains(&"log_timestamp_level".to_string())
        );
    }

    #[test]
    fn test_log_level_prefix() {
        let detector = create_log_detector();
        // level_prefix(0.5) + iso_timestamp(0.3) >= 0.5, min_matching_rules=2
        let block = make_block(
            &[
                "2024-01-15T10:30:00Z Starting up...",
                "[INFO] Server started on port 8080",
                "[DEBUG] Loading configuration",
                "[ERROR] Connection refused",
            ],
            None,
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
    }

    #[test]
    fn test_iso_timestamp_with_level() {
        let detector = create_log_detector();
        // iso_timestamp(0.3) + timestamp_level(0.7) >= 0.5
        let block = make_block(
            &[
                "2024-01-15T10:30:00.123Z INFO request processed",
                "2024-01-15T10:30:01.456Z WARN slow query detected",
            ],
            None,
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
    }

    #[test]
    fn test_syslog_format() {
        let detector = create_log_detector();
        // syslog(0.4) + level_prefix(0.5) >= 0.5, min_matching_rules=2
        let block = make_block(
            &[
                "Jan 15 10:30:00 myhost sshd[1234]: connection from 10.0.0.1",
                "INFO Accepted publickey for user root",
                "Jan 15 10:30:01 myhost sshd[1234]: session opened",
            ],
            None,
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
    }

    #[test]
    fn test_json_structured_log() {
        let detector = create_log_detector();
        // json_line(0.6) + level_prefix(0.5) >= 0.5, min_matching_rules=2
        let block = make_block(
            &[
                r#"{"timestamp":"2024-01-15T10:30:00Z","level":"INFO","message":"started"}"#,
                "INFO Application initialized",
                r#"{"timestamp":"2024-01-15T10:30:01Z","level":"ERROR","message":"failed"}"#,
            ],
            None,
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
    }

    #[test]
    fn test_not_log_plain_text() {
        let detector = create_log_detector();
        let block = make_block(&["Hello world", "This is plain text"], None);
        let result = detector.detect(&block);
        assert!(result.is_none());
    }

    #[test]
    fn test_quick_match_with_timestamp_level() {
        let detector = create_log_detector();
        assert!(detector.quick_match(&["2024-01-15T10:30:00Z INFO Starting"]));
    }

    #[test]
    fn test_quick_match_plain_text() {
        let detector = create_log_detector();
        assert!(!detector.quick_match(&["just plain text"]));
    }

    #[test]
    fn test_registration_enabled() {
        let config = RenderersConfig::default();
        let mut registry = RendererRegistry::new(0.6);
        register_log(&mut registry, &config);
        assert_eq!(registry.detector_count(), 1);
    }

    #[test]
    fn test_registration_disabled() {
        let mut config = RenderersConfig::default();
        config.log.enabled = false;
        let mut registry = RendererRegistry::new(0.6);
        register_log(&mut registry, &config);
        assert_eq!(registry.detector_count(), 0);
    }
}
