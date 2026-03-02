//! Core `Config` struct definition.
//!
//! This module contains the main configuration struct with all terminal,
//! display, input, and feature settings.
//!
//! # Sub-modules
//!
//! - [`default_impl`] — `impl Default for Config`
//! - [`copy_mode_config`] — [`CopyModeConfig`]: vi-style copy mode settings
//! - [`global_shader_config`] — [`GlobalShaderConfig`]: all `custom_shader_*` and `cursor_shader_*` fields
//! - [`search_config`] — [`SearchConfig`]: search highlight and options
//! - [`ssh_config`] — [`SshConfig`]: SSH discovery and profile switching
//! - [`unicode_config`] — [`UnicodeConfig`]: Unicode width and normalization
//! - [`update`] — [`UpdateConfig`]: automatic update checking
//!
//! # Splitting Strategy
//!
//! Fields are grouped into sub-structs using `#[serde(flatten)]` so that
//! existing YAML config files remain 100% compatible — flattened fields are
//! serialised at the top level, indistinguishable from direct struct fields.
//!
//! Remaining inline sections (not yet extracted to sub-structs) and their
//! candidate names are:
//!
//! | Section comment              | Candidate sub-struct   |
//! |------------------------------|------------------------|
//! | Window & Display             | `WindowConfig`         |
//! | Inline Image Settings        | `ImageConfig`          |
//! | File Transfer Settings       | `FileTransferConfig`   |
//! | Background + Cursor Shaders  | `GlobalShaderConfig` (done) |
//! | Keyboard Input               | `InputConfig`          |
//! | Selection & Clipboard        | `SelectionConfig`      |
//! | Mouse Behavior               | `MouseConfig`          |
//! | Word Selection               | `WordSelectionConfig`  |
//! | Scrollback & Cursor          | `ScrollbackConfig`     |
//! | Cursor Enhancements          | `CursorConfig`         |
//! | Scrollbar                    | `ScrollbarConfig`      |
//! | Theme & Colors               | `ThemeConfig`          |
//! | Screenshot                   | `ScreenshotConfig`     |
//! | Shell Behavior               | `ShellConfig`          |
//! | Semantic History             | `SemanticHistoryConfig`|
//! | Scrollbar (GUI)              | `ScrollbarUiConfig`    |
//! | Command Separator Lines      | `CommandSeparatorConfig`|
//! | Clipboard Sync Limits        | `ClipboardConfig`      |
//! | Command History              | `CommandHistoryConfig` |
//! | Notifications                | `NotificationConfig`   |
//! | Tab Settings                 | `TabConfig`            |
//! | Tab Bar Colors               | `TabBarColorsConfig`   |
//! | Split Pane Settings          | `PaneConfig`           |
//! | tmux Integration             | `TmuxConfig`           |
//! | Focus/Blur Power Saving      | `PowerConfig`          |
//! | Shader Hot Reload            | `ShaderWatchConfig`    |
//! | Per-Shader Configuration     | `ShaderOverridesConfig`|
//! | Keybindings                  | `KeybindingsConfig`    |
//! | Shader Installation          | `ShaderInstallConfig`  |
//! | Window Arrangements          | `ArrangementConfig`    |
//! | Session Logging              | `SessionLogConfig`     |
//! | Debug Logging                | `DebugConfig`          |
//! | Badge Settings               | `BadgeConfig`          |
//! | Status Bar Settings          | `StatusBarConfig`      |
//! | Progress Bar Settings        | `ProgressBarConfig`    |
//! | Triggers & Automation        | `AutomationConfig`     |
//! | Snippets & Actions           | `SnippetsConfig`       |
//! | Content Prettifier           | `PrettifierConfig`     |
//! | UI State                     | `UiStateConfig`        |
//! | Dynamic Profile Sources      | `ProfileSourcesConfig` |
//! | Security                     | `SecurityConfig`       |
//! | AI Inspector                 | `AiInspectorConfig`    |

mod ai_inspector_config;
mod copy_mode_config;
mod default_impl;
mod global_shader_config;
mod search_config;
mod ssh_config;
mod status_bar_config;
mod unicode_config;
mod update;

pub use ai_inspector_config::AiInspectorConfig;
pub use copy_mode_config::CopyModeConfig;
pub use global_shader_config::GlobalShaderConfig;
pub use search_config::SearchConfig;
pub use ssh_config::SshConfig;
pub use status_bar_config::StatusBarConfig;
pub use unicode_config::UnicodeConfig;
pub use update::UpdateConfig;

use crate::snippets::{CustomActionConfig, SnippetConfig};
use crate::types::{
    AlertEvent, AlertSoundConfig, BackgroundImageMode, BackgroundMode, CursorShaderConfig,
    CursorStyle, DividerStyle, DownloadSaveLocation, DroppedFileQuoteStyle, FontRange,
    ImageScalingMode, InstallPromptState, IntegrationVersions, KeyBinding, LogLevel,
    ModifierRemapping, OptionKeyMode, PaneTitlePosition, PowerPreference, ProgressBarPosition,
    ProgressBarStyle, SemanticHistoryEditorMode, SessionLogFormat, ShaderConfig,
    ShaderInstallPrompt, ShellExitAction, SmartSelectionRule, StartupDirectoryMode, TabBarMode,
    TabBarPosition, TabStyle, TabTitleMode, ThinStrokesMode, UnfocusedCursorStyle, VsyncMode,
    WindowType,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

    /// Line height multiplier (1.0 = tight, 1.2 = default, 1.5 = spacious)
    #[serde(default = "crate::defaults::line_spacing")]
    pub line_spacing: f32,

    /// Character width multiplier (0.5 = narrow, 0.6 = default, 0.7 = wide)
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

    /// Enable anti-aliasing for font rendering
    /// When false, text is rendered without smoothing (aliased/pixelated)
    #[serde(default = "crate::defaults::bool_true")]
    pub font_antialias: bool,

    /// Enable hinting for font rendering
    /// Hinting improves text clarity at small sizes by aligning glyphs to pixel boundaries
    /// Disable for a softer, more "true to design" appearance
    #[serde(default = "crate::defaults::bool_true")]
    pub font_hinting: bool,

    /// Thin strokes / font smoothing mode
    /// Controls stroke weight adjustment for improved rendering on different displays.
    /// - never: Standard stroke weight everywhere
    /// - retina_only: Lighter strokes on HiDPI displays (default)
    /// - dark_backgrounds_only: Lighter strokes on dark backgrounds
    /// - retina_dark_backgrounds_only: Lighter strokes only on HiDPI + dark backgrounds
    /// - always: Always use lighter strokes
    #[serde(default)]
    pub font_thin_strokes: ThinStrokesMode,

    /// Minimum contrast ratio for text against background (WCAG standard)
    /// When set, adjusts foreground colors to ensure they meet the specified contrast ratio.
    /// - 1.0: No adjustment (disabled)
    /// - 4.5: WCAG AA standard for normal text
    /// - 7.0: WCAG AAA standard for normal text
    ///
    /// Range: 1.0 to 21.0 (maximum possible contrast)
    #[serde(default = "crate::defaults::minimum_contrast")]
    pub minimum_contrast: f32,

    /// Window title
    #[serde(default = "crate::defaults::window_title")]
    pub window_title: String,

    /// Allow applications to change the window title via OSC escape sequences
    /// When false, the window title will always be the configured window_title
    #[serde(default = "crate::defaults::bool_true")]
    pub allow_title_change: bool,

    /// Maximum frames per second (FPS) target
    /// Controls how frequently the terminal requests screen redraws.
    /// Note: On macOS, actual FPS may be lower (~22-25) due to system-level
    /// VSync throttling in wgpu/Metal, regardless of this setting.
    /// Default: 60
    #[serde(default = "crate::defaults::max_fps", alias = "refresh_rate")]
    pub max_fps: u32,

    /// VSync mode - controls GPU frame synchronization
    /// - immediate: No VSync, render as fast as possible (lowest latency, highest power)
    /// - mailbox: Cap at monitor refresh rate with triple buffering (balanced)
    /// - fifo: Strict VSync with double buffering (lowest power, slight input lag)
    ///
    /// Default: immediate (for maximum performance)
    #[serde(default)]
    pub vsync_mode: VsyncMode,

    /// GPU power preference for adapter selection
    /// - none: Let the system decide (default)
    /// - low_power: Prefer integrated GPU (saves battery)
    /// - high_performance: Prefer discrete GPU (maximum performance)
    ///
    /// Note: Requires app restart to take effect.
    #[serde(default)]
    pub power_preference: PowerPreference,

    /// Reduce flicker by delaying redraws while cursor is hidden (DECTCEM off).
    /// Many terminal programs hide cursor during bulk updates to prevent visual artifacts.
    #[serde(default = "crate::defaults::reduce_flicker")]
    pub reduce_flicker: bool,

    /// Maximum delay in milliseconds when reduce_flicker is enabled.
    /// Rendering occurs when cursor becomes visible OR this delay expires.
    /// Range: 1-100ms. Default: 16ms (~1 frame at 60fps).
    #[serde(default = "crate::defaults::reduce_flicker_delay_ms")]
    pub reduce_flicker_delay_ms: u32,

    /// Enable throughput mode to batch rendering during bulk output.
    /// When enabled, rendering is throttled to reduce CPU overhead for large outputs.
    /// Toggle with Cmd+Shift+T (macOS) or Ctrl+Shift+T (other platforms).
    #[serde(default = "crate::defaults::maximize_throughput")]
    pub maximize_throughput: bool,

    /// Render interval in milliseconds when maximize_throughput is enabled.
    /// Higher values = better throughput but delayed display. Range: 50-500ms.
    #[serde(default = "crate::defaults::throughput_render_interval_ms")]
    pub throughput_render_interval_ms: u32,

    /// Window padding in pixels
    #[serde(default = "crate::defaults::window_padding")]
    pub window_padding: f32,

    /// Automatically hide window padding when panes are split
    /// When true (default), window padding becomes 0 when the active tab has multiple panes
    #[serde(default = "crate::defaults::bool_true")]
    pub hide_window_padding_on_split: bool,

    /// Window opacity/transparency (0.0 = fully transparent, 1.0 = fully opaque)
    #[serde(default = "crate::defaults::window_opacity")]
    pub window_opacity: f32,

    /// Keep window always on top of other windows
    #[serde(default = "crate::defaults::bool_false")]
    pub window_always_on_top: bool,

    /// Show window decorations (title bar, borders)
    #[serde(default = "crate::defaults::bool_true")]
    pub window_decorations: bool,

    /// Window type (normal, fullscreen, or edge-anchored)
    /// - normal: Standard window (default)
    /// - fullscreen: Start in fullscreen mode
    /// - edge_top/edge_bottom/edge_left/edge_right: Edge-anchored dropdown-style window
    #[serde(default)]
    pub window_type: WindowType,

    /// Target monitor index for window placement (0 = primary)
    /// Use None to let the OS decide window placement
    #[serde(default)]
    pub target_monitor: Option<usize>,

    /// Target macOS Space (virtual desktop) for window placement (1-based ordinal)
    /// Use None to let the OS decide which Space to open on.
    /// Only effective on macOS; ignored on other platforms.
    #[serde(default)]
    pub target_space: Option<u32>,

    /// Lock window size to prevent resize
    /// When true, the window cannot be resized by the user
    #[serde(default = "crate::defaults::bool_false")]
    pub lock_window_size: bool,

    /// Show window number in title bar
    /// Useful when multiple par-term windows are open
    #[serde(default = "crate::defaults::bool_false")]
    pub show_window_number: bool,

    /// When true, only the default terminal background is transparent.
    /// Colored backgrounds (syntax highlighting, status bars, etc.) remain opaque.
    /// This keeps text readable while allowing window transparency.
    #[serde(default = "crate::defaults::bool_true")]
    pub transparency_affects_only_default_background: bool,

    /// When true, text is always rendered at full opacity regardless of window transparency.
    /// This ensures text remains crisp and readable even with transparent backgrounds.
    #[serde(default = "crate::defaults::bool_true")]
    pub keep_text_opaque: bool,

    /// Enable window blur effect (macOS only)
    /// Blurs content behind the transparent window for better readability
    #[serde(default = "crate::defaults::bool_false")]
    pub blur_enabled: bool,

    /// Blur radius in points (0-64, macOS only)
    /// Higher values = more blur. Default: 10
    #[serde(default = "crate::defaults::blur_radius")]
    pub blur_radius: u32,

    /// Background image path (optional, supports ~ for home directory)
    #[serde(default)]
    pub background_image: Option<String>,

    /// Enable or disable background image rendering (even if a path is set)
    #[serde(default = "crate::defaults::bool_true")]
    pub background_image_enabled: bool,

    /// Background image display mode
    /// - fit: Scale to fit window while maintaining aspect ratio (default)
    /// - fill: Scale to fill window while maintaining aspect ratio (may crop)
    /// - stretch: Stretch to fill window (ignores aspect ratio)
    /// - tile: Repeat image in a tiled pattern
    /// - center: Center image at original size
    #[serde(default)]
    pub background_image_mode: BackgroundImageMode,

    /// Background image opacity (0.0 = fully transparent, 1.0 = fully opaque)
    #[serde(default = "crate::defaults::background_image_opacity")]
    pub background_image_opacity: f32,

    // ========================================================================
    // Inline Image Settings (Sixel, iTerm2, Kitty)
    // ========================================================================
    /// Scaling quality for inline images (nearest = sharp/pixel art, linear = smooth)
    #[serde(default)]
    pub image_scaling_mode: ImageScalingMode,

    /// Preserve aspect ratio when scaling inline images to fit terminal cells
    #[serde(default = "crate::defaults::bool_true")]
    pub image_preserve_aspect_ratio: bool,

    /// Background mode selection (default, color, or image)
    #[serde(default)]
    pub background_mode: BackgroundMode,

    /// Per-pane background image configurations
    #[serde(default)]
    pub pane_backgrounds: Vec<crate::config::PaneBackgroundConfig>,

    /// Custom solid background color [R, G, B] (0-255)
    /// Used when background_mode is "color"
    /// Transparency is controlled by window_opacity
    #[serde(default = "crate::defaults::background_color")]
    pub background_color: [u8; 3],

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
    /// Left Option key (macOS) / Left Alt key (Linux/Windows) behavior
    /// - normal: Sends special characters (default macOS behavior)
    /// - meta: Sets the high bit (8th bit) on the character
    /// - esc: Sends Escape prefix before the character (most compatible for emacs/vim)
    #[serde(default)]
    pub left_option_key_mode: OptionKeyMode,

    /// Right Option key (macOS) / Right Alt key (Linux/Windows) behavior
    /// Can be configured independently from left Option key
    /// - normal: Sends special characters (default macOS behavior)
    /// - meta: Sets the high bit (8th bit) on the character
    /// - esc: Sends Escape prefix before the character (most compatible for emacs/vim)
    #[serde(default)]
    pub right_option_key_mode: OptionKeyMode,

    /// Modifier key remapping configuration
    /// Allows remapping modifier keys to different functions (e.g., swap Ctrl and Caps Lock)
    #[serde(default)]
    pub modifier_remapping: ModifierRemapping,

    /// Use physical key positions for keybindings instead of logical characters
    /// When enabled, keybindings work based on key position (scan code) rather than
    /// the character produced, making shortcuts consistent across keyboard layouts.
    /// For example, Ctrl+Z will always be the bottom-left key regardless of QWERTY/AZERTY/Dvorak.
    #[serde(default = "crate::defaults::bool_false")]
    pub use_physical_keys: bool,

    // ========================================================================
    // Selection & Clipboard
    // ========================================================================
    /// Automatically copy selected text to clipboard
    #[serde(default = "crate::defaults::bool_true")]
    pub auto_copy_selection: bool,

    /// Include trailing newline when copying lines
    /// Note: Inverted logic from old strip_trailing_newline_on_copy
    #[serde(
        default = "crate::defaults::bool_false",
        alias = "strip_trailing_newline_on_copy"
    )]
    pub copy_trailing_newline: bool,

    /// Paste on middle mouse button click
    #[serde(default = "crate::defaults::bool_true")]
    pub middle_click_paste: bool,

    /// Delay in milliseconds between pasted lines (0 = no delay)
    /// Useful for slow terminals or remote connections that can't handle rapid paste
    #[serde(default = "crate::defaults::paste_delay_ms")]
    pub paste_delay_ms: u64,

    /// Quote style for dropped file paths
    /// - single_quotes: Wrap in single quotes (safest for most shells)
    /// - double_quotes: Wrap in double quotes
    /// - backslash: Escape special characters with backslashes
    /// - none: Insert path as-is (not recommended)
    #[serde(default)]
    pub dropped_file_quote_style: DroppedFileQuoteStyle,

    // ========================================================================
    // Mouse Behavior
    // ========================================================================
    /// Mouse wheel scroll speed multiplier
    #[serde(default = "crate::defaults::scroll_speed")]
    pub mouse_scroll_speed: f32,

    /// Double-click timing threshold in milliseconds
    #[serde(default = "crate::defaults::double_click_threshold")]
    pub mouse_double_click_threshold: u64,

    /// Triple-click timing threshold in milliseconds (typically same as double-click)
    #[serde(default = "crate::defaults::triple_click_threshold")]
    pub mouse_triple_click_threshold: u64,

    /// Option+Click (macOS) / Alt+Click (Linux/Windows) moves cursor to clicked position
    /// Sends cursor movement escape sequences to position text cursor at click location
    /// Useful for quick cursor positioning in shells and editors
    #[serde(default = "crate::defaults::bool_true")]
    pub option_click_moves_cursor: bool,

    /// Focus window automatically when mouse enters (without requiring a click)
    /// This is an accessibility feature that some users prefer
    #[serde(default = "crate::defaults::bool_false")]
    pub focus_follows_mouse: bool,

    /// Report horizontal scroll events to terminal applications when mouse reporting is enabled
    /// Horizontal scroll uses button codes 6 (left) and 7 (right) in the mouse protocol
    #[serde(default = "crate::defaults::bool_true")]
    pub report_horizontal_scroll: bool,

    // ========================================================================
    // Word Selection
    // ========================================================================
    /// Characters considered part of a word for double-click selection (in addition to alphanumeric)
    /// Default: "/-+\\~_." (matches iTerm2)
    /// Example: If you want to select entire paths, add "/" to include path separators
    #[serde(default = "crate::defaults::word_characters")]
    pub word_characters: String,

    /// Enable smart selection rules for pattern-based double-click selection
    /// When enabled, double-click will try to match patterns like URLs, emails, paths
    /// before falling back to word boundary selection
    #[serde(default = "crate::defaults::smart_selection_enabled")]
    pub smart_selection_enabled: bool,

    /// Smart selection rules for pattern-based double-click selection
    /// Rules are evaluated by precision (highest first). If a pattern matches
    /// at the cursor position, that text is selected instead of using word boundaries.
    #[serde(default = "crate::types::default_smart_selection_rules")]
    pub smart_selection_rules: Vec<SmartSelectionRule>,

    // ========================================================================
    // Copy Mode (vi-style keyboard-driven selection)
    // ========================================================================
    /// Vi-style copy mode settings (see [`CopyModeConfig`]).
    #[serde(flatten)]
    pub copy_mode: CopyModeConfig,

    // ========================================================================
    // Scrollback & Cursor
    // ========================================================================
    /// Maximum number of lines to keep in scrollback buffer
    #[serde(default = "crate::defaults::scrollback", alias = "scrollback_size")]
    pub scrollback_lines: usize,

    // ========================================================================
    // Unicode Width Settings
    // ========================================================================
    /// Unicode character width and normalization settings (see [`UnicodeConfig`]).
    #[serde(flatten)]
    pub unicode: UnicodeConfig,

    /// Enable cursor blinking
    #[serde(default = "crate::defaults::bool_false")]
    pub cursor_blink: bool,

    /// Cursor blink interval in milliseconds
    #[serde(default = "crate::defaults::cursor_blink_interval")]
    pub cursor_blink_interval: u64,

    /// Cursor style (block, beam, underline)
    #[serde(default)]
    pub cursor_style: CursorStyle,

    /// Cursor color [R, G, B] (0-255)
    #[serde(default = "crate::defaults::cursor_color")]
    pub cursor_color: [u8; 3],

    /// Color of text under block cursor [R, G, B] (0-255)
    /// If not set (None), uses automatic contrast color
    /// Only affects block cursor style (beam and underline don't obscure text)
    #[serde(default)]
    pub cursor_text_color: Option<[u8; 3]>,

    /// Lock cursor visibility - prevent applications from hiding the cursor
    /// When true, the cursor remains visible regardless of DECTCEM escape sequences
    #[serde(default = "crate::defaults::bool_false")]
    pub lock_cursor_visibility: bool,

    /// Lock cursor style - prevent applications from changing the cursor style
    /// When true, the cursor style from config is always used, ignoring DECSCUSR escape sequences
    #[serde(default = "crate::defaults::bool_false")]
    pub lock_cursor_style: bool,

    /// Lock cursor blink - prevent applications from enabling cursor blink
    /// When true and cursor_blink is false, applications cannot enable blinking cursor
    #[serde(default = "crate::defaults::bool_false")]
    pub lock_cursor_blink: bool,

    // ========================================================================
    // Cursor Enhancements (iTerm2-style features)
    // ========================================================================
    /// Enable horizontal guide line at cursor row for better tracking in wide terminals
    #[serde(default = "crate::defaults::bool_false")]
    pub cursor_guide_enabled: bool,

    /// Cursor guide color [R, G, B, A] (0-255), subtle highlight spanning full terminal width
    #[serde(default = "crate::defaults::cursor_guide_color")]
    pub cursor_guide_color: [u8; 4],

    /// Enable drop shadow behind cursor for better visibility against varying backgrounds
    #[serde(default = "crate::defaults::bool_false")]
    pub cursor_shadow_enabled: bool,

    /// Cursor shadow color [R, G, B, A] (0-255)
    #[serde(default = "crate::defaults::cursor_shadow_color")]
    pub cursor_shadow_color: [u8; 4],

    /// Cursor shadow offset in pixels [x, y]
    #[serde(default = "crate::defaults::cursor_shadow_offset")]
    pub cursor_shadow_offset: [f32; 2],

    /// Cursor shadow blur radius in pixels
    #[serde(default = "crate::defaults::cursor_shadow_blur")]
    pub cursor_shadow_blur: f32,

    /// Cursor boost (glow) intensity (0.0 = off, 1.0 = maximum boost)
    /// Adds a glow/highlight effect around the cursor for visibility
    #[serde(default = "crate::defaults::cursor_boost")]
    pub cursor_boost: f32,

    /// Cursor boost glow color [R, G, B] (0-255)
    #[serde(default = "crate::defaults::cursor_boost_color")]
    pub cursor_boost_color: [u8; 3],

    /// Cursor appearance when window is unfocused
    /// - hollow: Show outline-only block cursor (default, standard terminal behavior)
    /// - same: Keep same cursor style as when focused
    /// - hidden: Hide cursor completely when unfocused
    #[serde(default)]
    pub unfocused_cursor_style: UnfocusedCursorStyle,

    // ========================================================================
    // Scrollbar
    // ========================================================================
    /// Auto-hide scrollbar after inactivity (milliseconds, 0 = never hide)
    #[serde(default = "crate::defaults::scrollbar_autohide_delay")]
    pub scrollbar_autohide_delay: u64,

    // ========================================================================
    // Theme & Colors
    // ========================================================================
    /// Color theme name to use for terminal colors
    #[serde(default = "crate::defaults::theme")]
    pub theme: String,

    /// Automatically switch theme based on system light/dark mode
    #[serde(default)]
    pub auto_dark_mode: bool,

    /// Theme to use when system is in light mode (used when auto_dark_mode is true)
    #[serde(default = "crate::defaults::light_theme")]
    pub light_theme: String,

    /// Theme to use when system is in dark mode (used when auto_dark_mode is true)
    #[serde(default = "crate::defaults::dark_theme")]
    pub dark_theme: String,

    // ========================================================================
    // Screenshot
    // ========================================================================
    /// File format for screenshots (png, jpeg, svg, html)
    #[serde(default = "crate::defaults::screenshot_format")]
    pub screenshot_format: String,

    // ========================================================================
    // Shell Behavior
    // ========================================================================
    /// Action to take when the shell process exits
    /// Supports: close, keep, restart_immediately, restart_with_prompt, restart_after_delay
    /// For backward compatibility, also accepts boolean values (true=close, false=keep)
    #[serde(
        default,
        deserialize_with = "deserialize_shell_exit_action",
        alias = "exit_on_shell_exit",
        alias = "close_on_shell_exit"
    )]
    pub shell_exit_action: ShellExitAction,

    /// Custom shell command (defaults to system shell if not specified)
    #[serde(default)]
    pub custom_shell: Option<String>,

    /// Arguments to pass to the shell
    #[serde(default)]
    pub shell_args: Option<Vec<String>>,

    /// Working directory for the shell (legacy, use startup_directory_mode instead)
    /// When set, overrides startup_directory_mode for backward compatibility
    #[serde(default)]
    pub working_directory: Option<String>,

    /// Startup directory mode: controls where new sessions start
    /// - home: Start in user's home directory (default)
    /// - previous: Remember and restore last working directory from previous session
    /// - custom: Start in the directory specified by startup_directory
    #[serde(default)]
    pub startup_directory_mode: StartupDirectoryMode,

    /// Custom startup directory (used when startup_directory_mode is "custom")
    /// Supports ~ for home directory expansion
    #[serde(default)]
    pub startup_directory: Option<String>,

    /// Last working directory from previous session (auto-managed)
    /// Used when startup_directory_mode is "previous"
    #[serde(default)]
    pub last_working_directory: Option<String>,

    /// Environment variables to set for the shell
    #[serde(default)]
    pub shell_env: Option<std::collections::HashMap<String, String>>,

    /// Whether to spawn the shell as a login shell (passes -l flag)
    /// This is important on macOS to properly initialize PATH from Homebrew, /etc/paths.d, etc.
    /// Default: true
    #[serde(default = "crate::defaults::login_shell")]
    pub login_shell: bool,

    /// Text to send automatically when a terminal session starts
    /// Supports escape sequences: \n (newline), \r (carriage return), \t (tab), \xHH (hex), \e (ESC)
    #[serde(default = "crate::defaults::initial_text")]
    pub initial_text: String,

    /// Delay in milliseconds before sending the initial text (to allow shell to be ready)
    #[serde(default = "crate::defaults::initial_text_delay_ms")]
    pub initial_text_delay_ms: u64,

    /// Whether to append a newline after sending the initial text
    #[serde(default = "crate::defaults::initial_text_send_newline")]
    pub initial_text_send_newline: bool,

    /// Answerback string sent in response to ENQ (0x05) control character
    /// This is a legacy terminal feature used for terminal identification.
    /// Default: empty (disabled) for security
    /// Common values: "par-term", "vt100", or custom identification
    /// Security note: Setting this may expose terminal identification to applications
    #[serde(default = "crate::defaults::answerback_string")]
    pub answerback_string: String,

    /// Show confirmation dialog before quitting the application
    /// When enabled, closing the window will show a confirmation dialog
    /// if there are any open terminal sessions.
    /// Default: false (close immediately without confirmation)
    #[serde(default = "crate::defaults::bool_false")]
    pub prompt_on_quit: bool,

    /// Show confirmation dialog before closing a tab with running jobs
    /// When enabled, closing a tab that has a running command will show a confirmation dialog.
    /// Default: false (close immediately without confirmation)
    #[serde(default = "crate::defaults::bool_false")]
    pub confirm_close_running_jobs: bool,

    /// List of job/process names to ignore when checking for running jobs
    /// These jobs will not trigger a close confirmation dialog.
    /// Common examples: "bash", "zsh", "fish", "cat", "less", "man", "sleep"
    /// Default: common shell names that shouldn't block tab close
    #[serde(default = "crate::defaults::jobs_to_ignore")]
    pub jobs_to_ignore: Vec<String>,

    // ========================================================================
    // Semantic History
    // ========================================================================
    /// Enable semantic history (file path detection and opening)
    /// When enabled, Cmd/Ctrl+Click on detected file paths opens them in the editor.
    #[serde(default = "crate::defaults::bool_true")]
    pub semantic_history_enabled: bool,

    /// Editor selection mode for semantic history
    ///
    /// - `custom` - Use the editor command specified in `semantic_history_editor`
    /// - `environment_variable` - Use `$EDITOR` or `$VISUAL` environment variable (default)
    /// - `system_default` - Use system default application for each file type
    #[serde(default)]
    pub semantic_history_editor_mode: SemanticHistoryEditorMode,

    /// Editor command for semantic history (when mode is `custom`).
    ///
    /// Placeholders: `{file}` = file path, `{line}` = line number (if available)
    ///
    /// Examples:
    /// - `code -g {file}:{line}` (VS Code with line number)
    /// - `subl {file}:{line}` (Sublime Text)
    /// - `vim +{line} {file}` (Vim)
    /// - `emacs +{line} {file}` (Emacs)
    #[serde(default = "crate::defaults::semantic_history_editor")]
    pub semantic_history_editor: String,

    /// Color for highlighted links (URLs and file paths) [R, G, B] (0-255)
    #[serde(default = "crate::defaults::link_highlight_color")]
    pub link_highlight_color: [u8; 3],

    /// Underline highlighted links (URLs and file paths)
    #[serde(default = "crate::defaults::bool_true")]
    pub link_highlight_underline: bool,

    /// Style for link highlight underlines (solid or stipple)
    #[serde(default)]
    pub link_underline_style: crate::types::LinkUnderlineStyle,

    /// Custom command to open URLs. When set, used instead of system default browser.
    ///
    /// Use `{url}` as placeholder for the URL.
    ///
    /// Examples:
    /// - `firefox {url}` (open in Firefox)
    /// - `open -a Safari {url}` (macOS: open in Safari)
    /// - `chromium-browser {url}` (Linux: open in Chromium)
    ///
    /// When empty or unset, uses the system default browser.
    #[serde(default)]
    pub link_handler_command: String,

    // ========================================================================
    // Scrollbar (GUI-specific)
    // ========================================================================
    /// Scrollbar position (left or right)
    #[serde(default = "crate::defaults::scrollbar_position")]
    pub scrollbar_position: String,

    /// Scrollbar width in pixels
    #[serde(default = "crate::defaults::scrollbar_width")]
    pub scrollbar_width: f32,

    /// Scrollbar thumb color (RGBA: [r, g, b, a] where each is 0.0-1.0)
    #[serde(default = "crate::defaults::scrollbar_thumb_color")]
    pub scrollbar_thumb_color: [f32; 4],

    /// Scrollbar track color (RGBA: [r, g, b, a] where each is 0.0-1.0)
    #[serde(default = "crate::defaults::scrollbar_track_color")]
    pub scrollbar_track_color: [f32; 4],

    /// Show command markers on the scrollbar (requires shell integration)
    #[serde(default = "crate::defaults::bool_true")]
    pub scrollbar_command_marks: bool,

    /// Show tooltips when hovering over scrollbar command markers
    #[serde(default = "crate::defaults::bool_false")]
    pub scrollbar_mark_tooltips: bool,

    // ========================================================================
    // Command Separator Lines
    // ========================================================================
    /// Show horizontal separator lines between commands (requires shell integration)
    #[serde(default = "crate::defaults::bool_false")]
    pub command_separator_enabled: bool,

    /// Thickness of command separator lines in pixels
    #[serde(default = "crate::defaults::command_separator_thickness")]
    pub command_separator_thickness: f32,

    /// Opacity of command separator lines (0.0-1.0)
    #[serde(default = "crate::defaults::command_separator_opacity")]
    pub command_separator_opacity: f32,

    /// Color separator lines by exit code (green=success, red=failure, gray=unknown)
    #[serde(default = "crate::defaults::bool_true")]
    pub command_separator_exit_color: bool,

    /// Custom color for separator lines when exit_color is disabled [R, G, B]
    #[serde(default = "crate::defaults::command_separator_color")]
    pub command_separator_color: [u8; 3],

    // ========================================================================
    // Clipboard Sync Limits
    // ========================================================================
    /// Maximum clipboard sync events retained for diagnostics
    #[serde(
        default = "crate::defaults::clipboard_max_sync_events",
        alias = "max_clipboard_sync_events"
    )]
    pub clipboard_max_sync_events: usize,

    /// Maximum bytes stored per clipboard sync event
    #[serde(
        default = "crate::defaults::clipboard_max_event_bytes",
        alias = "max_clipboard_event_bytes"
    )]
    pub clipboard_max_event_bytes: usize,

    // ========================================================================
    // Command History
    // ========================================================================
    /// Maximum number of commands to persist in fuzzy search history
    #[serde(default = "crate::defaults::command_history_max_entries")]
    pub command_history_max_entries: usize,

    // ========================================================================
    // Notifications
    // ========================================================================
    /// Forward BEL events to desktop notification centers
    #[serde(default = "crate::defaults::bool_false", alias = "bell_desktop")]
    pub notification_bell_desktop: bool,

    /// Volume (0-100) for backend bell sound alerts (0 disables)
    #[serde(default = "crate::defaults::bell_sound", alias = "bell_sound")]
    pub notification_bell_sound: u8,

    /// Enable backend visual bell overlay
    #[serde(default = "crate::defaults::bool_true", alias = "bell_visual")]
    pub notification_bell_visual: bool,

    /// Enable notifications when activity resumes after inactivity
    #[serde(
        default = "crate::defaults::bool_false",
        alias = "activity_notifications"
    )]
    pub notification_activity_enabled: bool,

    /// Seconds of inactivity required before an activity alert fires
    #[serde(
        default = "crate::defaults::activity_threshold",
        alias = "activity_threshold"
    )]
    pub notification_activity_threshold: u64,

    /// Enable anti-idle keep-alive (sends code after idle period)
    #[serde(default = "crate::defaults::bool_false")]
    pub anti_idle_enabled: bool,

    /// Seconds of inactivity before sending keep-alive code
    #[serde(default = "crate::defaults::anti_idle_seconds")]
    pub anti_idle_seconds: u64,

    /// ASCII code to send as keep-alive (e.g., 0 = NUL, 27 = ESC)
    #[serde(default = "crate::defaults::anti_idle_code")]
    pub anti_idle_code: u8,

    /// Enable notifications after prolonged silence
    #[serde(
        default = "crate::defaults::bool_false",
        alias = "silence_notifications"
    )]
    pub notification_silence_enabled: bool,

    /// Seconds of silence before a silence alert fires
    #[serde(
        default = "crate::defaults::silence_threshold",
        alias = "silence_threshold"
    )]
    pub notification_silence_threshold: u64,

    /// Enable notification when a shell/session exits
    #[serde(default = "crate::defaults::bool_false", alias = "session_ended")]
    pub notification_session_ended: bool,

    /// Suppress desktop notifications when the terminal window is focused
    #[serde(default = "crate::defaults::bool_true")]
    pub suppress_notifications_when_focused: bool,

    /// Maximum number of OSC 9/777 notification entries retained by backend
    #[serde(
        default = "crate::defaults::notification_max_buffer",
        alias = "max_notifications"
    )]
    pub notification_max_buffer: usize,

    /// Alert sound configuration per event type
    /// Maps AlertEvent variants to their sound settings
    #[serde(default)]
    pub alert_sounds: HashMap<AlertEvent, AlertSoundConfig>,

    // ========================================================================
    // SSH Settings
    // ========================================================================
    /// SSH discovery and profile-switching settings (see [`SshConfig`]).
    #[serde(flatten)]
    pub ssh: SshConfig,

    // ========================================================================
    // Tab Settings
    // ========================================================================
    /// Tab visual style preset (dark, light, compact, minimal, high_contrast, automatic)
    /// Applies cosmetic color/size/spacing presets to the tab bar
    #[serde(default)]
    pub tab_style: TabStyle,

    /// Tab style to use when system is in light mode (used when tab_style is Automatic)
    #[serde(default = "crate::defaults::light_tab_style")]
    pub light_tab_style: TabStyle,

    /// Tab style to use when system is in dark mode (used when tab_style is Automatic)
    #[serde(default = "crate::defaults::dark_tab_style")]
    pub dark_tab_style: TabStyle,

    /// Tab bar visibility mode (always, when_multiple, never)
    #[serde(default)]
    pub tab_bar_mode: TabBarMode,

    /// Controls how tab titles are automatically updated (auto or osc_only)
    #[serde(default)]
    pub tab_title_mode: TabTitleMode,

    /// Tab bar height in pixels
    #[serde(default = "crate::defaults::tab_bar_height")]
    pub tab_bar_height: f32,

    /// Tab bar position (top, bottom, left)
    #[serde(default)]
    pub tab_bar_position: TabBarPosition,

    /// Tab bar width in pixels (used when tab_bar_position is Left)
    #[serde(default = "crate::defaults::tab_bar_width")]
    pub tab_bar_width: f32,

    /// Show close button on tabs
    #[serde(default = "crate::defaults::bool_true")]
    pub tab_show_close_button: bool,

    /// Show tab index numbers (for Cmd+1-9)
    #[serde(default = "crate::defaults::bool_false")]
    pub tab_show_index: bool,

    /// New tab inherits working directory from active tab
    #[serde(default = "crate::defaults::bool_true")]
    pub tab_inherit_cwd: bool,

    /// Maximum tabs per window (0 = unlimited)
    #[serde(default = "crate::defaults::zero")]
    pub max_tabs: usize,

    /// Show the profile drawer toggle button on the right edge of the terminal
    /// When disabled, the profile drawer can still be opened via keyboard shortcut
    #[serde(default = "crate::defaults::bool_false")]
    pub show_profile_drawer_button: bool,

    /// When true, the new-tab keyboard shortcut (Cmd+T / Ctrl+Shift+T) shows the
    /// profile selection dropdown instead of immediately opening a default tab
    #[serde(default = "crate::defaults::bool_false")]
    pub new_tab_shortcut_shows_profiles: bool,

    // ========================================================================
    // Tab Bar Colors
    // ========================================================================
    /// Tab bar background color [R, G, B] (0-255)
    #[serde(default = "crate::defaults::tab_bar_background")]
    pub tab_bar_background: [u8; 3],

    /// Active tab background color [R, G, B] (0-255)
    #[serde(default = "crate::defaults::tab_active_background")]
    pub tab_active_background: [u8; 3],

    /// Inactive tab background color [R, G, B] (0-255)
    #[serde(default = "crate::defaults::tab_inactive_background")]
    pub tab_inactive_background: [u8; 3],

    /// Hovered tab background color [R, G, B] (0-255)
    #[serde(default = "crate::defaults::tab_hover_background")]
    pub tab_hover_background: [u8; 3],

    /// Active tab text color [R, G, B] (0-255)
    #[serde(default = "crate::defaults::tab_active_text")]
    pub tab_active_text: [u8; 3],

    /// Inactive tab text color [R, G, B] (0-255)
    #[serde(default = "crate::defaults::tab_inactive_text")]
    pub tab_inactive_text: [u8; 3],

    /// Active tab indicator/underline color [R, G, B] (0-255)
    #[serde(default = "crate::defaults::tab_active_indicator")]
    pub tab_active_indicator: [u8; 3],

    /// Activity indicator dot color [R, G, B] (0-255)
    #[serde(default = "crate::defaults::tab_activity_indicator")]
    pub tab_activity_indicator: [u8; 3],

    /// Bell indicator color [R, G, B] (0-255)
    #[serde(default = "crate::defaults::tab_bell_indicator")]
    pub tab_bell_indicator: [u8; 3],

    /// Close button color [R, G, B] (0-255)
    #[serde(default = "crate::defaults::tab_close_button")]
    pub tab_close_button: [u8; 3],

    /// Close button hover color [R, G, B] (0-255)
    #[serde(default = "crate::defaults::tab_close_button_hover")]
    pub tab_close_button_hover: [u8; 3],

    /// Enable visual dimming of inactive tabs
    /// When true, inactive tabs are rendered with reduced opacity
    #[serde(default = "crate::defaults::bool_true")]
    pub dim_inactive_tabs: bool,

    /// Opacity level for inactive tabs (0.0-1.0)
    /// Only used when dim_inactive_tabs is true
    /// Lower values make inactive tabs more transparent/dimmed
    #[serde(default = "crate::defaults::inactive_tab_opacity")]
    pub inactive_tab_opacity: f32,

    /// Minimum tab width in pixels before horizontal scrolling is enabled
    /// When tabs cannot fit at this width, scroll buttons appear
    #[serde(default = "crate::defaults::tab_min_width")]
    pub tab_min_width: f32,

    /// Stretch tabs to fill the available tab bar width evenly (iTerm2 style)
    /// When false, tabs keep their minimum width and excess space is left unused
    #[serde(default = "crate::defaults::tab_stretch_to_fill")]
    pub tab_stretch_to_fill: bool,

    /// Render tab titles as limited HTML (bold/italic/underline/color spans)
    /// When false, titles are rendered as plain text
    #[serde(default = "crate::defaults::tab_html_titles")]
    pub tab_html_titles: bool,

    /// Tab border color [R, G, B] (0-255)
    /// A thin border around each tab to help distinguish them
    #[serde(default = "crate::defaults::tab_border_color")]
    pub tab_border_color: [u8; 3],

    /// Tab border width in pixels (0 = no border)
    #[serde(default = "crate::defaults::tab_border_width")]
    pub tab_border_width: f32,

    /// Render inactive tabs as outline only (no fill)
    /// When true, inactive tabs show only a border stroke with no background fill.
    /// Hovered inactive tabs brighten the outline instead of filling.
    #[serde(default = "crate::defaults::bool_false")]
    pub tab_inactive_outline_only: bool,

    // ========================================================================
    // Split Pane Settings
    // ========================================================================
    /// Width of dividers between panes in pixels (visual width)
    #[serde(default = "crate::defaults::pane_divider_width")]
    pub pane_divider_width: Option<f32>,

    /// Width of the drag hit area for resizing panes (should be >= divider width)
    /// A larger hit area makes it easier to grab the divider for resizing
    #[serde(default = "crate::defaults::pane_divider_hit_width")]
    pub pane_divider_hit_width: f32,

    /// Padding inside panes in pixels (space between content and border/divider)
    #[serde(default = "crate::defaults::pane_padding")]
    pub pane_padding: f32,

    /// Minimum pane size in cells (columns for horizontal splits, rows for vertical)
    /// Prevents panes from being resized too small to be useful
    #[serde(default = "crate::defaults::pane_min_size")]
    pub pane_min_size: usize,

    /// Pane background opacity (0.0 = fully transparent, 1.0 = fully opaque)
    /// Lower values allow background image/shader to show through pane backgrounds
    #[serde(default = "crate::defaults::pane_background_opacity")]
    pub pane_background_opacity: f32,

    /// Pane divider color [R, G, B] (0-255)
    #[serde(default = "crate::defaults::pane_divider_color")]
    pub pane_divider_color: [u8; 3],

    /// Pane divider hover color [R, G, B] (0-255) - shown when mouse hovers over divider
    #[serde(default = "crate::defaults::pane_divider_hover_color")]
    pub pane_divider_hover_color: [u8; 3],

    /// Enable visual dimming of inactive panes
    #[serde(default = "crate::defaults::bool_false")]
    pub dim_inactive_panes: bool,

    /// Opacity level for inactive panes (0.0-1.0)
    #[serde(default = "crate::defaults::inactive_pane_opacity")]
    pub inactive_pane_opacity: f32,

    /// Show title bar on each pane
    #[serde(default = "crate::defaults::bool_false")]
    pub show_pane_titles: bool,

    /// Height of pane title bars in pixels
    #[serde(default = "crate::defaults::pane_title_height")]
    pub pane_title_height: f32,

    /// Position of pane title bars (top or bottom)
    #[serde(default)]
    pub pane_title_position: PaneTitlePosition,

    /// Pane title text color [R, G, B] (0-255)
    #[serde(default = "crate::defaults::pane_title_color")]
    pub pane_title_color: [u8; 3],

    /// Pane title background color [R, G, B] (0-255)
    #[serde(default = "crate::defaults::pane_title_bg_color")]
    pub pane_title_bg_color: [u8; 3],

    /// Pane title font family (empty string = use terminal font)
    #[serde(default)]
    pub pane_title_font: String,

    /// Style of dividers between panes (solid, double, dashed, shadow)
    #[serde(default)]
    pub pane_divider_style: DividerStyle,

    /// Maximum panes per tab (0 = unlimited)
    #[serde(default = "crate::defaults::max_panes")]
    pub max_panes: usize,

    /// Show visual indicator (border) around focused pane
    #[serde(default = "crate::defaults::bool_true")]
    pub pane_focus_indicator: bool,

    /// Color of the focused pane indicator [R, G, B] (0-255)
    #[serde(default = "crate::defaults::pane_focus_color")]
    pub pane_focus_color: [u8; 3],

    /// Width of the focused pane indicator border in pixels
    #[serde(default = "crate::defaults::pane_focus_width")]
    pub pane_focus_width: f32,

    // ========================================================================
    // tmux Integration
    // ========================================================================
    /// Enable tmux control mode integration
    #[serde(default = "crate::defaults::bool_false")]
    pub tmux_enabled: bool,

    /// Path to tmux executable (default: "tmux" - uses PATH)
    #[serde(default = "crate::defaults::tmux_path")]
    pub tmux_path: String,

    /// Default session name when creating new tmux sessions
    #[serde(default = "crate::defaults::tmux_default_session")]
    pub tmux_default_session: Option<String>,

    /// Auto-attach to existing tmux session on startup
    #[serde(default = "crate::defaults::bool_false")]
    pub tmux_auto_attach: bool,

    /// Session name to auto-attach to (if tmux_auto_attach is true)
    #[serde(default = "crate::defaults::tmux_auto_attach_session")]
    pub tmux_auto_attach_session: Option<String>,

    /// Sync clipboard with tmux paste buffer
    /// When copying in par-term, also update tmux's paste buffer via set-buffer
    #[serde(default = "crate::defaults::bool_true")]
    pub tmux_clipboard_sync: bool,

    /// Profile to switch to when connected to tmux
    /// When profiles feature is implemented, this will automatically
    /// switch to the specified profile when entering tmux mode
    #[serde(default)]
    pub tmux_profile: Option<String>,

    /// Show tmux status bar in par-term UI
    /// When connected to tmux, display the status bar at the bottom of the terminal
    #[serde(default = "crate::defaults::bool_false")]
    pub tmux_show_status_bar: bool,

    /// Tmux status bar refresh interval in milliseconds
    /// How often to poll tmux for updated status bar content.
    /// Lower values mean more frequent updates but slightly more CPU usage.
    /// Default: 1000 (1 second)
    #[serde(default = "crate::defaults::tmux_status_bar_refresh_ms")]
    pub tmux_status_bar_refresh_ms: u64,

    /// Tmux prefix key for control mode
    /// In control mode, par-term intercepts this key combination and waits for a command key.
    /// Format: "C-b" (Ctrl+B, default), "C-Space" (Ctrl+Space), "C-a" (Ctrl+A), etc.
    /// The prefix + command key is translated to the appropriate tmux command.
    #[serde(default = "crate::defaults::tmux_prefix_key")]
    pub tmux_prefix_key: String,

    /// Use native tmux format strings for status bar content
    /// When true, queries tmux for the actual status-left and status-right values
    /// using `display-message -p '#{T:status-left}'` command.
    /// When false, uses par-term's configurable format strings below.
    #[serde(default = "crate::defaults::bool_false")]
    pub tmux_status_bar_use_native_format: bool,

    /// Tmux status bar left side format string.
    ///
    /// Supported variables:
    /// - `{session}` - Session name
    /// - `{windows}` - Window list with active marker (*)
    /// - `{pane}` - Focused pane ID
    /// - `{time:FORMAT}` - Current time with strftime format (e.g., `{time:%H:%M}`)
    /// - `{hostname}` - Machine hostname
    /// - `{user}` - Current username
    ///
    /// Default: `[{session}] {windows}`
    #[serde(default = "crate::defaults::tmux_status_bar_left")]
    pub tmux_status_bar_left: String,

    /// Tmux status bar right side format string.
    ///
    /// Same variables as `tmux_status_bar_left`.
    ///
    /// Default: `{pane} | {time:%H:%M}`
    #[serde(default = "crate::defaults::tmux_status_bar_right")]
    pub tmux_status_bar_right: String,

    // ========================================================================
    // Focus/Blur Power Saving
    // ========================================================================
    /// Pause shader animations when window loses focus
    /// This reduces GPU usage when the terminal is not actively being viewed
    #[serde(default = "crate::defaults::bool_true")]
    pub pause_shaders_on_blur: bool,

    /// Reduce refresh rate when window is not focused
    /// When true, uses unfocused_fps instead of max_fps when window is blurred
    #[serde(default = "crate::defaults::bool_false")]
    pub pause_refresh_on_blur: bool,

    /// Target FPS when window is not focused (only used if pause_refresh_on_blur is true)
    /// Lower values save more power but may delay terminal output visibility
    #[serde(default = "crate::defaults::unfocused_fps")]
    pub unfocused_fps: u32,

    /// Target FPS for inactive (non-visible) tabs
    /// Inactive tabs only need enough polling to detect activity, bells, and shell exit.
    /// Lower values significantly reduce CPU from mutex contention with many tabs open.
    /// Default: 2 (500ms interval)
    #[serde(default = "crate::defaults::inactive_tab_fps")]
    pub inactive_tab_fps: u32,

    // ========================================================================
    // Shader Hot Reload
    // ========================================================================
    /// Enable automatic shader reloading when shader files are modified
    /// This watches custom_shader and cursor_shader files for changes
    #[serde(default = "crate::defaults::bool_false")]
    pub shader_hot_reload: bool,

    /// Debounce delay in milliseconds before reloading shader after file change
    /// Helps avoid multiple reloads during rapid saves from editors
    #[serde(default = "crate::defaults::shader_hot_reload_delay")]
    pub shader_hot_reload_delay: u64,

    // ========================================================================
    // Per-Shader Configuration Overrides
    // ========================================================================
    /// Per-shader configuration overrides (key = shader filename)
    /// These override settings embedded in shader metadata and global defaults
    #[serde(default)]
    pub shader_configs: HashMap<String, ShaderConfig>,

    /// Per-cursor-shader configuration overrides (key = shader filename)
    #[serde(default)]
    pub cursor_shader_configs: HashMap<String, CursorShaderConfig>,

    // ========================================================================
    // Keybindings
    // ========================================================================
    /// Custom keybindings (checked before built-in shortcuts)
    /// Format: key = "CmdOrCtrl+Shift+B", action = "toggle_background_shader"
    #[serde(default = "crate::defaults::keybindings")]
    pub keybindings: Vec<KeyBinding>,

    // ========================================================================
    // Shader Installation
    // ========================================================================
    /// Shader install prompt preference
    /// - ask: Prompt user to install shaders if folder is missing/empty (default)
    /// - never: User declined, don't ask again
    /// - installed: Shaders have been installed
    #[serde(default)]
    pub shader_install_prompt: ShaderInstallPrompt,

    /// Shell integration install state
    #[serde(default)]
    pub shell_integration_state: InstallPromptState,

    /// Version tracking for integrations
    #[serde(default)]
    pub integration_versions: IntegrationVersions,

    // ========================================================================
    // Update Checking
    // ========================================================================
    /// Configuration for automatic update checking
    #[serde(flatten)]
    pub updates: UpdateConfig,

    // ========================================================================
    // Window Arrangements
    // ========================================================================
    /// Name of arrangement to auto-restore on startup (None = disabled)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_restore_arrangement: Option<String>,

    /// Whether to restore the previous session (tabs, panes, CWDs) on startup
    #[serde(default = "crate::defaults::bool_false")]
    pub restore_session: bool,

    /// Seconds to keep closed tab metadata for undo (0 = disabled)
    #[serde(default = "crate::defaults::session_undo_timeout_secs")]
    pub session_undo_timeout_secs: u32,

    /// Maximum number of closed tabs to remember for undo
    #[serde(default = "crate::defaults::session_undo_max_entries")]
    pub session_undo_max_entries: usize,

    /// When true, closing a tab hides the shell instead of killing it.
    /// Undo restores the full session with scrollback and running processes.
    #[serde(default = "crate::defaults::session_undo_preserve_shell")]
    pub session_undo_preserve_shell: bool,

    // ========================================================================
    // Search Settings
    // ========================================================================
    /// Terminal search settings (see [`SearchConfig`]).
    #[serde(flatten)]
    pub search: SearchConfig,

    // ========================================================================
    // Session Logging
    // ========================================================================
    /// Automatically record all terminal sessions
    /// When enabled, all terminal output is logged to files in the log directory
    #[serde(default = "crate::defaults::bool_false")]
    pub auto_log_sessions: bool,

    /// Log format for session recording
    /// - plain: Simple text output without escape sequences
    /// - html: Rendered output with colors preserved
    /// - asciicast: asciinema-compatible format for replay/sharing (default)
    #[serde(default)]
    pub session_log_format: SessionLogFormat,

    /// Directory where session logs are saved
    /// Default: ~/.local/share/par-term/logs/
    #[serde(default = "crate::defaults::session_log_directory")]
    pub session_log_directory: String,

    /// Automatically save session log when tab/window closes
    /// When true, ensures the session is fully written before the tab closes
    #[serde(default = "crate::defaults::bool_true")]
    pub archive_on_close: bool,

    /// Redact input during password prompts in session logs.
    /// When enabled, the session logger detects password prompts (sudo, ssh, etc.)
    /// by monitoring terminal output for common prompt patterns, and replaces
    /// any keyboard input recorded during those prompts with a redaction marker.
    /// This prevents passwords and other credentials from being written to disk.
    ///
    /// WARNING: Session logs may still contain sensitive data even with this
    /// enabled. This heuristic catches common password prompts but cannot
    /// guarantee detection of all sensitive input scenarios.
    #[serde(default = "crate::defaults::bool_true")]
    pub session_log_redact_passwords: bool,

    // ========================================================================
    // Debug Logging
    // ========================================================================
    /// Log level for debug log file output.
    /// Controls verbosity of `/tmp/par_term_debug.log`.
    /// Environment variable RUST_LOG and --log-level CLI flag take precedence.
    #[serde(default)]
    pub log_level: LogLevel,

    // ========================================================================
    // Badge Settings (iTerm2-style session labels)
    // ========================================================================
    /// Enable badge display
    #[serde(default = "crate::defaults::bool_false")]
    pub badge_enabled: bool,

    /// Badge text format with variable interpolation
    /// Supports \(session.username), \(session.hostname), \(session.path), etc.
    #[serde(default = "crate::defaults::badge_format")]
    pub badge_format: String,

    /// Badge text color [R, G, B] (0-255)
    #[serde(default = "crate::defaults::badge_color")]
    pub badge_color: [u8; 3],

    /// Badge opacity (0.0-1.0)
    #[serde(default = "crate::defaults::badge_color_alpha")]
    pub badge_color_alpha: f32,

    /// Badge font family (uses system font if not found)
    #[serde(default = "crate::defaults::badge_font")]
    pub badge_font: String,

    /// Use bold weight for badge font
    #[serde(default = "crate::defaults::bool_true")]
    pub badge_font_bold: bool,

    /// Top margin in pixels from terminal edge
    #[serde(default = "crate::defaults::badge_top_margin")]
    pub badge_top_margin: f32,

    /// Right margin in pixels from terminal edge
    #[serde(default = "crate::defaults::badge_right_margin")]
    pub badge_right_margin: f32,

    /// Maximum badge width as fraction of terminal width (0.0-1.0)
    #[serde(default = "crate::defaults::badge_max_width")]
    pub badge_max_width: f32,

    /// Maximum badge height as fraction of terminal height (0.0-1.0)
    #[serde(default = "crate::defaults::badge_max_height")]
    pub badge_max_height: f32,

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
    /// Enable progress bar overlay
    /// When enabled, progress bars from OSC 9;4 and OSC 934 sequences are displayed
    #[serde(default = "crate::defaults::bool_true")]
    pub progress_bar_enabled: bool,

    /// Progress bar visual style
    /// - bar: Simple thin bar (default)
    /// - bar_with_text: Bar with percentage text and labels
    #[serde(default)]
    pub progress_bar_style: ProgressBarStyle,

    /// Progress bar position
    /// - bottom: Display at the bottom of the terminal (default)
    /// - top: Display at the top of the terminal
    #[serde(default)]
    pub progress_bar_position: ProgressBarPosition,

    /// Progress bar height in pixels
    #[serde(default = "crate::defaults::progress_bar_height")]
    pub progress_bar_height: f32,

    /// Progress bar opacity (0.0-1.0)
    #[serde(default = "crate::defaults::progress_bar_opacity")]
    pub progress_bar_opacity: f32,

    /// Color for normal progress state [R, G, B] (0-255)
    #[serde(default = "crate::defaults::progress_bar_normal_color")]
    pub progress_bar_normal_color: [u8; 3],

    /// Color for warning progress state [R, G, B] (0-255)
    #[serde(default = "crate::defaults::progress_bar_warning_color")]
    pub progress_bar_warning_color: [u8; 3],

    /// Color for error progress state [R, G, B] (0-255)
    #[serde(default = "crate::defaults::progress_bar_error_color")]
    pub progress_bar_error_color: [u8; 3],

    /// Color for indeterminate progress state [R, G, B] (0-255)
    #[serde(default = "crate::defaults::progress_bar_indeterminate_color")]
    pub progress_bar_indeterminate_color: [u8; 3],

    // ========================================================================
    // Triggers & Automation
    // ========================================================================
    /// Regex trigger definitions that match terminal output and fire actions
    #[serde(default)]
    pub triggers: Vec<crate::automation::TriggerConfig>,

    /// Coprocess definitions for piped subprocess management
    #[serde(default)]
    pub coprocesses: Vec<crate::automation::CoprocessDefConfig>,

    /// External observer script definitions
    #[serde(default)]
    pub scripts: Vec<crate::scripting::ScriptConfig>,

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
    // Content Prettifier
    // ========================================================================
    /// Master switch for the content prettifier system.
    /// When false, no detection or rendering occurs.
    #[serde(default)]
    pub enable_prettifier: bool,

    /// Detailed prettifier configuration.
    #[serde(default)]
    pub content_prettifier: super::prettifier::PrettifierYamlConfig,

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
    /// Allow all environment variables in config `${VAR}` substitution.
    ///
    /// When `false` (default), only a safe allowlist of environment variables
    /// (HOME, USER, SHELL, XDG_*, PAR_TERM_*, LC_*, etc.) can be substituted.
    /// This prevents shared or downloaded config files from exfiltrating
    /// sensitive environment variables such as API keys or tokens.
    ///
    /// Set to `true` to restore the unrestricted pre-0.24 behaviour.
    #[serde(default = "crate::defaults::bool_false")]
    pub allow_all_env_vars: bool,

    /// Allow dynamic profile sources to be fetched over plain HTTP (not HTTPS).
    ///
    /// When `false` (the default), any `dynamic_profile_sources` entry whose URL
    /// uses the `http://` scheme will be refused with an error at fetch time.
    /// This prevents a network-level attacker from injecting malicious profiles
    /// via a man-in-the-middle attack on an untrusted network.
    ///
    /// Set to `true` only if you must fetch profiles from a server that does not
    /// support HTTPS (e.g., an internal dev server without TLS). A warning will
    /// still be logged in that case.
    #[serde(default = "crate::defaults::bool_false")]
    pub allow_http_profiles: bool,

    // ========================================================================
    // AI Inspector
    // ========================================================================
    /// AI Inspector side panel settings (see [`AiInspectorConfig`]).
    ///
    /// All `ai_inspector_*` fields are flattened here for YAML backward-compatibility.
    #[serde(flatten)]
    pub ai_inspector: AiInspectorConfig,
}
