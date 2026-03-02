//! Global shader configuration sub-struct extracted from `Config`.
//!
//! Contains all `custom_shader_*` and `cursor_shader_*` top-level config fields.

use serde::{Deserialize, Serialize};

/// Global shader settings for both background (custom) and cursor shaders.
///
/// Extracted from `Config` via `#[serde(flatten)]` for YAML backward-compatibility.
/// Fields serialise at the top level, so existing `config.yaml` files need no changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GlobalShaderConfig {
    // ========================================================================
    // Background (Custom) Shader Settings
    // ========================================================================
    /// Custom shader file path (GLSL format, relative to shaders folder or absolute)
    /// Shaders are loaded from ~/.config/par-term/shaders/ by default
    /// Supports Ghostty/Shadertoy-style GLSL shaders with iTime, iResolution, iChannel0-4
    pub custom_shader: Option<String>,

    /// Enable or disable the custom shader (even if a path is set)
    pub custom_shader_enabled: bool,

    /// Enable animation in custom shader (updates iTime uniform each frame)
    /// When disabled, iTime is fixed at 0.0 for static effects
    pub custom_shader_animation: bool,

    /// Animation speed multiplier for custom shader (1.0 = normal speed)
    pub custom_shader_animation_speed: f32,

    /// Text opacity when using custom shader (0.0 = transparent, 1.0 = fully opaque)
    /// This allows text to remain readable while the shader effect shows through the background
    pub custom_shader_text_opacity: f32,

    /// When enabled, the shader receives the full rendered terminal content (text + background)
    /// and can manipulate/distort it. When disabled (default), the shader only provides
    /// a background and text is composited on top cleanly.
    pub custom_shader_full_content: bool,

    /// Brightness multiplier for custom shader output (0.05 = very dark, 1.0 = full brightness)
    /// This dims the shader background to improve text readability
    pub custom_shader_brightness: f32,

    /// Texture file path for custom shader iChannel0 (optional, Shadertoy compatible)
    /// Supports ~ for home directory. Example: "~/textures/noise.png"
    pub custom_shader_channel0: Option<String>,

    /// Texture file path for custom shader iChannel1 (optional)
    pub custom_shader_channel1: Option<String>,

    /// Texture file path for custom shader iChannel2 (optional)
    pub custom_shader_channel2: Option<String>,

    /// Texture file path for custom shader iChannel3 (optional)
    pub custom_shader_channel3: Option<String>,

    /// Cubemap texture path prefix for custom shaders (optional)
    /// Expects 6 face files: {prefix}-px.{ext}, -nx.{ext}, -py.{ext}, -ny.{ext}, -pz.{ext}, -nz.{ext}
    /// Supported formats: .png, .jpg, .jpeg, .hdr
    /// Example: "textures/cubemaps/env-outside" will load env-outside-px.png, etc.
    pub custom_shader_cubemap: Option<String>,

    /// Enable cubemap sampling in custom shaders
    /// When enabled and a cubemap path is set, iCubemap uniform is available in shaders
    pub custom_shader_cubemap_enabled: bool,

    /// Use the app's background image as iChannel0 for custom shaders
    /// When enabled, the configured background image is bound as iChannel0 instead of
    /// the custom_shader_channel0 texture. This allows shaders to incorporate the
    /// background image without requiring a separate texture file.
    pub custom_shader_use_background_as_channel0: bool,

    // ========================================================================
    // Cursor Shader Settings (separate from background shader)
    // ========================================================================
    /// Cursor shader file path (GLSL format, relative to shaders folder or absolute)
    /// This is a separate shader specifically for cursor effects (trails, glows, etc.)
    pub cursor_shader: Option<String>,

    /// Enable or disable the cursor shader (even if a path is set)
    pub cursor_shader_enabled: bool,

    /// Enable animation in cursor shader (updates iTime uniform each frame)
    pub cursor_shader_animation: bool,

    /// Animation speed multiplier for cursor shader (1.0 = normal speed)
    pub cursor_shader_animation_speed: f32,

    /// Cursor color for shader effects [R, G, B] (0-255)
    /// This color is passed to the shader via iCursorShaderColor uniform
    pub cursor_shader_color: [u8; 3],

    /// Duration of cursor trail effect in seconds
    /// Passed to shader via iCursorTrailDuration uniform
    pub cursor_shader_trail_duration: f32,

    /// Radius of cursor glow effect in pixels
    /// Passed to shader via iCursorGlowRadius uniform
    pub cursor_shader_glow_radius: f32,

    /// Intensity of cursor glow effect (0.0 = none, 1.0 = full)
    /// Passed to shader via iCursorGlowIntensity uniform
    pub cursor_shader_glow_intensity: f32,

    /// Hide the default cursor when cursor shader is enabled
    /// When true and cursor_shader_enabled is true, the normal cursor is not drawn
    /// This allows cursor shaders to fully replace the cursor rendering
    pub cursor_shader_hides_cursor: bool,

    /// Disable cursor shader while in alt screen (vim, less, htop)
    /// Keeps current behavior by default for TUI compatibility
    pub cursor_shader_disable_in_alt_screen: bool,
}

impl Default for GlobalShaderConfig {
    fn default() -> Self {
        Self {
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
        }
    }
}
