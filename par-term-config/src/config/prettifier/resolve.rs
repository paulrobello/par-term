//! Resolution and normalization logic for the Content Prettifier system.
//!
//! Merges global prettifier config with optional profile-level overrides to produce
//! a fully resolved [`ResolvedPrettifierConfig`].

use std::collections::HashMap;

use super::renderers::{
    DiagramRendererConfig, DiffRendererConfig, RendererToggle, RendererToggleOverride,
    RenderersConfig, RenderersConfigOverride,
};
use super::{
    CacheConfig, ClaudeCodeConfig, ClaudeCodeConfigOverride, ClipboardConfig, CustomRendererConfig,
    DetectionConfig, DetectionConfigOverride, FormatDetectionRulesConfig, PrettifierConfigOverride,
    PrettifierYamlConfig,
};

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
    /// Allowlist of permitted command names for `ExternalCommandRenderer`.
    /// Propagated from `PrettifierYamlConfig::allowed_commands`.
    pub allowed_commands: Vec<String>,
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
        allowed_commands: global_config.allowed_commands.clone(),
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
