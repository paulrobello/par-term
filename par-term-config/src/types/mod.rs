//! Configuration types and enums.
//!
//! This module is split into focused sub-modules by domain:
//! - `alert`      — Alert sound event and config types
//! - `color`      — Color conversion helper functions
//! - `font`       — Font range, thin strokes, file drop, download location
//! - `integration`— Shader/shell install prompts, update frequency, progress bar
//! - `keybinding` — KeyModifier, KeyBinding
//! - `rendering`  — GPU/VSync/power, image scaling, background, pane/divider layout
//! - `selection`  — Smart selection rules and defaults
//! - `shader`     — Shader metadata, config, and resolved shader types
//! - `shell`      — Shell type detection, exit action, startup directory
//! - `tab_bar`    — Tab style/position/mode, window type, status bar position
//! - `terminal`   — Cursor, input modes, session logging, link styles

pub mod alert;
pub mod color;
pub mod font;
pub mod integration;
pub mod keybinding;
pub mod rendering;
pub mod selection;
pub mod shader;
pub mod shell;
pub mod tab_bar;
pub mod terminal;

// Re-export everything so callers of `types::*` continue to work.

pub use alert::{AlertEvent, AlertSoundConfig};
pub use color::{
    color_tuple_to_f32_a, color_u8_to_f32, color_u8_to_f32_a, color_u8x4_rgb_to_f32,
    color_u8x4_rgb_to_f32_a, color_u8x4_to_f32,
};
pub use font::{DownloadSaveLocation, DroppedFileQuoteStyle, FontRange, ThinStrokesMode};
pub use integration::{
    InstallPromptState, IntegrationVersions, ProgressBarPosition, ProgressBarStyle,
    ShaderInstallPrompt, UpdateCheckFrequency,
};
pub use keybinding::{KeyBinding, KeyModifier};
pub use rendering::{
    BackgroundImageMode, BackgroundMode, DividerRect, DividerStyle, ImageScalingMode,
    PaneBackground, PaneBackgroundConfig, PaneId, PaneTitlePosition, PowerPreference,
    SeparatorMark, TabId, VsyncMode,
};
pub use selection::{SmartSelectionPrecision, SmartSelectionRule, default_smart_selection_rules};
pub use shader::{
    CursorShaderConfig, CursorShaderMetadata, ResolvedCursorShaderConfig, ResolvedShaderConfig,
    ShaderConfig, ShaderMetadata,
};
pub use shell::{ShellExitAction, ShellType, StartupDirectoryMode};
pub use tab_bar::{
    StatusBarPosition, TabBarMode, TabBarPosition, TabStyle, TabTitleMode, WindowType,
};
pub use terminal::{
    CursorStyle, LinkUnderlineStyle, LogLevel, ModifierRemapping, ModifierTarget, OptionKeyMode,
    SemanticHistoryEditorMode, SessionLogFormat, UnfocusedCursorStyle,
};
