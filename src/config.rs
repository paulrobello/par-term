use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// VSync mode (presentation mode)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum VsyncMode {
    /// No VSync - render as fast as possible (lowest latency, highest GPU usage)
    #[default]
    Immediate,
    /// Mailbox VSync - cap at monitor refresh rate with triple buffering (balanced)
    Mailbox,
    /// FIFO VSync - strict vsync with double buffering (lowest GPU usage, slight input lag)
    Fifo,
}

impl VsyncMode {
    /// Convert to wgpu::PresentMode
    pub fn to_present_mode(self) -> wgpu::PresentMode {
        match self {
            VsyncMode::Immediate => wgpu::PresentMode::Immediate,
            VsyncMode::Mailbox => wgpu::PresentMode::Mailbox,
            VsyncMode::Fifo => wgpu::PresentMode::Fifo,
        }
    }
}

use crate::themes::Theme;

/// Cursor style
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CursorStyle {
    /// Block cursor (fills entire cell)
    #[default]
    Block,
    /// Beam cursor (vertical line at cell start)
    Beam,
    /// Underline cursor (horizontal line at cell bottom)
    Underline,
}

/// Background image display mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BackgroundImageMode {
    /// Scale to fit window while maintaining aspect ratio (may have letterboxing)
    Fit,
    /// Scale to fill window while maintaining aspect ratio (may crop edges)
    Fill,
    /// Stretch to fill window exactly (ignores aspect ratio)
    #[default]
    Stretch,
    /// Repeat image in a tiled pattern at original size
    Tile,
    /// Center image at original size (no scaling)
    Center,
}

/// Font mapping for a specific Unicode range
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontRange {
    /// Start of Unicode range (inclusive), e.g., 0x4E00 for CJK
    pub start: u32,
    /// End of Unicode range (inclusive), e.g., 0x9FFF for CJK
    pub end: u32,
    /// Font family name to use for this range
    pub font_family: String,
}

/// Configuration for the terminal emulator
/// Aligned with par-tui-term naming conventions for consistency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    // ========================================================================
    // Window & Display (GUI-specific)
    // ========================================================================
    /// Number of columns in the terminal
    #[serde(default = "default_cols")]
    pub cols: usize,

    /// Number of rows in the terminal
    #[serde(default = "default_rows")]
    pub rows: usize,

    /// Font size in points
    #[serde(default = "default_font_size")]
    pub font_size: f32,

    /// Font family name (regular/normal weight)
    #[serde(default = "default_font_family")]
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
    #[serde(default = "default_line_spacing")]
    pub line_spacing: f32,

    /// Character width multiplier (0.5 = narrow, 0.6 = default, 0.7 = wide)
    #[serde(default = "default_char_spacing")]
    pub char_spacing: f32,

    /// Enable text shaping for ligatures and complex scripts
    /// When enabled, uses HarfBuzz for proper ligature, emoji, and complex script rendering
    #[serde(default = "default_text_shaping")]
    pub enable_text_shaping: bool,

    /// Enable ligatures (requires enable_text_shaping)
    #[serde(default = "default_true")]
    pub enable_ligatures: bool,

    /// Enable kerning adjustments (requires enable_text_shaping)
    #[serde(default = "default_true")]
    pub enable_kerning: bool,

    /// Window title
    #[serde(default = "default_window_title")]
    pub window_title: String,

    /// Allow applications to change the window title via OSC escape sequences
    /// When false, the window title will always be the configured window_title
    #[serde(default = "default_true")]
    pub allow_title_change: bool,

    /// Maximum frames per second (FPS) target
    /// Controls how frequently the terminal requests screen redraws.
    /// Note: On macOS, actual FPS may be lower (~22-25) due to system-level
    /// VSync throttling in wgpu/Metal, regardless of this setting.
    /// Default: 60
    #[serde(default = "default_max_fps", alias = "refresh_rate")]
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
    #[serde(default = "default_window_padding")]
    pub window_padding: f32,

    /// Window opacity/transparency (0.0 = fully transparent, 1.0 = fully opaque)
    #[serde(default = "default_window_opacity")]
    pub window_opacity: f32,

    /// Keep window always on top of other windows
    #[serde(default = "default_false")]
    pub window_always_on_top: bool,

    /// Show window decorations (title bar, borders)
    #[serde(default = "default_true")]
    pub window_decorations: bool,

    /// Initial window width in pixels
    #[serde(default = "default_window_width")]
    pub window_width: u32,

    /// Initial window height in pixels
    #[serde(default = "default_window_height")]
    pub window_height: u32,

    /// Background image path (optional, supports ~ for home directory)
    #[serde(default)]
    pub background_image: Option<String>,

    /// Enable or disable background image rendering (even if a path is set)
    #[serde(default = "default_true")]
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
    #[serde(default = "default_background_image_opacity")]
    pub background_image_opacity: f32,

    /// Custom shader file path (GLSL format, relative to shaders folder or absolute)
    /// Shaders are loaded from ~/.config/par-term/shaders/ by default
    /// Supports Ghostty/Shadertoy-style GLSL shaders with iTime, iResolution, iChannel0
    #[serde(default)]
    pub custom_shader: Option<String>,

    /// Enable or disable the custom shader (even if a path is set)
    #[serde(default = "default_true")]
    pub custom_shader_enabled: bool,

    /// Enable animation in custom shader (updates iTime uniform each frame)
    /// When disabled, iTime is fixed at 0.0 for static effects
    #[serde(default = "default_true")]
    pub custom_shader_animation: bool,

    /// Animation speed multiplier for custom shader (1.0 = normal speed)
    #[serde(default = "default_custom_shader_speed")]
    pub custom_shader_animation_speed: f32,

    /// Text opacity when using custom shader (0.0 = transparent, 1.0 = fully opaque)
    /// This allows text to remain readable while the shader effect shows through the background
    #[serde(default = "default_text_opacity")]
    pub custom_shader_text_opacity: f32,

    /// When enabled, the shader receives the full rendered terminal content (text + background)
    /// and can manipulate/distort it. When disabled (default), the shader only provides
    /// a background and text is composited on top cleanly.
    #[serde(default = "default_false")]
    pub custom_shader_full_content: bool,

    // ========================================================================
    // Cursor Shader Settings (separate from background shader)
    // ========================================================================
    /// Cursor shader file path (GLSL format, relative to shaders folder or absolute)
    /// This is a separate shader specifically for cursor effects (trails, glows, etc.)
    #[serde(default)]
    pub cursor_shader: Option<String>,

    /// Enable or disable the cursor shader (even if a path is set)
    #[serde(default = "default_false")]
    pub cursor_shader_enabled: bool,

    /// Enable animation in cursor shader (updates iTime uniform each frame)
    #[serde(default = "default_true")]
    pub cursor_shader_animation: bool,

    /// Animation speed multiplier for cursor shader (1.0 = normal speed)
    #[serde(default = "default_custom_shader_speed")]
    pub cursor_shader_animation_speed: f32,

    /// Cursor color for shader effects [R, G, B] (0-255)
    /// This color is passed to the shader via iCursorShaderColor uniform
    #[serde(default = "default_cursor_shader_color")]
    pub cursor_shader_color: [u8; 3],

    /// Duration of cursor trail effect in seconds
    /// Passed to shader via iCursorTrailDuration uniform
    #[serde(default = "default_cursor_trail_duration")]
    pub cursor_shader_trail_duration: f32,

    /// Radius of cursor glow effect in pixels
    /// Passed to shader via iCursorGlowRadius uniform
    #[serde(default = "default_cursor_glow_radius")]
    pub cursor_shader_glow_radius: f32,

    /// Intensity of cursor glow effect (0.0 = none, 1.0 = full)
    /// Passed to shader via iCursorGlowIntensity uniform
    #[serde(default = "default_cursor_glow_intensity")]
    pub cursor_shader_glow_intensity: f32,

    // ========================================================================
    // Selection & Clipboard
    // ========================================================================
    /// Automatically copy selected text to clipboard
    #[serde(default = "default_true")]
    pub auto_copy_selection: bool,

    /// Include trailing newline when copying lines
    /// Note: Inverted logic from old strip_trailing_newline_on_copy
    #[serde(default = "default_false", alias = "strip_trailing_newline_on_copy")]
    pub copy_trailing_newline: bool,

    /// Paste on middle mouse button click
    #[serde(default = "default_true")]
    pub middle_click_paste: bool,

    // ========================================================================
    // Mouse Behavior
    // ========================================================================
    /// Mouse wheel scroll speed multiplier
    #[serde(default = "default_scroll_speed")]
    pub mouse_scroll_speed: f32,

    /// Double-click timing threshold in milliseconds
    #[serde(default = "default_double_click_threshold")]
    pub mouse_double_click_threshold: u64,

    /// Triple-click timing threshold in milliseconds (typically same as double-click)
    #[serde(default = "default_triple_click_threshold")]
    pub mouse_triple_click_threshold: u64,

    // ========================================================================
    // Scrollback & Cursor
    // ========================================================================
    /// Maximum number of lines to keep in scrollback buffer
    #[serde(default = "default_scrollback", alias = "scrollback_size")]
    pub scrollback_lines: usize,

    /// Enable cursor blinking
    #[serde(default = "default_false")]
    pub cursor_blink: bool,

    /// Cursor blink interval in milliseconds
    #[serde(default = "default_cursor_blink_interval")]
    pub cursor_blink_interval: u64,

    /// Cursor style (block, beam, underline)
    #[serde(default)]
    pub cursor_style: CursorStyle,

    /// Cursor color [R, G, B] (0-255)
    #[serde(default = "default_cursor_color")]
    pub cursor_color: [u8; 3],

    // ========================================================================
    // Scrollbar
    // ========================================================================
    /// Auto-hide scrollbar after inactivity (milliseconds, 0 = never hide)
    #[serde(default = "default_scrollbar_autohide_delay")]
    pub scrollbar_autohide_delay: u64,

    // ========================================================================
    // Theme & Colors
    // ========================================================================
    /// Color theme name to use for terminal colors
    #[serde(default = "default_theme")]
    pub theme: String,

    // ========================================================================
    // Screenshot
    // ========================================================================
    /// File format for screenshots (png, jpeg, svg, html)
    #[serde(default = "default_screenshot_format")]
    pub screenshot_format: String,

    // ========================================================================
    // Shell Behavior
    // ========================================================================
    /// Exit when shell exits
    #[serde(default = "default_true", alias = "close_on_shell_exit")]
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
    #[serde(default = "default_login_shell")]
    pub login_shell: bool,

    // ========================================================================
    // Scrollbar (GUI-specific)
    // ========================================================================
    /// Scrollbar position (left or right)
    #[serde(default = "default_scrollbar_position")]
    pub scrollbar_position: String,

    /// Scrollbar width in pixels
    #[serde(default = "default_scrollbar_width")]
    pub scrollbar_width: f32,

    /// Scrollbar thumb color (RGBA: [r, g, b, a] where each is 0.0-1.0)
    #[serde(default = "default_scrollbar_thumb_color")]
    pub scrollbar_thumb_color: [f32; 4],

    /// Scrollbar track color (RGBA: [r, g, b, a] where each is 0.0-1.0)
    #[serde(default = "default_scrollbar_track_color")]
    pub scrollbar_track_color: [f32; 4],

    // ========================================================================
    // Clipboard Sync Limits
    // ========================================================================
    /// Maximum clipboard sync events retained for diagnostics
    #[serde(
        default = "default_clipboard_max_sync_events",
        alias = "max_clipboard_sync_events"
    )]
    pub clipboard_max_sync_events: usize,

    /// Maximum bytes stored per clipboard sync event
    #[serde(
        default = "default_clipboard_max_event_bytes",
        alias = "max_clipboard_event_bytes"
    )]
    pub clipboard_max_event_bytes: usize,

    // ========================================================================
    // Notifications
    // ========================================================================
    /// Forward BEL events to desktop notification centers
    #[serde(default = "default_false", alias = "bell_desktop")]
    pub notification_bell_desktop: bool,

    /// Volume (0-100) for backend bell sound alerts (0 disables)
    #[serde(default = "default_bell_sound", alias = "bell_sound")]
    pub notification_bell_sound: u8,

    /// Enable backend visual bell overlay
    #[serde(default = "default_true", alias = "bell_visual")]
    pub notification_bell_visual: bool,

    /// Enable notifications when activity resumes after inactivity
    #[serde(default = "default_false", alias = "activity_notifications")]
    pub notification_activity_enabled: bool,

    /// Seconds of inactivity required before an activity alert fires
    #[serde(default = "default_activity_threshold", alias = "activity_threshold")]
    pub notification_activity_threshold: u64,

    /// Enable notifications after prolonged silence
    #[serde(default = "default_false", alias = "silence_notifications")]
    pub notification_silence_enabled: bool,

    /// Seconds of silence before a silence alert fires
    #[serde(default = "default_silence_threshold", alias = "silence_threshold")]
    pub notification_silence_threshold: u64,

    /// Maximum number of OSC 9/777 notification entries retained by backend
    #[serde(
        default = "default_notification_max_buffer",
        alias = "max_notifications"
    )]
    pub notification_max_buffer: usize,
}

// Default value functions
fn default_cols() -> usize {
    80
}

fn default_rows() -> usize {
    24
}

fn default_font_size() -> f32 {
    13.0
}

fn default_font_family() -> String {
    "JetBrains Mono".to_string()
}

fn default_line_spacing() -> f32 {
    1.0 // Default line height multiplier
}

fn default_char_spacing() -> f32 {
    1.0 // Default character width multiplier
}

fn default_text_shaping() -> bool {
    true // Enabled by default - OpenType features now properly configured via Feature::from_str()
}

fn default_scrollback() -> usize {
    10000
}

fn default_window_title() -> String {
    "par-term".to_string()
}

fn default_theme() -> String {
    "dark-background".to_string()
}

fn default_screenshot_format() -> String {
    "png".to_string()
}

fn default_max_fps() -> u32 {
    60
}

fn default_window_padding() -> f32 {
    10.0
}

fn default_login_shell() -> bool {
    true
}

fn default_scrollbar_position() -> String {
    "right".to_string()
}

fn default_scrollbar_width() -> f32 {
    15.0
}

fn default_scrollbar_thumb_color() -> [f32; 4] {
    [0.4, 0.4, 0.4, 0.95] // Medium gray, nearly opaque
}

fn default_scrollbar_track_color() -> [f32; 4] {
    [0.15, 0.15, 0.15, 0.6] // Dark gray, semi-transparent
}

fn default_clipboard_max_sync_events() -> usize {
    64 // Aligned with sister project
}

fn default_clipboard_max_event_bytes() -> usize {
    2048 // Aligned with sister project
}

fn default_activity_threshold() -> u64 {
    10 // Aligned with sister project (10 seconds)
}

fn default_silence_threshold() -> u64 {
    300 // 5 minutes
}

fn default_notification_max_buffer() -> usize {
    64 // Aligned with sister project
}

fn default_scroll_speed() -> f32 {
    3.0 // Lines per scroll tick
}

fn default_double_click_threshold() -> u64 {
    500 // 500 milliseconds
}

fn default_triple_click_threshold() -> u64 {
    500 // 500 milliseconds (same as double-click)
}

fn default_cursor_blink_interval() -> u64 {
    500 // 500 milliseconds (blink twice per second)
}

fn default_cursor_color() -> [u8; 3] {
    [255, 255, 255] // White cursor
}

fn default_scrollbar_autohide_delay() -> u64 {
    0 // 0 = never auto-hide (always visible when scrollback exists)
}

fn default_window_opacity() -> f32 {
    1.0 // Fully opaque by default
}

fn default_window_width() -> u32 {
    1600 // Default initial width
}

fn default_window_height() -> u32 {
    600 // Default initial height
}

fn default_background_image_opacity() -> f32 {
    1.0 // Fully opaque by default
}

fn default_false() -> bool {
    false
}

fn default_true() -> bool {
    true
}

fn default_text_opacity() -> f32 {
    1.0 // Fully opaque text by default
}

fn default_custom_shader_speed() -> f32 {
    1.0 // Normal animation speed
}

fn default_cursor_shader_color() -> [u8; 3] {
    [255, 255, 255] // White cursor for shader effects
}

fn default_cursor_trail_duration() -> f32 {
    0.5 // 500ms trail duration
}

fn default_cursor_glow_radius() -> f32 {
    80.0 // 80 pixel glow radius
}

fn default_cursor_glow_intensity() -> f32 {
    0.3 // 30% glow intensity
}

fn default_bell_sound() -> u8 {
    50 // Default to 50% volume
}

impl Default for Config {
    fn default() -> Self {
        Self {
            cols: default_cols(),
            rows: default_rows(),
            font_size: default_font_size(),
            font_family: default_font_family(),
            font_family_bold: None,
            font_family_italic: None,
            font_family_bold_italic: None,
            font_ranges: Vec::new(),
            line_spacing: default_line_spacing(),
            char_spacing: default_char_spacing(),
            enable_text_shaping: default_text_shaping(),
            enable_ligatures: default_true(),
            enable_kerning: default_true(),
            scrollback_lines: default_scrollback(),
            cursor_blink: default_false(),
            cursor_blink_interval: default_cursor_blink_interval(),
            cursor_style: CursorStyle::default(),
            cursor_color: default_cursor_color(),
            scrollbar_autohide_delay: default_scrollbar_autohide_delay(),
            window_title: default_window_title(),
            allow_title_change: default_true(),
            theme: default_theme(),
            auto_copy_selection: default_true(),
            copy_trailing_newline: default_false(),
            middle_click_paste: default_true(),
            mouse_scroll_speed: default_scroll_speed(),
            mouse_double_click_threshold: default_double_click_threshold(),
            mouse_triple_click_threshold: default_triple_click_threshold(),
            screenshot_format: default_screenshot_format(),
            max_fps: default_max_fps(),
            vsync_mode: VsyncMode::default(),
            window_padding: default_window_padding(),
            window_opacity: default_window_opacity(),
            window_always_on_top: default_false(),
            window_decorations: default_true(),
            window_width: default_window_width(),
            window_height: default_window_height(),
            background_image: None,
            background_image_enabled: default_true(),
            background_image_mode: BackgroundImageMode::default(),
            background_image_opacity: default_background_image_opacity(),
            custom_shader: None,
            custom_shader_enabled: default_true(),
            custom_shader_animation: default_true(),
            custom_shader_animation_speed: default_custom_shader_speed(),
            custom_shader_text_opacity: default_text_opacity(),
            custom_shader_full_content: default_false(),
            cursor_shader: None,
            cursor_shader_enabled: default_false(),
            cursor_shader_animation: default_true(),
            cursor_shader_animation_speed: default_custom_shader_speed(),
            cursor_shader_color: default_cursor_shader_color(),
            cursor_shader_trail_duration: default_cursor_trail_duration(),
            cursor_shader_glow_radius: default_cursor_glow_radius(),
            cursor_shader_glow_intensity: default_cursor_glow_intensity(),
            exit_on_shell_exit: default_true(),
            custom_shell: None,
            shell_args: None,
            working_directory: None,
            shell_env: None,
            login_shell: default_login_shell(),
            scrollbar_position: default_scrollbar_position(),
            scrollbar_width: default_scrollbar_width(),
            scrollbar_thumb_color: default_scrollbar_thumb_color(),
            scrollbar_track_color: default_scrollbar_track_color(),
            clipboard_max_sync_events: default_clipboard_max_sync_events(),
            clipboard_max_event_bytes: default_clipboard_max_event_bytes(),
            notification_bell_desktop: default_false(),
            notification_bell_sound: default_bell_sound(),
            notification_bell_visual: default_true(),
            notification_activity_enabled: default_false(),
            notification_activity_threshold: default_activity_threshold(),
            notification_silence_enabled: default_false(),
            notification_silence_threshold: default_silence_threshold(),
            notification_max_buffer: default_notification_max_buffer(),
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
