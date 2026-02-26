//! Renderer initialization helpers for WindowState.
//!
//! This module provides DRY helpers for initializing the renderer,
//! eliminating duplicate parameter passing between `rebuild_renderer()`
//! and `initialize_async()`.

use crate::config::{
    BackgroundImageMode, BackgroundMode, Config, CursorShaderMetadata, FontRange, PowerPreference,
    ShaderMetadata, ThinStrokesMode, UnfocusedCursorStyle, VsyncMode, resolve_cursor_shader_config,
    resolve_shader_config,
};

/// Expand tilde in path to home directory
fn expand_path(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest).to_string_lossy().to_string();
    }
    path.to_string()
}
use crate::renderer::{Renderer, RendererParams};
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
    pub font_antialias: bool,
    pub font_hinting: bool,
    pub font_thin_strokes: ThinStrokesMode,
    pub minimum_contrast: f32,
    pub vsync_mode: VsyncMode,
    pub power_preference: PowerPreference,
    pub window_opacity: f32,
    /// Theme background color (used for Default mode and cell backgrounds)
    pub background_color: [u8; 3],
    /// Background mode: Default (theme), Color (solid), or Image
    pub background_mode: BackgroundMode,
    /// Solid background color from config (used when background_mode is Color)
    pub solid_background_color: [u8; 3],
    pub background_image_path: Option<String>,
    pub background_image_enabled: bool,
    pub background_image_mode: BackgroundImageMode,
    pub background_image_opacity: f32,
    pub custom_shader_path: Option<String>,
    pub custom_shader_enabled: bool,
    pub custom_shader_animation: bool,
    pub custom_shader_animation_speed: f32,
    pub custom_shader_full_content: bool,
    pub custom_shader_brightness: f32,
    pub custom_shader_channel_paths: [Option<PathBuf>; 4],
    pub custom_shader_cubemap_path: Option<PathBuf>,
    pub use_background_as_channel0: bool,
    pub image_scaling_mode: crate::config::ImageScalingMode,
    pub image_preserve_aspect_ratio: bool,
    pub cursor_shader_path: Option<String>,
    pub cursor_shader_enabled: bool,
    pub cursor_shader_animation: bool,
    pub cursor_shader_animation_speed: f32,
    pub cursor_shader_hides_cursor: bool,
    pub cursor_shader_glow_radius: f32,
    pub cursor_shader_glow_intensity: f32,
    pub cursor_shader_trail_duration: f32,
    pub cursor_shader_color: [u8; 3],
    pub transparency_affects_only_default_background: bool,
    pub keep_text_opaque: bool,
    pub link_underline_style: par_term_config::LinkUnderlineStyle,
    // Cursor enhancements
    pub cursor_guide_enabled: bool,
    pub cursor_guide_color: [u8; 4],
    pub cursor_shadow_enabled: bool,
    pub cursor_shadow_color: [u8; 4],
    pub cursor_shadow_offset: [f32; 2],
    pub cursor_shadow_blur: f32,
    pub cursor_boost: f32,
    pub cursor_boost_color: [u8; 3],
    pub unfocused_cursor_style: UnfocusedCursorStyle,
    // Command separator settings
    pub command_separator_enabled: bool,
    pub command_separator_thickness: f32,
    pub command_separator_opacity: f32,
    pub command_separator_exit_color: bool,
    pub command_separator_color: [u8; 3],
    // Per-pane background configs
    pub pane_backgrounds: Vec<crate::config::PaneBackgroundConfig>,
}

impl RendererInitParams {
    /// Create renderer init params from config, theme, and optional shader metadata
    ///
    /// The metadata parameters allow full 3-tier resolution:
    /// 1. User per-shader override (from shader_configs / cursor_shader_configs)
    /// 2. Shader metadata defaults (from the shader file)
    /// 3. Global config defaults
    pub fn from_config(
        config: &Config,
        theme: &Theme,
        metadata: Option<&ShaderMetadata>,
        cursor_metadata: Option<&CursorShaderMetadata>,
    ) -> Self {
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

        // Resolve per-cursor-shader settings
        let cursor_shader_override = config
            .cursor_shader
            .as_ref()
            .and_then(|name| config.cursor_shader_configs.get(name));
        let resolved_cursor =
            resolve_cursor_shader_config(cursor_shader_override, cursor_metadata, config);

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
            font_antialias: config.font_antialias,
            font_hinting: config.font_hinting,
            font_thin_strokes: config.font_thin_strokes,
            minimum_contrast: config.minimum_contrast,
            vsync_mode: config.vsync_mode,
            power_preference: config.power_preference,
            window_opacity: config.window_opacity,
            background_color: theme.background.as_array(),
            background_mode: config.background_mode,
            solid_background_color: config.background_color,
            background_image_path: {
                let path = config.background_image.as_ref().map(|p| expand_path(p));
                log::info!(
                    "RendererInitParams: background_mode={:?}, solid_color={:?}, image_path={:?}, enabled={}",
                    config.background_mode,
                    config.background_color,
                    path,
                    config.background_image_enabled
                );
                path
            },
            background_image_enabled: config.background_image_enabled,
            background_image_mode: config.background_image_mode,
            background_image_opacity: config.background_image_opacity,
            custom_shader_path: config.custom_shader.clone(),
            custom_shader_enabled: config.custom_shader_enabled,
            custom_shader_animation: config.custom_shader_animation,
            custom_shader_animation_speed: resolved.animation_speed,
            custom_shader_full_content: resolved.full_content,
            custom_shader_brightness: resolved.brightness,
            custom_shader_channel_paths: resolved.channel_paths(),
            custom_shader_cubemap_path: resolved.cubemap_path().cloned(),
            use_background_as_channel0: resolved.use_background_as_channel0,
            image_scaling_mode: config.image_scaling_mode,
            image_preserve_aspect_ratio: config.image_preserve_aspect_ratio,
            cursor_shader_path: config.cursor_shader.clone(),
            cursor_shader_enabled: config.cursor_shader_enabled,
            cursor_shader_animation: config.cursor_shader_animation,
            cursor_shader_animation_speed: resolved_cursor.base.animation_speed,
            cursor_shader_hides_cursor: resolved_cursor.hides_cursor,
            cursor_shader_glow_radius: resolved_cursor.glow_radius,
            cursor_shader_glow_intensity: resolved_cursor.glow_intensity,
            cursor_shader_trail_duration: resolved_cursor.trail_duration,
            cursor_shader_color: resolved_cursor.cursor_color,
            transparency_affects_only_default_background: config
                .transparency_affects_only_default_background,
            keep_text_opaque: config.keep_text_opaque,
            link_underline_style: config.link_underline_style,
            cursor_guide_enabled: config.cursor_guide_enabled,
            cursor_guide_color: config.cursor_guide_color,
            cursor_shadow_enabled: config.cursor_shadow_enabled,
            cursor_shadow_color: config.cursor_shadow_color,
            cursor_shadow_offset: config.cursor_shadow_offset,
            cursor_shadow_blur: config.cursor_shadow_blur,
            cursor_boost: config.cursor_boost,
            cursor_boost_color: config.cursor_boost_color,
            unfocused_cursor_style: config.unfocused_cursor_style,
            command_separator_enabled: config.command_separator_enabled,
            command_separator_thickness: config.command_separator_thickness,
            command_separator_opacity: config.command_separator_opacity,
            command_separator_exit_color: config.command_separator_exit_color,
            command_separator_color: config.command_separator_color,
            pane_backgrounds: config.pane_backgrounds.clone(),
        }
    }

    /// Create a new Renderer using these params
    pub async fn create_renderer(&self, window: Arc<Window>) -> anyhow::Result<Renderer> {
        let mut renderer = Renderer::new(RendererParams {
            window,
            font_family: self.font_family.as_deref(),
            font_family_bold: self.font_family_bold.as_deref(),
            font_family_italic: self.font_family_italic.as_deref(),
            font_family_bold_italic: self.font_family_bold_italic.as_deref(),
            font_ranges: &self.font_ranges,
            font_size: self.font_size,
            window_padding: self.window_padding,
            line_spacing: self.line_spacing,
            char_spacing: self.char_spacing,
            scrollbar_position: &self.scrollbar_position,
            scrollbar_width: self.scrollbar_width,
            scrollbar_thumb_color: self.scrollbar_thumb_color,
            scrollbar_track_color: self.scrollbar_track_color,
            enable_text_shaping: self.enable_text_shaping,
            enable_ligatures: self.enable_ligatures,
            enable_kerning: self.enable_kerning,
            font_antialias: self.font_antialias,
            font_hinting: self.font_hinting,
            font_thin_strokes: self.font_thin_strokes,
            minimum_contrast: self.minimum_contrast,
            vsync_mode: self.vsync_mode,
            power_preference: self.power_preference,
            window_opacity: self.window_opacity,
            background_color: self.background_color,
            background_image_path: self.background_image_path.as_deref(),
            background_image_enabled: self.background_image_enabled,
            background_image_mode: self.background_image_mode,
            background_image_opacity: self.background_image_opacity,
            custom_shader_path: self.custom_shader_path.as_deref(),
            custom_shader_enabled: self.custom_shader_enabled,
            custom_shader_animation: self.custom_shader_animation,
            custom_shader_animation_speed: self.custom_shader_animation_speed,
            custom_shader_full_content: self.custom_shader_full_content,
            custom_shader_brightness: self.custom_shader_brightness,
            custom_shader_channel_paths: &self.custom_shader_channel_paths,
            custom_shader_cubemap_path: self.custom_shader_cubemap_path.as_deref(),
            use_background_as_channel0: self.use_background_as_channel0,
            image_scaling_mode: self.image_scaling_mode,
            image_preserve_aspect_ratio: self.image_preserve_aspect_ratio,
            cursor_shader_path: self.cursor_shader_path.as_deref(),
            cursor_shader_enabled: self.cursor_shader_enabled,
            cursor_shader_animation: self.cursor_shader_animation,
            cursor_shader_animation_speed: self.cursor_shader_animation_speed,
        })
        .await?;

        // Apply transparency mode settings
        renderer.set_transparency_affects_only_default_background(
            self.transparency_affects_only_default_background,
        );
        renderer.set_keep_text_opaque(self.keep_text_opaque);
        renderer.set_link_underline_style(self.link_underline_style);

        // Apply background mode (Default, Color, or Image)
        // This must be called after renderer creation to properly set up solid color mode
        renderer.set_background(
            self.background_mode,
            self.solid_background_color,
            self.background_image_path.as_deref(),
            self.background_image_mode,
            self.background_image_opacity,
            self.background_image_enabled,
        );

        // Sync background texture with shader if use_background_as_channel0 is enabled
        // This must be called AFTER set_background() so the texture exists for Color mode
        if self.use_background_as_channel0 {
            renderer.update_background_as_channel0_with_mode(
                true,
                self.background_mode,
                self.solid_background_color,
            );
        }

        // Apply cursor enhancement settings
        renderer.update_cursor_guide(self.cursor_guide_enabled, self.cursor_guide_color);
        renderer.update_cursor_shadow(
            self.cursor_shadow_enabled,
            self.cursor_shadow_color,
            self.cursor_shadow_offset,
            self.cursor_shadow_blur,
        );
        renderer.update_cursor_boost(self.cursor_boost, self.cursor_boost_color);
        renderer.update_unfocused_cursor_style(self.unfocused_cursor_style);

        // Apply command separator settings
        renderer.update_command_separator(
            self.command_separator_enabled,
            self.command_separator_thickness,
            self.command_separator_opacity,
            self.command_separator_exit_color,
            self.command_separator_color,
        );

        // Pre-load per-pane background textures into the renderer cache
        for pb_config in &self.pane_backgrounds {
            if let Err(e) = renderer.load_pane_background(&pb_config.image) {
                log::error!(
                    "Failed to load pane {} background '{}': {}",
                    pb_config.index,
                    pb_config.image,
                    e
                );
            }
        }

        Ok(renderer)
    }
}

use super::window_state::WindowState;

impl WindowState {
    /// Apply cursor shader configuration to the renderer
    ///
    /// Uses the resolved cursor shader settings from RendererInitParams,
    /// which properly resolves user overrides -> metadata defaults -> global config.
    pub(crate) fn apply_cursor_shader_config(
        &self,
        renderer: &mut Renderer,
        params: &RendererInitParams,
    ) {
        // Initialize cursor shader config using resolved values
        renderer.update_cursor_shader_config(
            params.cursor_shader_color,
            params.cursor_shader_trail_duration,
            params.cursor_shader_glow_radius,
            params.cursor_shader_glow_intensity,
        );

        // Initialize cursor color from config
        renderer.update_cursor_color(self.config.cursor_color);

        // Initialize cursor text color from config
        renderer.update_cursor_text_color(self.config.cursor_text_color);

        // Hide cursor if cursor shader is enabled and configured to hide
        renderer.set_cursor_hidden_for_shader(
            params.cursor_shader_enabled && params.cursor_shader_hides_cursor,
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
        crate::settings_ui::nerd_font::configure_nerd_font(&egui_ctx);

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
