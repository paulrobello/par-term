//! Cursor shader initialisation and runtime management.
//!
//! Covers the cursor `CustomShaderRenderer` lifecycle: creation at startup via
//! [`init_cursor_shader`] and runtime enable/disable/reload operations
//! exposed as `impl Renderer` methods.

use super::super::Renderer;
use super::CursorShaderInitParams;
use crate::cell_renderer::CellRenderer;
use crate::custom_shader_renderer::CustomShaderRenderer;

/// Initialize the cursor shader renderer if configured.
///
/// Returns `(renderer, shader_path)` where both are `Some` if initialization succeeded.
pub(super) fn init_cursor_shader(
    cell_renderer: &CellRenderer,
    params: CursorShaderInitParams<'_>,
) -> (Option<CustomShaderRenderer>, Option<String>) {
    let CursorShaderInitParams {
        size_width,
        size_height,
        window_padding,
        path: cursor_shader_path,
        enabled: cursor_shader_enabled,
        animation: cursor_shader_animation,
        animation_speed: cursor_shader_animation_speed,
        window_opacity,
    } = params;
    log::debug!(
        "[cursor-shader] Init: enabled={}, path={:?}, animation={}, speed={}",
        cursor_shader_enabled,
        cursor_shader_path,
        cursor_shader_animation,
        cursor_shader_animation_speed
    );

    if !cursor_shader_enabled {
        log::info!("[cursor-shader] Disabled by config");
        return (None, None);
    }

    let Some(shader_path) = cursor_shader_path else {
        log::info!("[cursor-shader] Enabled but no path provided");
        return (None, None);
    };

    let path = par_term_config::Config::shader_path(shader_path);
    let empty_channels: [Option<std::path::PathBuf>; 4] = [None, None, None, None];
    let empty_custom_uniforms = std::collections::BTreeMap::new();

    match CustomShaderRenderer::new(
        cell_renderer.device(),
        cell_renderer.queue(),
        crate::custom_shader_renderer::CustomShaderRendererConfig {
            surface_format: cell_renderer.surface_format(),
            shader_path: &path,
            width: size_width,
            height: size_height,
            animation_enabled: cursor_shader_animation,
            animation_speed: cursor_shader_animation_speed,
            window_opacity,
            full_content_mode: true, // Cursor shader always uses full content
            channel_paths: &empty_channels,
            cubemap_path: None, // Cursor shaders don't use cubemaps
            custom_uniforms: &empty_custom_uniforms,
        },
    ) {
        Ok(mut renderer) => {
            let cell_w = cell_renderer.cell_width();
            let cell_h = cell_renderer.cell_height();
            renderer.update_cell_dimensions(cell_w, cell_h, window_padding);
            renderer.set_scale_factor(cell_renderer.scale_factor);
            log::info!(
                "[SHADER] Cursor shader renderer initialized from: {} (cell={}x{}, padding={})",
                path.display(),
                cell_w,
                cell_h,
                window_padding
            );
            (Some(renderer), Some(shader_path.to_string()))
        }
        Err(e) => {
            log::info!(
                "[SHADER] ERROR: Failed to load cursor shader '{}': {}",
                path.display(),
                e
            );
            (None, None)
        }
    }
}

// ============================================================================
// Cursor shader impl Renderer methods
// ============================================================================

impl Renderer {
    /// Get the current cursor shader path
    pub fn cursor_shader_path(&self) -> Option<&str> {
        self.cursor_shader_path.as_deref()
    }

    /// Reload the cursor shader from source code
    pub fn reload_cursor_shader_from_source(
        &mut self,
        source: &str,
    ) -> Result<(), crate::error::RenderError> {
        if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            cursor_shader
                .reload_from_source(self.cell_renderer.device(), source, "cursor_editor")
                .map_err(|e| crate::error::RenderError::NoActiveShader(format!("{:#}", e)))?;
            self.dirty = true;
            Ok(())
        } else {
            Err(crate::error::RenderError::NoActiveShader(
                "No cursor shader renderer active".to_string(),
            ))
        }
    }

    /// Enable or disable the cursor shader at runtime.
    ///
    /// # Returns
    /// `Ok(())` if successful, `Err` with error message if compilation fails
    pub fn set_cursor_shader_enabled(
        &mut self,
        enabled: bool,
        path: Option<&str>,
        window_opacity: f32,
        animation_enabled: bool,
        animation_speed: f32,
    ) -> Result<(), String> {
        log::debug!(
            "[cursor-shader] Toggle: enabled={}, path={:?}, animation={}, speed={}, opacity={}",
            enabled,
            path,
            animation_enabled,
            animation_speed,
            window_opacity
        );
        match (enabled, path) {
            (true, Some(path)) => {
                let path_changed = self.cursor_shader_path.as_ref().is_none_or(|p| p != path);

                // If we already have a shader renderer and path hasn't changed, just update flags
                if let Some(renderer) = &mut self.cursor_shader_renderer
                    && !path_changed
                {
                    renderer.set_animation_enabled(animation_enabled);
                    renderer.set_animation_speed(animation_speed);
                    renderer.set_opacity(window_opacity);
                    self.dirty = true;
                    log::info!("[cursor-shader] Already loaded; updated animation/opacities");
                    return Ok(());
                }

                let shader_path_full = par_term_config::Config::shader_path(path);
                // Cursor shader doesn't use channel textures or cubemaps
                let empty_channels: [Option<std::path::PathBuf>; 4] = [None, None, None, None];
                let empty_custom_uniforms = std::collections::BTreeMap::new();
                match CustomShaderRenderer::new(
                    self.cell_renderer.device(),
                    self.cell_renderer.queue(),
                    crate::custom_shader_renderer::CustomShaderRendererConfig {
                        surface_format: self.cell_renderer.surface_format(),
                        shader_path: &shader_path_full,
                        width: self.size.width,
                        height: self.size.height,
                        animation_enabled,
                        animation_speed,
                        window_opacity,
                        full_content_mode: true, // Cursor shader always uses full content
                        channel_paths: &empty_channels,
                        cubemap_path: None, // Cursor shaders don't use cubemaps
                        custom_uniforms: &empty_custom_uniforms,
                    },
                ) {
                    Ok(mut renderer) => {
                        // Sync cell dimensions for cursor position calculation
                        renderer.update_cell_dimensions(
                            self.cell_renderer.cell_width(),
                            self.cell_renderer.cell_height(),
                            self.cell_renderer.window_padding(),
                        );
                        // Sync DPI scale factor for cursor sizing
                        renderer.set_scale_factor(self.cell_renderer.scale_factor);
                        // Sync keep_text_opaque from cell renderer
                        renderer.set_keep_text_opaque(self.cell_renderer.keep_text_opaque());
                        // When background shader is enabled and chained into cursor shader,
                        // don't give cursor shader its own background - background shader handles it
                        let has_background_shader = self.custom_shader_renderer.is_some();

                        if has_background_shader {
                            // Background shader handles the background, cursor shader just passes through
                            renderer.set_background_color([0.0, 0.0, 0.0], false);
                            renderer.set_background_texture(self.cell_renderer.device(), None);
                            renderer.update_use_background_as_channel0(
                                self.cell_renderer.device(),
                                false,
                            );
                        } else {
                            // Sync background color for solid color mode
                            renderer.set_background_color(
                                self.cell_renderer.solid_background_color(),
                                self.cell_renderer.is_solid_color_background(),
                            );
                            // Sync background image for image mode
                            let is_image_mode = self.cell_renderer.has_background_image()
                                && !self.cell_renderer.is_solid_color_background();
                            if is_image_mode {
                                let bg_texture =
                                    self.cell_renderer.get_background_as_channel_texture();
                                renderer.set_background_texture(
                                    self.cell_renderer.device(),
                                    bg_texture,
                                );
                                renderer.update_use_background_as_channel0(
                                    self.cell_renderer.device(),
                                    true,
                                );
                            }
                        }
                        log::info!(
                            "[cursor-shader] Enabled at runtime: {}",
                            shader_path_full.display()
                        );
                        self.cursor_shader_renderer = Some(renderer);
                        self.cursor_shader_path = Some(path.to_string());
                        self.dirty = true;
                        Ok(())
                    }
                    Err(e) => {
                        let error_msg = format!(
                            "Failed to load cursor shader '{}': {}",
                            shader_path_full.display(),
                            e
                        );
                        log::error!("[cursor-shader] {}", error_msg);
                        Err(error_msg)
                    }
                }
            }
            _ => {
                if self.cursor_shader_renderer.is_some() {
                    log::info!("[cursor-shader] Disabled at runtime");
                } else {
                    log::debug!("[cursor-shader] Already disabled");
                }
                self.cursor_shader_renderer = None;
                self.cursor_shader_path = None;
                self.dirty = true;
                Ok(())
            }
        }
    }

    /// Sync the cursor shader's background state based on whether the background shader is enabled.
    ///
    /// When background shader is enabled, cursor shader should NOT have its own background
    /// (the background shader handles it). When background shader is disabled, cursor shader
    /// should have its own background.
    pub(super) fn sync_cursor_shader_background_state(&mut self) {
        let Some(ref mut cursor_shader) = self.cursor_shader_renderer else {
            return;
        };

        let has_background_shader = self.custom_shader_renderer.is_some();

        if has_background_shader {
            cursor_shader.set_background_color([0.0, 0.0, 0.0], false);
            cursor_shader.set_background_texture(self.cell_renderer.device(), None);
            cursor_shader.update_use_background_as_channel0(self.cell_renderer.device(), false);
        } else {
            cursor_shader.set_background_color(
                self.cell_renderer.solid_background_color(),
                self.cell_renderer.is_solid_color_background(),
            );

            let is_image_mode = self.cell_renderer.has_background_image()
                && !self.cell_renderer.is_solid_color_background();
            if is_image_mode {
                let bg_texture = self.cell_renderer.get_background_as_channel_texture();
                cursor_shader.set_background_texture(self.cell_renderer.device(), bg_texture);
                cursor_shader.update_use_background_as_channel0(self.cell_renderer.device(), true);
            } else {
                cursor_shader.set_background_texture(self.cell_renderer.device(), None);
                cursor_shader.update_use_background_as_channel0(self.cell_renderer.device(), false);
            }
        }
    }
}
