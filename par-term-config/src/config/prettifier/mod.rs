//! Configuration structures for the Content Prettifier system.
//!
//! Maps to the `content_prettifier:` section in `config.yaml`.
//!
//! # Sub-modules
//!
//! - [`renderers`] — Per-renderer config types (Markdown, JSON, YAML, TOML, XML, CSV,
//!   Diff, Log, Diagrams, SQL results, Stack Trace) and profile-level renderer overrides.
//! - [`resolve`] — Resolution/normalization logic: [`resolve_prettifier_config`] merges
//!   global config with optional profile overrides into a [`ResolvedPrettifierConfig`].

pub mod renderers;
pub mod resolve;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// Re-export everything from sub-modules so external callers continue to use
// paths like `config::prettifier::RenderersConfig`, `config::prettifier::resolve_prettifier_config`, etc.
pub use renderers::{
    CustomRendererConfig, DiagramRendererConfig, DiffRendererConfig, FormatDetectionRulesConfig,
    RendererToggle, RendererToggleOverride, RenderersConfig, RenderersConfigOverride, RuleOverride,
    UserDetectionRule,
};
pub use resolve::{ResolvedPrettifierConfig, resolve_prettifier_config};

// ---------------------------------------------------------------------------
// Default value functions (shared across this file and sub-modules)
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

    /// Allowlist of command names (basename or full path) that `ExternalCommandRenderer`
    /// is permitted to execute.
    ///
    /// When non-empty, any custom renderer `render_command` whose basename or full path
    /// does not match an entry here will be refused at render time and a warning logged.
    ///
    /// When empty (the default), all commands defined in `custom_renderers` are allowed
    /// to execute but a security warning is emitted for each execution to alert operators.
    ///
    /// Example:
    /// ```yaml
    /// content_prettifier:
    ///   allowed_commands:
    ///     - bat
    ///     - /usr/local/bin/protoc
    /// ```
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_commands: Vec<String>,

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
            allowed_commands: Vec::new(),
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
}

impl Default for ClipboardConfig {
    fn default() -> Self {
        Self {
            default_copy: default_clipboard_copy(),
        }
    }
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
        let config: PrettifierYamlConfig = serde_yaml_ng::from_str(yaml).unwrap();
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
        let config: PrettifierYamlConfig = serde_yaml_ng::from_str(yaml).unwrap();
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
        let config: PrettifierYamlConfig = serde_yaml_ng::from_str(yaml).unwrap();
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
        let config: PrettifierYamlConfig = serde_yaml_ng::from_str(yaml).unwrap();
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
        let yaml = serde_yaml_ng::to_string(&override_config).unwrap();
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
