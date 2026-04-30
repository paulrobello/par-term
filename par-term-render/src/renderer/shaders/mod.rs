//! Shader management for the renderer.
//!
//! Manages two independent shader renderers:
//!
//! - **Background shader** (`custom_shader_renderer`) — full-screen post-processing
//!   effect rendered before the terminal cell grid, compatible with Ghostty/Shadertoy GLSL.
//! - **Cursor shader** (`cursor_shader_renderer`) — cursor overlay effect rendered after
//!   the cell grid, enabling glow, trail, and other cursor effects.
//!
//! ## Sub-modules
//!
//! - [`background`] — init and `impl Renderer` methods for the background shader
//! - [`cursor`] — init and `impl Renderer` methods for the cursor shader
//! - [`shared`] — `impl Renderer` methods that update both renderers (mouse, cursor state, etc.)

pub(super) mod background;
pub(super) mod cursor;
pub(super) mod shared;

use crate::cell_renderer::CellRenderer;
use crate::custom_shader_renderer::CustomShaderRenderer;
use par_term_config::{ShaderBackgroundBlendMode, ShaderUniformValue};
use std::collections::BTreeMap;

/// Parameters for initialising the background custom shader renderer.
pub struct CustomShaderInitParams<'a> {
    pub size_width: u32,
    pub size_height: u32,
    pub window_padding: f32,
    pub path: Option<&'a str>,
    pub enabled: bool,
    pub animation: bool,
    pub animation_speed: f32,
    pub window_opacity: f32,
    pub full_content: bool,
    pub brightness: f32,
    pub channel_paths: &'a [Option<std::path::PathBuf>; 4],
    pub cubemap_path: Option<&'a std::path::Path>,
    pub custom_uniforms: &'a BTreeMap<String, ShaderUniformValue>,
    pub use_background_as_channel0: bool,
    pub background_channel0_blend_mode: ShaderBackgroundBlendMode,
    pub auto_dim_under_text: bool,
    pub auto_dim_strength: f32,
}

/// Parameters for initialising the cursor shader renderer.
pub struct CursorShaderInitParams<'a> {
    pub size_width: u32,
    pub size_height: u32,
    pub window_padding: f32,
    pub path: Option<&'a str>,
    pub enabled: bool,
    pub animation: bool,
    pub animation_speed: f32,
    pub window_opacity: f32,
}

/// Parameters for enabling/updating the background custom shader at runtime.
pub struct CustomShaderEnableParams<'a> {
    pub enabled: bool,
    pub shader_path: Option<&'a str>,
    pub window_opacity: f32,
    pub animation_enabled: bool,
    pub animation_speed: f32,
    pub full_content: bool,
    pub brightness: f32,
    pub channel_paths: &'a [Option<std::path::PathBuf>; 4],
    pub cubemap_path: Option<&'a std::path::Path>,
    pub custom_uniforms: &'a BTreeMap<String, ShaderUniformValue>,
    pub background_channel0_blend_mode: ShaderBackgroundBlendMode,
    pub auto_dim_under_text: bool,
    pub auto_dim_strength: f32,
}

/// Initialize the custom (background) shader renderer if configured.
///
/// Returns `(renderer, shader_path)` where both are `Some` if initialization succeeded.
pub(super) fn init_custom_shader(
    cell_renderer: &CellRenderer,
    params: CustomShaderInitParams<'_>,
) -> (Option<CustomShaderRenderer>, Option<String>) {
    background::init_custom_shader(cell_renderer, params)
}

/// Initialize the cursor shader renderer if configured.
///
/// Returns `(renderer, shader_path)` where both are `Some` if initialization succeeded.
pub(super) fn init_cursor_shader(
    cell_renderer: &CellRenderer,
    params: CursorShaderInitParams<'_>,
) -> (Option<CustomShaderRenderer>, Option<String>) {
    cursor::init_cursor_shader(cell_renderer, params)
}
