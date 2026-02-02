//! Terminal configuration management.
//!
//! This module provides configuration loading, saving, and default values
//! for the terminal emulator.

mod defaults;
pub mod shader_config;
pub mod shader_metadata;
mod types;

// Re-export shader config resolution functions (used by consumers)
#[allow(unused_imports)]
pub use shader_config::{resolve_cursor_shader_config, resolve_shader_config};
// Re-export shader metadata types and functions
pub use shader_metadata::{CursorShaderMetadataCache, ShaderMetadataCache};
#[allow(unused_imports)]
pub use shader_metadata::{
    parse_cursor_shader_metadata, parse_shader_metadata, update_cursor_shader_metadata_file,
    update_shader_metadata_file,
};
// Re-export config types
pub use types::{
    BackgroundImageMode, BackgroundMode, CursorShaderConfig, CursorShaderMetadata, CursorStyle,
    DroppedFileQuoteStyle, FontRange, KeyBinding, OptionKeyMode, SessionLogFormat, ShaderConfig,
    ShaderInstallPrompt, ShaderMetadata, SmartSelectionPrecision, SmartSelectionRule, TabBarMode,
    ThinStrokesMode, UnfocusedCursorStyle, UpdateCheckFrequency, VsyncMode, WindowType,
    default_smart_selection_rules,
};
// KeyModifier is exported for potential future use (e.g., custom keybinding UI)
#[allow(unused_imports)]
pub use types::KeyModifier;
#[allow(unused_imports)]
pub use types::{ResolvedCursorShaderConfig, ResolvedShaderConfig};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::themes::Theme;

/// Configuration for the terminal emulator
/// Aligned with par-tui-term naming conventions for consistency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    // ========================================================================
    // Window & Display (GUI-specific)
    // ========================================================================
    /// Number of columns in the terminal
    #[serde(default = "defaults::cols")]
    pub cols: usize,

    /// Number of rows in the terminal
    #[serde(default = "defaults::rows")]
    pub rows: usize,

    /// Font size in points
    #[serde(default = "defaults::font_size")]
    pub font_size: f32,

    /// Font family name (regular/normal weight)
    #[serde(default = "defaults::font_family")]
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
    #[serde(default = "defaults::line_spacing")]
    pub line_spacing: f32,

    /// Character width multiplier (0.5 = narrow, 0.6 = default, 0.7 = wide)
    #[serde(default = "defaults::char_spacing")]
    pub char_spacing: f32,

    /// Enable text shaping for ligatures and complex scripts
    /// When enabled, uses HarfBuzz for proper ligature, emoji, and complex script rendering
    #[serde(default = "defaults::text_shaping")]
    pub enable_text_shaping: bool,

    /// Enable ligatures (requires enable_text_shaping)
    #[serde(default = "defaults::bool_true")]
    pub enable_ligatures: bool,

    /// Enable kerning adjustments (requires enable_text_shaping)
    #[serde(default = "defaults::bool_true")]
    pub enable_kerning: bool,

    /// Enable anti-aliasing for font rendering
    /// When false, text is rendered without smoothing (aliased/pixelated)
    #[serde(default = "defaults::bool_true")]
    pub font_antialias: bool,

    /// Enable hinting for font rendering
    /// Hinting improves text clarity at small sizes by aligning glyphs to pixel boundaries
    /// Disable for a softer, more "true to design" appearance
    #[serde(default = "defaults::bool_true")]
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

    /// Window title
    #[serde(default = "defaults::window_title")]
    pub window_title: String,

    /// Allow applications to change the window title via OSC escape sequences
    /// When false, the window title will always be the configured window_title
    #[serde(default = "defaults::bool_true")]
    pub allow_title_change: bool,

    /// Maximum frames per second (FPS) target
    /// Controls how frequently the terminal requests screen redraws.
    /// Note: On macOS, actual FPS may be lower (~22-25) due to system-level
    /// VSync throttling in wgpu/Metal, regardless of this setting.
    /// Default: 60
    #[serde(default = "defaults::max_fps", alias = "refresh_rate")]
    pub max_fps: u32,

    /// VSync mode - controls GPU frame synchronization
    /// - immediate: No VSync, render as fast as possible (lowest latency, highest power)
    /// - mailbox: Cap at monitor refresh rate with triple buffering (balanced)
    /// - fifo: Strict VSync with double buffering (lowest power, slight input lag)
    ///
    /// Default: immediate (for maximum performance)
    #[serde(default)]
    pub vsync_mode: VsyncMode,

    /// Window padding in pixels
    #[serde(default = "defaults::window_padding")]
    pub window_padding: f32,

    /// Window opacity/transparency (0.0 = fully transparent, 1.0 = fully opaque)
    #[serde(default = "defaults::window_opacity")]
    pub window_opacity: f32,

    /// Keep window always on top of other windows
    #[serde(default = "defaults::bool_false")]
    pub window_always_on_top: bool,

    /// Show window decorations (title bar, borders)
    #[serde(default = "defaults::bool_true")]
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

    /// Lock window size to prevent resize
    /// When true, the window cannot be resized by the user
    #[serde(default = "defaults::bool_false")]
    pub lock_window_size: bool,

    /// Show window number in title bar
    /// Useful when multiple par-term windows are open
    #[serde(default = "defaults::bool_false")]
    pub show_window_number: bool,

    /// When true, only the default terminal background is transparent.
    /// Colored backgrounds (syntax highlighting, status bars, etc.) remain opaque.
    /// This keeps text readable while allowing window transparency.
    #[serde(default = "defaults::bool_true")]
    pub transparency_affects_only_default_background: bool,

    /// When true, text is always rendered at full opacity regardless of window transparency.
    /// This ensures text remains crisp and readable even with transparent backgrounds.
    #[serde(default = "defaults::bool_true")]
    pub keep_text_opaque: bool,

    /// Enable window blur effect (macOS only)
    /// Blurs content behind the transparent window for better readability
    #[serde(default = "defaults::bool_false")]
    pub blur_enabled: bool,

    /// Blur radius in points (0-64, macOS only)
    /// Higher values = more blur. Default: 10
    #[serde(default = "defaults::blur_radius")]
    pub blur_radius: u32,

    /// Background image path (optional, supports ~ for home directory)
    #[serde(default)]
    pub background_image: Option<String>,

    /// Enable or disable background image rendering (even if a path is set)
    #[serde(default = "defaults::bool_true")]
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
    #[serde(default = "defaults::background_image_opacity")]
    pub background_image_opacity: f32,

    /// Background mode selection (default, color, or image)
    #[serde(default)]
    pub background_mode: BackgroundMode,

    /// Custom solid background color [R, G, B] (0-255)
    /// Used when background_mode is "color"
    /// Transparency is controlled by window_opacity
    #[serde(default = "defaults::background_color")]
    pub background_color: [u8; 3],

    /// Custom shader file path (GLSL format, relative to shaders folder or absolute)
    /// Shaders are loaded from ~/.config/par-term/shaders/ by default
    /// Supports Ghostty/Shadertoy-style GLSL shaders with iTime, iResolution, iChannel0-4
    #[serde(default)]
    pub custom_shader: Option<String>,

    /// Enable or disable the custom shader (even if a path is set)
    #[serde(default = "defaults::bool_true")]
    pub custom_shader_enabled: bool,

    /// Enable animation in custom shader (updates iTime uniform each frame)
    /// When disabled, iTime is fixed at 0.0 for static effects
    #[serde(default = "defaults::bool_true")]
    pub custom_shader_animation: bool,

    /// Animation speed multiplier for custom shader (1.0 = normal speed)
    #[serde(default = "defaults::custom_shader_speed")]
    pub custom_shader_animation_speed: f32,

    /// Text opacity when using custom shader (0.0 = transparent, 1.0 = fully opaque)
    /// This allows text to remain readable while the shader effect shows through the background
    #[serde(default = "defaults::text_opacity")]
    pub custom_shader_text_opacity: f32,

    /// When enabled, the shader receives the full rendered terminal content (text + background)
    /// and can manipulate/distort it. When disabled (default), the shader only provides
    /// a background and text is composited on top cleanly.
    #[serde(default = "defaults::bool_false")]
    pub custom_shader_full_content: bool,

    /// Brightness multiplier for custom shader output (0.05 = very dark, 1.0 = full brightness)
    /// This dims the shader background to improve text readability
    #[serde(default = "defaults::custom_shader_brightness")]
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
    #[serde(default = "defaults::cubemap_enabled")]
    pub custom_shader_cubemap_enabled: bool,

    /// Use the app's background image as iChannel0 for custom shaders
    /// When enabled, the configured background image is bound as iChannel0 instead of
    /// the custom_shader_channel0 texture. This allows shaders to incorporate the
    /// background image without requiring a separate texture file.
    #[serde(default = "defaults::use_background_as_channel0")]
    pub custom_shader_use_background_as_channel0: bool,

    // ========================================================================
    // Cursor Shader Settings (separate from background shader)
    // ========================================================================
    /// Cursor shader file path (GLSL format, relative to shaders folder or absolute)
    /// This is a separate shader specifically for cursor effects (trails, glows, etc.)
    #[serde(default)]
    pub cursor_shader: Option<String>,

    /// Enable or disable the cursor shader (even if a path is set)
    #[serde(default = "defaults::bool_false")]
    pub cursor_shader_enabled: bool,

    /// Enable animation in cursor shader (updates iTime uniform each frame)
    #[serde(default = "defaults::bool_true")]
    pub cursor_shader_animation: bool,

    /// Animation speed multiplier for cursor shader (1.0 = normal speed)
    #[serde(default = "defaults::custom_shader_speed")]
    pub cursor_shader_animation_speed: f32,

    /// Cursor color for shader effects [R, G, B] (0-255)
    /// This color is passed to the shader via iCursorShaderColor uniform
    #[serde(default = "defaults::cursor_shader_color")]
    pub cursor_shader_color: [u8; 3],

    /// Duration of cursor trail effect in seconds
    /// Passed to shader via iCursorTrailDuration uniform
    #[serde(default = "defaults::cursor_trail_duration")]
    pub cursor_shader_trail_duration: f32,

    /// Radius of cursor glow effect in pixels
    /// Passed to shader via iCursorGlowRadius uniform
    #[serde(default = "defaults::cursor_glow_radius")]
    pub cursor_shader_glow_radius: f32,

    /// Intensity of cursor glow effect (0.0 = none, 1.0 = full)
    /// Passed to shader via iCursorGlowIntensity uniform
    #[serde(default = "defaults::cursor_glow_intensity")]
    pub cursor_shader_glow_intensity: f32,

    /// Hide the default cursor when cursor shader is enabled
    /// When true and cursor_shader_enabled is true, the normal cursor is not drawn
    /// This allows cursor shaders to fully replace the cursor rendering
    #[serde(default = "defaults::bool_false")]
    pub cursor_shader_hides_cursor: bool,

    /// Disable cursor shader while in alt screen (vim, less, htop)
    /// Keeps current behavior by default for TUI compatibility
    #[serde(default = "defaults::cursor_shader_disable_in_alt_screen")]
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

    // ========================================================================
    // Selection & Clipboard
    // ========================================================================
    /// Automatically copy selected text to clipboard
    #[serde(default = "defaults::bool_true")]
    pub auto_copy_selection: bool,

    /// Include trailing newline when copying lines
    /// Note: Inverted logic from old strip_trailing_newline_on_copy
    #[serde(
        default = "defaults::bool_false",
        alias = "strip_trailing_newline_on_copy"
    )]
    pub copy_trailing_newline: bool,

    /// Paste on middle mouse button click
    #[serde(default = "defaults::bool_true")]
    pub middle_click_paste: bool,

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
    #[serde(default = "defaults::scroll_speed")]
    pub mouse_scroll_speed: f32,

    /// Double-click timing threshold in milliseconds
    #[serde(default = "defaults::double_click_threshold")]
    pub mouse_double_click_threshold: u64,

    /// Triple-click timing threshold in milliseconds (typically same as double-click)
    #[serde(default = "defaults::triple_click_threshold")]
    pub mouse_triple_click_threshold: u64,

    /// Option+Click (macOS) / Alt+Click (Linux/Windows) moves cursor to clicked position
    /// Sends cursor movement escape sequences to position text cursor at click location
    /// Useful for quick cursor positioning in shells and editors
    #[serde(default = "defaults::bool_true")]
    pub option_click_moves_cursor: bool,

    /// Focus window automatically when mouse enters (without requiring a click)
    /// This is an accessibility feature that some users prefer
    #[serde(default = "defaults::bool_false")]
    pub focus_follows_mouse: bool,

    /// Report horizontal scroll events to terminal applications when mouse reporting is enabled
    /// Horizontal scroll uses button codes 6 (left) and 7 (right) in the mouse protocol
    #[serde(default = "defaults::bool_true")]
    pub report_horizontal_scroll: bool,

    // ========================================================================
    // Word Selection
    // ========================================================================
    /// Characters considered part of a word for double-click selection (in addition to alphanumeric)
    /// Default: "/-+\\~_." (matches iTerm2)
    /// Example: If you want to select entire paths, add "/" to include path separators
    #[serde(default = "defaults::word_characters")]
    pub word_characters: String,

    /// Enable smart selection rules for pattern-based double-click selection
    /// When enabled, double-click will try to match patterns like URLs, emails, paths
    /// before falling back to word boundary selection
    #[serde(default = "defaults::smart_selection_enabled")]
    pub smart_selection_enabled: bool,

    /// Smart selection rules for pattern-based double-click selection
    /// Rules are evaluated by precision (highest first). If a pattern matches
    /// at the cursor position, that text is selected instead of using word boundaries.
    #[serde(default = "types::default_smart_selection_rules")]
    pub smart_selection_rules: Vec<SmartSelectionRule>,

    // ========================================================================
    // Scrollback & Cursor
    // ========================================================================
    /// Maximum number of lines to keep in scrollback buffer
    #[serde(default = "defaults::scrollback", alias = "scrollback_size")]
    pub scrollback_lines: usize,

    // ========================================================================
    // Unicode Width Settings
    // ========================================================================
    /// Unicode version for character width calculations
    /// Different versions have different width tables, particularly for emoji.
    /// Options: unicode_9, unicode_10, ..., unicode_16, auto (default)
    #[serde(default = "defaults::unicode_version")]
    pub unicode_version: par_term_emu_core_rust::UnicodeVersion,

    /// Treatment of East Asian Ambiguous width characters
    /// - narrow: 1 cell width (Western default)
    /// - wide: 2 cell width (CJK default)
    #[serde(default = "defaults::ambiguous_width")]
    pub ambiguous_width: par_term_emu_core_rust::AmbiguousWidth,

    /// Enable cursor blinking
    #[serde(default = "defaults::bool_false")]
    pub cursor_blink: bool,

    /// Cursor blink interval in milliseconds
    #[serde(default = "defaults::cursor_blink_interval")]
    pub cursor_blink_interval: u64,

    /// Cursor style (block, beam, underline)
    #[serde(default)]
    pub cursor_style: CursorStyle,

    /// Cursor color [R, G, B] (0-255)
    #[serde(default = "defaults::cursor_color")]
    pub cursor_color: [u8; 3],

    /// Color of text under block cursor [R, G, B] (0-255)
    /// If not set (None), uses automatic contrast color
    /// Only affects block cursor style (beam and underline don't obscure text)
    #[serde(default)]
    pub cursor_text_color: Option<[u8; 3]>,

    /// Lock cursor visibility - prevent applications from hiding the cursor
    /// When true, the cursor remains visible regardless of DECTCEM escape sequences
    #[serde(default = "defaults::bool_false")]
    pub lock_cursor_visibility: bool,

    /// Lock cursor style - prevent applications from changing the cursor style
    /// When true, the cursor style from config is always used, ignoring DECSCUSR escape sequences
    #[serde(default = "defaults::bool_false")]
    pub lock_cursor_style: bool,

    /// Lock cursor blink - prevent applications from enabling cursor blink
    /// When true and cursor_blink is false, applications cannot enable blinking cursor
    #[serde(default = "defaults::bool_false")]
    pub lock_cursor_blink: bool,

    // ========================================================================
    // Cursor Enhancements (iTerm2-style features)
    // ========================================================================
    /// Enable horizontal guide line at cursor row for better tracking in wide terminals
    #[serde(default = "defaults::bool_false")]
    pub cursor_guide_enabled: bool,

    /// Cursor guide color [R, G, B, A] (0-255), subtle highlight spanning full terminal width
    #[serde(default = "defaults::cursor_guide_color")]
    pub cursor_guide_color: [u8; 4],

    /// Enable drop shadow behind cursor for better visibility against varying backgrounds
    #[serde(default = "defaults::bool_false")]
    pub cursor_shadow_enabled: bool,

    /// Cursor shadow color [R, G, B, A] (0-255)
    #[serde(default = "defaults::cursor_shadow_color")]
    pub cursor_shadow_color: [u8; 4],

    /// Cursor shadow offset in pixels [x, y]
    #[serde(default = "defaults::cursor_shadow_offset")]
    pub cursor_shadow_offset: [f32; 2],

    /// Cursor shadow blur radius in pixels
    #[serde(default = "defaults::cursor_shadow_blur")]
    pub cursor_shadow_blur: f32,

    /// Cursor boost (glow) intensity (0.0 = off, 1.0 = maximum boost)
    /// Adds a glow/highlight effect around the cursor for visibility
    #[serde(default = "defaults::cursor_boost")]
    pub cursor_boost: f32,

    /// Cursor boost glow color [R, G, B] (0-255)
    #[serde(default = "defaults::cursor_boost_color")]
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
    #[serde(default = "defaults::scrollbar_autohide_delay")]
    pub scrollbar_autohide_delay: u64,

    // ========================================================================
    // Theme & Colors
    // ========================================================================
    /// Color theme name to use for terminal colors
    #[serde(default = "defaults::theme")]
    pub theme: String,

    // ========================================================================
    // Screenshot
    // ========================================================================
    /// File format for screenshots (png, jpeg, svg, html)
    #[serde(default = "defaults::screenshot_format")]
    pub screenshot_format: String,

    // ========================================================================
    // Shell Behavior
    // ========================================================================
    /// Exit when shell exits
    #[serde(default = "defaults::bool_true", alias = "close_on_shell_exit")]
    pub exit_on_shell_exit: bool,

    /// Custom shell command (defaults to system shell if not specified)
    #[serde(default)]
    pub custom_shell: Option<String>,

    /// Arguments to pass to the shell
    #[serde(default)]
    pub shell_args: Option<Vec<String>>,

    /// Working directory for the shell (defaults to current directory)
    #[serde(default)]
    pub working_directory: Option<String>,

    /// Environment variables to set for the shell
    #[serde(default)]
    pub shell_env: Option<std::collections::HashMap<String, String>>,

    /// Whether to spawn the shell as a login shell (passes -l flag)
    /// This is important on macOS to properly initialize PATH from Homebrew, /etc/paths.d, etc.
    /// Default: true
    #[serde(default = "defaults::login_shell")]
    pub login_shell: bool,

    /// Text to send automatically when a terminal session starts
    /// Supports escape sequences: \n (newline), \r (carriage return), \t (tab), \xHH (hex), \e (ESC)
    #[serde(default = "defaults::initial_text")]
    pub initial_text: String,

    /// Delay in milliseconds before sending the initial text (to allow shell to be ready)
    #[serde(default = "defaults::initial_text_delay_ms")]
    pub initial_text_delay_ms: u64,

    /// Whether to append a newline after sending the initial text
    #[serde(default = "defaults::initial_text_send_newline")]
    pub initial_text_send_newline: bool,

    /// Answerback string sent in response to ENQ (0x05) control character
    /// This is a legacy terminal feature used for terminal identification.
    /// Default: empty (disabled) for security
    /// Common values: "par-term", "vt100", or custom identification
    /// Security note: Setting this may expose terminal identification to applications
    #[serde(default = "defaults::answerback_string")]
    pub answerback_string: String,

    // ========================================================================
    // Scrollbar (GUI-specific)
    // ========================================================================
    /// Scrollbar position (left or right)
    #[serde(default = "defaults::scrollbar_position")]
    pub scrollbar_position: String,

    /// Scrollbar width in pixels
    #[serde(default = "defaults::scrollbar_width")]
    pub scrollbar_width: f32,

    /// Scrollbar thumb color (RGBA: [r, g, b, a] where each is 0.0-1.0)
    #[serde(default = "defaults::scrollbar_thumb_color")]
    pub scrollbar_thumb_color: [f32; 4],

    /// Scrollbar track color (RGBA: [r, g, b, a] where each is 0.0-1.0)
    #[serde(default = "defaults::scrollbar_track_color")]
    pub scrollbar_track_color: [f32; 4],

    // ========================================================================
    // Clipboard Sync Limits
    // ========================================================================
    /// Maximum clipboard sync events retained for diagnostics
    #[serde(
        default = "defaults::clipboard_max_sync_events",
        alias = "max_clipboard_sync_events"
    )]
    pub clipboard_max_sync_events: usize,

    /// Maximum bytes stored per clipboard sync event
    #[serde(
        default = "defaults::clipboard_max_event_bytes",
        alias = "max_clipboard_event_bytes"
    )]
    pub clipboard_max_event_bytes: usize,

    // ========================================================================
    // Notifications
    // ========================================================================
    /// Forward BEL events to desktop notification centers
    #[serde(default = "defaults::bool_false", alias = "bell_desktop")]
    pub notification_bell_desktop: bool,

    /// Volume (0-100) for backend bell sound alerts (0 disables)
    #[serde(default = "defaults::bell_sound", alias = "bell_sound")]
    pub notification_bell_sound: u8,

    /// Enable backend visual bell overlay
    #[serde(default = "defaults::bool_true", alias = "bell_visual")]
    pub notification_bell_visual: bool,

    /// Enable notifications when activity resumes after inactivity
    #[serde(default = "defaults::bool_false", alias = "activity_notifications")]
    pub notification_activity_enabled: bool,

    /// Seconds of inactivity required before an activity alert fires
    #[serde(default = "defaults::activity_threshold", alias = "activity_threshold")]
    pub notification_activity_threshold: u64,

    /// Enable anti-idle keep-alive (sends code after idle period)
    #[serde(default = "defaults::bool_false")]
    pub anti_idle_enabled: bool,

    /// Seconds of inactivity before sending keep-alive code
    #[serde(default = "defaults::anti_idle_seconds")]
    pub anti_idle_seconds: u64,

    /// ASCII code to send as keep-alive (e.g., 0 = NUL, 27 = ESC)
    #[serde(default = "defaults::anti_idle_code")]
    pub anti_idle_code: u8,

    /// Enable notifications after prolonged silence
    #[serde(default = "defaults::bool_false", alias = "silence_notifications")]
    pub notification_silence_enabled: bool,

    /// Seconds of silence before a silence alert fires
    #[serde(default = "defaults::silence_threshold", alias = "silence_threshold")]
    pub notification_silence_threshold: u64,

    /// Enable notification when a shell/session exits
    #[serde(default = "defaults::bool_false", alias = "session_ended")]
    pub notification_session_ended: bool,

    /// Suppress desktop notifications when the terminal window is focused
    #[serde(default = "defaults::bool_true")]
    pub suppress_notifications_when_focused: bool,

    /// Maximum number of OSC 9/777 notification entries retained by backend
    #[serde(
        default = "defaults::notification_max_buffer",
        alias = "max_notifications"
    )]
    pub notification_max_buffer: usize,

    // ========================================================================
    // Tab Settings
    // ========================================================================
    /// Tab bar visibility mode (always, when_multiple, never)
    #[serde(default)]
    pub tab_bar_mode: TabBarMode,

    /// Tab bar height in pixels
    #[serde(default = "defaults::tab_bar_height")]
    pub tab_bar_height: f32,

    /// Show close button on tabs
    #[serde(default = "defaults::bool_true")]
    pub tab_show_close_button: bool,

    /// Show tab index numbers (for Cmd+1-9)
    #[serde(default = "defaults::bool_false")]
    pub tab_show_index: bool,

    /// New tab inherits working directory from active tab
    #[serde(default = "defaults::bool_true")]
    pub tab_inherit_cwd: bool,

    /// Maximum tabs per window (0 = unlimited)
    #[serde(default = "defaults::zero")]
    pub max_tabs: usize,

    // ========================================================================
    // Tab Bar Colors
    // ========================================================================
    /// Tab bar background color [R, G, B] (0-255)
    #[serde(default = "defaults::tab_bar_background")]
    pub tab_bar_background: [u8; 3],

    /// Active tab background color [R, G, B] (0-255)
    #[serde(default = "defaults::tab_active_background")]
    pub tab_active_background: [u8; 3],

    /// Inactive tab background color [R, G, B] (0-255)
    #[serde(default = "defaults::tab_inactive_background")]
    pub tab_inactive_background: [u8; 3],

    /// Hovered tab background color [R, G, B] (0-255)
    #[serde(default = "defaults::tab_hover_background")]
    pub tab_hover_background: [u8; 3],

    /// Active tab text color [R, G, B] (0-255)
    #[serde(default = "defaults::tab_active_text")]
    pub tab_active_text: [u8; 3],

    /// Inactive tab text color [R, G, B] (0-255)
    #[serde(default = "defaults::tab_inactive_text")]
    pub tab_inactive_text: [u8; 3],

    /// Active tab indicator/underline color [R, G, B] (0-255)
    #[serde(default = "defaults::tab_active_indicator")]
    pub tab_active_indicator: [u8; 3],

    /// Activity indicator dot color [R, G, B] (0-255)
    #[serde(default = "defaults::tab_activity_indicator")]
    pub tab_activity_indicator: [u8; 3],

    /// Bell indicator color [R, G, B] (0-255)
    #[serde(default = "defaults::tab_bell_indicator")]
    pub tab_bell_indicator: [u8; 3],

    /// Close button color [R, G, B] (0-255)
    #[serde(default = "defaults::tab_close_button")]
    pub tab_close_button: [u8; 3],

    /// Close button hover color [R, G, B] (0-255)
    #[serde(default = "defaults::tab_close_button_hover")]
    pub tab_close_button_hover: [u8; 3],

    /// Enable visual dimming of inactive tabs
    /// When true, inactive tabs are rendered with reduced opacity
    #[serde(default = "defaults::bool_true")]
    pub dim_inactive_tabs: bool,

    /// Opacity level for inactive tabs (0.0-1.0)
    /// Only used when dim_inactive_tabs is true
    /// Lower values make inactive tabs more transparent/dimmed
    #[serde(default = "defaults::inactive_tab_opacity")]
    pub inactive_tab_opacity: f32,

    /// Minimum tab width in pixels before horizontal scrolling is enabled
    /// When tabs cannot fit at this width, scroll buttons appear
    #[serde(default = "defaults::tab_min_width")]
    pub tab_min_width: f32,

    /// Tab border color [R, G, B] (0-255)
    /// A thin border around each tab to help distinguish them
    #[serde(default = "defaults::tab_border_color")]
    pub tab_border_color: [u8; 3],

    /// Tab border width in pixels (0 = no border)
    #[serde(default = "defaults::tab_border_width")]
    pub tab_border_width: f32,

    // ========================================================================
    // Split Pane Settings
    // ========================================================================
    /// Width of dividers between panes in pixels (visual width)
    #[serde(default = "defaults::pane_divider_width")]
    pub pane_divider_width: Option<f32>,

    /// Width of the drag hit area for resizing panes (should be >= divider width)
    /// A larger hit area makes it easier to grab the divider for resizing
    #[serde(default = "defaults::pane_divider_hit_width")]
    pub pane_divider_hit_width: f32,

    /// Padding inside panes in pixels (space between content and border/divider)
    #[serde(default = "defaults::pane_padding")]
    pub pane_padding: f32,

    /// Minimum pane size in cells (columns for horizontal splits, rows for vertical)
    /// Prevents panes from being resized too small to be useful
    #[serde(default = "defaults::pane_min_size")]
    pub pane_min_size: usize,

    /// Pane background opacity (0.0 = fully transparent, 1.0 = fully opaque)
    /// Lower values allow background image/shader to show through pane backgrounds
    #[serde(default = "defaults::pane_background_opacity")]
    pub pane_background_opacity: f32,

    /// Pane divider color [R, G, B] (0-255)
    #[serde(default = "defaults::pane_divider_color")]
    pub pane_divider_color: [u8; 3],

    /// Pane divider hover color [R, G, B] (0-255) - shown when mouse hovers over divider
    #[serde(default = "defaults::pane_divider_hover_color")]
    pub pane_divider_hover_color: [u8; 3],

    /// Enable visual dimming of inactive panes
    #[serde(default = "defaults::bool_false")]
    pub dim_inactive_panes: bool,

    /// Opacity level for inactive panes (0.0-1.0)
    #[serde(default = "defaults::inactive_pane_opacity")]
    pub inactive_pane_opacity: f32,

    /// Show title bar on each pane
    #[serde(default = "defaults::bool_false")]
    pub show_pane_titles: bool,

    /// Height of pane title bars in pixels
    #[serde(default = "defaults::pane_title_height")]
    pub pane_title_height: f32,

    /// Maximum panes per tab (0 = unlimited)
    #[serde(default = "defaults::max_panes")]
    pub max_panes: usize,

    /// Show visual indicator (border) around focused pane
    #[serde(default = "defaults::bool_true")]
    pub pane_focus_indicator: bool,

    /// Color of the focused pane indicator [R, G, B] (0-255)
    #[serde(default = "defaults::pane_focus_color")]
    pub pane_focus_color: [u8; 3],

    /// Width of the focused pane indicator border in pixels
    #[serde(default = "defaults::pane_focus_width")]
    pub pane_focus_width: f32,

    // ========================================================================
    // tmux Integration
    // ========================================================================
    /// Enable tmux control mode integration
    #[serde(default = "defaults::bool_false")]
    pub tmux_enabled: bool,

    /// Path to tmux executable (default: "tmux" - uses PATH)
    #[serde(default = "defaults::tmux_path")]
    pub tmux_path: String,

    /// Default session name when creating new tmux sessions
    #[serde(default = "defaults::tmux_default_session")]
    pub tmux_default_session: Option<String>,

    /// Auto-attach to existing tmux session on startup
    #[serde(default = "defaults::bool_false")]
    pub tmux_auto_attach: bool,

    /// Session name to auto-attach to (if tmux_auto_attach is true)
    #[serde(default = "defaults::tmux_auto_attach_session")]
    pub tmux_auto_attach_session: Option<String>,

    /// Sync clipboard with tmux paste buffer
    /// When copying in par-term, also update tmux's paste buffer via set-buffer
    #[serde(default = "defaults::bool_true")]
    pub tmux_clipboard_sync: bool,

    /// Profile to switch to when connected to tmux
    /// When profiles feature is implemented, this will automatically
    /// switch to the specified profile when entering tmux mode
    #[serde(default)]
    pub tmux_profile: Option<String>,

    /// Show tmux status bar in par-term UI
    /// When connected to tmux, display the status bar at the bottom of the terminal
    #[serde(default = "defaults::bool_false")]
    pub tmux_show_status_bar: bool,

    /// Tmux prefix key for control mode
    /// In control mode, par-term intercepts this key combination and waits for a command key.
    /// Format: "C-b" (Ctrl+B, default), "C-Space" (Ctrl+Space), "C-a" (Ctrl+A), etc.
    /// The prefix + command key is translated to the appropriate tmux command.
    #[serde(default = "defaults::tmux_prefix_key")]
    pub tmux_prefix_key: String,

    // ========================================================================
    // Focus/Blur Power Saving
    // ========================================================================
    /// Pause shader animations when window loses focus
    /// This reduces GPU usage when the terminal is not actively being viewed
    #[serde(default = "defaults::bool_true")]
    pub pause_shaders_on_blur: bool,

    /// Reduce refresh rate when window is not focused
    /// When true, uses unfocused_fps instead of max_fps when window is blurred
    #[serde(default = "defaults::bool_false")]
    pub pause_refresh_on_blur: bool,

    /// Target FPS when window is not focused (only used if pause_refresh_on_blur is true)
    /// Lower values save more power but may delay terminal output visibility
    #[serde(default = "defaults::unfocused_fps")]
    pub unfocused_fps: u32,

    // ========================================================================
    // Shader Hot Reload
    // ========================================================================
    /// Enable automatic shader reloading when shader files are modified
    /// This watches custom_shader and cursor_shader files for changes
    #[serde(default = "defaults::bool_false")]
    pub shader_hot_reload: bool,

    /// Debounce delay in milliseconds before reloading shader after file change
    /// Helps avoid multiple reloads during rapid saves from editors
    #[serde(default = "defaults::shader_hot_reload_delay")]
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
    #[serde(default = "defaults::keybindings")]
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

    // ========================================================================
    // Update Checking
    // ========================================================================
    /// How often to check for new par-term releases
    /// - never: Disable automatic update checks
    /// - daily: Check once per day
    /// - weekly: Check once per week (default)
    /// - monthly: Check once per month
    #[serde(default = "defaults::update_check_frequency")]
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
    // Search Settings
    // ========================================================================
    /// Highlight color for search matches [R, G, B, A] (0-255)
    #[serde(default = "defaults::search_highlight_color")]
    pub search_highlight_color: [u8; 4],

    /// Highlight color for the current/active search match [R, G, B, A] (0-255)
    #[serde(default = "defaults::search_current_highlight_color")]
    pub search_current_highlight_color: [u8; 4],

    /// Default case sensitivity for search
    #[serde(default = "defaults::bool_false")]
    pub search_case_sensitive: bool,

    /// Default regex mode for search
    #[serde(default = "defaults::bool_false")]
    pub search_regex: bool,

    /// Wrap around when navigating search matches
    #[serde(default = "defaults::bool_true")]
    pub search_wrap_around: bool,

    // ========================================================================
    // Session Logging
    // ========================================================================
    /// Automatically record all terminal sessions
    /// When enabled, all terminal output is logged to files in the log directory
    #[serde(default = "defaults::bool_false")]
    pub auto_log_sessions: bool,

    /// Log format for session recording
    /// - plain: Simple text output without escape sequences
    /// - html: Rendered output with colors preserved
    /// - asciicast: asciinema-compatible format for replay/sharing (default)
    #[serde(default)]
    pub session_log_format: SessionLogFormat,

    /// Directory where session logs are saved
    /// Default: ~/.local/share/par-term/logs/
    #[serde(default = "defaults::session_log_directory")]
    pub session_log_directory: String,

    /// Automatically save session log when tab/window closes
    /// When true, ensures the session is fully written before the tab closes
    #[serde(default = "defaults::bool_true")]
    pub archive_on_close: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            cols: defaults::cols(),
            rows: defaults::rows(),
            font_size: defaults::font_size(),
            font_family: defaults::font_family(),
            font_family_bold: None,
            font_family_italic: None,
            font_family_bold_italic: None,
            font_ranges: Vec::new(),
            line_spacing: defaults::line_spacing(),
            char_spacing: defaults::char_spacing(),
            enable_text_shaping: defaults::text_shaping(),
            enable_ligatures: defaults::bool_true(),
            enable_kerning: defaults::bool_true(),
            font_antialias: defaults::bool_true(),
            font_hinting: defaults::bool_true(),
            font_thin_strokes: ThinStrokesMode::default(),
            scrollback_lines: defaults::scrollback(),
            unicode_version: defaults::unicode_version(),
            ambiguous_width: defaults::ambiguous_width(),
            cursor_blink: defaults::bool_false(),
            cursor_blink_interval: defaults::cursor_blink_interval(),
            cursor_style: CursorStyle::default(),
            cursor_color: defaults::cursor_color(),
            cursor_text_color: None,
            lock_cursor_visibility: defaults::bool_false(),
            lock_cursor_style: defaults::bool_false(),
            lock_cursor_blink: defaults::bool_false(),
            cursor_guide_enabled: defaults::bool_false(),
            cursor_guide_color: defaults::cursor_guide_color(),
            cursor_shadow_enabled: defaults::bool_false(),
            cursor_shadow_color: defaults::cursor_shadow_color(),
            cursor_shadow_offset: defaults::cursor_shadow_offset(),
            cursor_shadow_blur: defaults::cursor_shadow_blur(),
            cursor_boost: defaults::cursor_boost(),
            cursor_boost_color: defaults::cursor_boost_color(),
            unfocused_cursor_style: UnfocusedCursorStyle::default(),
            scrollbar_autohide_delay: defaults::scrollbar_autohide_delay(),
            window_title: defaults::window_title(),
            allow_title_change: defaults::bool_true(),
            theme: defaults::theme(),
            left_option_key_mode: OptionKeyMode::default(),
            right_option_key_mode: OptionKeyMode::default(),
            auto_copy_selection: defaults::bool_true(),
            copy_trailing_newline: defaults::bool_false(),
            middle_click_paste: defaults::bool_true(),
            dropped_file_quote_style: DroppedFileQuoteStyle::default(),
            mouse_scroll_speed: defaults::scroll_speed(),
            mouse_double_click_threshold: defaults::double_click_threshold(),
            mouse_triple_click_threshold: defaults::triple_click_threshold(),
            option_click_moves_cursor: defaults::bool_true(),
            focus_follows_mouse: defaults::bool_false(),
            report_horizontal_scroll: defaults::bool_true(),
            word_characters: defaults::word_characters(),
            smart_selection_enabled: defaults::smart_selection_enabled(),
            smart_selection_rules: default_smart_selection_rules(),
            screenshot_format: defaults::screenshot_format(),
            max_fps: defaults::max_fps(),
            vsync_mode: VsyncMode::default(),
            window_padding: defaults::window_padding(),
            window_opacity: defaults::window_opacity(),
            window_always_on_top: defaults::bool_false(),
            window_decorations: defaults::bool_true(),
            window_type: WindowType::default(),
            target_monitor: None,
            lock_window_size: defaults::bool_false(),
            show_window_number: defaults::bool_false(),
            transparency_affects_only_default_background: defaults::bool_true(),
            keep_text_opaque: defaults::bool_true(),
            blur_enabled: defaults::bool_false(),
            blur_radius: defaults::blur_radius(),
            background_image: None,
            background_image_enabled: defaults::bool_true(),
            background_image_mode: BackgroundImageMode::default(),
            background_image_opacity: defaults::background_image_opacity(),
            background_mode: BackgroundMode::default(),
            background_color: defaults::background_color(),
            custom_shader: None,
            custom_shader_enabled: defaults::bool_true(),
            custom_shader_animation: defaults::bool_true(),
            custom_shader_animation_speed: defaults::custom_shader_speed(),
            custom_shader_text_opacity: defaults::text_opacity(),
            custom_shader_full_content: defaults::bool_false(),
            custom_shader_brightness: defaults::custom_shader_brightness(),
            custom_shader_channel0: None,
            custom_shader_channel1: None,
            custom_shader_channel2: None,
            custom_shader_channel3: None,
            custom_shader_cubemap: None,
            custom_shader_cubemap_enabled: defaults::cubemap_enabled(),
            custom_shader_use_background_as_channel0: defaults::use_background_as_channel0(),
            cursor_shader: None,
            cursor_shader_enabled: defaults::bool_false(),
            cursor_shader_animation: defaults::bool_true(),
            cursor_shader_animation_speed: defaults::custom_shader_speed(),
            cursor_shader_color: defaults::cursor_shader_color(),
            cursor_shader_trail_duration: defaults::cursor_trail_duration(),
            cursor_shader_glow_radius: defaults::cursor_glow_radius(),
            cursor_shader_glow_intensity: defaults::cursor_glow_intensity(),
            cursor_shader_hides_cursor: defaults::bool_false(),
            cursor_shader_disable_in_alt_screen: defaults::cursor_shader_disable_in_alt_screen(),
            exit_on_shell_exit: defaults::bool_true(),
            custom_shell: None,
            shell_args: None,
            working_directory: None,
            shell_env: None,
            login_shell: defaults::login_shell(),
            initial_text: defaults::initial_text(),
            initial_text_delay_ms: defaults::initial_text_delay_ms(),
            initial_text_send_newline: defaults::initial_text_send_newline(),
            answerback_string: defaults::answerback_string(),
            scrollbar_position: defaults::scrollbar_position(),
            scrollbar_width: defaults::scrollbar_width(),
            scrollbar_thumb_color: defaults::scrollbar_thumb_color(),
            scrollbar_track_color: defaults::scrollbar_track_color(),
            clipboard_max_sync_events: defaults::clipboard_max_sync_events(),
            clipboard_max_event_bytes: defaults::clipboard_max_event_bytes(),
            notification_bell_desktop: defaults::bool_false(),
            notification_bell_sound: defaults::bell_sound(),
            notification_bell_visual: defaults::bool_true(),
            notification_activity_enabled: defaults::bool_false(),
            notification_activity_threshold: defaults::activity_threshold(),
            anti_idle_enabled: defaults::bool_false(),
            anti_idle_seconds: defaults::anti_idle_seconds(),
            anti_idle_code: defaults::anti_idle_code(),
            notification_silence_enabled: defaults::bool_false(),
            notification_silence_threshold: defaults::silence_threshold(),
            notification_session_ended: defaults::bool_false(),
            suppress_notifications_when_focused: defaults::bool_true(),
            notification_max_buffer: defaults::notification_max_buffer(),
            tab_bar_mode: TabBarMode::default(),
            tab_bar_height: defaults::tab_bar_height(),
            tab_show_close_button: defaults::bool_true(),
            tab_show_index: defaults::bool_false(),
            tab_inherit_cwd: defaults::bool_true(),
            max_tabs: defaults::zero(),
            tab_bar_background: defaults::tab_bar_background(),
            tab_active_background: defaults::tab_active_background(),
            tab_inactive_background: defaults::tab_inactive_background(),
            tab_hover_background: defaults::tab_hover_background(),
            tab_active_text: defaults::tab_active_text(),
            tab_inactive_text: defaults::tab_inactive_text(),
            tab_active_indicator: defaults::tab_active_indicator(),
            tab_activity_indicator: defaults::tab_activity_indicator(),
            tab_bell_indicator: defaults::tab_bell_indicator(),
            tab_close_button: defaults::tab_close_button(),
            tab_close_button_hover: defaults::tab_close_button_hover(),
            dim_inactive_tabs: defaults::bool_true(),
            inactive_tab_opacity: defaults::inactive_tab_opacity(),
            tab_min_width: defaults::tab_min_width(),
            tab_border_color: defaults::tab_border_color(),
            tab_border_width: defaults::tab_border_width(),
            // Split panes
            pane_divider_width: defaults::pane_divider_width(),
            pane_divider_hit_width: defaults::pane_divider_hit_width(),
            pane_padding: defaults::pane_padding(),
            pane_min_size: defaults::pane_min_size(),
            pane_background_opacity: defaults::pane_background_opacity(),
            pane_divider_color: defaults::pane_divider_color(),
            pane_divider_hover_color: defaults::pane_divider_hover_color(),
            dim_inactive_panes: defaults::bool_false(),
            inactive_pane_opacity: defaults::inactive_pane_opacity(),
            show_pane_titles: defaults::bool_false(),
            pane_title_height: defaults::pane_title_height(),
            max_panes: defaults::max_panes(),
            pane_focus_indicator: defaults::bool_true(),
            pane_focus_color: defaults::pane_focus_color(),
            pane_focus_width: defaults::pane_focus_width(),
            tmux_enabled: defaults::bool_false(),
            tmux_path: defaults::tmux_path(),
            tmux_default_session: defaults::tmux_default_session(),
            tmux_auto_attach: defaults::bool_false(),
            tmux_auto_attach_session: defaults::tmux_auto_attach_session(),
            tmux_clipboard_sync: defaults::bool_true(),
            tmux_profile: None,
            tmux_show_status_bar: defaults::bool_false(),
            tmux_prefix_key: defaults::tmux_prefix_key(),
            pause_shaders_on_blur: defaults::bool_true(),
            pause_refresh_on_blur: defaults::bool_false(),
            unfocused_fps: defaults::unfocused_fps(),
            shader_hot_reload: defaults::bool_false(),
            shader_hot_reload_delay: defaults::shader_hot_reload_delay(),
            shader_configs: HashMap::new(),
            cursor_shader_configs: HashMap::new(),
            keybindings: defaults::keybindings(),
            shader_install_prompt: ShaderInstallPrompt::default(),
            update_check_frequency: defaults::update_check_frequency(),
            last_update_check: None,
            skipped_version: None,
            last_notified_version: None,
            search_highlight_color: defaults::search_highlight_color(),
            search_current_highlight_color: defaults::search_current_highlight_color(),
            search_case_sensitive: defaults::bool_false(),
            search_regex: defaults::bool_false(),
            search_wrap_around: defaults::bool_true(),
            // Session logging
            auto_log_sessions: defaults::bool_false(),
            session_log_format: SessionLogFormat::default(),
            session_log_directory: defaults::session_log_directory(),
            archive_on_close: defaults::bool_true(),
        }
    }
}

impl Config {
    /// Create a new configuration with default values
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Load configuration from file or create default
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path();
        log::info!("Config path: {:?}", config_path);

        if config_path.exists() {
            log::info!("Loading existing config from {:?}", config_path);
            let contents = fs::read_to_string(&config_path)?;
            let mut config: Config = serde_yaml::from_str(&contents)?;

            // Merge in any new default keybindings that don't exist in user's config
            config.merge_default_keybindings();

            Ok(config)
        } else {
            log::info!(
                "Config file not found, creating default at {:?}",
                config_path
            );
            // Create default config and save it
            let config = Self::default();
            if let Err(e) = config.save() {
                log::error!("Failed to save default config: {}", e);
                return Err(e);
            }
            log::info!("Default config created successfully");
            Ok(config)
        }
    }

    /// Merge default keybindings into the user's config.
    /// Only adds keybindings for actions that don't already exist in the user's config.
    /// This ensures new features with default keybindings are available to existing users.
    fn merge_default_keybindings(&mut self) {
        let default_keybindings = defaults::keybindings();

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

    /// Set window dimensions
    #[allow(dead_code)]
    pub fn with_dimensions(mut self, cols: usize, rows: usize) -> Self {
        self.cols = cols;
        self.rows = rows;
        self
    }

    /// Set font size
    #[allow(dead_code)]
    pub fn with_font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    /// Set font family
    #[allow(dead_code)]
    pub fn with_font_family(mut self, family: impl Into<String>) -> Self {
        self.font_family = family.into();
        self
    }

    /// Set the window title
    #[allow(dead_code)]
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.window_title = title.into();
        self
    }

    /// Set the scrollback buffer size
    #[allow(dead_code)]
    pub fn with_scrollback(mut self, size: usize) -> Self {
        self.scrollback_lines = size;
        self
    }

    /// Load theme configuration
    pub fn load_theme(&self) -> Theme {
        Theme::by_name(&self.theme).unwrap_or_default()
    }

    /// Get the user override config for a specific shader (if any)
    #[allow(dead_code)]
    pub fn get_shader_override(&self, shader_name: &str) -> Option<&ShaderConfig> {
        self.shader_configs.get(shader_name)
    }

    /// Get the user override config for a specific cursor shader (if any)
    #[allow(dead_code)]
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
    #[allow(dead_code)]
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
    #[allow(dead_code)]
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
}
