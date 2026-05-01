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
//!
//! # Organization
//!
//! All public types are re-exported at the crate root for backward compatibility.
//! For better discoverability, they are also grouped into prelude sub-modules:
//!
//! - [`prelude::core`] — Config, ConfigError, Cell, Theme, Color
//! - [`prelude::types`] — All config enums and structs (cursor, shell, rendering, etc.)
//! - [`prelude::automation`] — Triggers, coprocesses, and scripting
//! - [`prelude::shader`] — Shader controls, metadata, bundles, and resolution
//! - [`prelude::assistant`] — AI assistant prompts and input history
//! - [`prelude::snippets`] — Snippets, custom actions, and built-in variables
//! - [`prelude::status_bar`] — Status bar widgets and layout
//! - [`prelude::profile`] — Profiles, profile manager, and dynamic sources
//! - [`prelude::unicode`] — Unicode width, normalization, and version types
//! - [`prelude::color`] — Color conversion helper functions

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

// ---------------------------------------------------------------------------
// Domain-organized prelude sub-modules for discoverability.
// All items here are also re-exported at the crate root below for backward
// compatibility. Consumers can either `use par_term_config::Config` (root)
// or `use par_term_config::prelude::core::Config` (grouped) interchangeably.
// ---------------------------------------------------------------------------

/// Organized prelude modules grouping re-exports by domain.
///
/// Each sub-module re-exports a cohesive set of types so that consumers can
/// narrow their imports to just the domain they need. The crate root still
/// re-exports everything for backward compatibility — nothing changes for
/// existing `use par_term_config::X` paths.
pub mod prelude {
    /// Core config types: the main [`Config`] struct, error type, cell, theme, and color.
    ///
    /// These are the types most downstream crates need on every import.
    pub mod core {
        pub use crate::cell::Cell;
        pub use crate::config::{
            ALLOWED_ENV_VARS, AiInspectorConfig, AssistantInputHistoryMode, Config, CursorConfig,
            CustomAcpAgentActionConfig, CustomAcpAgentConfig, FontRenderingConfig,
            GlobalShaderConfig, MouseConfig, StatusBarConfig, WindowConfig, is_env_var_allowed,
            substitute_variables, substitute_variables_with_allowlist,
        };
        pub use crate::error::ConfigError;
        pub use crate::scrollback_mark::ScrollbackMark;
        pub use crate::snapshot_types::TabSnapshot;
        pub use crate::themes::{Color, Theme};
    }

    /// Configuration enums and structs organized by subsystem.
    ///
    /// This re-exports every type from the internal `types` module, grouped
    /// under a single namespace so consumers can `use prelude::types::*` or
    /// import individual items.
    pub mod types {
        // Alert sounds
        pub use crate::types::alert::{AlertEvent, AlertSoundConfig};
        // Font and display
        pub use crate::types::font::{
            DownloadSaveLocation, DroppedFileQuoteStyle, FontRange, ThinStrokesMode,
        };
        // Integration / install prompts
        pub use crate::types::integration::{
            InstallPromptState, IntegrationVersions, ProgressBarPosition, ProgressBarStyle,
            ShaderInstallPrompt, UpdateCheckFrequency,
        };
        // Keybindings
        pub use crate::types::keybinding::KeyBinding;
        #[allow(unused_imports)]
        pub use crate::types::keybinding::KeyModifier;
        // Rendering and layout
        pub use crate::types::rendering::{
            BackgroundImageMode, BackgroundMode, DividerRect, DividerStyle, ImageScalingMode,
            PaneBackground, PaneBackgroundConfig, PaneId, PaneTitlePosition, PowerPreference,
            SeparatorMark, TabId, VsyncMode,
        };
        // Selection
        pub use crate::types::selection::{
            SmartSelectionPrecision, SmartSelectionRule, default_smart_selection_rules,
        };
        // Shader types
        pub use crate::types::shader::{
            CursorShaderConfig, CursorShaderMetadata, ResolvedCursorShaderConfig,
            ResolvedShaderConfig, ShaderBackgroundBlendMode, ShaderConfig, ShaderMetadata,
            ShaderSafetyBadge,
        };
        #[allow(unused_imports)]
        pub use crate::types::shader::{ShaderColorValue, ShaderUniformValue};
        // Shell
        pub use crate::types::shell::{ShellExitAction, ShellType, StartupDirectoryMode};
        // Tab bar and window
        pub use crate::types::tab_bar::{
            NewTabPosition, RemoteTabTitleFormat, StatusBarPosition, TabBarMode, TabBarPosition,
            TabStyle, TabTitleMode, WindowType,
        };
        // Terminal / cursor / input
        pub use crate::types::terminal::{
            CursorStyle, LinkUnderlineStyle, LogLevel, ModifierRemapping, ModifierTarget,
            OptionKeyMode, SemanticHistoryEditorMode, SessionLogFormat, UnfocusedCursorStyle,
        };
    }

    /// Automation types: triggers, coprocesses, rate limiting, and command safety checks.
    pub mod automation {
        pub use crate::automation::{
            CoprocessDefConfig, RestartPolicy, SplitPaneCommand, TriggerActionConfig,
            TriggerConfig, TriggerRateLimiter, TriggerSplitDirection, TriggerSplitTarget,
            check_command_allowlist, check_command_denylist, warn_prompt_before_run_false,
        };
        pub use crate::scripting::ScriptConfig;
    }

    /// Shader system: controls, metadata, bundles, config resolution, and cached metadata.
    pub mod shader {
        pub use crate::shader_bundle::ShaderBundleManifest;
        pub use crate::shader_config::{resolve_cursor_shader_config, resolve_shader_config};
        pub use crate::shader_controls::{
            AngleUnit, ShaderControl, ShaderControlKind, ShaderControlParseResult,
            ShaderControlWarning, SliderScale, fallback_value_for_control, parse_shader_controls,
        };
        pub use crate::shader_metadata::{
            CursorShaderMetadataCache, ShaderMetadataCache, parse_cursor_shader_metadata,
            parse_shader_metadata, update_cursor_shader_metadata_file,
            update_shader_metadata_file,
        };
    }

    /// AI assistant types: prompt library, input history, and serialization helpers.
    pub mod assistant {
        pub use crate::assistant_input_history::{
            MAX_ASSISTANT_INPUT_HISTORY_ENTRIES, assistant_input_history_path,
            load_assistant_input_history, merge_assistant_input_history,
            normalize_assistant_input_history, save_assistant_input_history,
        };
        pub use crate::assistant_prompts::{
            AssistantPrompt, AssistantPromptDraft, assistant_prompts_dir, delete_prompt,
            list_prompts, list_prompts_in_dir, parse_prompt_markdown, safe_prompt_filename,
            save_prompt, save_prompt_in_dir, serialize_prompt_markdown,
        };
    }

    /// Snippets and custom actions: user-defined commands, built-in variables, and the snippet library.
    pub mod snippets {
        pub use crate::snippets::{BuiltInVariable, CustomActionConfig, SnippetConfig, SnippetLibrary};
    }

    /// Status bar widgets, sections, layout, and default widget configuration.
    pub mod status_bar {
        pub use crate::status_bar::{StatusBarSection, StatusBarWidgetConfig, WidgetId, default_widgets};
    }

    /// Profile management: profiles, the profile manager, dynamic sources, and conflict resolution.
    pub mod profile {
        pub use crate::profile::{ConflictResolution, DynamicProfileSource};
        pub use crate::profile_types::{
            Profile, ProfileId, ProfileManager, ProfileSource, TmuxConnectionMode,
        };
    }

    /// Unicode configuration: ambiguous-width handling, normalization form, and Unicode version.
    ///
    /// These types mirror the par-term-emu-core-rust enums but belong to the config layer,
    /// removing the upward dependency from the foundation config crate to the emulation core.
    /// Higher-level crates (par-term-terminal, root crate) convert via `From` impls.
    pub mod unicode {
        pub use crate::types::unicode::{AmbiguousWidth, NormalizationForm, UnicodeVersion};
    }

    /// Color conversion helpers for translating u8/tuple colors to f32 GPU-ready values.
    pub mod color {
        pub use crate::types::color::{
            color_tuple_to_f32_a, color_u8_to_f32, color_u8_to_f32_a, color_u8x4_rgb_to_f32,
            color_u8x4_rgb_to_f32_a, color_u8x4_to_f32,
        };
    }
}

// ---------------------------------------------------------------------------
// Top-level re-exports — backward-compatible public API.
// Every item below is also available via a prelude sub-module above.
// Do NOT remove any of these without auditing all downstream crates first.
// ---------------------------------------------------------------------------

// Assistant prompt-library storage types and helpers
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

// Error types
pub use error::ConfigError;

// Core types
pub use cell::Cell;
pub use config::{
    ALLOWED_ENV_VARS, AiInspectorConfig, AssistantInputHistoryMode, Config, CursorConfig,
    CustomAcpAgentActionConfig, CustomAcpAgentConfig, FontRenderingConfig, GlobalShaderConfig,
    MouseConfig, StatusBarConfig, WindowConfig, is_env_var_allowed, substitute_variables,
    substitute_variables_with_allowlist,
};
pub use scrollback_mark::ScrollbackMark;
pub use themes::{Color, Theme};

// Color conversion helpers
pub use types::{
    color_tuple_to_f32_a, color_u8_to_f32, color_u8_to_f32_a, color_u8x4_rgb_to_f32,
    color_u8x4_rgb_to_f32_a, color_u8x4_to_f32,
};

// Automation types
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

// Unicode types (ARC-003: native config-layer definitions)
pub use types::{AmbiguousWidth, NormalizationForm, UnicodeVersion};
// KeyModifier and Resolved*ShaderConfig — unused-import suppressions are intentional;
// these are re-exported for downstream crates (root crate src/config/mod.rs facade).
#[allow(unused_imports)]
pub use types::KeyModifier;
#[allow(unused_imports)]
pub use types::shader::{ShaderColorValue, ShaderUniformValue};
pub use types::{ResolvedCursorShaderConfig, ResolvedShaderConfig};
