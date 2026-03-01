//! Per-renderer configuration types for the Content Prettifier system.
//!
//! Each renderer (Markdown, JSON, YAML, TOML, XML, CSV, Diff, Log, SQL, Stack Trace,
//! Diagrams) has its own config type here, along with profile-level override types.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Default value functions (renderer-specific)
// ---------------------------------------------------------------------------

pub(super) fn default_true() -> bool {
    true
}

pub(super) fn default_priority() -> i32 {
    50
}

pub(super) fn default_diagrams_priority() -> i32 {
    55
}

pub(super) fn default_rule_weight() -> f32 {
    0.3
}

pub(super) fn default_rule_scope() -> String {
    "any_line".to_string()
}

// ---------------------------------------------------------------------------
// Per-renderer config types
// ---------------------------------------------------------------------------

/// Enable/disable and priority for a renderer.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RendererToggle {
    /// Whether this renderer is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Priority (higher = checked first in detection).
    #[serde(default = "default_priority")]
    pub priority: i32,
}

impl Default for RendererToggle {
    fn default() -> Self {
        Self {
            enabled: true,
            priority: default_priority(),
        }
    }
}

/// Diff renderer with side-by-side option.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiffRendererConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,

    #[serde(default = "default_priority")]
    pub priority: i32,

    /// Display mode: "unified" or "side_by_side".
    #[serde(default)]
    pub display_mode: Option<String>,
}

impl Default for DiffRendererConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            priority: default_priority(),
            display_mode: None,
        }
    }
}

/// Diagram renderer with engine selection.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiagramRendererConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,

    #[serde(default = "default_diagrams_priority")]
    pub priority: i32,

    /// Rendering engine: "auto" (default — tries native → local → kroki),
    /// "native" (pure-Rust mermaid only), "local" (CLI tools), "kroki" (API),
    /// or "text_fallback" (source display only).
    #[serde(default)]
    pub engine: Option<String>,

    /// Kroki server URL (only used when engine = "kroki").
    #[serde(default)]
    pub kroki_server: Option<String>,
}

impl Default for DiagramRendererConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            priority: default_diagrams_priority(),
            engine: None,
            kroki_server: None,
        }
    }
}

/// Per-renderer enable/disable and priority settings.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RenderersConfig {
    #[serde(default)]
    pub markdown: RendererToggle,
    #[serde(default)]
    pub json: RendererToggle,
    #[serde(default)]
    pub yaml: RendererToggle,
    #[serde(default)]
    pub toml: RendererToggle,
    #[serde(default)]
    pub xml: RendererToggle,
    #[serde(default)]
    pub csv: RendererToggle,
    #[serde(default)]
    pub diff: DiffRendererConfig,
    #[serde(default)]
    pub log: RendererToggle,
    #[serde(default)]
    pub diagrams: DiagramRendererConfig,
    #[serde(default)]
    pub sql_results: RendererToggle,
    #[serde(default)]
    pub stack_trace: RendererToggle,
}

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

// ---------------------------------------------------------------------------
// Profile override types — every field is Option<T> so omitted values
// inherit from global config.
// ---------------------------------------------------------------------------

/// Profile-level override for per-renderer settings.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct RenderersConfigOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub markdown: Option<RendererToggleOverride>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub json: Option<RendererToggleOverride>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yaml: Option<RendererToggleOverride>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub toml: Option<RendererToggleOverride>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub xml: Option<RendererToggleOverride>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub csv: Option<RendererToggleOverride>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff: Option<RendererToggleOverride>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log: Option<RendererToggleOverride>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diagrams: Option<RendererToggleOverride>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sql_results: Option<RendererToggleOverride>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stack_trace: Option<RendererToggleOverride>,
}

/// Profile-level override for a single renderer's toggle.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct RendererToggleOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
}
