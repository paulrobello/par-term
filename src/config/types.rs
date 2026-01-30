//! Configuration types and enums.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ============================================================================
// Keybinding Types
// ============================================================================

/// Keyboard modifier for keybindings.
///
/// This enum is exported for potential future use (e.g., custom keybinding UI).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[allow(dead_code)]
pub enum KeyModifier {
    /// Control key
    Ctrl,
    /// Alt/Option key
    Alt,
    /// Shift key
    Shift,
    /// Cmd on macOS, Ctrl on other platforms (cross-platform convenience)
    CmdOrCtrl,
    /// Always the Cmd/Super/Windows key
    Super,
}

/// A keybinding configuration entry
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyBinding {
    /// Key combination string, e.g., "CmdOrCtrl+Shift+B"
    pub key: String,
    /// Action name, e.g., "toggle_background_shader"
    pub action: String,
}

/// VSync mode (presentation mode)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum VsyncMode {
    /// No VSync - render as fast as possible (lowest latency, highest GPU usage)
    Immediate,
    /// Mailbox VSync - cap at monitor refresh rate with triple buffering (balanced)
    Mailbox,
    /// FIFO VSync - strict vsync with double buffering (lowest GPU usage, most compatible)
    #[default]
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

/// Unfocused cursor style - how the cursor appears when window loses focus
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum UnfocusedCursorStyle {
    /// Show outline-only (hollow) block cursor when unfocused
    #[default]
    Hollow,
    /// Keep same cursor style when unfocused
    Same,
    /// Hide cursor completely when unfocused
    Hidden,
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

/// Background source selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BackgroundMode {
    /// Use theme's default background color
    #[default]
    Default,
    /// Use a custom solid color
    Color,
    /// Use a background image
    Image,
}

/// Tab bar visibility mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TabBarMode {
    /// Always show tab bar
    Always,
    /// Show tab bar only when there are multiple tabs (default)
    #[default]
    WhenMultiple,
    /// Never show tab bar
    Never,
}

/// Option/Alt key behavior mode
///
/// Controls what happens when Option (macOS) or Alt (Linux/Windows) key is pressed
/// with a character key. This is essential for emacs and vim users who rely on
/// Meta key combinations (M-x, M-f, M-b, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum OptionKeyMode {
    /// Normal - sends special characters (default macOS behavior)
    /// Option+f → ƒ (special character)
    Normal,
    /// Meta - sets the high bit (8th bit) on the character
    /// Option+f → 0xE6 (f with high bit set)
    Meta,
    /// Esc - sends Escape prefix before the character (most compatible)
    /// Option+f → ESC f (escape then f)
    /// This is the most compatible mode for terminal applications like emacs and vim
    #[default]
    Esc,
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

/// Thin strokes / font smoothing mode
///
/// Controls font stroke weight adjustment for improved rendering,
/// particularly on high-DPI/Retina displays. Inspired by iTerm2's thin strokes feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThinStrokesMode {
    /// Never apply thin strokes
    Never,
    /// Apply thin strokes only on Retina/HiDPI displays (default)
    #[default]
    RetinaOnly,
    /// Apply thin strokes only on dark backgrounds
    DarkBackgroundsOnly,
    /// Apply thin strokes only on Retina displays with dark backgrounds
    RetinaDarkBackgroundsOnly,
    /// Always apply thin strokes
    Always,
}

/// Shader install prompt mode
///
/// Controls whether the user is prompted to install shaders when the shaders
/// folder is missing or empty on startup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ShaderInstallPrompt {
    /// Ask the user if they want to install shaders (default)
    #[default]
    Ask,
    /// Never ask - user declined installation
    Never,
    /// Shaders have been installed
    Installed,
}

/// Update check frequency
///
/// Controls how often par-term checks GitHub for new releases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UpdateCheckFrequency {
    /// Never check for updates
    Never,
    /// Check once per day
    Daily,
    /// Check once per week (default)
    #[default]
    Weekly,
    /// Check once per month
    Monthly,
}

impl UpdateCheckFrequency {
    /// Get the duration in seconds for this frequency
    pub fn as_seconds(&self) -> Option<u64> {
        match self {
            UpdateCheckFrequency::Never => None,
            UpdateCheckFrequency::Daily => Some(24 * 60 * 60), // 86400
            UpdateCheckFrequency::Weekly => Some(7 * 24 * 60 * 60), // 604800
            UpdateCheckFrequency::Monthly => Some(30 * 24 * 60 * 60), // 2592000
        }
    }

    /// Display name for UI
    pub fn display_name(&self) -> &'static str {
        match self {
            UpdateCheckFrequency::Never => "Never",
            UpdateCheckFrequency::Daily => "Daily",
            UpdateCheckFrequency::Weekly => "Weekly",
            UpdateCheckFrequency::Monthly => "Monthly",
        }
    }
}

// ============================================================================
// Per-Shader Configuration Types
// ============================================================================

/// Metadata embedded in shader files via YAML block comments.
///
/// Parsed from `/*! par-term shader metadata ... */` blocks at the top of shader files.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ShaderMetadata {
    /// Human-readable name for the shader (e.g., "CRT Effect")
    pub name: Option<String>,
    /// Author of the shader
    pub author: Option<String>,
    /// Description of what the shader does
    pub description: Option<String>,
    /// Version string (e.g., "1.0.0")
    pub version: Option<String>,
    /// Default configuration values for this shader
    #[serde(default)]
    pub defaults: ShaderConfig,
}

/// Per-shader configuration settings.
///
/// Used both for embedded defaults in shader files and for user overrides in config.yaml.
/// All fields are optional to allow partial overrides.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ShaderConfig {
    /// Animation speed multiplier (1.0 = normal speed)
    pub animation_speed: Option<f32>,
    /// Brightness multiplier (0.05-1.0)
    pub brightness: Option<f32>,
    /// Text opacity when using this shader (0.0-1.0)
    pub text_opacity: Option<f32>,
    /// When true, shader receives full terminal content for manipulation
    pub full_content: Option<bool>,
    /// Path to texture for iChannel0
    pub channel0: Option<String>,
    /// Path to texture for iChannel1
    pub channel1: Option<String>,
    /// Path to texture for iChannel2
    pub channel2: Option<String>,
    /// Path to texture for iChannel3
    pub channel3: Option<String>,
    /// Path prefix for cubemap faces
    pub cubemap: Option<String>,
    /// Whether cubemap sampling is enabled
    pub cubemap_enabled: Option<bool>,
    /// Use the app's background image as iChannel0 instead of a separate texture
    pub use_background_as_channel0: Option<bool>,
}

/// Cursor shader specific configuration.
///
/// Extends base ShaderConfig with cursor-specific settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CursorShaderConfig {
    /// Base shader configuration
    #[serde(flatten)]
    pub base: ShaderConfig,
    /// Hide the default cursor when this shader is enabled
    pub hides_cursor: Option<bool>,
    /// Disable cursor shader while in alt screen (vim, less, htop)
    pub disable_in_alt_screen: Option<bool>,
    /// Cursor glow radius in pixels
    pub glow_radius: Option<f32>,
    /// Cursor glow intensity (0.0-1.0)
    pub glow_intensity: Option<f32>,
    /// Duration of cursor trail effect in seconds
    pub trail_duration: Option<f32>,
    /// Cursor color for shader effects [R, G, B] (0-255)
    pub cursor_color: Option<[u8; 3]>,
}

/// Metadata embedded in cursor shader files via YAML block comments.
///
/// Parsed from `/*! par-term shader metadata ... */` blocks at the top of cursor shader files.
/// Similar to `ShaderMetadata` but with cursor-specific defaults.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CursorShaderMetadata {
    /// Human-readable name for the shader (e.g., "Cursor Glow Effect")
    pub name: Option<String>,
    /// Author of the shader
    pub author: Option<String>,
    /// Description of what the shader does
    pub description: Option<String>,
    /// Version string (e.g., "1.0.0")
    pub version: Option<String>,
    /// Default configuration values for this cursor shader
    #[serde(default)]
    pub defaults: CursorShaderConfig,
}

/// Fully resolved shader configuration with all values filled in.
///
/// Created by merging user overrides, shader metadata defaults, and global defaults.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ResolvedShaderConfig {
    /// Animation speed multiplier
    pub animation_speed: f32,
    /// Brightness multiplier
    pub brightness: f32,
    /// Text opacity
    pub text_opacity: f32,
    /// Full content mode enabled
    pub full_content: bool,
    /// Resolved path to iChannel0 texture
    pub channel0: Option<PathBuf>,
    /// Resolved path to iChannel1 texture
    pub channel1: Option<PathBuf>,
    /// Resolved path to iChannel2 texture
    pub channel2: Option<PathBuf>,
    /// Resolved path to iChannel3 texture
    pub channel3: Option<PathBuf>,
    /// Resolved cubemap path prefix
    pub cubemap: Option<PathBuf>,
    /// Cubemap sampling enabled
    pub cubemap_enabled: bool,
    /// Use the app's background image as iChannel0
    pub use_background_as_channel0: bool,
}

impl Default for ResolvedShaderConfig {
    fn default() -> Self {
        Self {
            animation_speed: 1.0,
            brightness: 1.0,
            text_opacity: 1.0,
            full_content: false,
            channel0: None,
            channel1: None,
            channel2: None,
            channel3: None,
            cubemap: None,
            cubemap_enabled: true,
            use_background_as_channel0: false,
        }
    }
}

/// Fully resolved cursor shader configuration with all values filled in.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ResolvedCursorShaderConfig {
    /// Base resolved shader config
    pub base: ResolvedShaderConfig,
    /// Hide the default cursor when this shader is enabled
    pub hides_cursor: bool,
    /// Disable cursor shader while in alt screen (vim, less, htop)
    pub disable_in_alt_screen: bool,
    /// Cursor glow radius in pixels
    pub glow_radius: f32,
    /// Cursor glow intensity (0.0-1.0)
    pub glow_intensity: f32,
    /// Duration of cursor trail effect in seconds
    pub trail_duration: f32,
    /// Cursor color for shader effects [R, G, B] (0-255)
    pub cursor_color: [u8; 3],
}

impl Default for ResolvedCursorShaderConfig {
    fn default() -> Self {
        Self {
            base: ResolvedShaderConfig::default(),
            hides_cursor: false,
            disable_in_alt_screen: true,
            glow_radius: 80.0,
            glow_intensity: 0.3,
            trail_duration: 0.5,
            cursor_color: [255, 255, 255],
        }
    }
}
