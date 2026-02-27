//! Smart selection rule types and defaults.

use serde::{Deserialize, Serialize};

// ============================================================================
// Smart Selection Types
// ============================================================================

/// Precision level for smart selection rules.
///
/// Higher precision rules are checked first and match more specific patterns.
/// Based on iTerm2's smart selection precision levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SmartSelectionPrecision {
    /// Very low precision (0.00001) - matches almost anything
    VeryLow,
    /// Low precision (0.001) - broad patterns
    Low,
    /// Normal precision (1.0) - standard patterns
    #[default]
    Normal,
    /// High precision (1000.0) - specific patterns
    High,
    /// Very high precision (1000000.0) - most specific patterns (checked first)
    VeryHigh,
}

impl SmartSelectionPrecision {
    /// Get the numeric precision value for sorting
    pub fn value(&self) -> f64 {
        match self {
            SmartSelectionPrecision::VeryLow => 0.00001,
            SmartSelectionPrecision::Low => 0.001,
            SmartSelectionPrecision::Normal => 1.0,
            SmartSelectionPrecision::High => 1000.0,
            SmartSelectionPrecision::VeryHigh => 1_000_000.0,
        }
    }

    /// Display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            SmartSelectionPrecision::VeryLow => "Very Low",
            SmartSelectionPrecision::Low => "Low",
            SmartSelectionPrecision::Normal => "Normal",
            SmartSelectionPrecision::High => "High",
            SmartSelectionPrecision::VeryHigh => "Very High",
        }
    }
}

/// A smart selection rule for pattern-based text selection.
///
/// When double-clicking, rules are evaluated by precision (highest first).
/// If a pattern matches at the cursor position, that text is selected.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartSelectionRule {
    /// Human-readable name for this rule (e.g., "HTTP URL", "Email address")
    pub name: String,
    /// Regular expression pattern to match
    pub regex: String,
    /// Precision level - higher precision rules are checked first
    #[serde(default)]
    pub precision: SmartSelectionPrecision,
    /// Whether this rule is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

impl SmartSelectionRule {
    /// Create a new smart selection rule
    pub fn new(
        name: impl Into<String>,
        regex: impl Into<String>,
        precision: SmartSelectionPrecision,
    ) -> Self {
        Self {
            name: name.into(),
            regex: regex.into(),
            precision,
            enabled: true,
        }
    }
}

/// Get the default smart selection rules (based on iTerm2's defaults)
pub fn default_smart_selection_rules() -> Vec<SmartSelectionRule> {
    vec![
        // Very High precision - most specific, checked first
        SmartSelectionRule::new(
            "HTTP URL",
            r"https?://[^\s<>\[\]{}|\\^`\x00-\x1f]+",
            SmartSelectionPrecision::VeryHigh,
        ),
        SmartSelectionRule::new(
            "SSH URL",
            r"\bssh://([a-zA-Z0-9_]+@)?([a-zA-Z0-9\-]+\.)*[a-zA-Z0-9\-]+(/[^\s]*)?",
            SmartSelectionPrecision::VeryHigh,
        ),
        SmartSelectionRule::new(
            "Git URL",
            r"\bgit://([a-zA-Z0-9_]+@)?([a-zA-Z0-9\-]+\.)*[a-zA-Z0-9\-]+(/[^\s]*)?",
            SmartSelectionPrecision::VeryHigh,
        ),
        SmartSelectionRule::new(
            "File URL",
            r"file://[^\s]+",
            SmartSelectionPrecision::VeryHigh,
        ),
        // High precision
        SmartSelectionRule::new(
            "Email address",
            r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b",
            SmartSelectionPrecision::High,
        ),
        SmartSelectionRule::new(
            "IPv4 address",
            r"\b(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\b",
            SmartSelectionPrecision::High,
        ),
        // Normal precision
        SmartSelectionRule::new(
            "File path",
            r"~?/?(?:[a-zA-Z0-9._-]+/)+[a-zA-Z0-9._-]+/?",
            SmartSelectionPrecision::Normal,
        ),
        SmartSelectionRule::new(
            "Java/Python import",
            // Require at least 2 dots to avoid matching simple filenames like "file.txt"
            r"(?:[a-zA-Z_][a-zA-Z0-9_]*\.){2,}[a-zA-Z_][a-zA-Z0-9_]*",
            SmartSelectionPrecision::Normal,
        ),
        SmartSelectionRule::new(
            "C++ namespace",
            r"(?:[a-zA-Z_][a-zA-Z0-9_]*::)+[a-zA-Z_][a-zA-Z0-9_]*",
            SmartSelectionPrecision::Normal,
        ),
        SmartSelectionRule::new(
            "Quoted string",
            r#""(?:[^"\\]|\\.)*""#,
            SmartSelectionPrecision::Normal,
        ),
        SmartSelectionRule::new(
            "UUID",
            r"\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\b",
            SmartSelectionPrecision::Normal,
        ),
        // Note: No "whitespace-bounded" catch-all pattern here - that would defeat
        // the purpose of configurable word_characters. If no smart pattern matches,
        // selection falls back to word boundary detection using word_characters.
    ]
}
