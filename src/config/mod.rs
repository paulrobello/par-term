//! Terminal configuration management.
//!
//! Re-exports all configuration types from the `par-term-config` sub-crate.
//! All configuration types, defaults, and utilities are defined in `par-term-config`.
//!
//! # Re-exports from `par-term-config`
//!
//! This module is a thin facade: it re-exports every public type from `par-term-config`
//! so the rest of the main crate can write `crate::config::Config` instead of
//! `par_term_config::Config`. This keeps call sites insulated from the sub-crate
//! boundary and makes future refactoring easier (types can be relocated inside
//! `par-term-config` without touching every call site in the main crate).
//!
//! Direct dependencies on `par-term-config` types should route through this module
//! rather than importing from `par_term_config` directly, unless the code is
//! inside a sub-crate that already depends on `par-term-config` explicitly.

// --- Modules ---
pub use par_term_config::automation;
pub use par_term_config::cell;
pub use par_term_config::config;
pub use par_term_config::defaults;
pub use par_term_config::profile;
pub use par_term_config::profile_types;
pub use par_term_config::scripting;
pub use par_term_config::scrollback_mark;
pub use par_term_config::shader_config;
pub use par_term_config::shader_metadata;
pub use par_term_config::snippets;
pub use par_term_config::status_bar;
pub use par_term_config::themes;
pub use par_term_config::watcher;

// --- Types and structs ---
pub use par_term_config::{
    ALLOWED_ENV_VARS, Cell, Color, Config, CustomAcpAgentActionConfig, CustomAcpAgentConfig,
    ScrollbackMark, Theme, is_env_var_allowed, substitute_variables,
    substitute_variables_with_allowlist, substitute_variables_with_lookup,
};

// --- Color conversion helpers ---
pub use par_term_config::{
    color_tuple_to_f32_a, color_u8_to_f32, color_u8_to_f32_a, color_u8x4_rgb_to_f32,
    color_u8x4_rgb_to_f32_a, color_u8x4_to_f32,
};

// --- Config types ---
pub use par_term_config::{
    AlertEvent, AlertSoundConfig, AutomationConfig, BackgroundConfig, BackgroundImageMode,
    BackgroundMode, BadgeConfig, ClipboardConfig, CommandSeparatorConfig, CursorShaderConfig,
    CursorShaderMetadata, CursorStyle, DividerRect, DividerStyle, DownloadSaveLocation,
    DroppedFileQuoteStyle, FontRange, ImageConfig, ImageScalingMode, InputConfig,
    InstallPromptState, IntegrationConfig, IntegrationVersions, KeyBinding, KeyModifier,
    LinkUnderlineStyle, LogLevel, ModifierRemapping, ModifierTarget, NewTabPosition, OptionKeyMode,
    PaneBackground, PaneBackgroundConfig, PaneConfig, PaneId, PaneTitlePosition, PowerConfig,
    PowerPreference, ProgressBarConfig, ProgressBarPosition, ProgressBarStyle, RenderingConfig,
    ResolvedCursorShaderConfig, ResolvedShaderConfig, ScrollbarConfig, SecurityConfig,
    SelectionConfig, SemanticHistoryConfig, SemanticHistoryEditorMode, SeparatorMark,
    SessionLogConfig, SessionLogFormat, SessionRestoreConfig, ShaderConfig, ShaderInstallPrompt,
    ShaderMetadata, ShaderOverridesConfig, ShaderWatchConfig, ShellConfig, ShellExitAction,
    ShellType, SmartSelectionPrecision, SmartSelectionRule, StartupDirectoryMode,
    StatusBarPosition, TabBarColorsConfig, TabBarMode, TabBarPosition, TabConfig, TabId, TabStyle,
    TabTitleMode, ThemeColorsConfig, ThinStrokesMode, TmuxConfig, UnfocusedCursorStyle,
    UpdateCheckFrequency, VsyncMode, WindowPlacementConfig, WindowType, WordSelectionConfig,
    default_smart_selection_rules,
};

// --- Automation ---
pub use par_term_config::{
    CoprocessDefConfig, RestartPolicy, TriggerActionConfig, TriggerConfig, TriggerRateLimiter,
    check_command_allowlist, check_command_denylist,
};

// --- Scripting ---
pub use par_term_config::ScriptConfig;

// --- Snippets ---
pub use par_term_config::{BuiltInVariable, CustomActionConfig, SnippetConfig, SnippetLibrary};

// --- Status bar ---
pub use par_term_config::{StatusBarSection, StatusBarWidgetConfig, WidgetId, default_widgets};

// --- Profiles ---
pub use par_term_config::{ConflictResolution, DynamicProfileSource};
pub use par_term_config::{Profile, ProfileId, ProfileManager, ProfileSource};

// --- Shader config/metadata ---
pub use par_term_config::{CursorShaderMetadataCache, ShaderMetadataCache};
pub use par_term_config::{
    parse_cursor_shader_metadata, parse_shader_metadata, update_cursor_shader_metadata_file,
    update_shader_metadata_file,
};
pub use par_term_config::{resolve_cursor_shader_config, resolve_shader_config};

// --- Core re-exports ---
pub use par_term_config::{AmbiguousWidth, NormalizationForm, UnicodeVersion};
