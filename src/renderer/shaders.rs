use super::Renderer;
use crate::cell_renderer::CellRenderer;
use crate::custom_shader_renderer::CustomShaderRenderer;
use anyhow::Result;

/// Initialize the custom shader renderer if configured.
///
/// Returns (renderer, shader_path) tuple where both are Some if initialization succeeded.
#[allow(clippy::too_many_arguments)]
pub(super) fn init_custom_shader(
    cell_renderer: &CellRenderer,
    size_width: u32,
    size_height: u32,
    window_padding: f32,
    custom_shader_path: Option<&str>,
    custom_shader_enabled: bool,
    custom_shader_animation: bool,
    custom_shader_animation_speed: f32,
    window_opacity: f32,
    custom_shader_text_opacity: f32,
    custom_shader_full_content: bool,
    custom_shader_brightness: f32,
    custom_shader_channel_paths: &[Option<std::path::PathBuf>; 4],
    custom_shader_cubemap_path: Option<&std::path::Path>,
) -> (Option<CustomShaderRenderer>, Option<String>) {
    if !custom_shader_enabled {
        return (None, None);
    }

    let Some(shader_path) = custom_shader_path else {
        return (None, None);
    };

    let path = crate::config::Config::shader_path(shader_path);
    match CustomShaderRenderer::new(
        cell_renderer.device(),
        cell_renderer.queue(),
        cell_renderer.surface_format(),
        &path,
        size_width,
        size_height,
        custom_shader_animation,
        custom_shader_animation_speed,
        window_opacity,
        custom_shader_text_opacity,
        custom_shader_full_content,
        custom_shader_channel_paths,
        custom_shader_cubemap_path,
    ) {
        Ok(mut renderer) => {
            renderer.update_cell_dimensions(
                cell_renderer.cell_width(),
                cell_renderer.cell_height(),
                window_padding,
            );
            renderer.set_brightness(custom_shader_brightness);
            log::info!(
                "Custom shader renderer initialized from: {}",
                path.display()
            );
            (Some(renderer), Some(shader_path.to_string()))
        }
        Err(e) => {
            log::error!("Failed to load custom shader '{}': {}", path.display(), e);
            (None, None)
        }
    }
}

/// Initialize the cursor shader renderer if configured.
///
/// Returns (renderer, shader_path) tuple where both are Some if initialization succeeded.
#[allow(clippy::too_many_arguments)]
pub(super) fn init_cursor_shader(
    cell_renderer: &CellRenderer,
    size_width: u32,
    size_height: u32,
    window_padding: f32,
    cursor_shader_path: Option<&str>,
    cursor_shader_enabled: bool,
    cursor_shader_animation: bool,
    cursor_shader_animation_speed: f32,
    window_opacity: f32,
) -> (Option<CustomShaderRenderer>, Option<String>) {
    debug_log!(
        "cursor-shader",
        "Init: enabled={}, path={:?}, animation={}, speed={}",
        cursor_shader_enabled,
        cursor_shader_path,
        cursor_shader_animation,
        cursor_shader_animation_speed
    );

    if !cursor_shader_enabled {
        debug_info!("cursor-shader", "Disabled by config");
        return (None, None);
    }

    let Some(shader_path) = cursor_shader_path else {
        debug_info!("cursor-shader", "Enabled but no path provided");
        return (None, None);
    };

    let path = crate::config::Config::shader_path(shader_path);
    let empty_channels: [Option<std::path::PathBuf>; 4] = [None, None, None, None];

    match CustomShaderRenderer::new(
        cell_renderer.device(),
        cell_renderer.queue(),
        cell_renderer.surface_format(),
        &path,
        size_width,
        size_height,
        cursor_shader_animation,
        cursor_shader_animation_speed,
        window_opacity,
        1.0,  // Text opacity (cursor shader always uses 1.0)
        true, // Full content mode (cursor shader always uses full content)
        &empty_channels,
        None, // Cursor shaders don't use cubemaps
    ) {
        Ok(mut renderer) => {
            let cell_w = cell_renderer.cell_width();
            let cell_h = cell_renderer.cell_height();
            renderer.update_cell_dimensions(cell_w, cell_h, window_padding);
            log::info!(
                "Cursor shader renderer initialized from: {} (cell={}x{}, padding={})",
                path.display(),
                cell_w,
                cell_h,
                window_padding
            );
            (Some(renderer), Some(shader_path.to_string()))
        }
        Err(e) => {
            log::error!("Failed to load cursor shader '{}': {}", path.display(), e);
            (None, None)
        }
    }
}

impl Renderer {
    /// Enable or disable animation for the custom shader at runtime
    #[allow(dead_code)]
    pub fn set_custom_shader_animation(&mut self, enabled: bool) {
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.set_animation_enabled(enabled);
            self.dirty = true;
        }
    }

    /// Update mouse position for custom shader (iMouse uniform)
    ///
    /// # Arguments
    /// * `x` - Mouse X position in pixels (0 = left edge)
    /// * `y` - Mouse Y position in pixels (0 = top edge)
    pub fn set_shader_mouse_position(&mut self, x: f32, y: f32) {
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.set_mouse_position(x, y);
        }
    }

    /// Update mouse button state for custom shader (iMouse uniform)
    ///
    /// # Arguments
    /// * `pressed` - True if left mouse button is pressed
    /// * `x` - Mouse X position at time of click/release
    /// * `y` - Mouse Y position at time of click/release
    pub fn set_shader_mouse_button(&mut self, pressed: bool, x: f32, y: f32) {
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.set_mouse_button(pressed, x, y);
        }
    }

    /// Update cursor state for custom shader (Ghostty-compatible cursor uniforms)
    ///
    /// This enables cursor trail effects and other cursor-based animations in custom shaders.
    ///
    /// # Arguments
    /// * `col` - Cursor column position (0-based)
    /// * `row` - Cursor row position (0-based)
    /// * `opacity` - Cursor opacity (0.0 = invisible, 1.0 = fully visible)
    /// * `color` - Cursor RGBA color
    /// * `style` - Cursor style (Block, Beam, Underline)
    pub fn update_shader_cursor(
        &mut self,
        col: usize,
        row: usize,
        opacity: f32,
        color: [f32; 4],
        style: par_term_emu_core_rust::cursor::CursorStyle,
    ) {
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.update_cursor(col, row, opacity, color, style);
        }
        // Also update cursor shader renderer
        if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            cursor_shader.update_cursor(col, row, opacity, color, style);
        }
    }

    /// Update cursor shader configuration from config values
    ///
    /// # Arguments
    /// * `color` - Cursor color for shader effects [R, G, B] (0-255)
    /// * `trail_duration` - Duration of cursor trail effect in seconds
    /// * `glow_radius` - Radius of cursor glow effect in pixels
    /// * `glow_intensity` - Intensity of cursor glow effect (0.0-1.0)
    pub fn update_cursor_shader_config(
        &mut self,
        color: [u8; 3],
        trail_duration: f32,
        glow_radius: f32,
        glow_intensity: f32,
    ) {
        // Update both shaders with cursor config
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.update_cursor_shader_config(
                color,
                trail_duration,
                glow_radius,
                glow_intensity,
            );
        }
        if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            cursor_shader.update_cursor_shader_config(
                color,
                trail_duration,
                glow_radius,
                glow_intensity,
            );
        }
    }

    /// Enable or disable the cursor shader at runtime
    ///
    /// # Arguments
    /// * `enabled` - Whether to enable the cursor shader
    /// * `path` - Optional shader path (relative to shaders folder or absolute)
    /// * `window_opacity` - Current window opacity
    /// * `animation_enabled` - Whether animation is enabled
    /// * `animation_speed` - Animation speed multiplier
    ///
    /// # Returns
    /// Ok(()) if successful, Err with error message if compilation fails
    #[allow(clippy::too_many_arguments)]
    pub fn set_cursor_shader_enabled(
        &mut self,
        enabled: bool,
        path: Option<&str>,
        window_opacity: f32,
        animation_enabled: bool,
        animation_speed: f32,
    ) -> Result<(), String> {
        debug_log!(
            "cursor-shader",
            "Toggle: enabled={}, path={:?}, animation={}, speed={}, opacity={}",
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
                    debug_info!(
                        "cursor-shader",
                        "Already loaded; updated animation/opacities"
                    );
                    return Ok(());
                }

                let shader_path_full = crate::config::Config::shader_path(path);
                // Cursor shader doesn't use channel textures or cubemaps
                let empty_channels: [Option<std::path::PathBuf>; 4] = [None, None, None, None];
                match CustomShaderRenderer::new(
                    self.cell_renderer.device(),
                    self.cell_renderer.queue(),
                    self.cell_renderer.surface_format(),
                    &shader_path_full,
                    self.size.width,
                    self.size.height,
                    animation_enabled,
                    animation_speed,
                    window_opacity,
                    1.0,  // Text opacity (cursor shader always uses 1.0)
                    true, // Full content mode (cursor shader always uses full content)
                    &empty_channels,
                    None, // Cursor shaders don't use cubemaps
                ) {
                    Ok(mut renderer) => {
                        // Sync cell dimensions for cursor position calculation
                        renderer.update_cell_dimensions(
                            self.cell_renderer.cell_width(),
                            self.cell_renderer.cell_height(),
                            self.cell_renderer.window_padding(),
                        );
                        debug_info!(
                            "cursor-shader",
                            "Enabled at runtime: {}",
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
                        debug_error!("cursor-shader", "{}", error_msg);
                        Err(error_msg)
                    }
                }
            }
            _ => {
                if self.cursor_shader_renderer.is_some() {
                    debug_info!("cursor-shader", "Disabled at runtime");
                } else {
                    debug_log!("cursor-shader", "Already disabled");
                }
                self.cursor_shader_renderer = None;
                self.cursor_shader_path = None;
                self.dirty = true;
                Ok(())
            }
        }
    }

    /// Get the current cursor shader path
    #[allow(dead_code)]
    pub fn cursor_shader_path(&self) -> Option<&str> {
        self.cursor_shader_path.as_deref()
    }

    /// Reload the cursor shader from source code
    pub fn reload_cursor_shader_from_source(&mut self, source: &str) -> Result<()> {
        if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            cursor_shader.reload_from_source(
                self.cell_renderer.device(),
                source,
                "cursor_editor",
            )?;
            self.dirty = true;
            Ok(())
        } else {
            Err(anyhow::anyhow!("No cursor shader renderer active"))
        }
    }

    /// Reload the custom shader from source code
    ///
    /// This method compiles the new shader source and replaces the current pipeline.
    /// If compilation fails, returns an error and the old shader remains active.
    ///
    /// # Arguments
    /// * `source` - The GLSL shader source code
    ///
    /// # Returns
    /// Ok(()) if successful, Err with error message if compilation fails
    pub fn reload_shader_from_source(&mut self, source: &str) -> Result<()> {
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.reload_from_source(self.cell_renderer.device(), source, "editor")?;
            self.dirty = true;
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "No custom shader is currently loaded. Enable a custom shader first."
            ))
        }
    }

    /// Enable/disable custom shader at runtime. When enabling, tries to
    /// (re)load the shader from the given path; when disabling, drops the
    /// renderer instance.
    ///
    /// Returns Ok(()) on success, or Err with error message on failure.
    #[allow(clippy::too_many_arguments)]
    pub fn set_custom_shader_enabled(
        &mut self,
        enabled: bool,
        shader_path: Option<&str>,
        window_opacity: f32,
        text_opacity: f32,
        animation_enabled: bool,
        animation_speed: f32,
        full_content: bool,
        brightness: f32,
        channel_paths: &[Option<std::path::PathBuf>; 4],
        cubemap_path: Option<&std::path::Path>,
    ) -> Result<(), String> {
        match (enabled, shader_path) {
            (true, Some(path)) => {
                // Check if the shader path has changed
                let path_changed = self.custom_shader_path.as_deref() != Some(path);

                // If we already have a shader renderer and path hasn't changed, just update flags
                if let Some(renderer) = &mut self.custom_shader_renderer {
                    if !path_changed {
                        renderer.set_animation_enabled(animation_enabled);
                        renderer.set_animation_speed(animation_speed);
                        renderer.set_opacity(window_opacity);
                        renderer.set_full_content_mode(full_content);
                        renderer.set_brightness(brightness);
                        return Ok(());
                    }
                    // Path changed - we need to reload, so drop the old renderer
                    log::info!("Shader path changed, reloading shader");
                }

                let shader_path_full = crate::config::Config::shader_path(path);
                match CustomShaderRenderer::new(
                    self.cell_renderer.device(),
                    self.cell_renderer.queue(),
                    self.cell_renderer.surface_format(),
                    &shader_path_full,
                    self.size.width,
                    self.size.height,
                    animation_enabled,
                    animation_speed,
                    window_opacity,
                    text_opacity,
                    full_content,
                    channel_paths,
                    cubemap_path,
                ) {
                    Ok(mut renderer) => {
                        // Sync cell dimensions for cursor position calculation
                        renderer.update_cell_dimensions(
                            self.cell_renderer.cell_width(),
                            self.cell_renderer.cell_height(),
                            self.cell_renderer.window_padding(),
                        );
                        // Apply brightness setting
                        renderer.set_brightness(brightness);
                        log::info!(
                            "Custom shader enabled at runtime: {}",
                            shader_path_full.display()
                        );
                        self.custom_shader_renderer = Some(renderer);
                        self.custom_shader_path = Some(path.to_string());
                        self.dirty = true;
                        Ok(())
                    }
                    Err(e) => {
                        let error_msg = format!(
                            "Failed to load shader '{}': {}",
                            shader_path_full.display(),
                            e
                        );
                        log::error!("{}", error_msg);
                        Err(error_msg)
                    }
                }
            }
            _ => {
                if self.custom_shader_renderer.is_some() {
                    log::info!("Custom shader disabled at runtime");
                }
                self.custom_shader_renderer = None;
                self.custom_shader_path = None;
                self.dirty = true;
                Ok(())
            }
        }
    }
}
