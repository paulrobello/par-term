//! Terminal configuration management.
//!
//! Re-exports all configuration types from the `par-term-config` crate.
//! All configuration types, defaults, and utilities are defined in `par-term-config`.

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
    substitute_variables_with_allowlist,
};

// --- Color conversion helpers ---
pub use par_term_config::{
    color_tuple_to_f32_a, color_u8_to_f32, color_u8_to_f32_a, color_u8x4_rgb_to_f32,
    color_u8x4_rgb_to_f32_a, color_u8x4_to_f32,
};

// --- Config types ---
pub use par_term_config::{
    AlertEvent, AlertSoundConfig, BackgroundImageMode, BackgroundMode, CursorShaderConfig,
    CursorShaderMetadata, CursorStyle, DividerRect, DividerStyle, DownloadSaveLocation,
    DroppedFileQuoteStyle, FontRange, ImageScalingMode, InstallPromptState, IntegrationVersions,
    KeyBinding, KeyModifier, LinkUnderlineStyle, LogLevel, ModifierRemapping, ModifierTarget,
    OptionKeyMode, PaneBackground, PaneBackgroundConfig, PaneId, PaneTitlePosition,
    PowerPreference, ProgressBarPosition, ProgressBarStyle, ResolvedCursorShaderConfig,
    ResolvedShaderConfig, SemanticHistoryEditorMode, SeparatorMark, SessionLogFormat, ShaderConfig,
    ShaderInstallPrompt, ShaderMetadata, ShellExitAction, ShellType, SmartSelectionPrecision,
    SmartSelectionRule, StartupDirectoryMode, StatusBarPosition, TabBarMode, TabBarPosition, TabId,
    TabStyle, TabTitleMode, ThinStrokesMode, UnfocusedCursorStyle, UpdateCheckFrequency, VsyncMode,
    WindowType, default_smart_selection_rules,
};

// --- Automation ---
pub use par_term_config::{CoprocessDefConfig, RestartPolicy, TriggerActionConfig, TriggerConfig};

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

// --- Prettifier config ---
pub use par_term_config::{
    PrettifierConfigOverride, PrettifierYamlConfig, ResolvedPrettifierConfig,
    resolve_prettifier_config,
};

// --- Core re-exports ---
pub use par_term_config::{AmbiguousWidth, NormalizationForm, UnicodeVersion};

// Re-export the prettifier submodule so `crate::config::prettifier::*` works
pub use par_term_config::config::prettifier;
