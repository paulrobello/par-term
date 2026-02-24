//! Configuration structures for the Content Prettifier system.
//!
//! Maps to the `content_prettifier:` section in `config.yaml`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Default value functions
// ---------------------------------------------------------------------------

fn default_true() -> bool {
    true
}

fn default_global_toggle_key() -> String {
    "Ctrl+Shift+P".to_string()
}

fn default_detection_scope() -> String {
    "all".to_string()
}

fn default_confidence_threshold() -> f32 {
    0.6
}

fn default_max_scan_lines() -> usize {
    500
}

fn default_debounce_ms() -> u64 {
    100
}

fn default_clipboard_copy() -> String {
    "rendered".to_string()
}

fn default_source_copy_modifier() -> String {
    "Alt".to_string()
}

fn default_vi_copy_mode() -> String {
    "source".to_string()
}

fn default_priority() -> i32 {
    50
}

fn default_cache_max_entries() -> usize {
    64
}

// ---------------------------------------------------------------------------
// Top-level prettifier config
// ---------------------------------------------------------------------------

/// Top-level prettifier configuration (lives under `content_prettifier:` in config.yaml).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrettifierYamlConfig {
    /// Whether to respect alternate-screen transitions as boundaries.
    #[serde(default = "default_true")]
    pub respect_alternate_screen: bool,

    /// Key binding for the global prettifier toggle.
    #[serde(default = "default_global_toggle_key")]
    pub global_toggle_key: String,

    /// Whether per-block source/rendered toggling is enabled.
    #[serde(default = "default_true")]
    pub per_block_toggle: bool,

    /// Detection settings.
    #[serde(default)]
    pub detection: DetectionConfig,

    /// Clipboard behavior settings.
    #[serde(default)]
    pub clipboard: ClipboardConfig,

    /// Per-renderer enable/disable and priority.
    #[serde(default)]
    pub renderers: RenderersConfig,

    /// User-defined custom renderer configurations.
    #[serde(default)]
    pub custom_renderers: Vec<CustomRendererConfig>,

    /// Claude Code integration settings.
    #[serde(default)]
    pub claude_code_integration: ClaudeCodeConfig,

    /// User-defined detection rule overrides, keyed by format ID.
    #[serde(default)]
    pub detection_rules: HashMap<String, FormatDetectionRulesConfig>,

    /// Render cache settings.
    #[serde(default)]
    pub cache: CacheConfig,
}

impl Default for PrettifierYamlConfig {
    fn default() -> Self {
        Self {
            respect_alternate_screen: true,
            global_toggle_key: default_global_toggle_key(),
            per_block_toggle: true,
            detection: DetectionConfig::default(),
            clipboard: ClipboardConfig::default(),
            renderers: RenderersConfig::default(),
            custom_renderers: Vec::new(),
            claude_code_integration: ClaudeCodeConfig::default(),
            detection_rules: HashMap::new(),
            cache: CacheConfig::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Sub-configs
// ---------------------------------------------------------------------------

/// Detection pipeline settings.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DetectionConfig {
    /// When to detect: "command_output", "all", or "manual_only".
    #[serde(default = "default_detection_scope")]
    pub scope: String,

    /// Minimum confidence score (0.0–1.0) for auto-detection.
    #[serde(default = "default_confidence_threshold")]
    pub confidence_threshold: f32,

    /// Maximum lines to scan before forcing emission.
    #[serde(default = "default_max_scan_lines")]
    pub max_scan_lines: usize,

    /// Milliseconds of inactivity before emitting a block.
    #[serde(default = "default_debounce_ms")]
    pub debounce_ms: u64,
}

impl Default for DetectionConfig {
    fn default() -> Self {
        Self {
            scope: default_detection_scope(),
            confidence_threshold: default_confidence_threshold(),
            max_scan_lines: default_max_scan_lines(),
            debounce_ms: default_debounce_ms(),
        }
    }
}

/// Clipboard behavior for prettified content.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClipboardConfig {
    /// What to copy by default: "rendered" or "source".
    #[serde(default = "default_clipboard_copy")]
    pub default_copy: String,

    /// Modifier key to copy the alternative form.
    #[serde(default = "default_source_copy_modifier")]
    pub source_copy_modifier: String,

    /// What to copy in vi copy mode: "source" or "rendered".
    #[serde(default = "default_vi_copy_mode")]
    pub vi_copy_mode: String,
}

impl Default for ClipboardConfig {
    fn default() -> Self {
        Self {
            default_copy: default_clipboard_copy(),
            source_copy_modifier: default_source_copy_modifier(),
            vi_copy_mode: default_vi_copy_mode(),
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

    #[serde(default = "default_priority")]
    pub priority: i32,

    /// Rendering engine: "kroki", "mermaid_cli", or "text_fallback".
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
            priority: default_priority(),
            engine: None,
            kroki_server: None,
        }
    }
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

    /// Priority relative to built-in renderers.
    #[serde(default = "default_priority")]
    pub priority: i32,
}

/// Claude Code integration settings.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClaudeCodeConfig {
    /// Auto-detect Claude Code sessions.
    #[serde(default = "default_true")]
    pub auto_detect: bool,

    /// Enable markdown rendering in Claude Code output.
    #[serde(default = "default_true")]
    pub render_markdown: bool,

    /// Enable diff rendering in Claude Code output.
    #[serde(default = "default_true")]
    pub render_diffs: bool,

    /// Automatically render content when a collapsed block is expanded (Ctrl+O).
    #[serde(default = "default_true")]
    pub auto_render_on_expand: bool,

    /// Show format badges on collapsed blocks (e.g., "MD", "{} JSON").
    #[serde(default = "default_true")]
    pub show_format_badges: bool,
}

impl Default for ClaudeCodeConfig {
    fn default() -> Self {
        Self {
            auto_detect: true,
            render_markdown: true,
            render_diffs: true,
            auto_render_on_expand: true,
            show_format_badges: true,
        }
    }
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

fn default_rule_weight() -> f32 {
    0.3
}

fn default_rule_scope() -> String {
    "any_line".to_string()
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

/// Render cache settings.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Maximum number of entries in the render cache.
    #[serde(default = "default_cache_max_entries")]
    pub max_entries: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_entries: default_cache_max_entries(),
        }
    }
}

// ---------------------------------------------------------------------------
// Profile override types — every field is Option<T> so omitted values
// inherit from global config.
// ---------------------------------------------------------------------------

/// Profile-level override for prettifier configuration.
/// All fields are optional; `None` means "inherit from global".
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct PrettifierConfigOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub respect_alternate_screen: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub per_block_toggle: Option<bool>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detection: Option<DetectionConfigOverride>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub renderers: Option<RenderersConfigOverride>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claude_code_integration: Option<ClaudeCodeConfigOverride>,
}

/// Profile-level override for detection settings.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct DetectionConfigOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence_threshold: Option<f32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_scan_lines: Option<usize>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debounce_ms: Option<u64>,
}

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

/// Profile-level override for Claude Code integration.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ClaudeCodeConfigOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_detect: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub render_markdown: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub render_diffs: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_render_on_expand: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show_format_badges: Option<bool>,
}

// ---------------------------------------------------------------------------
// Resolved config — the final merged result from global + profile.
// ---------------------------------------------------------------------------

/// Fully resolved prettifier config after merging global + profile overrides.
#[derive(Clone, Debug)]
pub struct ResolvedPrettifierConfig {
    pub enabled: bool,
    pub respect_alternate_screen: bool,
    pub global_toggle_key: String,
    pub per_block_toggle: bool,
    pub detection: DetectionConfig,
    pub clipboard: ClipboardConfig,
    pub renderers: RenderersConfig,
    pub custom_renderers: Vec<CustomRendererConfig>,
    pub claude_code_integration: ClaudeCodeConfig,
    pub detection_rules: HashMap<String, FormatDetectionRulesConfig>,
    pub cache: CacheConfig,
}

/// Resolve effective prettifier config by merging global defaults with profile overrides.
///
/// Precedence (highest to lowest):
/// 1. Profile-level setting (if present)
/// 2. Global config-level setting
/// 3. Built-in default
pub fn resolve_prettifier_config(
    global_enabled: bool,
    global_config: &PrettifierYamlConfig,
    profile_enabled: Option<bool>,
    profile_config: Option<&PrettifierConfigOverride>,
) -> ResolvedPrettifierConfig {
    let enabled = profile_enabled.unwrap_or(global_enabled);

    let (detection, renderers, claude_code_integration, respect_alternate_screen, per_block_toggle) =
        if let Some(overrides) = profile_config {
            let detection = merge_detection(&global_config.detection, overrides.detection.as_ref());
            let renderers = merge_renderers(&global_config.renderers, overrides.renderers.as_ref());
            let claude = merge_claude_code(
                &global_config.claude_code_integration,
                overrides.claude_code_integration.as_ref(),
            );
            let respect_alt = overrides
                .respect_alternate_screen
                .unwrap_or(global_config.respect_alternate_screen);
            let per_block = overrides
                .per_block_toggle
                .unwrap_or(global_config.per_block_toggle);
            (detection, renderers, claude, respect_alt, per_block)
        } else {
            (
                global_config.detection.clone(),
                global_config.renderers.clone(),
                global_config.claude_code_integration.clone(),
                global_config.respect_alternate_screen,
                global_config.per_block_toggle,
            )
        };

    ResolvedPrettifierConfig {
        enabled,
        respect_alternate_screen,
        global_toggle_key: global_config.global_toggle_key.clone(),
        per_block_toggle,
        detection,
        clipboard: global_config.clipboard.clone(),
        renderers,
        custom_renderers: global_config.custom_renderers.clone(),
        claude_code_integration,
        detection_rules: global_config.detection_rules.clone(),
        cache: global_config.cache.clone(),
    }
}

fn merge_detection(
    global: &DetectionConfig,
    profile: Option<&DetectionConfigOverride>,
) -> DetectionConfig {
    let Some(p) = profile else {
        return global.clone();
    };
    DetectionConfig {
        scope: p.scope.clone().unwrap_or_else(|| global.scope.clone()),
        confidence_threshold: p
            .confidence_threshold
            .unwrap_or(global.confidence_threshold),
        max_scan_lines: p.max_scan_lines.unwrap_or(global.max_scan_lines),
        debounce_ms: p.debounce_ms.unwrap_or(global.debounce_ms),
    }
}

fn merge_renderers(
    global: &RenderersConfig,
    profile: Option<&RenderersConfigOverride>,
) -> RenderersConfig {
    let Some(p) = profile else {
        return global.clone();
    };

    RenderersConfig {
        markdown: merge_toggle(&global.markdown, p.markdown.as_ref()),
        json: merge_toggle(&global.json, p.json.as_ref()),
        yaml: merge_toggle(&global.yaml, p.yaml.as_ref()),
        toml: merge_toggle(&global.toml, p.toml.as_ref()),
        xml: merge_toggle(&global.xml, p.xml.as_ref()),
        csv: merge_toggle(&global.csv, p.csv.as_ref()),
        diff: DiffRendererConfig {
            enabled: p
                .diff
                .as_ref()
                .and_then(|d| d.enabled)
                .unwrap_or(global.diff.enabled),
            priority: p
                .diff
                .as_ref()
                .and_then(|d| d.priority)
                .unwrap_or(global.diff.priority),
            display_mode: global.diff.display_mode.clone(),
        },
        log: merge_toggle(&global.log, p.log.as_ref()),
        diagrams: DiagramRendererConfig {
            enabled: p
                .diagrams
                .as_ref()
                .and_then(|d| d.enabled)
                .unwrap_or(global.diagrams.enabled),
            priority: p
                .diagrams
                .as_ref()
                .and_then(|d| d.priority)
                .unwrap_or(global.diagrams.priority),
            engine: global.diagrams.engine.clone(),
            kroki_server: global.diagrams.kroki_server.clone(),
        },
        sql_results: merge_toggle(&global.sql_results, p.sql_results.as_ref()),
        stack_trace: merge_toggle(&global.stack_trace, p.stack_trace.as_ref()),
    }
}

fn merge_toggle(
    global: &RendererToggle,
    profile: Option<&RendererToggleOverride>,
) -> RendererToggle {
    let Some(p) = profile else {
        return global.clone();
    };
    RendererToggle {
        enabled: p.enabled.unwrap_or(global.enabled),
        priority: p.priority.unwrap_or(global.priority),
    }
}

fn merge_claude_code(
    global: &ClaudeCodeConfig,
    profile: Option<&ClaudeCodeConfigOverride>,
) -> ClaudeCodeConfig {
    let Some(p) = profile else {
        return global.clone();
    };
    ClaudeCodeConfig {
        auto_detect: p.auto_detect.unwrap_or(global.auto_detect),
        render_markdown: p.render_markdown.unwrap_or(global.render_markdown),
        render_diffs: p.render_diffs.unwrap_or(global.render_diffs),
        auto_render_on_expand: p
            .auto_render_on_expand
            .unwrap_or(global.auto_render_on_expand),
        show_format_badges: p.show_format_badges.unwrap_or(global.show_format_badges),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prettifier_yaml_config_defaults() {
        let config = PrettifierYamlConfig::default();
        assert!(config.respect_alternate_screen);
        assert!(config.per_block_toggle);
        assert_eq!(config.global_toggle_key, "Ctrl+Shift+P");
        assert!(config.custom_renderers.is_empty());
        assert!(config.detection_rules.is_empty());
    }

    #[test]
    fn test_detection_config_defaults() {
        let config = DetectionConfig::default();
        assert_eq!(config.scope, "all");
        assert!((config.confidence_threshold - 0.6).abs() < f32::EPSILON);
        assert_eq!(config.max_scan_lines, 500);
        assert_eq!(config.debounce_ms, 100);
    }

    #[test]
    fn test_renderer_toggle_defaults() {
        let toggle = RendererToggle::default();
        assert!(toggle.enabled);
        assert_eq!(toggle.priority, 50);
    }

    #[test]
    fn test_renderers_config_defaults() {
        let config = RenderersConfig::default();
        assert!(config.markdown.enabled);
        assert!(config.json.enabled);
        assert!(config.diff.enabled);
        assert!(config.diagrams.enabled);
    }

    #[test]
    fn test_clipboard_config_defaults() {
        let config = ClipboardConfig::default();
        assert_eq!(config.default_copy, "rendered");
        assert_eq!(config.source_copy_modifier, "Alt");
        assert_eq!(config.vi_copy_mode, "source");
    }

    #[test]
    fn test_claude_code_config_defaults() {
        let config = ClaudeCodeConfig::default();
        assert!(config.auto_detect);
        assert!(config.render_markdown);
        assert!(config.render_diffs);
        assert!(config.auto_render_on_expand);
        assert!(config.show_format_badges);
    }

    #[test]
    fn test_cache_config_defaults() {
        let config = CacheConfig::default();
        assert_eq!(config.max_entries, 64);
    }

    #[test]
    fn test_yaml_deserialization_empty() {
        let yaml = "{}";
        let config: PrettifierYamlConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.respect_alternate_screen);
        assert_eq!(config.detection.scope, "all");
    }

    #[test]
    fn test_yaml_deserialization_partial() {
        let yaml = r#"
detection:
  scope: "all"
  confidence_threshold: 0.8
renderers:
  markdown:
    enabled: false
  json:
    priority: 100
"#;
        let config: PrettifierYamlConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.detection.scope, "all");
        assert!((config.detection.confidence_threshold - 0.8).abs() < f32::EPSILON);
        assert!(!config.renderers.markdown.enabled);
        assert_eq!(config.renderers.json.priority, 100);
        // Unspecified fields keep defaults
        assert!(config.renderers.yaml.enabled);
    }

    #[test]
    fn test_yaml_deserialization_custom_renderers() {
        let yaml = r#"
custom_renderers:
  - id: "protobuf"
    name: "Protocol Buffers"
    detect_patterns: ["^message\\s+\\w+"]
    render_command: "protoc --decode_raw"
    priority: 30
"#;
        let config: PrettifierYamlConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.custom_renderers.len(), 1);
        assert_eq!(config.custom_renderers[0].id, "protobuf");
        assert_eq!(config.custom_renderers[0].priority, 30);
    }

    #[test]
    fn test_yaml_deserialization_detection_rules() {
        let yaml = r#"
detection_rules:
  markdown:
    additional:
      - id: "md_custom_fence"
        pattern: "^```custom"
        weight: 0.4
        scope: "first_lines:5"
    overrides:
      - id: "md_atx_header"
        enabled: false
"#;
        let config: PrettifierYamlConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.detection_rules.contains_key("markdown"));
        let md_rules = &config.detection_rules["markdown"];
        assert_eq!(md_rules.additional.len(), 1);
        assert_eq!(md_rules.additional[0].id, "md_custom_fence");
        assert_eq!(md_rules.overrides.len(), 1);
        assert!(!md_rules.overrides[0].enabled.unwrap());
    }

    #[test]
    fn test_override_struct_defaults() {
        let override_config = PrettifierConfigOverride::default();
        assert!(override_config.respect_alternate_screen.is_none());
        assert!(override_config.per_block_toggle.is_none());
        assert!(override_config.detection.is_none());
        assert!(override_config.renderers.is_none());
        assert!(override_config.claude_code_integration.is_none());
    }

    #[test]
    fn test_override_serialization_skips_none() {
        let override_config = PrettifierConfigOverride::default();
        let yaml = serde_yaml::to_string(&override_config).unwrap();
        // All fields are None, so YAML should be essentially empty
        assert_eq!(yaml.trim(), "{}");
    }

    #[test]
    fn test_resolve_no_profile() {
        let global = PrettifierYamlConfig::default();
        let resolved = resolve_prettifier_config(true, &global, None, None);

        assert!(resolved.enabled);
        assert!(resolved.respect_alternate_screen);
        assert_eq!(resolved.detection.scope, "all");
        assert!(resolved.renderers.markdown.enabled);
    }

    #[test]
    fn test_resolve_profile_overrides_enabled() {
        let global = PrettifierYamlConfig::default();

        // Profile disables prettifier
        let resolved = resolve_prettifier_config(true, &global, Some(false), None);
        assert!(!resolved.enabled);

        // Profile enables prettifier when global is false
        let resolved = resolve_prettifier_config(false, &global, Some(true), None);
        assert!(resolved.enabled);
    }

    #[test]
    fn test_resolve_profile_overrides_detection() {
        let global = PrettifierYamlConfig::default();
        let profile = PrettifierConfigOverride {
            detection: Some(DetectionConfigOverride {
                scope: Some("all".to_string()),
                confidence_threshold: Some(0.9),
                ..Default::default()
            }),
            ..Default::default()
        };

        let resolved = resolve_prettifier_config(true, &global, None, Some(&profile));
        assert_eq!(resolved.detection.scope, "all");
        assert!((resolved.detection.confidence_threshold - 0.9).abs() < f32::EPSILON);
        // Non-overridden fields inherit global
        assert_eq!(resolved.detection.max_scan_lines, 500);
        assert_eq!(resolved.detection.debounce_ms, 100);
    }

    #[test]
    fn test_resolve_profile_overrides_renderers() {
        let global = PrettifierYamlConfig::default();
        let profile = PrettifierConfigOverride {
            renderers: Some(RenderersConfigOverride {
                markdown: Some(RendererToggleOverride {
                    enabled: Some(false),
                    ..Default::default()
                }),
                json: Some(RendererToggleOverride {
                    priority: Some(100),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };

        let resolved = resolve_prettifier_config(true, &global, None, Some(&profile));
        assert!(!resolved.renderers.markdown.enabled);
        assert_eq!(resolved.renderers.json.priority, 100);
        // Unoverridden renderers inherit global
        assert!(resolved.renderers.yaml.enabled);
        assert!(resolved.renderers.diff.enabled);
    }

    #[test]
    fn test_resolve_profile_overrides_claude_code() {
        let global = PrettifierYamlConfig::default();
        let profile = PrettifierConfigOverride {
            claude_code_integration: Some(ClaudeCodeConfigOverride {
                render_markdown: Some(false),
                ..Default::default()
            }),
            ..Default::default()
        };

        let resolved = resolve_prettifier_config(true, &global, None, Some(&profile));
        assert!(!resolved.claude_code_integration.render_markdown);
        assert!(resolved.claude_code_integration.auto_detect); // Inherited
        assert!(resolved.claude_code_integration.render_diffs); // Inherited
    }

    #[test]
    fn test_resolve_inherits_omitted_fields() {
        let mut global = PrettifierYamlConfig::default();
        global.respect_alternate_screen = false;
        global.per_block_toggle = false;

        // Profile overrides only one field
        let profile = PrettifierConfigOverride {
            respect_alternate_screen: Some(true),
            ..Default::default()
        };

        let resolved = resolve_prettifier_config(true, &global, None, Some(&profile));
        assert!(resolved.respect_alternate_screen); // Overridden
        assert!(!resolved.per_block_toggle); // Inherited from global
    }
}
