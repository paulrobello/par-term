//! Custom renderer and detection rule configuration types.

use serde::{Deserialize, Serialize};

use super::{default_priority, default_rule_scope, default_rule_weight, default_true};

/// A user-defined custom renderer definition.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustomRendererConfig {
    /// Unique ID for this custom renderer.
    pub id: String,

    /// Human-readable name.
    pub name: String,

    /// Detection regex patterns (at least one must match).
    #[serde(default)]
    pub detect_patterns: Vec<String>,

    /// Shell command to pipe content through for rendering.
    #[serde(default)]
    pub render_command: Option<String>,

    /// Arguments to pass to the render command.
    #[serde(default)]
    pub render_args: Vec<String>,

    /// Priority relative to built-in renderers.
    #[serde(default = "default_priority")]
    pub priority: i32,
}

/// User-defined detection rule overrides for a specific format.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct FormatDetectionRulesConfig {
    /// Additional user-defined rules.
    #[serde(default)]
    pub additional: Vec<UserDetectionRule>,

    /// Overrides for built-in rules (matched by rule ID).
    #[serde(default)]
    pub overrides: Vec<RuleOverride>,
}

/// A user-defined detection rule from config.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserDetectionRule {
    /// Rule identifier.
    pub id: String,

    /// Regex pattern.
    pub pattern: String,

    /// Confidence weight (0.0–1.0).
    #[serde(default = "default_rule_weight")]
    pub weight: f32,

    /// Scope: "any_line", "first_lines:N", "last_lines:N", "full_block", "preceding_command".
    #[serde(default = "default_rule_scope")]
    pub scope: String,

    /// Whether this rule is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Human-readable description.
    #[serde(default)]
    pub description: String,
}

/// Override settings for a built-in detection rule.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuleOverride {
    /// ID of the built-in rule to override.
    pub id: String,

    /// Override enabled state.
    #[serde(default)]
    pub enabled: Option<bool>,

    /// Override weight.
    #[serde(default)]
    pub weight: Option<f32>,
}
