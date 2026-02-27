//! Built-in stack trace detection rules.
//!
//! Creates a `RegexDetector` with 7 rules for identifying stack traces from
//! various languages/runtimes: Java/JVM, Python, Rust, Node.js/JavaScript,
//! Go, and generic error patterns.

use regex::Regex;

use crate::config::prettifier::RenderersConfig;
use crate::prettifier::regex_detector::RegexDetectorBuilder;
use crate::prettifier::registry::RendererRegistry;
use crate::prettifier::types::{DetectionRule, RuleScope, RuleSource, RuleStrength};

/// Create the built-in stack trace detector with default regex rules.
///
/// Seven rules covering multiple languages/runtimes:
/// - `stacktrace_java`: Java/JVM `at package.Class(File.java:N)` frames
/// - `stacktrace_python_header`: Python `Traceback (most recent call last):`
/// - `stacktrace_python_frame`: Python `File "...", line N` frames
/// - `stacktrace_rust_panic`: Rust `thread '...' panicked at` header
/// - `stacktrace_js`: Node.js/JavaScript `at Function (file:N:N)` frames
/// - `stacktrace_generic_error`: Generic `XxxError:` or `Exception:` headers
/// - `stacktrace_go_panic`: Go `goroutine N [...]` header
pub fn create_stack_trace_detector() -> crate::prettifier::regex_detector::RegexDetector {
    RegexDetectorBuilder::new("stack_trace", "Stack Trace")
        .confidence_threshold(0.6)
        .min_matching_rules(2)
        .definitive_rule_shortcircuit(true)
        // Java/JVM stack trace
        .rule(DetectionRule {
            id: "stacktrace_java".into(),
            pattern: Regex::new(r"^\s+at\s+[\w.$]+\([\w.]+:\d+\)")
                .expect("st_java_frame: pattern is valid and should always compile"),
            weight: 0.7,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Definitive,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Java/JVM stack frame: at package.Class(File.java:N)".into(),
            enabled: true,
        })
        // Python traceback header
        .rule(DetectionRule {
            id: "stacktrace_python_header".into(),
            pattern: Regex::new(r"^Traceback \(most recent call last\):")
                .expect("st_python_traceback: pattern is valid and should always compile"),
            weight: 0.9,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Definitive,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Python traceback header".into(),
            enabled: true,
        })
        // Python traceback frame
        .rule(DetectionRule {
            id: "stacktrace_python_frame".into(),
            pattern: Regex::new(r#"^\s+File ".*", line \d+"#)
                .expect("regex pattern is valid and should always compile"),
            weight: 0.6,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Strong,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: r#"Python stack frame: File "...", line N"#.into(),
            enabled: true,
        })
        // Rust panic
        .rule(DetectionRule {
            id: "stacktrace_rust_panic".into(),
            pattern: Regex::new(r"^thread '.*' panicked at")
                .expect("regex pattern is valid and should always compile"),
            weight: 0.9,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Definitive,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Rust panic header: thread '...' panicked at".into(),
            enabled: true,
        })
        // Node.js/JavaScript
        .rule(DetectionRule {
            id: "stacktrace_js".into(),
            pattern: Regex::new(r"^\s+at\s+\S+\s+\(.*:\d+:\d+\)")
                .expect("st_js_frame: pattern is valid and should always compile"),
            weight: 0.6,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Strong,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "JavaScript/Node.js stack frame: at Fn (file:N:N)".into(),
            enabled: true,
        })
        // Generic error header
        .rule(DetectionRule {
            id: "stacktrace_generic_error".into(),
            pattern: Regex::new(r"^(\w+Error|Exception|Caused by):")
                .expect("st_error_header: pattern is valid and should always compile"),
            weight: 0.4,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Strong,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Generic error/exception header (XxxError:, Caused by:)".into(),
            enabled: true,
        })
        // Go panic
        .rule(DetectionRule {
            id: "stacktrace_go_panic".into(),
            pattern: Regex::new(r"^goroutine \d+ \[")
                .expect("regex pattern is valid and should always compile"),
            weight: 0.8,
            scope: RuleScope::AnyLine,
            strength: RuleStrength::Definitive,
            source: RuleSource::BuiltIn,
            command_context: None,
            description: "Go panic header: goroutine N [status]".into(),
            enabled: true,
        })
        .build()
}

/// Register the stack trace detector with the registry.
pub fn register_stack_trace(registry: &mut RendererRegistry, config: &RenderersConfig) {
    if config.stack_trace.enabled {
        let detector = create_stack_trace_detector();
        registry.register_detector(config.stack_trace.priority, Box::new(detector));
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
        let detector = create_stack_trace_detector();
        assert_eq!(detector.detection_rules().len(), 7);
    }

    #[test]
    fn test_java_stack_trace() {
        let detector = create_stack_trace_detector();
        let block = make_block(
            &[
                "java.lang.NullPointerException: Cannot invoke method on null",
                "    at com.example.App.main(App.java:42)",
                "    at com.example.Runner.run(Runner.java:10)",
            ],
            None,
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
        let result = result.unwrap();
        // Java frame is definitive → 1.0
        assert!((result.confidence - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_python_traceback() {
        let detector = create_stack_trace_detector();
        let block = make_block(
            &[
                "Traceback (most recent call last):",
                "  File \"app.py\", line 42, in main",
                "    result = process(data)",
                "TypeError: unsupported operand type(s)",
            ],
            None,
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
        let result = result.unwrap();
        assert!((result.confidence - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_rust_panic() {
        let detector = create_stack_trace_detector();
        let block = make_block(
            &[
                "thread 'main' panicked at 'index out of bounds: the len is 3 but the index is 5'",
                "note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace",
            ],
            None,
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
        let result = result.unwrap();
        assert!((result.confidence - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_go_panic() {
        let detector = create_stack_trace_detector();
        let block = make_block(
            &[
                "goroutine 1 [running]:",
                "main.main()",
                "    /home/user/app/main.go:42 +0x1a2",
            ],
            None,
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
        let result = result.unwrap();
        assert!((result.confidence - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_javascript_stack_trace() {
        let detector = create_stack_trace_detector();
        let block = make_block(
            &[
                "TypeError: Cannot read property 'x' of undefined",
                "    at Object.main (/app/index.js:42:10)",
                "    at Module._compile (node:internal/modules/cjs/loader:1159:14)",
            ],
            None,
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
    }

    #[test]
    fn test_generic_error_with_frames() {
        let detector = create_stack_trace_detector();
        // generic_error(0.4) + js_frame(0.6) >= 0.6
        let block = make_block(
            &[
                "RangeError: Maximum call stack size exceeded",
                "    at recursive (/app/lib.js:10:5)",
            ],
            None,
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
    }

    #[test]
    fn test_not_stack_trace_plain_text() {
        let detector = create_stack_trace_detector();
        let block = make_block(&["Hello world", "This is plain text"], None);
        let result = detector.detect(&block);
        assert!(result.is_none());
    }

    #[test]
    fn test_quick_match_java() {
        let detector = create_stack_trace_detector();
        assert!(detector.quick_match(&["    at com.example.App.main(App.java:42)"]));
    }

    #[test]
    fn test_quick_match_python() {
        let detector = create_stack_trace_detector();
        assert!(detector.quick_match(&["Traceback (most recent call last):"]));
    }

    #[test]
    fn test_quick_match_plain_text() {
        let detector = create_stack_trace_detector();
        assert!(!detector.quick_match(&["just plain text"]));
    }

    #[test]
    fn test_registration_enabled() {
        let config = RenderersConfig::default();
        let mut registry = RendererRegistry::new(0.6);
        register_stack_trace(&mut registry, &config);
        assert_eq!(registry.detector_count(), 1);
    }

    #[test]
    fn test_registration_disabled() {
        let mut config = RenderersConfig::default();
        config.stack_trace.enabled = false;
        let mut registry = RendererRegistry::new(0.6);
        register_stack_trace(&mut registry, &config);
        assert_eq!(registry.detector_count(), 0);
    }

    #[test]
    fn test_caused_by_chain() {
        let detector = create_stack_trace_detector();
        let block = make_block(
            &[
                "Exception: Database connection failed",
                "Caused by: java.net.ConnectException: Connection refused",
                "    at java.net.Socket.connect(Socket.java:591)",
            ],
            None,
        );
        let result = detector.detect(&block);
        assert!(result.is_some());
    }
}
