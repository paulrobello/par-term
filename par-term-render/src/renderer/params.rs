//! Parameter struct for `Renderer::new()` to replace 46 individual arguments.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use winit::window::Window;

/// Bundles all parameters needed by [`super::Renderer::new()`].
///
/// Grouping the parameters into a struct eliminates the 46-argument function
/// signature (audit finding H4) without changing any behaviour.
pub struct RendererParams<'a> {
    // ── Window / GPU ──────────────────────────────────────────────────
    /// The winit window that owns the wgpu surface.
    pub window: Arc<Window>,
    /// Vsync presentation mode.
    pub vsync_mode: par_term_config::VsyncMode,
    /// GPU power preference (low-power vs high-performance).
    pub power_preference: par_term_config::PowerPreference,
    /// Window opacity (0.0 fully transparent – 1.0 fully opaque).
    pub window_opacity: f32,

    // ── Fonts ─────────────────────────────────────────────────────────
    /// Primary font family name (None = system default).
    pub font_family: Option<&'a str>,
    /// Bold variant family override.
    pub font_family_bold: Option<&'a str>,
    /// Italic variant family override.
    pub font_family_italic: Option<&'a str>,
    /// Bold-italic variant family override.
    pub font_family_bold_italic: Option<&'a str>,
    /// Additional Unicode ranges and their fallback fonts.
    pub font_ranges: &'a [par_term_config::FontRange],
    /// Font size in points.
    pub font_size: f32,
    /// Enable HarfBuzz text shaping.
    pub enable_text_shaping: bool,
    /// Enable OpenType ligatures.
    pub enable_ligatures: bool,
    /// Enable OpenType kerning.
    pub enable_kerning: bool,
    /// Enable font anti-aliasing.
    pub font_antialias: bool,
    /// Enable font hinting.
    pub font_hinting: bool,
    /// Thin-strokes rendering mode.
    pub font_thin_strokes: par_term_config::ThinStrokesMode,
    /// Minimum contrast ratio between foreground and background.
    pub minimum_contrast: f32,

    // ── Layout ────────────────────────────────────────────────────────
    /// Padding around the terminal content in logical pixels.
    pub window_padding: f32,
    /// Line height multiplier.
    pub line_spacing: f32,
    /// Character width multiplier.
    pub char_spacing: f32,

    // ── Scrollbar ─────────────────────────────────────────────────────
    /// Scrollbar position string ("left", "right", "hidden").
    pub scrollbar_position: &'a str,
    /// Scrollbar width in logical pixels.
    pub scrollbar_width: f32,
    /// Scrollbar thumb color [R, G, B, A].
    pub scrollbar_thumb_color: [f32; 4],
    /// Scrollbar track color [R, G, B, A].
    pub scrollbar_track_color: [f32; 4],

    // ── Background ────────────────────────────────────────────────────
    /// Theme background color [R, G, B].
    pub background_color: [u8; 3],
    /// Optional background image file path.
    pub background_image_path: Option<&'a str>,
    /// Whether the background image feature is enabled.
    pub background_image_enabled: bool,
    /// How the background image is displayed (stretch, tile, etc.).
    pub background_image_mode: par_term_config::BackgroundImageMode,
    /// Background image opacity (0.0 – 1.0).
    pub background_image_opacity: f32,

    // ── Custom (background) shader ────────────────────────────────────
    /// Name / path of the custom background shader.
    pub custom_shader_path: Option<&'a str>,
    /// Whether the custom shader is enabled.
    pub custom_shader_enabled: bool,
    /// Whether the custom shader is animated.
    pub custom_shader_animation: bool,
    /// Animation speed multiplier for the custom shader.
    pub custom_shader_animation_speed: f32,
    /// Whether the shader renders over the full surface (vs. terminal area).
    pub custom_shader_full_content: bool,
    /// Brightness multiplier applied to the shader output.
    pub custom_shader_brightness: f32,
    /// Channel texture paths (iChannel0..3).
    pub custom_shader_channel_paths: &'a [Option<PathBuf>; 4],
    /// Cubemap texture path prefix (iCubemap).
    pub custom_shader_cubemap_path: Option<&'a Path>,
    /// Use the background image as iChannel0.
    pub use_background_as_channel0: bool,

    // ── Inline image settings ─────────────────────────────────────────
    /// Scaling filter for inline images (nearest vs linear).
    pub image_scaling_mode: par_term_config::ImageScalingMode,
    /// Whether to preserve aspect ratio when scaling inline images.
    pub image_preserve_aspect_ratio: bool,

    // ── Cursor shader ─────────────────────────────────────────────────
    /// Name / path of the cursor shader.
    pub cursor_shader_path: Option<&'a str>,
    /// Whether the cursor shader is enabled.
    pub cursor_shader_enabled: bool,
    /// Whether the cursor shader is animated.
    pub cursor_shader_animation: bool,
    /// Animation speed multiplier for the cursor shader.
    pub cursor_shader_animation_speed: f32,
}
