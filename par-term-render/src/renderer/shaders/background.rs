//! Background (custom) shader initialisation and runtime management.
//!
//! Covers the `CustomShaderRenderer` lifecycle: creation at startup via
//! [`init_custom_shader`] and runtime enable/disable/reload operations
//! exposed as `impl Renderer` methods.

use super::super::Renderer;
use super::{CustomShaderEnableParams, CustomShaderInitParams};
use crate::cell_renderer::CellRenderer;
use crate::custom_shader_renderer::CustomShaderRenderer;

/// Initialize the custom shader renderer if configured.
///
/// Returns `(renderer, shader_path)` where both are `Some` if initialization succeeded.
pub(super) fn init_custom_shader(
    cell_renderer: &CellRenderer,
    params: CustomShaderInitParams<'_>,
) -> (Option<CustomShaderRenderer>, Option<String>) {
    let CustomShaderInitParams {
        size_width,
        size_height,
        window_padding,
        path: custom_shader_path,
        enabled: custom_shader_enabled,
        animation: custom_shader_animation,
        animation_speed: custom_shader_animation_speed,
        window_opacity,
        full_content: custom_shader_full_content,
        brightness: custom_shader_brightness,
        channel_paths: custom_shader_channel_paths,
        cubemap_path: custom_shader_cubemap_path,
        use_background_as_channel0,
    } = params;
    log::info!(
        "[shader-init] init_custom_shader: enabled={}, path={:?}",
        custom_shader_enabled,
        custom_shader_path
    );
    if !custom_shader_enabled {
        log::info!("[shader-init] Skipping: custom_shader_enabled=false");
        return (None, None);
    }

    let Some(shader_path) = custom_shader_path else {
        log::info!("[shader-init] Skipping: custom_shader_path is None");
        return (None, None);
    };

    let path = par_term_config::Config::shader_path(shader_path);
    let empty_custom_uniforms = std::collections::BTreeMap::new();
    match CustomShaderRenderer::new(
        cell_renderer.device(),
        cell_renderer.queue(),
        crate::custom_shader_renderer::CustomShaderRendererConfig {
            surface_format: cell_renderer.surface_format(),
            shader_path: &path,
            width: size_width,
            height: size_height,
            animation_enabled: custom_shader_animation,
            animation_speed: custom_shader_animation_speed,
            window_opacity,
            full_content_mode: custom_shader_full_content,
            channel_paths: custom_shader_channel_paths,
            cubemap_path: custom_shader_cubemap_path,
            custom_uniforms: &empty_custom_uniforms,
        },
    ) {
        Ok(mut renderer) => {
            renderer.update_cell_dimensions(
                cell_renderer.cell_width(),
                cell_renderer.cell_height(),
                window_padding,
            );
            renderer.set_scale_factor(cell_renderer.scale_factor);
            renderer.set_brightness(custom_shader_brightness);

            // Apply use_background_as_channel0 setting
            if use_background_as_channel0 {
                // Sync background texture and set flag
                let bg_texture = cell_renderer.get_background_as_channel_texture();
                renderer.set_background_texture(cell_renderer.device(), bg_texture);
                renderer.update_use_background_as_channel0(
                    cell_renderer.device(),
                    use_background_as_channel0,
                );
            }

            log::info!(
                "[SHADER] Custom shader renderer initialized from: {} (use_bg_as_ch0={})",
                path.display(),
                use_background_as_channel0
            );
            (Some(renderer), Some(shader_path.to_string()))
        }
        Err(e) => {
            log::info!(
                "[SHADER] ERROR: Failed to load custom shader '{}': {}",
                path.display(),
                e
            );
            (None, None)
        }
    }
}

// ============================================================================
// Background shader impl Renderer methods
// ============================================================================

impl Renderer {
    /// Enable or disable animation for the custom shader at runtime
    pub fn set_custom_shader_animation(&mut self, enabled: bool) {
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.set_animation_enabled(enabled);
            self.dirty = true;
        }
    }

    /// Reload the custom shader from source code.
    ///
    /// Compiles the new shader source and replaces the current pipeline.
    /// If compilation fails, returns an error and the old shader remains active.
    pub fn reload_shader_from_source(
        &mut self,
        source: &str,
    ) -> Result<(), crate::error::RenderError> {
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader
                .reload_from_source(self.cell_renderer.device(), source, "editor")
                .map_err(|e| crate::error::RenderError::NoActiveShader(format!("{:#}", e)))?;
            self.dirty = true;
            Ok(())
        } else {
            Err(crate::error::RenderError::NoActiveShader(
                "No custom shader is currently loaded. Enable a custom shader first.".to_string(),
            ))
        }
    }

    /// Enable/disable custom shader at runtime.
    ///
    /// When enabling, tries to (re)load the shader from the given path; when disabling,
    /// drops the renderer instance.
    pub fn set_custom_shader_enabled(
        &mut self,
        params: CustomShaderEnableParams<'_>,
    ) -> Result<(), String> {
        let CustomShaderEnableParams {
            enabled,
            shader_path,
            window_opacity,
            animation_enabled,
            animation_speed,
            full_content,
            brightness,
            channel_paths,
            cubemap_path,
        } = params;
        match (enabled, shader_path) {
            (true, Some(path)) => {
                // Check if the shader path has changed
                let path_changed = self.custom_shader_path.as_deref() != Some(path);

                // If we already have a shader renderer and path hasn't changed, just update flags and textures
                if let Some(renderer) = &mut self.custom_shader_renderer
                    && !path_changed
                {
                    renderer.set_animation_enabled(animation_enabled);
                    renderer.set_animation_speed(animation_speed);
                    renderer.set_opacity(window_opacity);
                    renderer.set_full_content_mode(full_content);
                    renderer.set_brightness(brightness);

                    // Update channel textures (they may have changed even if shader path didn't)
                    for (i, path) in channel_paths.iter().enumerate() {
                        if let Err(e) = renderer.update_channel_texture(
                            self.cell_renderer.device(),
                            self.cell_renderer.queue(),
                            (i + 1) as u8, // channel indices are 1-4
                            path.as_deref(),
                        ) {
                            log::warn!("Failed to update channel {} texture: {}", i, e);
                        }
                    }

                    // Update cubemap if provided
                    if let Some(cubemap) = cubemap_path
                        && let Err(e) = renderer.update_cubemap(
                            self.cell_renderer.device(),
                            self.cell_renderer.queue(),
                            Some(cubemap),
                        )
                    {
                        log::warn!("Failed to update cubemap: {}", e);
                    }

                    return Ok(());
                }

                let shader_path_full = par_term_config::Config::shader_path(path);
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
                        full_content_mode: full_content,
                        channel_paths,
                        cubemap_path,
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
                        // Apply brightness setting
                        renderer.set_brightness(brightness);
                        // Sync keep_text_opaque from cell renderer
                        renderer.set_keep_text_opaque(self.cell_renderer.keep_text_opaque());
                        // Pass background color but don't activate solid color mode
                        // Custom shaders handle their own background
                        renderer.set_background_color(
                            self.cell_renderer.solid_background_color(),
                            false,
                        );
                        log::info!(
                            "[SHADER] Custom shader enabled at runtime: {}",
                            shader_path_full.display()
                        );
                        self.custom_shader_renderer = Some(renderer);
                        self.custom_shader_path = Some(path.to_string());

                        // When background shader is enabled, cursor shader should not have its own background
                        self.sync_cursor_shader_background_state();

                        self.dirty = true;
                        Ok(())
                    }
                    Err(e) => {
                        let error_msg = format!(
                            "Failed to load shader '{}': {}",
                            shader_path_full.display(),
                            e
                        );
                        log::info!("[SHADER] ERROR: {}", error_msg);
                        Err(error_msg)
                    }
                }
            }
            _ => {
                if self.custom_shader_renderer.is_some() {
                    log::info!("[SHADER] Custom shader disabled at runtime");
                }
                self.custom_shader_renderer = None;
                self.custom_shader_path = None;

                // When background shader is disabled, cursor shader should get its own background back
                self.sync_cursor_shader_background_state();

                self.dirty = true;
                Ok(())
            }
        }
    }

    /// Set whether to use the background image as iChannel0 for the custom shader.
    pub fn set_use_background_as_channel0(&mut self, use_background: bool) {
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader
                .update_use_background_as_channel0(self.cell_renderer.device(), use_background);
            self.dirty = true;
        }
    }

    /// Update the background texture for use as iChannel0 in shaders.
    ///
    /// Call this whenever the background image changes to sync the shader's
    /// channel0 texture. Only has effect if use_background_as_channel0 is enabled.
    pub fn sync_background_texture_to_shader(&mut self) {
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            let bg_texture = self.cell_renderer.get_background_as_channel_texture();
            custom_shader.set_background_texture(self.cell_renderer.device(), bg_texture);
            self.dirty = true;
        }
    }

    /// Update both the use_background_as_channel0 flag and sync the texture.
    pub fn update_background_as_channel0(&mut self, use_background: bool) {
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            // Always sync the background texture first - it may have changed
            let bg_texture = self.cell_renderer.get_background_as_channel_texture();
            custom_shader.set_background_texture(self.cell_renderer.device(), bg_texture);

            // Then update the flag - this will recreate bind group if flag actually changed
            custom_shader
                .update_use_background_as_channel0(self.cell_renderer.device(), use_background);

            self.dirty = true;
        }
    }

    /// Update background as channel0 with solid color support.
    ///
    /// Handles the case where background_mode is Color and we need to
    /// create a solid color texture to pass as iChannel0 instead of an image.
    pub fn update_background_as_channel0_with_mode(
        &mut self,
        use_background: bool,
        background_mode: par_term_config::BackgroundMode,
        color: [u8; 3],
    ) {
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            let bg_texture = match background_mode {
                par_term_config::BackgroundMode::Default => {
                    log::info!("update_background_as_channel0_with_mode: Default mode, no texture");
                    None
                }
                par_term_config::BackgroundMode::Color => {
                    log::info!(
                        "update_background_as_channel0_with_mode: Color mode, creating solid color texture RGB({},{},{})",
                        color[0],
                        color[1],
                        color[2]
                    );
                    Some(self.cell_renderer.get_solid_color_as_channel_texture(color))
                }
                par_term_config::BackgroundMode::Image => {
                    let tex = self.cell_renderer.get_background_as_channel_texture();
                    log::info!(
                        "update_background_as_channel0_with_mode: Image mode, texture={}",
                        if tex.is_some() { "Some" } else { "None" }
                    );
                    tex
                }
            };

            let has_texture = bg_texture.is_some();
            custom_shader.set_background_texture(self.cell_renderer.device(), bg_texture);
            custom_shader
                .update_use_background_as_channel0(self.cell_renderer.device(), use_background);

            log::info!(
                "update_background_as_channel0_with_mode: use_background={}, has_texture={}",
                use_background,
                has_texture
            );

            self.dirty = true;
        }
    }
}
