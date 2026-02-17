//! Terminal configuration management.
//!
//! This module provides configuration loading, saving, and default values
//! for the terminal emulator.

use crate::snippets::{CustomActionConfig, SnippetConfig};
use crate::themes::Theme;
use crate::types::{
    AlertEvent, AlertSoundConfig, BackgroundImageMode, BackgroundMode, CursorShaderConfig,
    CursorStyle, DividerStyle, DownloadSaveLocation, DroppedFileQuoteStyle, FontRange,
    ImageScalingMode, InstallPromptState, IntegrationVersions, KeyBinding, LogLevel,
    ModifierRemapping, OptionKeyMode, PaneBackgroundConfig, PaneTitlePosition, PowerPreference,
    ProgressBarPosition, ProgressBarStyle, SemanticHistoryEditorMode, SessionLogFormat,
    ShaderConfig, ShaderInstallPrompt, ShellExitAction, SmartSelectionRule, StartupDirectoryMode,
    StatusBarPosition, TabBarMode, TabBarPosition, TabStyle, ThinStrokesMode, UnfocusedCursorStyle,
    UpdateCheckFrequency, VsyncMode, WindowType, default_smart_selection_rules,
};

use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Substitute `${VAR_NAME}` patterns in a string with environment variable values.
///
/// - `${VAR}` is replaced with the value of the environment variable `VAR`.
/// - If the variable is not set, the `${VAR}` placeholder is left unchanged.
/// - `$${VAR}` (doubled dollar sign) is an escape and produces the literal `${VAR}`.
/// - Supports `${VAR:-default}` syntax for providing a default value when the variable is unset.
///
/// This is applied to the raw YAML config string before deserialization, so all
/// string-typed config values benefit from substitution.
pub fn substitute_variables(input: &str) -> String {
    // First, replace escaped `$${` with a placeholder that won't match the regex
    let escaped_placeholder = "\x00ESC_DOLLAR\x00";
    let working = input.replace("$${", escaped_placeholder);

    // Match ${VAR_NAME} or ${VAR_NAME:-default_value}
    let re = Regex::new(r"\$\{([A-Za-z_][A-Za-z0-9_]*)(?::-((?:[^}\\]|\\.)*))?}")
        .expect("invalid regex");

    let result = re.replace_all(&working, |caps: &regex::Captures| {
        let var_name = &caps[1];
        match std::env::var(var_name) {
            Ok(val) => val,
            Err(_) => {
                // Use default value if provided, otherwise leave the placeholder as-is
                caps.get(2)
                    .map(|m| m.as_str().replace("\\}", "}"))
                    .unwrap_or_else(|| caps[0].to_string())
            }
        }
    });

    // Restore escaped dollar signs
    result.replace(escaped_placeholder, "${")
}

/// Custom deserializer for `ShellExitAction` that supports backward compatibility.
///
/// Accepts either:
/// - Boolean: `true` → `Close`, `false` → `Keep` (legacy format)
/// - String enum: `"close"`, `"keep"`, `"restart_immediately"`, etc.
fn deserialize_shell_exit_action<'de, D>(deserializer: D) -> Result<ShellExitAction, D::Error>
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
    /// Number of columns in the terminal
    #[serde(default = "crate::defaults::cols")]
    pub cols: usize,

    /// Number of rows in the terminal
    #[serde(default = "crate::defaults::rows")]
    pub rows: usize,

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

    /// Custom shader file path (GLSL format, relative to shaders folder or absolute)
    /// Shaders are loaded from ~/.config/par-term/shaders/ by default
    /// Supports Ghostty/Shadertoy-style GLSL shaders with iTime, iResolution, iChannel0-4
    #[serde(default)]
    pub custom_shader: Option<String>,

    /// Enable or disable the custom shader (even if a path is set)
    #[serde(default = "crate::defaults::bool_true")]
    pub custom_shader_enabled: bool,

    /// Enable animation in custom shader (updates iTime uniform each frame)
    /// When disabled, iTime is fixed at 0.0 for static effects
    #[serde(default = "crate::defaults::bool_true")]
    pub custom_shader_animation: bool,

    /// Animation speed multiplier for custom shader (1.0 = normal speed)
    #[serde(default = "crate::defaults::custom_shader_speed")]
    pub custom_shader_animation_speed: f32,

    /// Text opacity when using custom shader (0.0 = transparent, 1.0 = fully opaque)
    /// This allows text to remain readable while the shader effect shows through the background
    #[serde(default = "crate::defaults::text_opacity")]
    pub custom_shader_text_opacity: f32,

    /// When enabled, the shader receives the full rendered terminal content (text + background)
    /// and can manipulate/distort it. When disabled (default), the shader only provides
    /// a background and text is composited on top cleanly.
    #[serde(default = "crate::defaults::bool_false")]
    pub custom_shader_full_content: bool,

    /// Brightness multiplier for custom shader output (0.05 = very dark, 1.0 = full brightness)
    /// This dims the shader background to improve text readability
    #[serde(default = "crate::defaults::custom_shader_brightness")]
    pub custom_shader_brightness: f32,

    /// Texture file path for custom shader iChannel0 (optional, Shadertoy compatible)
    /// Supports ~ for home directory. Example: "~/textures/noise.png"
    #[serde(default)]
    pub custom_shader_channel0: Option<String>,

    /// Texture file path for custom shader iChannel1 (optional)
    #[serde(default)]
    pub custom_shader_channel1: Option<String>,

    /// Texture file path for custom shader iChannel2 (optional)
    #[serde(default)]
    pub custom_shader_channel2: Option<String>,

    /// Texture file path for custom shader iChannel3 (optional)
    #[serde(default)]
    pub custom_shader_channel3: Option<String>,

    /// Cubemap texture path prefix for custom shaders (optional)
    /// Expects 6 face files: {prefix}-px.{ext}, -nx.{ext}, -py.{ext}, -ny.{ext}, -pz.{ext}, -nz.{ext}
    /// Supported formats: .png, .jpg, .jpeg, .hdr
    /// Example: "textures/cubemaps/env-outside" will load env-outside-px.png, etc.
    #[serde(default)]
    pub custom_shader_cubemap: Option<String>,

    /// Enable cubemap sampling in custom shaders
    /// When enabled and a cubemap path is set, iCubemap uniform is available in shaders
    #[serde(default = "crate::defaults::cubemap_enabled")]
    pub custom_shader_cubemap_enabled: bool,

    /// Use the app's background image as iChannel0 for custom shaders
    /// When enabled, the configured background image is bound as iChannel0 instead of
    /// the custom_shader_channel0 texture. This allows shaders to incorporate the
    /// background image without requiring a separate texture file.
    #[serde(default = "crate::defaults::use_background_as_channel0")]
    pub custom_shader_use_background_as_channel0: bool,

    // ========================================================================
    // Cursor Shader Settings (separate from background shader)
    // ========================================================================
    /// Cursor shader file path (GLSL format, relative to shaders folder or absolute)
    /// This is a separate shader specifically for cursor effects (trails, glows, etc.)
    #[serde(default)]
    pub cursor_shader: Option<String>,

    /// Enable or disable the cursor shader (even if a path is set)
    #[serde(default = "crate::defaults::bool_false")]
    pub cursor_shader_enabled: bool,

    /// Enable animation in cursor shader (updates iTime uniform each frame)
    #[serde(default = "crate::defaults::bool_true")]
    pub cursor_shader_animation: bool,

    /// Animation speed multiplier for cursor shader (1.0 = normal speed)
    #[serde(default = "crate::defaults::custom_shader_speed")]
    pub cursor_shader_animation_speed: f32,

    /// Cursor color for shader effects [R, G, B] (0-255)
    /// This color is passed to the shader via iCursorShaderColor uniform
    #[serde(default = "crate::defaults::cursor_shader_color")]
    pub cursor_shader_color: [u8; 3],

    /// Duration of cursor trail effect in seconds
    /// Passed to shader via iCursorTrailDuration uniform
    #[serde(default = "crate::defaults::cursor_trail_duration")]
    pub cursor_shader_trail_duration: f32,

    /// Radius of cursor glow effect in pixels
    /// Passed to shader via iCursorGlowRadius uniform
    #[serde(default = "crate::defaults::cursor_glow_radius")]
    pub cursor_shader_glow_radius: f32,

    /// Intensity of cursor glow effect (0.0 = none, 1.0 = full)
    /// Passed to shader via iCursorGlowIntensity uniform
    #[serde(default = "crate::defaults::cursor_glow_intensity")]
    pub cursor_shader_glow_intensity: f32,

    /// Hide the default cursor when cursor shader is enabled
    /// When true and cursor_shader_enabled is true, the normal cursor is not drawn
    /// This allows cursor shaders to fully replace the cursor rendering
    #[serde(default = "crate::defaults::bool_false")]
    pub cursor_shader_hides_cursor: bool,

    /// Disable cursor shader while in alt screen (vim, less, htop)
    /// Keeps current behavior by default for TUI compatibility
    #[serde(default = "crate::defaults::cursor_shader_disable_in_alt_screen")]
    pub cursor_shader_disable_in_alt_screen: bool,

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
    /// Enable copy mode (vi-style keyboard-driven text selection and navigation).
    /// When enabled, users can enter copy mode via the `toggle_copy_mode` keybinding
    /// action to navigate the terminal buffer with vi keys and yank text.
    #[serde(default = "crate::defaults::bool_true")]
    pub copy_mode_enabled: bool,

    /// Automatically exit copy mode after yanking (copying) selected text.
    /// When true (default), pressing `y` in visual mode copies text and exits copy mode.
    /// When false, copy mode stays active after yanking so you can continue selecting.
    #[serde(default = "crate::defaults::bool_true")]
    pub copy_mode_auto_exit_on_yank: bool,

    /// Show a status bar at the bottom of the terminal when copy mode is active.
    /// The status bar displays the current mode (COPY/VISUAL/V-LINE/V-BLOCK/SEARCH)
    /// and cursor position information.
    #[serde(default = "crate::defaults::bool_true")]
    pub copy_mode_show_status: bool,

    // ========================================================================
    // Scrollback & Cursor
    // ========================================================================
    /// Maximum number of lines to keep in scrollback buffer
    #[serde(default = "crate::defaults::scrollback", alias = "scrollback_size")]
    pub scrollback_lines: usize,

    // ========================================================================
    // Unicode Width Settings
    // ========================================================================
    /// Unicode version for character width calculations
    /// Different versions have different width tables, particularly for emoji.
    /// Options: unicode_9, unicode_10, ..., unicode_16, auto (default)
    #[serde(default = "crate::defaults::unicode_version")]
    pub unicode_version: par_term_emu_core_rust::UnicodeVersion,

    /// Treatment of East Asian Ambiguous width characters
    /// - narrow: 1 cell width (Western default)
    /// - wide: 2 cell width (CJK default)
    #[serde(default = "crate::defaults::ambiguous_width")]
    pub ambiguous_width: par_term_emu_core_rust::AmbiguousWidth,

    /// Unicode normalization form for text processing
    /// Controls how Unicode text is normalized before being stored in terminal cells.
    /// - NFC: Canonical composition (default, most compatible)
    /// - NFD: Canonical decomposition (macOS HFS+ style)
    /// - NFKC: Compatibility composition (resolves ligatures like ﬁ → fi)
    /// - NFKD: Compatibility decomposition
    /// - none: No normalization
    #[serde(default = "crate::defaults::normalization_form")]
    pub normalization_form: par_term_emu_core_rust::NormalizationForm,

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
    /// Enable mDNS/Bonjour discovery for SSH hosts
    #[serde(default = "crate::defaults::bool_false")]
    pub enable_mdns_discovery: bool,

    /// mDNS scan timeout in seconds
    #[serde(default = "crate::defaults::mdns_timeout")]
    pub mdns_scan_timeout_secs: u32,

    /// Enable automatic profile switching based on SSH hostname
    #[serde(default = "crate::defaults::bool_true")]
    pub ssh_auto_profile_switch: bool,

    /// Revert profile when SSH session disconnects
    #[serde(default = "crate::defaults::bool_true")]
    pub ssh_revert_profile_on_disconnect: bool,

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
    /// How often to check for new par-term releases
    /// - never: Disable automatic update checks
    /// - daily: Check once per day
    /// - weekly: Check once per week (default)
    /// - monthly: Check once per month
    #[serde(default = "crate::defaults::update_check_frequency")]
    pub update_check_frequency: UpdateCheckFrequency,

    /// ISO 8601 timestamp of the last update check (auto-managed)
    #[serde(default)]
    pub last_update_check: Option<String>,

    /// Version that user chose to skip notifications for
    #[serde(default)]
    pub skipped_version: Option<String>,

    /// Last version we notified the user about (prevents repeat notifications)
    #[serde(default)]
    pub last_notified_version: Option<String>,

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
    /// Highlight color for search matches [R, G, B, A] (0-255)
    #[serde(default = "crate::defaults::search_highlight_color")]
    pub search_highlight_color: [u8; 4],

    /// Highlight color for the current/active search match [R, G, B, A] (0-255)
    #[serde(default = "crate::defaults::search_current_highlight_color")]
    pub search_current_highlight_color: [u8; 4],

    /// Default case sensitivity for search
    #[serde(default = "crate::defaults::bool_false")]
    pub search_case_sensitive: bool,

    /// Default regex mode for search
    #[serde(default = "crate::defaults::bool_false")]
    pub search_regex: bool,

    /// Wrap around when navigating search matches
    #[serde(default = "crate::defaults::bool_true")]
    pub search_wrap_around: bool,

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
    /// Enable the status bar
    #[serde(default = "crate::defaults::bool_false")]
    pub status_bar_enabled: bool,

    /// Status bar position (top or bottom)
    #[serde(default)]
    pub status_bar_position: StatusBarPosition,

    /// Status bar height in pixels
    #[serde(default = "crate::defaults::status_bar_height")]
    pub status_bar_height: f32,

    /// Status bar background color [R, G, B] (0-255)
    #[serde(default = "crate::defaults::status_bar_bg_color")]
    pub status_bar_bg_color: [u8; 3],

    /// Status bar background alpha (0.0-1.0)
    #[serde(default = "crate::defaults::status_bar_bg_alpha")]
    pub status_bar_bg_alpha: f32,

    /// Status bar foreground (text) color [R, G, B] (0-255)
    #[serde(default = "crate::defaults::status_bar_fg_color")]
    pub status_bar_fg_color: [u8; 3],

    /// Status bar font family (empty string = use terminal font)
    #[serde(default)]
    pub status_bar_font: String,

    /// Status bar font size in points
    #[serde(default = "crate::defaults::status_bar_font_size")]
    pub status_bar_font_size: f32,

    /// Separator string between widgets
    #[serde(default = "crate::defaults::status_bar_separator")]
    pub status_bar_separator: String,

    /// Auto-hide the status bar when in fullscreen mode
    #[serde(default = "crate::defaults::bool_true")]
    pub status_bar_auto_hide_fullscreen: bool,

    /// Auto-hide the status bar when mouse is inactive
    #[serde(default = "crate::defaults::bool_false")]
    pub status_bar_auto_hide_mouse_inactive: bool,

    /// Timeout in seconds before hiding status bar after last mouse activity
    #[serde(default = "crate::defaults::status_bar_mouse_inactive_timeout")]
    pub status_bar_mouse_inactive_timeout: f32,

    /// Polling interval in seconds for system monitor data (CPU, memory, network)
    #[serde(default = "crate::defaults::status_bar_system_poll_interval")]
    pub status_bar_system_poll_interval: f32,

    /// Polling interval in seconds for git branch detection
    #[serde(default = "crate::defaults::status_bar_git_poll_interval")]
    pub status_bar_git_poll_interval: f32,

    /// Time format string for the Clock widget (chrono strftime syntax)
    #[serde(default = "crate::defaults::status_bar_time_format")]
    pub status_bar_time_format: String,

    /// Show ahead/behind and dirty indicators on the Git Branch widget
    #[serde(default = "crate::defaults::bool_true")]
    pub status_bar_git_show_status: bool,

    /// Widget configuration list
    #[serde(default = "crate::status_bar::default_widgets")]
    pub status_bar_widgets: Vec<crate::status_bar::StatusBarWidgetConfig>,

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
    // AI Inspector
    // ========================================================================
    /// Enable AI Inspector side panel
    #[serde(default = "crate::defaults::ai_inspector_enabled")]
    pub ai_inspector_enabled: bool,

    /// Open the AI Inspector panel automatically on startup
    #[serde(default = "crate::defaults::ai_inspector_open_on_startup")]
    pub ai_inspector_open_on_startup: bool,

    /// Width of the AI Inspector panel in pixels
    #[serde(default = "crate::defaults::ai_inspector_width")]
    pub ai_inspector_width: f32,

    /// Default capture scope: "visible", "scrollback", or "selection"
    #[serde(default = "crate::defaults::ai_inspector_default_scope")]
    pub ai_inspector_default_scope: String,

    /// View mode for inspector results: "cards" or "raw"
    #[serde(default = "crate::defaults::ai_inspector_view_mode")]
    pub ai_inspector_view_mode: String,

    /// Automatically refresh inspector when terminal content changes
    #[serde(default = "crate::defaults::ai_inspector_live_update")]
    pub ai_inspector_live_update: bool,

    /// Show semantic zone overlays on terminal content
    #[serde(default = "crate::defaults::ai_inspector_show_zones")]
    pub ai_inspector_show_zones: bool,

    /// AI agent identifier for inspector queries
    #[serde(default = "crate::defaults::ai_inspector_agent")]
    pub ai_inspector_agent: String,

    /// Automatically launch AI agent when inspector opens
    #[serde(default = "crate::defaults::ai_inspector_auto_launch")]
    pub ai_inspector_auto_launch: bool,

    /// Automatically include terminal context with AI queries
    #[serde(default = "crate::defaults::ai_inspector_auto_context")]
    pub ai_inspector_auto_context: bool,

    /// Maximum number of terminal lines to include as AI context
    #[serde(default = "crate::defaults::ai_inspector_context_max_lines")]
    pub ai_inspector_context_max_lines: usize,

    /// Automatically approve AI-suggested actions without confirmation
    #[serde(default = "crate::defaults::ai_inspector_auto_approve")]
    pub ai_inspector_auto_approve: bool,

    /// Allow the AI agent to write input to the terminal (drive terminal)
    #[serde(default = "crate::defaults::ai_inspector_agent_terminal_access")]
    pub ai_inspector_agent_terminal_access: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            cols: crate::defaults::cols(),
            rows: crate::defaults::rows(),
            font_size: crate::defaults::font_size(),
            font_family: crate::defaults::font_family(),
            font_family_bold: None,
            font_family_italic: None,
            font_family_bold_italic: None,
            font_ranges: Vec::new(),
            line_spacing: crate::defaults::line_spacing(),
            char_spacing: crate::defaults::char_spacing(),
            enable_text_shaping: crate::defaults::text_shaping(),
            enable_ligatures: crate::defaults::bool_true(),
            enable_kerning: crate::defaults::bool_true(),
            font_antialias: crate::defaults::bool_true(),
            font_hinting: crate::defaults::bool_true(),
            font_thin_strokes: ThinStrokesMode::default(),
            minimum_contrast: crate::defaults::minimum_contrast(),
            copy_mode_enabled: crate::defaults::bool_true(),
            copy_mode_auto_exit_on_yank: crate::defaults::bool_true(),
            copy_mode_show_status: crate::defaults::bool_true(),
            scrollback_lines: crate::defaults::scrollback(),
            unicode_version: crate::defaults::unicode_version(),
            ambiguous_width: crate::defaults::ambiguous_width(),
            normalization_form: crate::defaults::normalization_form(),
            cursor_blink: crate::defaults::bool_false(),
            cursor_blink_interval: crate::defaults::cursor_blink_interval(),
            cursor_style: CursorStyle::default(),
            cursor_color: crate::defaults::cursor_color(),
            cursor_text_color: None,
            lock_cursor_visibility: crate::defaults::bool_false(),
            lock_cursor_style: crate::defaults::bool_false(),
            lock_cursor_blink: crate::defaults::bool_false(),
            cursor_guide_enabled: crate::defaults::bool_false(),
            cursor_guide_color: crate::defaults::cursor_guide_color(),
            cursor_shadow_enabled: crate::defaults::bool_false(),
            cursor_shadow_color: crate::defaults::cursor_shadow_color(),
            cursor_shadow_offset: crate::defaults::cursor_shadow_offset(),
            cursor_shadow_blur: crate::defaults::cursor_shadow_blur(),
            cursor_boost: crate::defaults::cursor_boost(),
            cursor_boost_color: crate::defaults::cursor_boost_color(),
            unfocused_cursor_style: UnfocusedCursorStyle::default(),
            scrollbar_autohide_delay: crate::defaults::scrollbar_autohide_delay(),
            window_title: crate::defaults::window_title(),
            allow_title_change: crate::defaults::bool_true(),
            theme: crate::defaults::theme(),
            auto_dark_mode: false,
            light_theme: crate::defaults::light_theme(),
            dark_theme: crate::defaults::dark_theme(),
            left_option_key_mode: OptionKeyMode::default(),
            right_option_key_mode: OptionKeyMode::default(),
            modifier_remapping: ModifierRemapping::default(),
            use_physical_keys: crate::defaults::bool_false(),
            auto_copy_selection: crate::defaults::bool_true(),
            copy_trailing_newline: crate::defaults::bool_false(),
            middle_click_paste: crate::defaults::bool_true(),
            paste_delay_ms: crate::defaults::paste_delay_ms(),
            dropped_file_quote_style: DroppedFileQuoteStyle::default(),
            mouse_scroll_speed: crate::defaults::scroll_speed(),
            mouse_double_click_threshold: crate::defaults::double_click_threshold(),
            mouse_triple_click_threshold: crate::defaults::triple_click_threshold(),
            option_click_moves_cursor: crate::defaults::bool_true(),
            focus_follows_mouse: crate::defaults::bool_false(),
            report_horizontal_scroll: crate::defaults::bool_true(),
            word_characters: crate::defaults::word_characters(),
            smart_selection_enabled: crate::defaults::smart_selection_enabled(),
            smart_selection_rules: default_smart_selection_rules(),
            screenshot_format: crate::defaults::screenshot_format(),
            max_fps: crate::defaults::max_fps(),
            vsync_mode: VsyncMode::default(),
            power_preference: PowerPreference::default(),
            reduce_flicker: crate::defaults::reduce_flicker(),
            reduce_flicker_delay_ms: crate::defaults::reduce_flicker_delay_ms(),
            maximize_throughput: crate::defaults::maximize_throughput(),
            throughput_render_interval_ms: crate::defaults::throughput_render_interval_ms(),
            window_padding: crate::defaults::window_padding(),
            window_opacity: crate::defaults::window_opacity(),
            window_always_on_top: crate::defaults::bool_false(),
            window_decorations: crate::defaults::bool_true(),
            window_type: WindowType::default(),
            target_monitor: None,
            target_space: None,
            lock_window_size: crate::defaults::bool_false(),
            show_window_number: crate::defaults::bool_false(),
            transparency_affects_only_default_background: crate::defaults::bool_true(),
            keep_text_opaque: crate::defaults::bool_true(),
            blur_enabled: crate::defaults::bool_false(),
            blur_radius: crate::defaults::blur_radius(),
            background_image: None,
            background_image_enabled: crate::defaults::bool_true(),
            background_image_mode: BackgroundImageMode::default(),
            background_image_opacity: crate::defaults::background_image_opacity(),
            image_scaling_mode: ImageScalingMode::default(),
            image_preserve_aspect_ratio: crate::defaults::bool_true(),
            background_mode: BackgroundMode::default(),
            pane_backgrounds: Vec::new(),
            background_color: crate::defaults::background_color(),
            download_save_location: DownloadSaveLocation::default(),
            last_download_directory: None,
            custom_shader: None,
            custom_shader_enabled: crate::defaults::bool_true(),
            custom_shader_animation: crate::defaults::bool_true(),
            custom_shader_animation_speed: crate::defaults::custom_shader_speed(),
            custom_shader_text_opacity: crate::defaults::text_opacity(),
            custom_shader_full_content: crate::defaults::bool_false(),
            custom_shader_brightness: crate::defaults::custom_shader_brightness(),
            custom_shader_channel0: None,
            custom_shader_channel1: None,
            custom_shader_channel2: None,
            custom_shader_channel3: None,
            custom_shader_cubemap: None,
            custom_shader_cubemap_enabled: crate::defaults::cubemap_enabled(),
            custom_shader_use_background_as_channel0: crate::defaults::use_background_as_channel0(),
            cursor_shader: None,
            cursor_shader_enabled: crate::defaults::bool_false(),
            cursor_shader_animation: crate::defaults::bool_true(),
            cursor_shader_animation_speed: crate::defaults::custom_shader_speed(),
            cursor_shader_color: crate::defaults::cursor_shader_color(),
            cursor_shader_trail_duration: crate::defaults::cursor_trail_duration(),
            cursor_shader_glow_radius: crate::defaults::cursor_glow_radius(),
            cursor_shader_glow_intensity: crate::defaults::cursor_glow_intensity(),
            cursor_shader_hides_cursor: crate::defaults::bool_false(),
            cursor_shader_disable_in_alt_screen:
                crate::defaults::cursor_shader_disable_in_alt_screen(),
            shell_exit_action: ShellExitAction::default(),
            custom_shell: None,
            shell_args: None,
            working_directory: None,
            startup_directory_mode: StartupDirectoryMode::default(),
            startup_directory: None,
            last_working_directory: None,
            shell_env: None,
            login_shell: crate::defaults::login_shell(),
            initial_text: crate::defaults::initial_text(),
            initial_text_delay_ms: crate::defaults::initial_text_delay_ms(),
            initial_text_send_newline: crate::defaults::initial_text_send_newline(),
            answerback_string: crate::defaults::answerback_string(),
            prompt_on_quit: crate::defaults::bool_false(),
            confirm_close_running_jobs: crate::defaults::bool_false(),
            jobs_to_ignore: crate::defaults::jobs_to_ignore(),
            semantic_history_enabled: crate::defaults::bool_true(),
            semantic_history_editor_mode: SemanticHistoryEditorMode::default(),
            semantic_history_editor: crate::defaults::semantic_history_editor(),
            link_handler_command: String::new(),
            scrollbar_position: crate::defaults::scrollbar_position(),
            scrollbar_width: crate::defaults::scrollbar_width(),
            scrollbar_thumb_color: crate::defaults::scrollbar_thumb_color(),
            scrollbar_track_color: crate::defaults::scrollbar_track_color(),
            scrollbar_command_marks: crate::defaults::bool_true(),
            scrollbar_mark_tooltips: crate::defaults::bool_false(),
            command_separator_enabled: crate::defaults::bool_false(),
            command_separator_thickness: crate::defaults::command_separator_thickness(),
            command_separator_opacity: crate::defaults::command_separator_opacity(),
            command_separator_exit_color: crate::defaults::bool_true(),
            command_separator_color: crate::defaults::command_separator_color(),
            clipboard_max_sync_events: crate::defaults::clipboard_max_sync_events(),
            clipboard_max_event_bytes: crate::defaults::clipboard_max_event_bytes(),
            command_history_max_entries: crate::defaults::command_history_max_entries(),
            notification_bell_desktop: crate::defaults::bool_false(),
            notification_bell_sound: crate::defaults::bell_sound(),
            notification_bell_visual: crate::defaults::bool_true(),
            notification_activity_enabled: crate::defaults::bool_false(),
            notification_activity_threshold: crate::defaults::activity_threshold(),
            anti_idle_enabled: crate::defaults::bool_false(),
            anti_idle_seconds: crate::defaults::anti_idle_seconds(),
            anti_idle_code: crate::defaults::anti_idle_code(),
            notification_silence_enabled: crate::defaults::bool_false(),
            notification_silence_threshold: crate::defaults::silence_threshold(),
            notification_session_ended: crate::defaults::bool_false(),
            suppress_notifications_when_focused: crate::defaults::bool_true(),
            notification_max_buffer: crate::defaults::notification_max_buffer(),
            alert_sounds: HashMap::new(),
            enable_mdns_discovery: crate::defaults::bool_false(),
            mdns_scan_timeout_secs: crate::defaults::mdns_timeout(),
            ssh_auto_profile_switch: crate::defaults::bool_true(),
            ssh_revert_profile_on_disconnect: crate::defaults::bool_true(),
            tab_style: TabStyle::default(),
            light_tab_style: crate::defaults::light_tab_style(),
            dark_tab_style: crate::defaults::dark_tab_style(),
            tab_bar_mode: TabBarMode::default(),
            tab_bar_height: crate::defaults::tab_bar_height(),
            tab_bar_position: TabBarPosition::default(),
            tab_bar_width: crate::defaults::tab_bar_width(),
            tab_show_close_button: crate::defaults::bool_true(),
            tab_show_index: crate::defaults::bool_false(),
            tab_inherit_cwd: crate::defaults::bool_true(),
            max_tabs: crate::defaults::zero(),
            show_profile_drawer_button: crate::defaults::bool_false(),
            new_tab_shortcut_shows_profiles: crate::defaults::bool_false(),
            tab_bar_background: crate::defaults::tab_bar_background(),
            tab_active_background: crate::defaults::tab_active_background(),
            tab_inactive_background: crate::defaults::tab_inactive_background(),
            tab_hover_background: crate::defaults::tab_hover_background(),
            tab_active_text: crate::defaults::tab_active_text(),
            tab_inactive_text: crate::defaults::tab_inactive_text(),
            tab_active_indicator: crate::defaults::tab_active_indicator(),
            tab_activity_indicator: crate::defaults::tab_activity_indicator(),
            tab_bell_indicator: crate::defaults::tab_bell_indicator(),
            tab_close_button: crate::defaults::tab_close_button(),
            tab_close_button_hover: crate::defaults::tab_close_button_hover(),
            dim_inactive_tabs: crate::defaults::bool_true(),
            inactive_tab_opacity: crate::defaults::inactive_tab_opacity(),
            tab_min_width: crate::defaults::tab_min_width(),
            tab_stretch_to_fill: crate::defaults::tab_stretch_to_fill(),
            tab_html_titles: crate::defaults::tab_html_titles(),
            tab_border_color: crate::defaults::tab_border_color(),
            tab_border_width: crate::defaults::tab_border_width(),
            // Split panes
            pane_divider_width: crate::defaults::pane_divider_width(),
            pane_divider_hit_width: crate::defaults::pane_divider_hit_width(),
            pane_padding: crate::defaults::pane_padding(),
            pane_min_size: crate::defaults::pane_min_size(),
            pane_background_opacity: crate::defaults::pane_background_opacity(),
            pane_divider_color: crate::defaults::pane_divider_color(),
            pane_divider_hover_color: crate::defaults::pane_divider_hover_color(),
            dim_inactive_panes: crate::defaults::bool_false(),
            inactive_pane_opacity: crate::defaults::inactive_pane_opacity(),
            show_pane_titles: crate::defaults::bool_false(),
            pane_title_height: crate::defaults::pane_title_height(),
            pane_title_position: PaneTitlePosition::default(),
            pane_title_color: crate::defaults::pane_title_color(),
            pane_title_bg_color: crate::defaults::pane_title_bg_color(),
            pane_title_font: String::new(),
            pane_divider_style: DividerStyle::default(),
            max_panes: crate::defaults::max_panes(),
            pane_focus_indicator: crate::defaults::bool_true(),
            pane_focus_color: crate::defaults::pane_focus_color(),
            pane_focus_width: crate::defaults::pane_focus_width(),
            tmux_enabled: crate::defaults::bool_false(),
            tmux_path: crate::defaults::tmux_path(),
            tmux_default_session: crate::defaults::tmux_default_session(),
            tmux_auto_attach: crate::defaults::bool_false(),
            tmux_auto_attach_session: crate::defaults::tmux_auto_attach_session(),
            tmux_clipboard_sync: crate::defaults::bool_true(),
            tmux_profile: None,
            tmux_show_status_bar: crate::defaults::bool_false(),
            tmux_status_bar_refresh_ms: crate::defaults::tmux_status_bar_refresh_ms(),
            tmux_prefix_key: crate::defaults::tmux_prefix_key(),
            tmux_status_bar_use_native_format: crate::defaults::bool_false(),
            tmux_status_bar_left: crate::defaults::tmux_status_bar_left(),
            tmux_status_bar_right: crate::defaults::tmux_status_bar_right(),
            pause_shaders_on_blur: crate::defaults::bool_true(),
            pause_refresh_on_blur: crate::defaults::bool_false(),
            unfocused_fps: crate::defaults::unfocused_fps(),
            shader_hot_reload: crate::defaults::bool_false(),
            shader_hot_reload_delay: crate::defaults::shader_hot_reload_delay(),
            shader_configs: HashMap::new(),
            cursor_shader_configs: HashMap::new(),
            keybindings: crate::defaults::keybindings(),
            shader_install_prompt: ShaderInstallPrompt::default(),
            shell_integration_state: InstallPromptState::default(),
            integration_versions: IntegrationVersions::default(),
            update_check_frequency: crate::defaults::update_check_frequency(),
            last_update_check: None,
            skipped_version: None,
            last_notified_version: None,
            auto_restore_arrangement: None,
            restore_session: crate::defaults::bool_false(),
            session_undo_timeout_secs: crate::defaults::session_undo_timeout_secs(),
            session_undo_max_entries: crate::defaults::session_undo_max_entries(),
            session_undo_preserve_shell: crate::defaults::session_undo_preserve_shell(),
            search_highlight_color: crate::defaults::search_highlight_color(),
            search_current_highlight_color: crate::defaults::search_current_highlight_color(),
            search_case_sensitive: crate::defaults::bool_false(),
            search_regex: crate::defaults::bool_false(),
            search_wrap_around: crate::defaults::bool_true(),
            // Session logging
            auto_log_sessions: crate::defaults::bool_false(),
            session_log_format: SessionLogFormat::default(),
            session_log_directory: crate::defaults::session_log_directory(),
            archive_on_close: crate::defaults::bool_true(),
            // Debug Logging
            log_level: LogLevel::default(),
            // Badge
            badge_enabled: crate::defaults::bool_false(),
            badge_format: crate::defaults::badge_format(),
            badge_color: crate::defaults::badge_color(),
            badge_color_alpha: crate::defaults::badge_color_alpha(),
            badge_font: crate::defaults::badge_font(),
            badge_font_bold: crate::defaults::bool_true(),
            badge_top_margin: crate::defaults::badge_top_margin(),
            badge_right_margin: crate::defaults::badge_right_margin(),
            badge_max_width: crate::defaults::badge_max_width(),
            badge_max_height: crate::defaults::badge_max_height(),
            // Status Bar
            status_bar_enabled: crate::defaults::bool_false(),
            status_bar_position: StatusBarPosition::default(),
            status_bar_height: crate::defaults::status_bar_height(),
            status_bar_bg_color: crate::defaults::status_bar_bg_color(),
            status_bar_bg_alpha: crate::defaults::status_bar_bg_alpha(),
            status_bar_fg_color: crate::defaults::status_bar_fg_color(),
            status_bar_font: String::new(),
            status_bar_font_size: crate::defaults::status_bar_font_size(),
            status_bar_separator: crate::defaults::status_bar_separator(),
            status_bar_auto_hide_fullscreen: crate::defaults::bool_true(),
            status_bar_auto_hide_mouse_inactive: crate::defaults::bool_false(),
            status_bar_mouse_inactive_timeout: crate::defaults::status_bar_mouse_inactive_timeout(),
            status_bar_system_poll_interval: crate::defaults::status_bar_system_poll_interval(),
            status_bar_git_poll_interval: crate::defaults::status_bar_git_poll_interval(),
            status_bar_time_format: crate::defaults::status_bar_time_format(),
            status_bar_git_show_status: crate::defaults::bool_true(),
            status_bar_widgets: crate::status_bar::default_widgets(),
            // Progress Bar
            progress_bar_enabled: crate::defaults::bool_true(),
            progress_bar_style: ProgressBarStyle::default(),
            progress_bar_position: ProgressBarPosition::default(),
            progress_bar_height: crate::defaults::progress_bar_height(),
            progress_bar_opacity: crate::defaults::progress_bar_opacity(),
            progress_bar_normal_color: crate::defaults::progress_bar_normal_color(),
            progress_bar_warning_color: crate::defaults::progress_bar_warning_color(),
            progress_bar_error_color: crate::defaults::progress_bar_error_color(),
            progress_bar_indeterminate_color: crate::defaults::progress_bar_indeterminate_color(),
            triggers: Vec::new(),
            coprocesses: Vec::new(),
            scripts: Vec::new(),
            snippets: Vec::new(),
            actions: Vec::new(),
            collapsed_settings_sections: Vec::new(),
            dynamic_profile_sources: Vec::new(),
            // AI Inspector
            ai_inspector_enabled: crate::defaults::ai_inspector_enabled(),
            ai_inspector_open_on_startup: crate::defaults::ai_inspector_open_on_startup(),
            ai_inspector_width: crate::defaults::ai_inspector_width(),
            ai_inspector_default_scope: crate::defaults::ai_inspector_default_scope(),
            ai_inspector_view_mode: crate::defaults::ai_inspector_view_mode(),
            ai_inspector_live_update: crate::defaults::ai_inspector_live_update(),
            ai_inspector_show_zones: crate::defaults::ai_inspector_show_zones(),
            ai_inspector_agent: crate::defaults::ai_inspector_agent(),
            ai_inspector_auto_launch: crate::defaults::ai_inspector_auto_launch(),
            ai_inspector_auto_context: crate::defaults::ai_inspector_auto_context(),
            ai_inspector_context_max_lines: crate::defaults::ai_inspector_context_max_lines(),
            ai_inspector_auto_approve: crate::defaults::ai_inspector_auto_approve(),
            ai_inspector_agent_terminal_access: crate::defaults::ai_inspector_agent_terminal_access(
            ),
        }
    }
}

impl Config {
    /// Apply tab style preset, overwriting the tab bar color/size fields.
    ///
    /// This is called when the user changes `tab_style` in settings.
    /// The `Dark` style corresponds to the existing defaults and does nothing.
    pub fn apply_tab_style(&mut self) {
        match self.tab_style {
            TabStyle::Dark => {
                // Default dark theme - restore original defaults
                self.tab_bar_background = crate::defaults::tab_bar_background();
                self.tab_active_background = crate::defaults::tab_active_background();
                self.tab_inactive_background = crate::defaults::tab_inactive_background();
                self.tab_hover_background = crate::defaults::tab_hover_background();
                self.tab_active_text = crate::defaults::tab_active_text();
                self.tab_inactive_text = crate::defaults::tab_inactive_text();
                self.tab_active_indicator = crate::defaults::tab_active_indicator();
                self.tab_border_color = crate::defaults::tab_border_color();
                self.tab_border_width = crate::defaults::tab_border_width();
                self.tab_bar_height = crate::defaults::tab_bar_height();
            }
            TabStyle::Light => {
                self.tab_bar_background = [235, 235, 235];
                self.tab_active_background = [255, 255, 255];
                self.tab_inactive_background = [225, 225, 225];
                self.tab_hover_background = [240, 240, 240];
                self.tab_active_text = [30, 30, 30];
                self.tab_inactive_text = [100, 100, 100];
                self.tab_active_indicator = [50, 120, 220];
                self.tab_border_color = [200, 200, 200];
                self.tab_border_width = 1.0;
                self.tab_bar_height = crate::defaults::tab_bar_height();
            }
            TabStyle::Compact => {
                // Smaller tabs, tighter spacing
                self.tab_bar_background = [35, 35, 35];
                self.tab_active_background = [55, 55, 55];
                self.tab_inactive_background = [35, 35, 35];
                self.tab_hover_background = [45, 45, 45];
                self.tab_active_text = [240, 240, 240];
                self.tab_inactive_text = [160, 160, 160];
                self.tab_active_indicator = [80, 140, 240];
                self.tab_border_color = [60, 60, 60];
                self.tab_border_width = 0.5;
                self.tab_bar_height = 22.0;
            }
            TabStyle::Minimal => {
                // Very clean, flat look with minimal decoration
                self.tab_bar_background = [30, 30, 30];
                self.tab_active_background = [30, 30, 30];
                self.tab_inactive_background = [30, 30, 30];
                self.tab_hover_background = [40, 40, 40];
                self.tab_active_text = [255, 255, 255];
                self.tab_inactive_text = [120, 120, 120];
                self.tab_active_indicator = [100, 150, 255];
                self.tab_border_color = [30, 30, 30]; // No visible border
                self.tab_border_width = 0.0;
                self.tab_bar_height = 26.0;
            }
            TabStyle::HighContrast => {
                // Maximum contrast for accessibility
                self.tab_bar_background = [0, 0, 0];
                self.tab_active_background = [255, 255, 255];
                self.tab_inactive_background = [30, 30, 30];
                self.tab_hover_background = [60, 60, 60];
                self.tab_active_text = [0, 0, 0];
                self.tab_inactive_text = [255, 255, 255];
                self.tab_active_indicator = [255, 255, 0];
                self.tab_border_color = [255, 255, 255];
                self.tab_border_width = 2.0;
                self.tab_bar_height = 30.0;
            }
            TabStyle::Automatic => {
                // No-op here: actual style is resolved and applied by apply_system_tab_style()
            }
        }
    }

    /// Load configuration from file or create default
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path();
        log::info!("Config path: {:?}", config_path);

        if config_path.exists() {
            log::info!("Loading existing config from {:?}", config_path);
            let contents = fs::read_to_string(&config_path)?;
            let contents = substitute_variables(&contents);
            let mut config: Config = serde_yaml::from_str(&contents)?;

            // Merge in any new default keybindings that don't exist in user's config
            config.merge_default_keybindings();

            // Generate keybindings for snippets and actions
            config.generate_snippet_action_keybindings();

            // Load last working directory from state file (for "previous session" mode)
            config.load_last_working_directory();

            Ok(config)
        } else {
            log::info!(
                "Config file not found, creating default at {:?}",
                config_path
            );
            // Create default config and save it
            let mut config = Self::default();
            // Generate keybindings for snippets and actions
            config.generate_snippet_action_keybindings();
            if let Err(e) = config.save() {
                log::error!("Failed to save default config: {}", e);
                return Err(e);
            }

            // Load last working directory from state file (for "previous session" mode)
            config.load_last_working_directory();

            log::info!("Default config created successfully");
            Ok(config)
        }
    }

    /// Merge default keybindings into the user's config.
    /// Only adds keybindings for actions that don't already exist in the user's config.
    /// This ensures new features with default keybindings are available to existing users.
    fn merge_default_keybindings(&mut self) {
        let default_keybindings = crate::defaults::keybindings();

        // Get the set of actions already configured by the user (owned strings to avoid borrow issues)
        let existing_actions: std::collections::HashSet<String> = self
            .keybindings
            .iter()
            .map(|kb| kb.action.clone())
            .collect();

        // Add any default keybindings whose actions are not already configured
        let mut added_count = 0;
        for default_kb in default_keybindings {
            if !existing_actions.contains(&default_kb.action) {
                log::info!(
                    "Adding new default keybinding: {} -> {}",
                    default_kb.key,
                    default_kb.action
                );
                self.keybindings.push(default_kb);
                added_count += 1;
            }
        }

        if added_count > 0 {
            log::info!(
                "Merged {} new default keybinding(s) into user config",
                added_count
            );
        }
    }

    /// Generate keybindings for snippets and actions that have keybindings configured.
    ///
    /// This method adds or updates keybindings for snippets and actions in the keybindings list,
    /// using the format "snippet:<id>" for snippets and "action:<id>" for actions.
    /// If a keybinding for a snippet/action already exists, it will be updated with the new key.
    pub fn generate_snippet_action_keybindings(&mut self) {
        use crate::config::KeyBinding;

        // Track actions we've seen to remove stale keybindings later
        let mut seen_actions = std::collections::HashSet::new();
        let mut added_count = 0;
        let mut updated_count = 0;

        // Generate keybindings for snippets
        for snippet in &self.snippets {
            if let Some(key) = &snippet.keybinding {
                let action = format!("snippet:{}", snippet.id);
                seen_actions.insert(action.clone());

                if !key.is_empty() && snippet.enabled && snippet.keybinding_enabled {
                    // Check if this action already has a keybinding
                    if let Some(existing) =
                        self.keybindings.iter_mut().find(|kb| kb.action == action)
                    {
                        // Update existing keybinding if the key changed
                        if existing.key != *key {
                            log::info!(
                                "Updating keybinding for snippet '{}': {} -> {} (was: {})",
                                snippet.title,
                                key,
                                action,
                                existing.key
                            );
                            existing.key = key.clone();
                            updated_count += 1;
                        }
                    } else {
                        // Add new keybinding
                        log::info!(
                            "Adding keybinding for snippet '{}': {} -> {} (enabled={}, keybinding_enabled={})",
                            snippet.title,
                            key,
                            action,
                            snippet.enabled,
                            snippet.keybinding_enabled
                        );
                        self.keybindings.push(KeyBinding {
                            key: key.clone(),
                            action,
                        });
                        added_count += 1;
                    }
                } else if !key.is_empty() {
                    log::info!(
                        "Skipping keybinding for snippet '{}': {} (enabled={}, keybinding_enabled={})",
                        snippet.title,
                        key,
                        snippet.enabled,
                        snippet.keybinding_enabled
                    );
                }
            }
        }

        // Generate keybindings for actions
        for action_config in &self.actions {
            if let Some(key) = action_config.keybinding() {
                let action = format!("action:{}", action_config.id());
                seen_actions.insert(action.clone());

                if !key.is_empty() && action_config.keybinding_enabled() {
                    // Check if this action already has a keybinding
                    if let Some(existing) =
                        self.keybindings.iter_mut().find(|kb| kb.action == action)
                    {
                        // Update existing keybinding if the key changed
                        if existing.key != key {
                            log::info!(
                                "Updating keybinding for action '{}': {} -> {} (was: {})",
                                action_config.title(),
                                key,
                                action,
                                existing.key
                            );
                            existing.key = key.to_string();
                            updated_count += 1;
                        }
                    } else {
                        // Add new keybinding
                        log::info!(
                            "Adding keybinding for action '{}': {} -> {} (keybinding_enabled={})",
                            action_config.title(),
                            key,
                            action,
                            action_config.keybinding_enabled()
                        );
                        self.keybindings.push(KeyBinding {
                            key: key.to_string(),
                            action,
                        });
                        added_count += 1;
                    }
                } else if !key.is_empty() {
                    log::info!(
                        "Skipping keybinding for action '{}': {} (keybinding_enabled={})",
                        action_config.title(),
                        key,
                        action_config.keybinding_enabled()
                    );
                }
            }
        }

        // Remove stale keybindings for snippets that no longer have keybindings or are disabled
        let original_len = self.keybindings.len();
        self.keybindings.retain(|kb| {
            // Keep if it's not a snippet/action keybinding
            if !kb.action.starts_with("snippet:") && !kb.action.starts_with("action:") {
                return true;
            }
            // Keep if we saw it during our scan
            seen_actions.contains(&kb.action)
        });
        let removed_count = original_len - self.keybindings.len();

        if added_count > 0 || updated_count > 0 || removed_count > 0 {
            log::info!(
                "Snippet/Action keybindings: {} added, {} updated, {} removed",
                added_count,
                updated_count,
                removed_count
            );
        }
    }

    /// Save configuration to file
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path();

        // Create parent directory if it doesn't exist
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let yaml = serde_yaml::to_string(self)?;
        fs::write(&config_path, yaml)?;

        Ok(())
    }

    /// Get the configuration file path (using XDG convention)
    pub fn config_path() -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            if let Some(config_dir) = dirs::config_dir() {
                config_dir.join("par-term").join("config.yaml")
            } else {
                PathBuf::from("config.yaml")
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            // Use XDG convention on all platforms: ~/.config/par-term/config.yaml
            if let Some(home_dir) = dirs::home_dir() {
                home_dir
                    .join(".config")
                    .join("par-term")
                    .join("config.yaml")
            } else {
                // Fallback if home directory cannot be determined
                PathBuf::from("config.yaml")
            }
        }
    }

    /// Resolve the tmux executable path at runtime.
    /// If the configured path is absolute and exists, use it.
    /// If it's "tmux" (the default), search PATH and common installation locations.
    /// This handles cases where PATH may be incomplete (e.g., app launched from Finder).
    pub fn resolve_tmux_path(&self) -> String {
        let configured = &self.tmux_path;

        // If it's an absolute path and exists, use it directly
        if configured.starts_with('/') && std::path::Path::new(configured).exists() {
            return configured.clone();
        }

        // If it's not just "tmux", return it and let the OS try
        if configured != "tmux" {
            return configured.clone();
        }

        // Search for tmux in PATH
        if let Ok(path_env) = std::env::var("PATH") {
            let separator = if cfg!(windows) { ';' } else { ':' };
            let executable = if cfg!(windows) { "tmux.exe" } else { "tmux" };

            for dir in path_env.split(separator) {
                let candidate = std::path::Path::new(dir).join(executable);
                if candidate.exists() {
                    return candidate.to_string_lossy().to_string();
                }
            }
        }

        // Fall back to common paths for environments where PATH might be incomplete
        #[cfg(target_os = "macos")]
        {
            let macos_paths = [
                "/opt/homebrew/bin/tmux", // Homebrew on Apple Silicon
                "/usr/local/bin/tmux",    // Homebrew on Intel / MacPorts
            ];
            for path in macos_paths {
                if std::path::Path::new(path).exists() {
                    return path.to_string();
                }
            }
        }

        #[cfg(target_os = "linux")]
        {
            let linux_paths = [
                "/usr/bin/tmux",       // Most distros
                "/usr/local/bin/tmux", // Manual install
                "/snap/bin/tmux",      // Snap package
            ];
            for path in linux_paths {
                if std::path::Path::new(path).exists() {
                    return path.to_string();
                }
            }
        }

        // Final fallback - return configured value
        configured.clone()
    }

    /// Get the session logs directory path, resolving ~ if present
    /// Creates the directory if it doesn't exist
    pub fn logs_dir(&self) -> PathBuf {
        let path = if self.session_log_directory.starts_with("~/") {
            if let Some(home) = dirs::home_dir() {
                home.join(&self.session_log_directory[2..])
            } else {
                PathBuf::from(&self.session_log_directory)
            }
        } else {
            PathBuf::from(&self.session_log_directory)
        };

        // Create directory if it doesn't exist
        if !path.exists()
            && let Err(e) = std::fs::create_dir_all(&path)
        {
            log::warn!("Failed to create logs directory {:?}: {}", path, e);
        }

        path
    }

    /// Get the shaders directory path (using XDG convention)
    pub fn shaders_dir() -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            if let Some(config_dir) = dirs::config_dir() {
                config_dir.join("par-term").join("shaders")
            } else {
                PathBuf::from("shaders")
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            if let Some(home_dir) = dirs::home_dir() {
                home_dir.join(".config").join("par-term").join("shaders")
            } else {
                PathBuf::from("shaders")
            }
        }
    }

    /// Get the full path to a shader file
    /// If the shader path is absolute, returns it as-is
    /// Otherwise, resolves it relative to the shaders directory
    pub fn shader_path(shader_name: &str) -> PathBuf {
        let path = PathBuf::from(shader_name);
        if path.is_absolute() {
            path
        } else {
            Self::shaders_dir().join(shader_name)
        }
    }

    /// Resolve a texture path, expanding ~ to home directory
    /// and resolving relative paths relative to the shaders directory.
    /// Returns the expanded path or the original if expansion fails
    pub fn resolve_texture_path(path: &str) -> PathBuf {
        if path.starts_with("~/")
            && let Some(home) = dirs::home_dir()
        {
            return home.join(&path[2..]);
        }
        let path_buf = PathBuf::from(path);
        if path_buf.is_absolute() {
            path_buf
        } else {
            Self::shaders_dir().join(path)
        }
    }

    /// Get the channel texture paths as an array of Options
    /// Returns [channel0, channel1, channel2, channel3] for iChannel0-3
    #[allow(dead_code)]
    pub fn shader_channel_paths(&self) -> [Option<PathBuf>; 4] {
        [
            self.custom_shader_channel0
                .as_ref()
                .map(|p| Self::resolve_texture_path(p)),
            self.custom_shader_channel1
                .as_ref()
                .map(|p| Self::resolve_texture_path(p)),
            self.custom_shader_channel2
                .as_ref()
                .map(|p| Self::resolve_texture_path(p)),
            self.custom_shader_channel3
                .as_ref()
                .map(|p| Self::resolve_texture_path(p)),
        ]
    }

    /// Get the cubemap path prefix (resolved)
    /// Returns None if not configured, otherwise the resolved path prefix
    #[allow(dead_code)]
    pub fn shader_cubemap_path(&self) -> Option<PathBuf> {
        self.custom_shader_cubemap
            .as_ref()
            .map(|p| Self::resolve_texture_path(p))
    }

    /// Set the window title
    #[allow(dead_code)]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.window_title = title.into();
        self
    }

    /// Load theme configuration
    pub fn load_theme(&self) -> Theme {
        Theme::by_name(&self.theme).unwrap_or_default()
    }

    /// Apply system theme if auto_dark_mode is enabled.
    /// Returns true if the theme was changed.
    pub fn apply_system_theme(&mut self, is_dark: bool) -> bool {
        if !self.auto_dark_mode {
            return false;
        }
        let new_theme = if is_dark {
            &self.dark_theme
        } else {
            &self.light_theme
        };
        if self.theme != *new_theme {
            self.theme = new_theme.clone();
            true
        } else {
            false
        }
    }

    /// Apply tab style based on system theme when tab_style is Automatic.
    /// Returns true if the style was applied.
    pub fn apply_system_tab_style(&mut self, is_dark: bool) -> bool {
        if self.tab_style != TabStyle::Automatic {
            return false;
        }
        let target = if is_dark {
            self.dark_tab_style
        } else {
            self.light_tab_style
        };
        // Temporarily set to concrete style, apply colors, then restore Automatic
        self.tab_style = target;
        self.apply_tab_style();
        self.tab_style = TabStyle::Automatic;
        true
    }

    /// Get the user override config for a specific shader (if any)
    pub fn get_shader_override(&self, shader_name: &str) -> Option<&ShaderConfig> {
        self.shader_configs.get(shader_name)
    }

    /// Get the user override config for a specific cursor shader (if any)
    pub fn get_cursor_shader_override(&self, shader_name: &str) -> Option<&CursorShaderConfig> {
        self.cursor_shader_configs.get(shader_name)
    }

    /// Get or create a mutable reference to a shader's config override
    pub fn get_or_create_shader_override(&mut self, shader_name: &str) -> &mut ShaderConfig {
        self.shader_configs
            .entry(shader_name.to_string())
            .or_default()
    }

    /// Get or create a mutable reference to a cursor shader's config override
    pub fn get_or_create_cursor_shader_override(
        &mut self,
        shader_name: &str,
    ) -> &mut CursorShaderConfig {
        self.cursor_shader_configs
            .entry(shader_name.to_string())
            .or_default()
    }

    /// Remove a shader config override (revert to defaults)
    pub fn remove_shader_override(&mut self, shader_name: &str) {
        self.shader_configs.remove(shader_name);
    }

    /// Remove a cursor shader config override (revert to defaults)
    pub fn remove_cursor_shader_override(&mut self, shader_name: &str) {
        self.cursor_shader_configs.remove(shader_name);
    }

    /// Check if the shaders folder is missing or empty
    /// Returns true if user should be prompted to install shaders
    pub fn should_prompt_shader_install(&self) -> bool {
        // Only prompt if the preference is set to "ask"
        if self.shader_install_prompt != ShaderInstallPrompt::Ask {
            return false;
        }

        let shaders_dir = Self::shaders_dir();

        // Check if directory doesn't exist
        if !shaders_dir.exists() {
            return true;
        }

        // Check if directory is empty or has no .glsl files
        if let Ok(entries) = std::fs::read_dir(&shaders_dir) {
            for entry in entries.flatten() {
                if let Some(ext) = entry.path().extension()
                    && ext == "glsl"
                {
                    return false; // Found at least one shader
                }
            }
        }

        true // Directory exists but has no .glsl files
    }

    /// Get the configuration directory path (using XDG convention)
    pub fn config_dir() -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            if let Some(config_dir) = dirs::config_dir() {
                config_dir.join("par-term")
            } else {
                PathBuf::from(".")
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            if let Some(home_dir) = dirs::home_dir() {
                home_dir.join(".config").join("par-term")
            } else {
                PathBuf::from(".")
            }
        }
    }

    /// Get the shell integration directory (same as config dir)
    pub fn shell_integration_dir() -> PathBuf {
        Self::config_dir()
    }

    /// Check if shell integration should be prompted
    pub fn should_prompt_shell_integration(&self) -> bool {
        if self.shell_integration_state != InstallPromptState::Ask {
            return false;
        }

        let current_version = env!("CARGO_PKG_VERSION");

        // Check if already prompted for this version
        if let Some(ref prompted) = self.integration_versions.shell_integration_prompted_version
            && prompted == current_version
        {
            return false;
        }

        // Check if installed and up to date
        if let Some(ref installed) = self
            .integration_versions
            .shell_integration_installed_version
            && installed == current_version
        {
            return false;
        }

        true
    }

    /// Check if shaders should be prompted (version-aware logic)
    pub fn should_prompt_shader_install_versioned(&self) -> bool {
        if self.shader_install_prompt != ShaderInstallPrompt::Ask {
            return false;
        }

        let current_version = env!("CARGO_PKG_VERSION");

        // Check if already prompted for this version
        if let Some(ref prompted) = self.integration_versions.shaders_prompted_version
            && prompted == current_version
        {
            return false;
        }

        // Check if installed and up to date
        if let Some(ref installed) = self.integration_versions.shaders_installed_version
            && installed == current_version
        {
            return false;
        }

        // Also check if shaders folder exists and has files
        let shaders_dir = Self::shaders_dir();
        !shaders_dir.exists() || !Self::has_shader_files(&shaders_dir)
    }

    /// Check if a directory contains shader files (.glsl)
    fn has_shader_files(dir: &PathBuf) -> bool {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                if let Some(ext) = entry.path().extension()
                    && ext == "glsl"
                {
                    return true;
                }
            }
        }
        false
    }

    /// Check if either integration should be prompted
    pub fn should_prompt_integrations(&self) -> bool {
        self.should_prompt_shader_install_versioned() || self.should_prompt_shell_integration()
    }

    /// Get the effective startup directory based on configuration mode.
    ///
    /// Priority:
    /// 1. Legacy `working_directory` if set (backward compatibility)
    /// 2. Based on `startup_directory_mode`:
    ///    - Home: Returns user's home directory
    ///    - Previous: Returns `last_working_directory` if valid, else home
    ///    - Custom: Returns `startup_directory` if set and valid, else home
    ///
    /// Returns None if the effective directory doesn't exist (caller should fall back to default).
    pub fn get_effective_startup_directory(&self) -> Option<String> {
        // Legacy working_directory takes precedence for backward compatibility
        if let Some(ref wd) = self.working_directory {
            let expanded = Self::expand_home_dir(wd);
            if std::path::Path::new(&expanded).exists() {
                return Some(expanded);
            }
            log::warn!(
                "Configured working_directory '{}' does not exist, using default",
                wd
            );
        }

        match self.startup_directory_mode {
            StartupDirectoryMode::Home => {
                // Return home directory
                dirs::home_dir().map(|p| p.to_string_lossy().to_string())
            }
            StartupDirectoryMode::Previous => {
                // Return last working directory if it exists
                if let Some(ref last_dir) = self.last_working_directory {
                    let expanded = Self::expand_home_dir(last_dir);
                    if std::path::Path::new(&expanded).exists() {
                        return Some(expanded);
                    }
                    log::warn!(
                        "Previous session directory '{}' no longer exists, using home",
                        last_dir
                    );
                }
                // Fall back to home
                dirs::home_dir().map(|p| p.to_string_lossy().to_string())
            }
            StartupDirectoryMode::Custom => {
                // Return custom directory if set and exists
                if let Some(ref custom_dir) = self.startup_directory {
                    let expanded = Self::expand_home_dir(custom_dir);
                    if std::path::Path::new(&expanded).exists() {
                        return Some(expanded);
                    }
                    log::warn!(
                        "Custom startup directory '{}' does not exist, using home",
                        custom_dir
                    );
                }
                // Fall back to home
                dirs::home_dir().map(|p| p.to_string_lossy().to_string())
            }
        }
    }

    /// Expand ~ to home directory in a path string
    fn expand_home_dir(path: &str) -> String {
        if let Some(suffix) = path.strip_prefix("~/")
            && let Some(home) = dirs::home_dir()
        {
            return home.join(suffix).to_string_lossy().to_string();
        }
        path.to_string()
    }

    /// Get the state file path for storing session state (like last working directory)
    pub fn state_file_path() -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            if let Some(data_dir) = dirs::data_local_dir() {
                data_dir.join("par-term").join("state.yaml")
            } else {
                PathBuf::from("state.yaml")
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            if let Some(home_dir) = dirs::home_dir() {
                home_dir
                    .join(".local")
                    .join("share")
                    .join("par-term")
                    .join("state.yaml")
            } else {
                PathBuf::from("state.yaml")
            }
        }
    }

    /// Save the last working directory to state file
    pub fn save_last_working_directory(&mut self, directory: &str) -> Result<()> {
        self.last_working_directory = Some(directory.to_string());

        // Save to state file for persistence across sessions
        let state_path = Self::state_file_path();
        if let Some(parent) = state_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Create a minimal state struct for persistence
        #[derive(Serialize)]
        struct SessionState {
            last_working_directory: Option<String>,
        }

        let state = SessionState {
            last_working_directory: Some(directory.to_string()),
        };

        let yaml = serde_yaml::to_string(&state)?;
        fs::write(&state_path, yaml)?;

        log::debug!(
            "Saved last working directory to {:?}: {}",
            state_path,
            directory
        );
        Ok(())
    }

    /// Get per-pane background config for a given pane index, if configured
    /// Returns (image_path, mode, opacity) tuple for easy conversion to runtime type
    pub fn get_pane_background(&self, index: usize) -> Option<(String, BackgroundImageMode, f32)> {
        self.pane_backgrounds
            .iter()
            .find(|pb| pb.index == index)
            .map(|pb| (pb.image.clone(), pb.mode, pb.opacity))
    }

    /// Load the last working directory from state file
    pub fn load_last_working_directory(&mut self) {
        let state_path = Self::state_file_path();
        if !state_path.exists() {
            return;
        }

        #[derive(Deserialize)]
        struct SessionState {
            last_working_directory: Option<String>,
        }

        match fs::read_to_string(&state_path) {
            Ok(contents) => {
                if let Ok(state) = serde_yaml::from_str::<SessionState>(&contents)
                    && let Some(dir) = state.last_working_directory
                {
                    log::debug!("Loaded last working directory from state file: {}", dir);
                    self.last_working_directory = Some(dir);
                }
            }
            Err(e) => {
                log::warn!("Failed to read state file {:?}: {}", state_path, e);
            }
        }
    }
}
