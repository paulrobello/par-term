//! Configuration system for par-term terminal emulator.
//!
//! This crate provides configuration loading, saving, and default values
//! for the terminal emulator. It includes:
//!
//! - Terminal configuration types and settings
//! - Theme definitions and color schemes
//! - Shader configuration management
//! - Snippets and automation support
//! - Configuration file watching
//! - Status bar widget configuration
//! - Profile configuration types and manager

pub mod assistant_input_history;
pub mod assistant_prompts;
pub mod automation;
pub mod cell;
pub mod config;
pub mod defaults;
pub mod error;
pub mod layout_constants;
pub mod profile;
pub mod profile_types;
pub mod scripting;
pub mod scrollback_mark;
pub mod shader_bundle;
pub mod shader_config;
pub mod shader_controls;
pub mod shader_metadata;
pub mod shell_detection;
pub mod snapshot_types;
pub mod snippets;
pub mod status_bar;
pub mod themes;
mod types;
#[cfg(feature = "watcher")]
pub mod watcher;

// Re-export assistant prompt-library storage types and helpers
pub use assistant_input_history::{
    MAX_ASSISTANT_INPUT_HISTORY_ENTRIES, assistant_input_history_path,
    load_assistant_input_history, merge_assistant_input_history, normalize_assistant_input_history,
    save_assistant_input_history,
};
pub use assistant_prompts::{
    AssistantPrompt, AssistantPromptDraft, assistant_prompts_dir, delete_prompt, list_prompts,
    list_prompts_in_dir, parse_prompt_markdown, safe_prompt_filename, save_prompt,
    save_prompt_in_dir, serialize_prompt_markdown,
};

// Re-export error types
pub use error::ConfigError;

// Re-export main types for convenience
pub use cell::Cell;
pub use config::{
    ALLOWED_ENV_VARS, AiInspectorConfig, AssistantInputHistoryMode, Config, CursorConfig,
    CustomAcpAgentActionConfig, CustomAcpAgentConfig, FontRenderingConfig, GlobalShaderConfig,
    MouseConfig, StatusBarConfig, WindowConfig, is_env_var_allowed, substitute_variables,
    substitute_variables_with_allowlist,
};
pub use scrollback_mark::ScrollbackMark;
pub use themes::{Color, Theme};

// Re-export color conversion helpers
pub use types::{
    color_tuple_to_f32_a, color_u8_to_f32, color_u8_to_f32_a, color_u8x4_rgb_to_f32,
    color_u8x4_rgb_to_f32_a, color_u8x4_to_f32,
};

// Re-export config types
pub use automation::{
    CoprocessDefConfig, RestartPolicy, SplitPaneCommand, TriggerActionConfig, TriggerConfig,
    TriggerRateLimiter, TriggerSplitDirection, TriggerSplitTarget, check_command_allowlist,
    check_command_denylist, warn_prompt_before_run_false,
};
pub use types::{
    AlertEvent, AlertSoundConfig, BackgroundImageMode, BackgroundMode, CursorShaderConfig,
    CursorShaderMetadata, CursorStyle, DividerRect, DividerStyle, DownloadSaveLocation,
    DroppedFileQuoteStyle, FontRange, ImageScalingMode, InstallPromptState, IntegrationVersions,
    KeyBinding, LinkUnderlineStyle, LogLevel, ModifierRemapping, ModifierTarget, NewTabPosition,
    OptionKeyMode, PaneBackground, PaneBackgroundConfig, PaneId, PaneTitlePosition,
    PowerPreference, ProgressBarPosition, ProgressBarStyle, RemoteTabTitleFormat,
    SemanticHistoryEditorMode, SeparatorMark, SessionLogFormat, ShaderBackgroundBlendMode,
    ShaderConfig, ShaderInstallPrompt, ShaderMetadata, ShaderSafetyBadge, ShellExitAction,
    ShellType, SmartSelectionPrecision, SmartSelectionRule, StartupDirectoryMode,
    StatusBarPosition, TabBarMode, TabBarPosition, TabId, TabStyle, TabTitleMode, ThinStrokesMode,
    UnfocusedCursorStyle, UpdateCheckFrequency, VsyncMode, WindowType,
    default_smart_selection_rules,
};
// Scripting / observer scripts
pub use scripting::ScriptConfig;
// Snippets and custom actions
pub use snippets::{BuiltInVariable, CustomActionConfig, SnippetConfig, SnippetLibrary};
// Status bar configuration
pub use status_bar::{StatusBarSection, StatusBarWidgetConfig, WidgetId, default_widgets};
// Profile configuration
pub use profile::{ConflictResolution, DynamicProfileSource};
// Profile types and manager
pub use profile_types::{Profile, ProfileId, ProfileManager, ProfileSource, TmuxConnectionMode};
// Shader bundle manifests
pub use shader_bundle::ShaderBundleManifest;
// Shader config resolution
pub use shader_config::{resolve_cursor_shader_config, resolve_shader_config};
// Shader controls
pub use shader_controls::{
    AngleUnit, ShaderControl, ShaderControlKind, ShaderControlParseResult, ShaderControlWarning,
    SliderScale, fallback_value_for_control, parse_shader_controls,
};
// Shader metadata
pub use shader_metadata::{CursorShaderMetadataCache, ShaderMetadataCache};
pub use shader_metadata::{
    parse_cursor_shader_metadata, parse_shader_metadata, update_cursor_shader_metadata_file,
    update_shader_metadata_file,
};
// Shared snapshot types for session and arrangement persistence
pub use snapshot_types::TabSnapshot;

// ARC-003 RESOLVED: Native Unicode types defined in par-term-config/src/types/unicode.rs.
// These mirror the par-term-emu-core-rust enums but belong to the config layer,
// removing the upward dependency from the foundation config crate to the emulation core.
// Higher-level crates (par-term-terminal, root crate) convert via `From` impls.
pub use types::{AmbiguousWidth, NormalizationForm, UnicodeVersion};
// `KeyModifier` and the Resolved*ShaderConfig types are re-exported for downstream crates
// (e.g., root crate's src/config/mod.rs facade). Rust's unused-import lint fires here
// because nothing inside par-term-config itself consumes these re-exports directly.
// The suppressions are intentional — do not remove without auditing all consumers first.
#[allow(unused_imports)]
pub use types::KeyModifier;
#[allow(unused_imports)]
pub use types::shader::{ShaderColorValue, ShaderUniformValue};
pub use types::{ResolvedCursorShaderConfig, ResolvedShaderConfig};
