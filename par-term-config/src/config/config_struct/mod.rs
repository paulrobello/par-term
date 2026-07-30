//! Core `Config` struct definition.
//!
//! This module contains the main configuration struct with all terminal,
//! display, input, and feature settings.
//!
//! # Splitting Strategy
//!
//! Thematic field groups live in sibling `*_config.rs` sub-structs held here as
//! `#[serde(flatten)]` members. Flattening is what makes the split a pure
//! refactor: every field still serialises at the top level of `config.yaml`,
//! indistinguishable from a direct struct field, so a config written by any
//! release keeps loading unchanged.
//!
//! **New settings belong in the sub-struct for their area, not on `Config`.**
//! The root keeps only the handful of fields below, and adding to it is how
//! this file grew to 1,529 lines in the first place.
//!
//! Two rules the sub-structs must follow, both enforced by
//! `tests/config_yaml_compat.rs`:
//!
//! - The member carries `#[serde(flatten)]`. Without it the group's keys nest
//!   under the member name and vanish from every existing config file.
//! - `Default` is hand-written from the same expressions as the field's
//!   `#[serde(default = "…")]`, unless every field genuinely defaults to its
//!   type's `Default`. Most do not — deriving resets non-zero durations,
//!   opacities and `true` flags while compiling cleanly.
//!
//! # Still on the root
//!
//! Terminal size (`cols`/`rows`), the font family/metrics block, the window
//! title, and a scattering of one-field settings (`screenshot_format`,
//! `log_level`, `max_osc_data_length`, `command_history_max_entries`,
//! `collapsed_settings_sections`, `dynamic_profile_sources`, the file-transfer
//! pair, keybindings and snippets) plus the two `#[serde(skip)]` runtime
//! security lists, which are computed state rather than configuration.

mod ai_inspector_config;
mod automation_config;
mod background_config;
mod badge_config;
mod clipboard_config;
mod command_separator_config;
mod copy_mode_config;
mod cursor_config;
mod default_impl;
mod font_config;
mod global_shader_config;
mod image_config;
mod input_config;
mod integration_config;
mod mouse_config;
mod notification_config;
mod pane_config;
mod power_config;
mod progress_bar_config;
mod rendering_config;
mod scrollback_config;
mod scrollbar_config;
mod search_config;
mod security_config;
mod selection_config;
mod semantic_history_config;
mod session_log_config;
mod session_restore_config;
mod shader_overrides_config;
mod shader_watch_config;
mod shell_config;
mod ssh_config;
mod status_bar_config;
mod tab_bar_colors_config;
mod tab_config;
mod theme_colors_config;
mod tmux_config;
mod unicode_config;
mod update;
mod window_config;
mod window_placement_config;
mod word_selection_config;

pub use ai_inspector_config::{AiInspectorConfig, AssistantInputHistoryMode};
pub use automation_config::AutomationConfig;
pub use background_config::BackgroundConfig;
pub use badge_config::BadgeConfig;
pub use clipboard_config::ClipboardConfig;
pub use command_separator_config::CommandSeparatorConfig;
pub use copy_mode_config::CopyModeConfig;
pub use cursor_config::CursorConfig;
pub use font_config::FontRenderingConfig;
pub use global_shader_config::GlobalShaderConfig;
pub use image_config::ImageConfig;
pub use input_config::InputConfig;
pub use integration_config::IntegrationConfig;
pub use mouse_config::MouseConfig;
pub use notification_config::NotificationConfig;
pub use pane_config::PaneConfig;
pub use power_config::PowerConfig;
pub use progress_bar_config::ProgressBarConfig;
pub use rendering_config::RenderingConfig;
pub use scrollback_config::ScrollbackConfig;
pub use scrollbar_config::ScrollbarConfig;
pub use search_config::SearchConfig;
pub use security_config::SecurityConfig;
pub use selection_config::SelectionConfig;
pub use semantic_history_config::SemanticHistoryConfig;
pub use session_log_config::SessionLogConfig;
pub use session_restore_config::SessionRestoreConfig;
pub use shader_overrides_config::ShaderOverridesConfig;
pub use shader_watch_config::ShaderWatchConfig;
pub use shell_config::ShellConfig;
pub use ssh_config::SshConfig;
pub use status_bar_config::StatusBarConfig;
pub use tab_bar_colors_config::TabBarColorsConfig;
pub use tab_config::TabConfig;
pub use theme_colors_config::ThemeColorsConfig;
pub use tmux_config::TmuxConfig;
pub use unicode_config::UnicodeConfig;
pub use update::UpdateConfig;
pub use window_config::WindowConfig;
pub use window_placement_config::WindowPlacementConfig;
pub use word_selection_config::WordSelectionConfig;

use crate::snippets::{CustomActionConfig, SnippetConfig};
use crate::types::{DownloadSaveLocation, FontRange, KeyBinding, LogLevel, ShellExitAction};
use serde::{Deserialize, Serialize};

/// Custom deserializer for `ShellExitAction` that supports backward compatibility.
///
/// Accepts either:
/// - Boolean: `true` → `Close`, `false` → `Keep` (legacy format)
/// - String enum: `"close"`, `"keep"`, `"restart_immediately"`, etc.
pub(crate) fn deserialize_shell_exit_action<'de, D>(
    deserializer: D,
) -> Result<ShellExitAction, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum BoolOrAction {
        Bool(bool),
        Action(ShellExitAction),
    }

    match BoolOrAction::deserialize(deserializer)? {
        BoolOrAction::Bool(true) => Ok(ShellExitAction::Close),
        BoolOrAction::Bool(false) => Ok(ShellExitAction::Keep),
        BoolOrAction::Action(action) => Ok(action),
    }
}

/// Configuration for the terminal emulator
/// Aligned with par-tui-term naming conventions for consistency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    // ========================================================================
    // Window & Display (GUI-specific)
    // ========================================================================

    // --- Terminal Size ---
    /// Number of columns in the terminal
    #[serde(default = "crate::defaults::cols")]
    pub cols: usize,

    /// Number of rows in the terminal
    #[serde(default = "crate::defaults::rows")]
    pub rows: usize,

    // --- Font Settings ---
    /// Font size in points
    #[serde(default = "crate::defaults::font_size")]
    pub font_size: f32,

    /// Font family name (regular/normal weight)
    #[serde(default = "crate::defaults::font_family")]
    pub font_family: String,

    /// Bold font family name (optional, defaults to font_family)
    #[serde(default)]
    pub font_family_bold: Option<String>,

    /// Italic font family name (optional, defaults to font_family)
    #[serde(default)]
    pub font_family_italic: Option<String>,

    /// Bold italic font family name (optional, defaults to font_family)
    #[serde(default)]
    pub font_family_bold_italic: Option<String>,

    /// Custom font mappings for specific Unicode ranges
    /// Format: Vec of (start_codepoint, end_codepoint, font_family_name)
    /// Example: [(0x4E00, 0x9FFF, "Noto Sans CJK SC")] for CJK Unified Ideographs
    #[serde(default)]
    pub font_ranges: Vec<FontRange>,

    /// Line height multiplier (1.0 = default/tight, 1.2 = comfortable, 1.5 = spacious)
    #[serde(default = "crate::defaults::line_spacing")]
    pub line_spacing: f32,

    /// Character width multiplier (1.0 = default, values < 1.0 = narrow, values > 1.0 = wide)
    #[serde(default = "crate::defaults::char_spacing")]
    pub char_spacing: f32,

    /// Enable text shaping for ligatures and complex scripts
    /// When enabled, uses HarfBuzz for proper ligature, emoji, and complex script rendering
    #[serde(default = "crate::defaults::text_shaping")]
    pub enable_text_shaping: bool,

    /// Enable ligatures (requires enable_text_shaping)
    #[serde(default = "crate::defaults::bool_true")]
    pub enable_ligatures: bool,

    /// Enable kerning adjustments (requires enable_text_shaping)
    #[serde(default = "crate::defaults::bool_true")]
    pub enable_kerning: bool,

    // --- Font Rendering Quality (extracted to FontRenderingConfig) ---
    /// Font rendering quality settings: anti-aliasing, hinting, stroke weight, minimum contrast.
    ///
    /// Flattened into the top-level YAML so existing config files remain compatible.
    /// Access via `config.font_rendering.font_antialias`, etc.
    ///
    /// See [`FontRenderingConfig`] for field documentation.
    #[serde(flatten)]
    pub font_rendering: FontRenderingConfig,

    /// Window title
    #[serde(default = "crate::defaults::window_title")]
    pub window_title: String,

    /// Allow applications to change the window title via OSC escape sequences
    /// When false, the window title will always be the configured window_title
    #[serde(default = "crate::defaults::bool_true")]
    pub allow_title_change: bool,

    // ========================================================================
    // Frame Pacing & GPU
    // ========================================================================
    /// Frame rate target, VSync mode, GPU preference and output batching.
    ///
    /// Flattened into the top-level YAML so existing config files remain
    /// compatible. See [`RenderingConfig`].
    #[serde(flatten)]
    pub rendering: RenderingConfig,

    /// Window appearance settings (opacity, padding, decorations, blur, etc.)
    /// Serialised flat at the top level via `#[serde(flatten)]` so that
    /// existing YAML config files require no changes.
    #[serde(flatten)]
    pub window: WindowConfig,

    // ========================================================================
    // Window Placement
    // ========================================================================
    /// Window type, target monitor/Space, resize lock and window numbering.
    ///
    /// Flattened into the top-level YAML so existing config files remain
    /// compatible. See [`WindowPlacementConfig`].
    #[serde(flatten)]
    pub placement: WindowPlacementConfig,

    // ========================================================================
    // Background Image & Transparency
    // ========================================================================
    /// Background image source and how transparency applies to cells and text.
    ///
    /// Flattened into the top-level YAML so existing config files remain
    /// compatible. See [`BackgroundConfig`].
    #[serde(flatten)]
    pub background: BackgroundConfig,

    // ========================================================================
    // Inline Image Settings (Sixel, iTerm2, Kitty)
    // ========================================================================
    /// Inline image scaling and the terminal background source.
    ///
    /// Flattened into the top-level YAML so existing config files remain
    /// compatible. See [`ImageConfig`].
    #[serde(flatten)]
    pub image: ImageConfig,

    // ========================================================================
    // File Transfer Settings
    // ========================================================================
    /// Default save location for downloaded files
    #[serde(default)]
    pub download_save_location: DownloadSaveLocation,

    /// Last used download directory (persisted internally)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_download_directory: Option<String>,

    // ========================================================================
    // Shader Settings (background + cursor) — extracted to GlobalShaderConfig
    // ========================================================================
    /// All `custom_shader_*` and `cursor_shader_*` settings.
    ///
    /// Flattened into the top-level YAML so existing config files remain compatible.
    #[serde(flatten)]
    pub shader: GlobalShaderConfig,

    // ========================================================================
    // Keyboard Input
    // ========================================================================
    /// Option/Alt key behaviour, modifier remapping and physical key positions.
    ///
    /// Flattened into the top-level YAML so existing config files remain
    /// compatible. See [`InputConfig`].
    #[serde(flatten)]
    pub input: InputConfig,

    // ========================================================================
    // Selection & Clipboard
    // ========================================================================
    /// Copy-on-select, paste behaviour and dropped-file quoting.
    ///
    /// Flattened into the top-level YAML so existing config files remain
    /// compatible. See [`SelectionConfig`].
    #[serde(flatten)]
    pub selection: SelectionConfig,

    // ========================================================================
    // Mouse — extracted to MouseConfig
    // ========================================================================
    /// Mouse behavior settings (see [`MouseConfig`]).
    ///
    /// Flattened into the top-level YAML so existing config files remain compatible.
    #[serde(flatten)]
    pub mouse: MouseConfig,

    // ========================================================================
    // Word Selection
    // ========================================================================
    /// Word boundary characters and pattern-based smart selection rules.
    ///
    /// Flattened into the top-level YAML so existing config files remain
    /// compatible. See [`WordSelectionConfig`].
    #[serde(flatten)]
    pub word_selection: WordSelectionConfig,

    // ========================================================================
    // Copy Mode (vi-style keyboard-driven selection)
    // ========================================================================
    /// Vi-style copy mode settings (see [`CopyModeConfig`]).
    #[serde(flatten)]
    pub copy_mode: CopyModeConfig,

    // ========================================================================
    // Scrollback & Cursor
    // ========================================================================
    /// Scrollback buffer size settings (see [`ScrollbackConfig`]).
    ///
    /// Flattened into the top-level YAML so existing config files remain compatible.
    #[serde(flatten)]
    pub scrollback: ScrollbackConfig,

    // ========================================================================
    // Unicode Width Settings
    // ========================================================================
    /// Unicode character width and normalization settings (see [`UnicodeConfig`]).
    #[serde(flatten)]
    pub unicode: UnicodeConfig,

    // ========================================================================
    // Cursor — extracted to CursorConfig
    // ========================================================================
    /// Cursor appearance and behavior settings (see [`CursorConfig`]).
    ///
    /// Flattened into the top-level YAML so existing config files remain compatible.
    #[serde(flatten)]
    pub cursor: CursorConfig,

    // ========================================================================
    // Scrollbar
    // ========================================================================
    /// Scrollbar placement, size, colours, command marks and auto-hide.
    ///
    /// Flattened into the top-level YAML so existing config files remain
    /// compatible. See [`ScrollbarConfig`].
    #[serde(flatten)]
    pub scrollbar: ScrollbarConfig,

    // ========================================================================
    // Theme & Colors
    // ========================================================================
    /// Terminal colour theme and the light/dark pair used by auto dark mode.
    ///
    /// Flattened into the top-level YAML so existing config files remain
    /// compatible. See [`ThemeColorsConfig`].
    #[serde(flatten)]
    pub theme_colors: ThemeColorsConfig,

    // ========================================================================
    // Screenshot
    // ========================================================================
    /// File format for screenshots (png, jpeg, svg, html)
    #[serde(default = "crate::defaults::screenshot_format")]
    pub screenshot_format: String,

    // ========================================================================
    // Shell Behavior
    // ========================================================================
    /// Which shell runs, where it starts, what it is sent, and exit handling.
    ///
    /// Flattened into the top-level YAML so existing config files remain
    /// compatible. See [`ShellConfig`].
    #[serde(flatten)]
    pub shell: ShellConfig,

    // ========================================================================
    // Semantic History
    // ========================================================================
    /// File path/URL detection, the editor that opens them, and link highlighting.
    ///
    /// Flattened into the top-level YAML so existing config files remain
    /// compatible. See [`SemanticHistoryConfig`].
    #[serde(flatten)]
    pub semantic_history: SemanticHistoryConfig,

    // ========================================================================
    // Scrollbar (GUI-specific)
    // ========================================================================
    // ========================================================================
    // Command Separator Lines
    // ========================================================================
    /// Horizontal separator lines drawn between shell commands.
    ///
    /// Flattened into the top-level YAML so existing config files remain
    /// compatible. See [`CommandSeparatorConfig`].
    #[serde(flatten)]
    pub command_separator: CommandSeparatorConfig,

    // ========================================================================
    // Clipboard Sync Limits
    // ========================================================================
    /// Clipboard sync event retention and OSC 52 clipboard-set policy.
    ///
    /// Flattened into the top-level YAML so existing config files remain
    /// compatible. See [`ClipboardConfig`].
    #[serde(flatten)]
    pub clipboard: ClipboardConfig,

    // ========================================================================
    // OSC Sequence Limits
    // ========================================================================
    /// Maximum total OSC data length in bytes before a sequence is rejected
    /// by the core terminal (QA-012 memory-exhaustion guard). Must be large
    /// enough for inline images (iTerm2/Kitty base64) if used.
    #[serde(default = "crate::defaults::max_osc_data_length")]
    pub max_osc_data_length: usize,

    // ========================================================================
    // Command History
    // ========================================================================
    /// Maximum number of commands to persist in fuzzy search history
    #[serde(default = "crate::defaults::command_history_max_entries")]
    pub command_history_max_entries: usize,

    // ========================================================================
    // Notifications — extracted to NotificationConfig
    // ========================================================================
    /// Bell, activity/silence alerts, anti-idle keep-alive, and OSC 9/777 buffer
    /// settings (see [`NotificationConfig`]).
    ///
    /// Flattened into the top-level YAML so existing config files remain compatible.
    #[serde(flatten)]
    pub notifications: NotificationConfig,

    // ========================================================================
    // SSH Settings
    // ========================================================================
    /// SSH discovery and profile-switching settings (see [`SshConfig`]).
    #[serde(flatten)]
    pub ssh: SshConfig,

    // ========================================================================
    // Tab Settings
    // ========================================================================
    /// Tab style presets, tab bar placement and new-tab behaviour.
    ///
    /// Flattened into the top-level YAML so existing config files remain
    /// compatible. See [`TabConfig`].
    #[serde(flatten)]
    pub tabs: TabConfig,

    // ========================================================================
    // Tab Bar Colors
    // ========================================================================
    /// Tab bar palette, dimming, sizing and border appearance.
    ///
    /// Flattened into the top-level YAML so existing config files remain
    /// compatible. See [`TabBarColorsConfig`].
    #[serde(flatten)]
    pub tab_colors: TabBarColorsConfig,

    // ========================================================================
    // Split Pane Settings
    // ========================================================================
    /// Split-pane divider, padding, title bar and focus indicator settings.
    ///
    /// Flattened into the top-level YAML so existing config files remain
    /// compatible. See [`PaneConfig`].
    #[serde(flatten)]
    pub panes: PaneConfig,

    // ========================================================================
    // tmux Integration
    // ========================================================================
    /// tmux control-mode integration settings.
    ///
    /// Flattened into the top-level YAML so existing config files remain
    /// compatible. See [`TmuxConfig`].
    #[serde(flatten)]
    pub tmux: TmuxConfig,

    // ========================================================================
    // Focus/Blur Power Saving
    // ========================================================================
    /// Shader pausing and reduced frame rates when unfocused or inactive.
    ///
    /// Flattened into the top-level YAML so existing config files remain
    /// compatible. See [`PowerConfig`].
    #[serde(flatten)]
    pub power: PowerConfig,

    // ========================================================================
    // Shader Hot Reload
    // ========================================================================
    /// Automatic reloading of shader files when they change on disk.
    ///
    /// Flattened into the top-level YAML so existing config files remain
    /// compatible. See [`ShaderWatchConfig`].
    #[serde(flatten)]
    pub shader_watch: ShaderWatchConfig,

    // ========================================================================
    // Per-Shader Configuration Overrides
    // ========================================================================
    /// Per-file overrides for background and cursor shader settings.
    ///
    /// Flattened into the top-level YAML so existing config files remain
    /// compatible. See [`ShaderOverridesConfig`].
    #[serde(flatten)]
    pub shader_overrides: ShaderOverridesConfig,

    // ========================================================================
    // Keybindings
    // ========================================================================
    /// Custom keybindings (checked before built-in shortcuts)
    /// Format: key = "CmdOrCtrl+Shift+B", action = "toggle_background_shader"
    #[serde(default = "crate::defaults::keybindings")]
    pub keybindings: Vec<KeyBinding>,

    /// Optional global prefix key for custom actions using per-action prefix chars.
    /// Uses the same combo format as normal keybindings (for example "Ctrl+B").
    /// Leave empty to disable prefix-mode custom actions.
    #[serde(default = "crate::defaults::custom_action_prefix_key")]
    pub custom_action_prefix_key: String,

    // ========================================================================
    // Shader & Shell Integration Installation
    // ========================================================================
    /// Install prompts and version tracking for bundled integrations.
    ///
    /// Flattened into the top-level YAML so existing config files remain
    /// compatible. See [`IntegrationConfig`].
    #[serde(flatten)]
    pub integrations: IntegrationConfig,

    // ========================================================================
    // Update Checking
    // ========================================================================
    /// Configuration for automatic update checking
    #[serde(flatten)]
    pub updates: UpdateConfig,

    // ========================================================================
    // Window Arrangements & Session Restore
    // ========================================================================
    /// Startup arrangement restore, session restore and closed-tab undo.
    ///
    /// Flattened into the top-level YAML so existing config files remain
    /// compatible. See [`SessionRestoreConfig`].
    #[serde(flatten)]
    pub session_restore: SessionRestoreConfig,

    // ========================================================================
    // Search Settings
    // ========================================================================
    /// Terminal search settings (see [`SearchConfig`]).
    #[serde(flatten)]
    pub search: SearchConfig,

    // ========================================================================
    // Session Logging
    // ========================================================================
    /// Automatic session recording: format, destination and redaction.
    ///
    /// Flattened into the top-level YAML so existing config files remain
    /// compatible. See [`SessionLogConfig`].
    #[serde(flatten)]
    pub session_log: SessionLogConfig,

    // ========================================================================
    // Debug Logging
    // ========================================================================
    /// Log level for debug log file output.
    /// Controls verbosity of `par_term_debug.log` in the system temp directory
    /// (`$TMPDIR` on macOS, `/tmp` on Linux, `%TEMP%` on Windows).
    /// Environment variable RUST_LOG and --log-level CLI flag take precedence.
    #[serde(default)]
    pub log_level: LogLevel,

    // ========================================================================
    // Badge Settings (iTerm2-style session labels)
    // ========================================================================
    /// Badge overlay text, colour, font and placement.
    ///
    /// Flattened into the top-level YAML so existing config files remain
    /// compatible. See [`BadgeConfig`].
    #[serde(flatten)]
    pub badge: BadgeConfig,

    // ========================================================================
    // Status Bar Settings
    // ========================================================================
    /// Status bar settings (see [`StatusBarConfig`]).
    ///
    /// All `status_bar_*` fields are flattened here for YAML backward-compatibility.
    #[serde(flatten)]
    pub status_bar: StatusBarConfig,

    // ========================================================================
    // Progress Bar Settings (OSC 9;4 and OSC 934)
    // ========================================================================
    /// Progress bar overlay style, placement and state colours.
    ///
    /// Flattened into the top-level YAML so existing config files remain
    /// compatible. See [`ProgressBarConfig`].
    #[serde(flatten)]
    pub progress_bar: ProgressBarConfig,

    // ========================================================================
    // Triggers & Automation
    // ========================================================================
    /// Regex triggers, coprocess definitions and external observer scripts.
    ///
    /// Flattened into the top-level YAML so existing config files remain
    /// compatible. See [`AutomationConfig`].
    #[serde(flatten)]
    pub automation: AutomationConfig,

    // ========================================================================
    // Snippets & Actions
    // ========================================================================
    /// Text snippets for quick insertion
    #[serde(default)]
    pub snippets: Vec<SnippetConfig>,

    /// Custom actions (shell commands, text insertion, key sequences)
    #[serde(default)]
    pub actions: Vec<CustomActionConfig>,

    // ========================================================================
    // UI State (persisted across sessions)
    // ========================================================================
    /// Settings window section IDs that have been toggled from their default collapse state.
    /// Sections default to open unless specified otherwise; IDs in this set invert the default.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub collapsed_settings_sections: Vec<String>,

    // ========================================================================
    // Dynamic Profile Sources
    // ========================================================================
    /// Remote URLs to fetch profile definitions from
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dynamic_profile_sources: Vec<crate::profile::DynamicProfileSource>,

    // ========================================================================
    // Security
    // ========================================================================
    /// Environment variable substitution allowlist and plain-HTTP profile fetching.
    ///
    /// Flattened into the top-level YAML so existing config files remain
    /// compatible. See [`SecurityConfig`].
    #[serde(flatten)]
    pub security: SecurityConfig,

    // ========================================================================
    // AI Inspector
    // ========================================================================
    /// AI Inspector side panel settings (see [`AiInspectorConfig`]).
    ///
    /// All `ai_inspector_*` fields are flattened here for YAML backward-compatibility.
    #[serde(flatten)]
    pub ai_inspector: AiInspectorConfig,

    // ========================================================================
    // Runtime security state (never serialized to disk)
    // ========================================================================
    /// Names of triggers that have `prompt_before_run: false` with dangerous
    /// actions (`RunCommand` or `SendText`).
    ///
    /// Populated by `Config::warn_insecure_triggers` during config load.
    /// The UI reads this list to display a persistent visual warning banner
    /// so users are aware of the reduced security posture.
    ///
    /// This field is intentionally skipped by serde — it is computed at
    /// runtime and never written to or read from the config file.
    #[serde(skip)]
    pub insecure_trigger_names: Vec<String>,

    /// Names of triggers with `prompt_before_run: false` that are missing the
    /// explicit `i_accept_the_risk: true` opt-in.
    ///
    /// Dangerous actions for these triggers are **blocked** at execution time.
    /// Users must add `i_accept_the_risk: true` to each such trigger to permit
    /// automatic execution without the confirmation dialog.
    ///
    /// This field is intentionally skipped by serde — it is computed at
    /// runtime and never written to or read from the config file.
    #[serde(skip)]
    pub unaccepted_risk_trigger_names: Vec<String>,
}

#[cfg(test)]
mod new_tab_position_config_tests {
    use super::*;
    use crate::NewTabPosition;

    #[test]
    fn new_tab_position_defaults_to_end() {
        let config = Config::default();
        assert_eq!(config.tabs.new_tab_position, NewTabPosition::End);
    }

    #[test]
    fn new_tab_position_deserializes_from_yaml() {
        let yaml = "new_tab_position: after_active";
        let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
        assert_eq!(config.tabs.new_tab_position, NewTabPosition::AfterActive);
    }

    #[test]
    fn config_without_new_tab_position_deserializes_to_default() {
        // Existing configs that don't have this field must deserialize cleanly — zero migration.
        let yaml = "tab_inherit_cwd: true";
        let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
        assert_eq!(config.tabs.new_tab_position, NewTabPosition::End);
    }
}
