# Step 6: Config & Profile Integration

## Summary

Integrate the Content Prettifier settings into par-term's configuration system (`config.yaml`) and profile override system (`profiles.yaml`). This includes the `enable_prettifier` master toggle, all `content_prettifier` sub-settings, per-renderer configuration, detection rule loading, and the profile-level override chain.

## Dependencies

- **Step 1**: Core types
- **Step 4**: `PrettifierConfig` placeholder, `RendererRegistry`
- **Step 5**: `RenderCache` config (max_entries)

## What to Implement

### New File: `src/config/prettifier.rs`

Define all configuration structures that map to the YAML config:

```rust
use serde::{Deserialize, Serialize};

/// Top-level prettifier configuration (lives under `content_prettifier:` in config.yaml)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrettifierConfig {
    #[serde(default = "default_true")]
    pub respect_alternate_screen: bool,

    #[serde(default = "default_global_toggle_key")]
    pub global_toggle_key: String,

    #[serde(default = "default_true")]
    pub per_block_toggle: bool,

    #[serde(default)]
    pub detection: DetectionConfig,

    #[serde(default)]
    pub clipboard: ClipboardConfig,

    #[serde(default)]
    pub renderers: RenderersConfig,

    #[serde(default)]
    pub custom_renderers: Vec<CustomRendererConfig>,

    #[serde(default)]
    pub claude_code_integration: ClaudeCodeConfig,

    #[serde(default)]
    pub detection_rules: DetectionRulesConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DetectionConfig {
    #[serde(default = "default_detection_scope")]
    pub scope: String,  // "command_output" | "all" | "manual_only"

    #[serde(default = "default_confidence_threshold")]
    pub confidence_threshold: f32,

    #[serde(default = "default_max_scan_lines")]
    pub max_scan_lines: usize,

    #[serde(default = "default_debounce_ms")]
    pub debounce_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClipboardConfig {
    #[serde(default = "default_clipboard_copy")]
    pub default_copy: String,  // "rendered" | "source"

    #[serde(default = "default_source_copy_modifier")]
    pub source_copy_modifier: String,

    #[serde(default = "default_vi_copy_mode")]
    pub vi_copy_mode: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RendererToggle {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_priority")]
    pub priority: i32,
}

// Also: MarkdownRendererConfig, DiffRendererConfig, DiagramRendererConfig,
// CustomRendererConfig, ClaudeCodeConfig, DetectionRulesConfig
// (each with fields matching the spec's YAML examples)
```

### Modify: `src/config/mod.rs` (or `src/config.rs`)

Add new fields to the main `Config` struct:

```rust
/// Master switch for the content prettifier system
#[serde(default = "default_true")]
pub enable_prettifier: bool,

/// Detailed prettifier configuration
#[serde(default)]
pub content_prettifier: PrettifierConfig,
```

### Profile Override Integration

Modify the profile types (in `src/profile/types.rs` or wherever profiles are defined) to support prettifier overrides:

```rust
/// In the Profile struct, add optional prettifier overrides:
#[serde(default)]
pub enable_prettifier: Option<bool>,  // None = inherit global

#[serde(default)]
pub content_prettifier: Option<PrettifierConfigOverride>,
```

The override struct uses `Option<T>` for every field so that omitted fields inherit from global:

```rust
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct PrettifierConfigOverride {
    pub detection: Option<DetectionConfigOverride>,
    pub renderers: Option<RenderersConfigOverride>,
    pub claude_code_integration: Option<ClaudeCodeConfigOverride>,
    // ... etc, all fields Optional
}
```

### Config Resolution Function

Implement the merge logic that resolves effective config from global + profile:

```rust
/// Resolve effective prettifier config by merging global defaults with profile overrides.
pub fn resolve_prettifier_config(
    global: &Config,
    profile: Option<&Profile>,
) -> ResolvedPrettifierConfig { ... }
```

This follows the precedence chain (spec lines 534–541):
1. Profile-level setting (if present) ← wins
2. Global config-level setting ← fallback
3. Built-in default ← last resort

### Detection Rule Loading

Implement loading of built-in detection rules and merging with user-defined rules from config:

```rust
/// Load detection rules from config, merging built-in rules with user overrides.
pub fn load_detection_rules(config: &PrettifierConfig) -> HashMap<String, Vec<DetectionRule>> { ... }
```

User-defined rules under `detection_rules.markdown`, etc. are merged with built-in rules. Overrides (from `detection_rules.markdown.overrides`) are applied to matching rule IDs.

### Settings UI Keywords

Update `src/settings_ui/sidebar.rs` to add search keywords for the new config options:
- `"prettifier"`, `"prettify"`, `"content"`, `"detect"`, `"render"`, `"markdown"`, `"json"`, `"diagram"`, `"confidence"`, `"detection"`

## Key Files

| Action | Path |
|--------|------|
| Create | `src/config/prettifier.rs` |
| Modify | `src/config/mod.rs` (add `enable_prettifier` and `content_prettifier` fields) |
| Modify | `src/profile/types.rs` (add prettifier override fields) |
| Modify | `src/settings_ui/sidebar.rs` (add search keywords) |
| Modify | `src/prettifier/mod.rs` (add config loading utilities) |

## Relevant Spec Sections

- **Lines 509–558**: `enable_prettifier` — global & profile toggle, override rules, examples
- **Lines 730–744**: Configuration global vs profile override resolution order
- **Lines 750–835**: Full global configuration YAML structure
- **Lines 837–892**: Profile-level overrides YAML structure with examples
- **Lines 894–911**: Setting naming rationale and Settings UI label
- **Lines 1329–1330**: Profiles follow par-term's existing profile override pattern
- **Lines 1453–1460**: Acceptance criteria for config & profiles

## Verification Criteria

- [ ] `cargo build` succeeds
- [ ] `enable_prettifier` defaults to `true` when not specified in config
- [ ] All `content_prettifier` sub-settings deserialize correctly from YAML
- [ ] Profile-level `enable_prettifier: false` overrides global `true`
- [ ] Profile-level `enable_prettifier: true` overrides global `false`
- [ ] Omitted profile fields inherit from global config
- [ ] `resolve_prettifier_config()` correctly merges global + profile settings
- [ ] Per-renderer enable/disable and priority load correctly
- [ ] Detection rules load from config and merge with built-in rules
- [ ] User rule overrides (enable/disable, weight changes) apply correctly
- [ ] Custom renderer configs deserialize correctly
- [ ] Settings UI search keywords are updated
- [ ] Unit tests for config deserialization, profile override resolution, rule merging
