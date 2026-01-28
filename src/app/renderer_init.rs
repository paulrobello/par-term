//! Renderer initialization helpers for WindowState.
//!
//! This module provides DRY helpers for initializing the renderer,
//! eliminating duplicate parameter passing between `rebuild_renderer()`
//! and `initialize_async()`.

use crate::config::{
    BackgroundImageMode, Config, FontRange, ShaderMetadata, VsyncMode, resolve_shader_config,
};
use crate::renderer::Renderer;
use crate::themes::Theme;
use std::path::PathBuf;
use std::sync::Arc;
use winit::window::Window;

/// Captures all parameters needed for Renderer::new()
/// Built from Config and Theme to eliminate duplicate parameter extraction
pub(crate) struct RendererInitParams {
    pub font_family: Option<String>,
    pub font_family_bold: Option<String>,
    pub font_family_italic: Option<String>,
    pub font_family_bold_italic: Option<String>,
    pub font_ranges: Vec<FontRange>,
    pub font_size: f32,
    pub window_padding: f32,
    pub line_spacing: f32,
    pub char_spacing: f32,
    pub scrollbar_position: String,
    pub scrollbar_width: f32,
    pub scrollbar_thumb_color: [f32; 4],
    pub scrollbar_track_color: [f32; 4],
    pub enable_text_shaping: bool,
    pub enable_ligatures: bool,
    pub enable_kerning: bool,
    pub vsync_mode: VsyncMode,
    pub window_opacity: f32,
    pub background_color: [u8; 3],
    pub background_image_path: Option<String>,
    pub background_image_enabled: bool,
    pub background_image_mode: BackgroundImageMode,
    pub background_image_opacity: f32,
    pub custom_shader_path: Option<String>,
    pub custom_shader_enabled: bool,
    pub custom_shader_animation: bool,
    pub custom_shader_animation_speed: f32,
    pub custom_shader_text_opacity: f32,
    pub custom_shader_full_content: bool,
    pub custom_shader_brightness: f32,
    pub custom_shader_channel_paths: [Option<PathBuf>; 4],
    pub custom_shader_cubemap_path: Option<PathBuf>,
    pub cursor_shader_path: Option<String>,
    pub cursor_shader_enabled: bool,
    pub cursor_shader_animation: bool,
    pub cursor_shader_animation_speed: f32,
}

impl RendererInitParams {
    /// Create renderer init params from config, theme, and optional shader metadata
    ///
    /// The metadata parameter allows full 3-tier resolution:
    /// 1. User per-shader override (from shader_configs)
    /// 2. Shader metadata defaults (from the shader file)
    /// 3. Global config defaults
    pub fn from_config(config: &Config, theme: &Theme, metadata: Option<&ShaderMetadata>) -> Self {
        debug_log!(
            "cursor-shader",
            "Config snapshot: enabled={}, path={:?}, animation={}, speed={}, disable_alt_screen={}",
            config.cursor_shader_enabled,
            config.cursor_shader,
            config.cursor_shader_animation,
            config.cursor_shader_animation_speed,
            config.cursor_shader_disable_in_alt_screen
        );

        // Resolve per-shader settings (user override -> metadata defaults -> global)
        let shader_override = config
            .custom_shader
            .as_ref()
            .and_then(|name| config.shader_configs.get(name));
        let resolved = resolve_shader_config(shader_override, metadata, config);

        Self {
            font_family: if config.font_family.is_empty() {
                None
            } else {
                Some(config.font_family.clone())
            },
            font_family_bold: config.font_family_bold.clone(),
            font_family_italic: config.font_family_italic.clone(),
            font_family_bold_italic: config.font_family_bold_italic.clone(),
            font_ranges: config.font_ranges.clone(),
            font_size: config.font_size,
            window_padding: config.window_padding,
            line_spacing: config.line_spacing,
            char_spacing: config.char_spacing,
            scrollbar_position: config.scrollbar_position.clone(),
            scrollbar_width: config.scrollbar_width,
            scrollbar_thumb_color: config.scrollbar_thumb_color,
            scrollbar_track_color: config.scrollbar_track_color,
            enable_text_shaping: config.enable_text_shaping,
            enable_ligatures: config.enable_ligatures,
            enable_kerning: config.enable_kerning,
            vsync_mode: config.vsync_mode,
            window_opacity: config.window_opacity,
            background_color: theme.background.as_array(),
            background_image_path: config.background_image.clone(),
            background_image_enabled: config.background_image_enabled,
            background_image_mode: config.background_image_mode,
            background_image_opacity: config.background_image_opacity,
            custom_shader_path: config.custom_shader.clone(),
            custom_shader_enabled: config.custom_shader_enabled,
            custom_shader_animation: config.custom_shader_animation,
            custom_shader_animation_speed: resolved.animation_speed,
            custom_shader_text_opacity: resolved.text_opacity,
            custom_shader_full_content: resolved.full_content,
            custom_shader_brightness: resolved.brightness,
            custom_shader_channel_paths: resolved.channel_paths(),
            custom_shader_cubemap_path: resolved.cubemap_path().cloned(),
            cursor_shader_path: config.cursor_shader.clone(),
            cursor_shader_enabled: config.cursor_shader_enabled,
            cursor_shader_animation: config.cursor_shader_animation,
            cursor_shader_animation_speed: config.cursor_shader_animation_speed,
        }
    }

    /// Create a new Renderer using these params
    pub async fn create_renderer(&self, window: Arc<Window>) -> anyhow::Result<Renderer> {
        Renderer::new(
            window,
            self.font_family.as_deref(),
            self.font_family_bold.as_deref(),
            self.font_family_italic.as_deref(),
            self.font_family_bold_italic.as_deref(),
            &self.font_ranges,
            self.font_size,
            self.window_padding,
            self.line_spacing,
            self.char_spacing,
            &self.scrollbar_position,
            self.scrollbar_width,
            self.scrollbar_thumb_color,
            self.scrollbar_track_color,
            self.enable_text_shaping,
            self.enable_ligatures,
            self.enable_kerning,
            self.vsync_mode,
            self.window_opacity,
            self.background_color,
            self.background_image_path.as_deref(),
            self.background_image_enabled,
            self.background_image_mode,
            self.background_image_opacity,
            self.custom_shader_path.as_deref(),
            self.custom_shader_enabled,
            self.custom_shader_animation,
            self.custom_shader_animation_speed,
            self.custom_shader_text_opacity,
            self.custom_shader_full_content,
            self.custom_shader_brightness,
            &self.custom_shader_channel_paths,
            self.custom_shader_cubemap_path.as_deref(),
            self.cursor_shader_path.as_deref(),
            self.cursor_shader_enabled,
            self.cursor_shader_animation,
            self.cursor_shader_animation_speed,
        )
        .await
    }
}

use super::window_state::WindowState;

impl WindowState {
    /// Apply cursor shader configuration to the renderer
    pub(crate) fn apply_cursor_shader_config(&self, renderer: &mut Renderer) {
        // Initialize cursor shader config
        renderer.update_cursor_shader_config(
            self.config.cursor_shader_color,
            self.config.cursor_shader_trail_duration,
            self.config.cursor_shader_glow_radius,
            self.config.cursor_shader_glow_intensity,
        );

        // Initialize cursor color from config
        renderer.update_cursor_color(self.config.cursor_color);

        // Hide cursor if cursor shader is enabled and configured to hide
        renderer.set_cursor_hidden_for_shader(
            self.config.cursor_shader_enabled && self.config.cursor_shader_hides_cursor,
        );
    }

    /// Initialize egui context and state
    /// If preserve_memory is true, preserves window positions and collapse state
    pub(crate) fn init_egui(&mut self, window: &Arc<Window>, preserve_memory: bool) {
        let previous_memory = if preserve_memory {
            self.egui_ctx
                .as_ref()
                .map(|ctx| ctx.memory(|mem| mem.clone()))
        } else {
            None
        };

        let scale_factor = window.scale_factor() as f32;
        let egui_ctx = egui::Context::default();

        if let Some(memory) = previous_memory {
            egui_ctx.memory_mut(|mem| *mem = memory);
        }

        let egui_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            window,
            Some(scale_factor),
            None,
            None,
        );

        self.egui_ctx = Some(egui_ctx);
        self.egui_state = Some(egui_state);
    }
}
