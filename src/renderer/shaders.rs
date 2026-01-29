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
    use_background_as_channel0: bool,
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

            crate::debug_info!(
                "SHADER",
                "Custom shader renderer initialized from: {} (use_bg_as_ch0={})",
                path.display(),
                use_background_as_channel0
            );
            (Some(renderer), Some(shader_path.to_string()))
        }
        Err(e) => {
            crate::debug_info!(
                "SHADER",
                "ERROR: Failed to load custom shader '{}': {}",
                path.display(),
                e
            );
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
            crate::debug_info!(
                "SHADER",
                "Cursor shader renderer initialized from: {} (cell={}x{}, padding={})",
                path.display(),
                cell_w,
                cell_h,
                window_padding
            );
            (Some(renderer), Some(shader_path.to_string()))
        }
        Err(e) => {
            crate::debug_info!(
                "SHADER",
                "ERROR: Failed to load cursor shader '{}': {}",
                path.display(),
                e
            );
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

    /// Update key press time for custom shaders (iTimeKeyPress uniform)
    ///
    /// Call this when a key is pressed to enable key-press-based shader effects
    /// like screen pulses, typing animations, or keystroke visualizations.
    pub fn update_key_press_time(&mut self) {
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.update_key_press();
        }
        if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            cursor_shader.update_key_press();
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
                        // Sync keep_text_opaque from cell renderer
                        renderer.set_keep_text_opaque(self.cell_renderer.keep_text_opaque());
                        // When background shader is enabled and chained into cursor shader,
                        // don't give cursor shader its own background - background shader handles it
                        let has_background_shader = self.custom_shader_renderer.is_some();

                        if has_background_shader {
                            // Background shader handles the background, cursor shader just passes through
                            renderer.set_background_color([0.0, 0.0, 0.0], false);
                            renderer.set_background_texture(self.cell_renderer.device(), None);
                            renderer.update_use_background_as_channel0(self.cell_renderer.device(), false);
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
                                renderer
                                    .set_background_texture(self.cell_renderer.device(), bg_texture);
                                renderer.update_use_background_as_channel0(
                                    self.cell_renderer.device(),
                                    true,
                                );
                            }
                        }
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
                        // Sync keep_text_opaque from cell renderer
                        renderer.set_keep_text_opaque(self.cell_renderer.keep_text_opaque());
                        // Sync background color for solid color mode
                        renderer.set_background_color(
                            self.cell_renderer.solid_background_color(),
                            self.cell_renderer.is_solid_color_background(),
                        );
                        crate::debug_info!(
                            "SHADER",
                            "Custom shader enabled at runtime: {}",
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
                        crate::debug_info!("SHADER", "ERROR: {}", error_msg);
                        Err(error_msg)
                    }
                }
            }
            _ => {
                if self.custom_shader_renderer.is_some() {
                    crate::debug_info!("SHADER", "Custom shader disabled at runtime");
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

    /// Sync the cursor shader's background state based on whether the background shader is enabled.
    ///
    /// When background shader is enabled, cursor shader should NOT have its own background
    /// (the background shader handles it). When background shader is disabled, cursor shader
    /// should have its own background.
    fn sync_cursor_shader_background_state(&mut self) {
        let Some(ref mut cursor_shader) = self.cursor_shader_renderer else {
            return;
        };

        let has_background_shader = self.custom_shader_renderer.is_some();

        if has_background_shader {
            // Background shader handles the background, cursor shader just passes through
            cursor_shader.set_background_color([0.0, 0.0, 0.0], false);
            cursor_shader.set_background_texture(self.cell_renderer.device(), None);
            cursor_shader.update_use_background_as_channel0(self.cell_renderer.device(), false);
        } else {
            // Cursor shader needs its own background
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

    /// Set whether to use the background image as iChannel0 for the custom shader.
    ///
    /// When enabled, the app's configured background image is bound as iChannel0
    /// instead of the custom_shader_channel0 texture file.
    #[allow(dead_code)]
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
    /// channel0 texture. This only has an effect if use_background_as_channel0
    /// is enabled.
    pub fn sync_background_texture_to_shader(&mut self) {
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            let bg_texture = self.cell_renderer.get_background_as_channel_texture();
            custom_shader.set_background_texture(self.cell_renderer.device(), bg_texture);
            self.dirty = true;
        }
    }

    /// Update both the use_background_as_channel0 flag and sync the texture.
    ///
    /// This method should be called when:
    /// - The use_background_as_channel0 setting changes
    /// - The background image or solid color changes (to sync the new texture)
    /// - Per-shader config changes
    ///
    /// The background texture is always synced to ensure changes are reflected.
    #[allow(dead_code)]
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
    /// This method handles the case where background_mode is Color and we need to
    /// create a solid color texture to pass as iChannel0 instead of an image.
    ///
    /// # Arguments
    /// * `use_background` - Whether to use background as iChannel0
    /// * `background_mode` - The current background mode (Default, Color, or Image)
    /// * `color` - The solid background color (used if mode is Color)
    pub fn update_background_as_channel0_with_mode(
        &mut self,
        use_background: bool,
        background_mode: crate::config::BackgroundMode,
        color: [u8; 3],
    ) {
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            // Get the appropriate texture based on background mode
            let bg_texture = match background_mode {
                crate::config::BackgroundMode::Default => {
                    log::info!("update_background_as_channel0_with_mode: Default mode, no texture");
                    None
                }
                crate::config::BackgroundMode::Color => {
                    // Create a solid color texture for the shader
                    log::info!(
                        "update_background_as_channel0_with_mode: Color mode, creating solid color texture RGB({},{},{})",
                        color[0],
                        color[1],
                        color[2]
                    );
                    Some(self.cell_renderer.get_solid_color_as_channel_texture(color))
                }
                crate::config::BackgroundMode::Image => {
                    // Use the existing background image texture
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
