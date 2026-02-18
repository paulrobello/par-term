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

pub mod automation;
pub mod cell;
pub mod config;
pub mod defaults;
pub mod profile;
pub mod profile_types;
pub mod scripting;
pub mod shader_config;
pub mod shader_metadata;
pub mod snippets;
pub mod status_bar;
pub mod themes;
mod types;
#[cfg(feature = "watcher")]
pub mod watcher;

// Re-export main types for convenience
pub use cell::Cell;
pub use config::{Config, substitute_variables};
pub use themes::{Color, Theme};

// Re-export config types
pub use types::{
    AlertEvent, AlertSoundConfig, BackgroundImageMode, BackgroundMode, CursorShaderConfig,
    CursorShaderMetadata, CursorStyle, DividerRect, DividerStyle, DownloadSaveLocation,
    DroppedFileQuoteStyle, FontRange, ImageScalingMode, InstallPromptState, IntegrationVersions,
    KeyBinding, LogLevel, ModifierRemapping, ModifierTarget, OptionKeyMode, PaneBackground,
    PaneBackgroundConfig, PaneTitlePosition, PowerPreference, ProgressBarPosition,
    ProgressBarStyle, SemanticHistoryEditorMode, SeparatorMark, SessionLogFormat, ShaderConfig,
    ShaderInstallPrompt, ShaderMetadata, ShellExitAction, ShellType, SmartSelectionPrecision,
    SmartSelectionRule, StartupDirectoryMode, StatusBarPosition, TabBarMode, TabBarPosition,
    TabStyle, ThinStrokesMode, UnfocusedCursorStyle, UpdateCheckFrequency, VsyncMode, WindowType,
    default_smart_selection_rules,
};
// KeyModifier is exported for potential future use (e.g., custom keybinding UI)
pub use automation::{CoprocessDefConfig, RestartPolicy, TriggerActionConfig, TriggerConfig};
// Scripting / observer scripts
pub use scripting::ScriptConfig;
// Snippets and custom actions
pub use snippets::{BuiltInVariable, CustomActionConfig, SnippetConfig, SnippetLibrary};
// Status bar configuration
pub use status_bar::{StatusBarSection, StatusBarWidgetConfig, WidgetId, default_widgets};
// Profile configuration
pub use profile::{ConflictResolution, DynamicProfileSource};
// Profile types and manager
pub use profile_types::{Profile, ProfileId, ProfileManager, ProfileSource};
// Shader config resolution
pub use shader_config::{resolve_cursor_shader_config, resolve_shader_config};
// Shader metadata
pub use shader_metadata::{CursorShaderMetadataCache, ShaderMetadataCache};
pub use shader_metadata::{
    parse_cursor_shader_metadata, parse_shader_metadata, update_cursor_shader_metadata_file,
    update_shader_metadata_file,
};
#[allow(unused_imports)]
pub use types::KeyModifier;
#[allow(unused_imports)]
pub use types::{ResolvedCursorShaderConfig, ResolvedShaderConfig};
