//! Terminal configuration management.
//!
//! This module provides configuration loading, saving, and default values
//! for the terminal emulator.

mod defaults;
mod types;

pub use types::{BackgroundImageMode, CursorStyle, FontRange, TabBarMode, VsyncMode};

use anyhow::Result;
use serde::{Deserialize, Serialize};
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

    /// Initial window width in pixels
    #[serde(default = "defaults::window_width")]
    pub window_width: u32,

    /// Initial window height in pixels
    #[serde(default = "defaults::window_height")]
    pub window_height: u32,

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

    /// Custom shader file path (GLSL format, relative to shaders folder or absolute)
    /// Shaders are loaded from ~/.config/par-term/shaders/ by default
    /// Supports Ghostty/Shadertoy-style GLSL shaders with iTime, iResolution, iChannel0
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

    /// Texture file path for custom shader iChannel1 (optional)
    /// Supports ~ for home directory. Example: "~/textures/noise.png"
    #[serde(default)]
    pub custom_shader_channel1: Option<String>,

    /// Texture file path for custom shader iChannel2 (optional)
    #[serde(default)]
    pub custom_shader_channel2: Option<String>,

    /// Texture file path for custom shader iChannel3 (optional)
    #[serde(default)]
    pub custom_shader_channel3: Option<String>,

    /// Texture file path for custom shader iChannel4 (optional)
    #[serde(default)]
    pub custom_shader_channel4: Option<String>,

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

    // ========================================================================
    // Scrollback & Cursor
    // ========================================================================
    /// Maximum number of lines to keep in scrollback buffer
    #[serde(default = "defaults::scrollback", alias = "scrollback_size")]
    pub scrollback_lines: usize,

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

    /// Enable notifications after prolonged silence
    #[serde(default = "defaults::bool_false", alias = "silence_notifications")]
    pub notification_silence_enabled: bool,

    /// Seconds of silence before a silence alert fires
    #[serde(default = "defaults::silence_threshold", alias = "silence_threshold")]
    pub notification_silence_threshold: u64,

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
            scrollback_lines: defaults::scrollback(),
            cursor_blink: defaults::bool_false(),
            cursor_blink_interval: defaults::cursor_blink_interval(),
            cursor_style: CursorStyle::default(),
            cursor_color: defaults::cursor_color(),
            scrollbar_autohide_delay: defaults::scrollbar_autohide_delay(),
            window_title: defaults::window_title(),
            allow_title_change: defaults::bool_true(),
            theme: defaults::theme(),
            auto_copy_selection: defaults::bool_true(),
            copy_trailing_newline: defaults::bool_false(),
            middle_click_paste: defaults::bool_true(),
            mouse_scroll_speed: defaults::scroll_speed(),
            mouse_double_click_threshold: defaults::double_click_threshold(),
            mouse_triple_click_threshold: defaults::triple_click_threshold(),
            screenshot_format: defaults::screenshot_format(),
            max_fps: defaults::max_fps(),
            vsync_mode: VsyncMode::default(),
            window_padding: defaults::window_padding(),
            window_opacity: defaults::window_opacity(),
            window_always_on_top: defaults::bool_false(),
            window_decorations: defaults::bool_true(),
            window_width: defaults::window_width(),
            window_height: defaults::window_height(),
            background_image: None,
            background_image_enabled: defaults::bool_true(),
            background_image_mode: BackgroundImageMode::default(),
            background_image_opacity: defaults::background_image_opacity(),
            custom_shader: None,
            custom_shader_enabled: defaults::bool_true(),
            custom_shader_animation: defaults::bool_true(),
            custom_shader_animation_speed: defaults::custom_shader_speed(),
            custom_shader_text_opacity: defaults::text_opacity(),
            custom_shader_full_content: defaults::bool_false(),
            custom_shader_brightness: defaults::custom_shader_brightness(),
            custom_shader_channel1: None,
            custom_shader_channel2: None,
            custom_shader_channel3: None,
            custom_shader_channel4: None,
            cursor_shader: None,
            cursor_shader_enabled: defaults::bool_false(),
            cursor_shader_animation: defaults::bool_true(),
            cursor_shader_animation_speed: defaults::custom_shader_speed(),
            cursor_shader_color: defaults::cursor_shader_color(),
            cursor_shader_trail_duration: defaults::cursor_trail_duration(),
            cursor_shader_glow_radius: defaults::cursor_glow_radius(),
            cursor_shader_glow_intensity: defaults::cursor_glow_intensity(),
            cursor_shader_hides_cursor: defaults::bool_false(),
            exit_on_shell_exit: defaults::bool_true(),
            custom_shell: None,
            shell_args: None,
            working_directory: None,
            shell_env: None,
            login_shell: defaults::login_shell(),
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
            notification_silence_enabled: defaults::bool_false(),
            notification_silence_threshold: defaults::silence_threshold(),
            notification_max_buffer: defaults::notification_max_buffer(),
            tab_bar_mode: TabBarMode::default(),
            tab_bar_height: defaults::tab_bar_height(),
            tab_show_close_button: defaults::bool_true(),
            tab_show_index: defaults::bool_false(),
            tab_inherit_cwd: defaults::bool_true(),
            max_tabs: defaults::zero(),
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
            let config: Config = serde_yaml::from_str(&contents)?;
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
    /// Returns the expanded path or the original if expansion fails
    pub fn resolve_texture_path(path: &str) -> PathBuf {
        if path.starts_with("~/")
            && let Some(home) = dirs::home_dir()
        {
            return home.join(&path[2..]);
        }
        PathBuf::from(path)
    }

    /// Get the channel texture paths as an array of Options
    /// Returns [channel1, channel2, channel3, channel4]
    pub fn shader_channel_paths(&self) -> [Option<PathBuf>; 4] {
        [
            self.custom_shader_channel1
                .as_ref()
                .map(|p| Self::resolve_texture_path(p)),
            self.custom_shader_channel2
                .as_ref()
                .map(|p| Self::resolve_texture_path(p)),
            self.custom_shader_channel3
                .as_ref()
                .map(|p| Self::resolve_texture_path(p)),
            self.custom_shader_channel4
                .as_ref()
                .map(|p| Self::resolve_texture_path(p)),
        ]
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
}
