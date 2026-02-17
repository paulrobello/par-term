//! Terminal configuration management.
//!
//! This module re-exports types from the par-term-config crate for backward compatibility.
//! All configuration types, defaults, and utilities are defined in the par-term-config crate.

// Re-export everything from par-term-config
pub use par_term_config::{
    // Types
    AlertEvent,
    AlertSoundConfig,
    BackgroundImageMode,
    BackgroundMode,
    // Snippets
    BuiltInVariable,
    // Themes
    Color,
    // Core config
    Config,
    // Profile
    ConflictResolution,
    // Automation
    CoprocessDefConfig,
    CursorShaderConfig,
    CursorShaderMetadata,
    // Shader
    CursorShaderMetadataCache,
    CursorStyle,
    CustomActionConfig,
    DividerStyle,
    DownloadSaveLocation,
    DroppedFileQuoteStyle,
    DynamicProfileSource,
    FontRange,
    ImageScalingMode,
    InstallPromptState,
    IntegrationVersions,
    KeyBinding,
    KeyModifier,
    LogLevel,
    ModifierRemapping,
    ModifierTarget,
    OptionKeyMode,
    PaneBackgroundConfig,
    PaneTitlePosition,
    PowerPreference,
    ProgressBarPosition,
    ProgressBarStyle,
    ResolvedCursorShaderConfig,
    ResolvedShaderConfig,
    RestartPolicy,
    // Scripting
    ScriptConfig,
    SemanticHistoryEditorMode,
    SessionLogFormat,
    ShaderConfig,
    ShaderInstallPrompt,
    ShaderMetadata,
    ShaderMetadataCache,
    ShellExitAction,
    ShellType,
    SmartSelectionPrecision,
    SmartSelectionRule,
    SnippetConfig,
    SnippetLibrary,
    StartupDirectoryMode,
    StatusBarPosition,
    // Status bar
    StatusBarSection,
    StatusBarWidgetConfig,
    TabBarMode,
    TabBarPosition,
    TabStyle,
    Theme,
    ThinStrokesMode,
    TriggerActionConfig,
    TriggerConfig,
    UnfocusedCursorStyle,
    UpdateCheckFrequency,
    VsyncMode,
    WidgetId,
    WindowType,
    default_smart_selection_rules,
    default_widgets as default_status_bar_widgets,
    parse_cursor_shader_metadata,
    parse_shader_metadata,
    resolve_cursor_shader_config,
    resolve_shader_config,
    substitute_variables,
    update_cursor_shader_metadata_file,
    update_shader_metadata_file,
};

// Re-export submodules for backward compatibility
pub mod automation {
    pub use par_term_config::{
        CoprocessDefConfig, RestartPolicy, TriggerActionConfig, TriggerConfig,
    };
}

pub mod defaults {
    pub use par_term_config::defaults::*;
}

pub mod scripting {
    pub use par_term_config::ScriptConfig;
}

pub mod snippets {
    pub use par_term_config::{BuiltInVariable, CustomActionConfig, SnippetConfig, SnippetLibrary};
}

pub mod shader_config {
    pub use par_term_config::{resolve_cursor_shader_config, resolve_shader_config};
}

pub mod shader_metadata {
    pub use par_term_config::{
        CursorShaderMetadataCache, ShaderMetadataCache, parse_cursor_shader_metadata,
        parse_shader_metadata, update_cursor_shader_metadata_file, update_shader_metadata_file,
    };
}

pub mod status_bar {
    pub use par_term_config::{StatusBarSection, StatusBarWidgetConfig, WidgetId, default_widgets};
}

pub mod profile {
    pub use par_term_config::{ConflictResolution, DynamicProfileSource};
}

// Re-export the watcher module
pub mod watcher {
    pub use par_term_config::watcher::{ConfigReloadEvent, ConfigWatcher};
}
