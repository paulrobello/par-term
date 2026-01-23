use super::cell_renderer::{Cell, CellRenderer};
use super::graphics_renderer::GraphicsRenderer;
use crate::custom_shader_renderer::CustomShaderRenderer;
use anyhow::Result;
use std::sync::Arc;
use winit::dpi::PhysicalSize;
use winit::window::Window;

/// Renderer for the terminal using custom wgpu cell renderer
pub struct Renderer {
    // Cell renderer (owns the scrollbar)
    cell_renderer: CellRenderer,

    // Graphics renderer for sixel images
    graphics_renderer: GraphicsRenderer,

    // Current sixel graphics to render: (id, row, col, width_cells, height_cells, alpha, scroll_offset_rows)
    // Note: row is isize to allow negative values for graphics scrolled off top
    sixel_graphics: Vec<(u64, isize, usize, usize, usize, f32, usize)>,

    // egui renderer for settings UI
    egui_renderer: egui_wgpu::Renderer,

    // Custom shader renderer for post-processing effects (background shader)
    custom_shader_renderer: Option<CustomShaderRenderer>,
    // Track current shader path to detect changes
    custom_shader_path: Option<String>,

    // Cursor shader renderer for cursor-specific effects (separate from background shader)
    cursor_shader_renderer: Option<CustomShaderRenderer>,
    // Track current cursor shader path to detect changes
    cursor_shader_path: Option<String>,

    // Cached for convenience
    size: PhysicalSize<u32>,

    // Dirty flag for optimization - only render when content has changed
    dirty: bool,

    // Debug overlay text
    #[allow(dead_code)]
    #[allow(dead_code)]
    debug_text: Option<String>,
}

impl Renderer {
    /// Create a new renderer
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        window: Arc<Window>,
        font_family: Option<&str>,
        font_family_bold: Option<&str>,
        font_family_italic: Option<&str>,
        font_family_bold_italic: Option<&str>,
        font_ranges: &[crate::config::FontRange],
        font_size: f32,
        window_padding: f32,
        line_spacing: f32,
        char_spacing: f32,
        scrollbar_position: &str,
        scrollbar_width: f32,
        scrollbar_thumb_color: [f32; 4],
        scrollbar_track_color: [f32; 4],
        enable_text_shaping: bool,
        enable_ligatures: bool,
        enable_kerning: bool,
        vsync_mode: crate::config::VsyncMode,
        window_opacity: f32,
        background_color: [u8; 3],
        background_image_path: Option<&str>,
        background_image_enabled: bool,
        background_image_mode: crate::config::BackgroundImageMode,
        background_image_opacity: f32,
        custom_shader_path: Option<&str>,
        custom_shader_enabled: bool,
        custom_shader_animation: bool,
        custom_shader_animation_speed: f32,
        custom_shader_text_opacity: f32,
        custom_shader_full_content: bool,
        // Cursor shader settings (separate from background shader)
        cursor_shader_path: Option<&str>,
        cursor_shader_enabled: bool,
        cursor_shader_animation: bool,
        cursor_shader_animation_speed: f32,
    ) -> Result<Self> {
        let size = window.inner_size();
        let scale_factor = window.scale_factor();

        // Standard DPI for the platform
        // macOS typically uses 72 DPI for points, Windows and most Linux use 96 DPI
        let platform_dpi = if cfg!(target_os = "macos") {
            72.0
        } else {
            96.0
        };

        // Convert font size from points to pixels for cell size calculation, honoring DPI and scale
        let base_font_pixels = font_size * platform_dpi / 72.0;
        let font_size_pixels = (base_font_pixels * scale_factor as f32).max(1.0);

        // Preliminary font lookup to get metrics for accurate cell height
        let font_manager = crate::font_manager::FontManager::new(
            font_family,
            font_family_bold,
            font_family_italic,
            font_family_bold_italic,
            font_ranges,
        )?;

        let (font_ascent, font_descent, font_leading, char_advance) = {
            let primary_font = font_manager.get_font(0).unwrap();
            let metrics = primary_font.metrics(&[]);
            let scale = font_size_pixels / metrics.units_per_em as f32;

            // Get advance width of a standard character ('m' is common for monospace width)
            let glyph_id = primary_font.charmap().map('m');
            let advance = primary_font.glyph_metrics(&[]).advance_width(glyph_id) * scale;

            (
                metrics.ascent * scale,
                metrics.descent * scale,
                metrics.leading * scale,
                advance,
            )
        };

        // Use font metrics for cell height if line_spacing is 1.0
        // Natural line height = ascent + descent + leading
        let natural_line_height = font_ascent + font_descent + font_leading;
        let char_height = (natural_line_height * line_spacing).max(1.0);

        // Calculate available space after padding (padding on all sides)
        let available_width = (size.width as f32 - window_padding * 2.0).max(0.0);
        let available_height = (size.height as f32 - window_padding * 2.0).max(0.0);

        // Calculate terminal dimensions based on font size in pixels and spacing
        let char_width = (char_advance * char_spacing).max(1.0); // Configurable character width
        let cols = (available_width / char_width).max(1.0) as usize;
        let rows = (available_height / char_height).max(1.0) as usize;

        // Create cell renderer with font fallback support (owns scrollbar)
        let cell_renderer = CellRenderer::new(
            window.clone(),
            font_family,
            font_family_bold,
            font_family_italic,
            font_family_bold_italic,
            font_ranges,
            font_size,
            cols,
            rows,
            window_padding,
            line_spacing,
            char_spacing,
            scrollbar_position,
            scrollbar_width,
            scrollbar_thumb_color,
            scrollbar_track_color,
            enable_text_shaping,
            enable_ligatures,
            enable_kerning,
            vsync_mode,
            window_opacity,
            background_color,
            if background_image_enabled {
                background_image_path
            } else {
                None
            },
            background_image_mode,
            background_image_opacity,
        )
        .await?;

        // Create egui renderer for settings UI
        let egui_renderer = egui_wgpu::Renderer::new(
            cell_renderer.device(),
            cell_renderer.surface_format(),
            egui_wgpu::RendererOptions {
                msaa_samples: 1,
                depth_stencil_format: None,
                dithering: false,
                predictable_texture_filtering: false,
            },
        );

        // Create graphics renderer for sixel images
        let graphics_renderer = GraphicsRenderer::new(
            cell_renderer.device(),
            cell_renderer.surface_format(),
            cell_renderer.cell_width(),
            cell_renderer.cell_height(),
            cell_renderer.window_padding(),
        )?;

        // Create custom shader renderer if configured
        let (custom_shader_renderer, initial_shader_path) = if custom_shader_enabled {
            if let Some(shader_path) = custom_shader_path {
                let path = crate::config::Config::shader_path(shader_path);
                match CustomShaderRenderer::new(
                    cell_renderer.device(),
                    cell_renderer.queue(),
                    cell_renderer.surface_format(),
                    &path,
                    size.width,
                    size.height,
                    custom_shader_animation,
                    custom_shader_animation_speed,
                    window_opacity,
                    custom_shader_text_opacity,
                    custom_shader_full_content,
                ) {
                    Ok(mut renderer) => {
                        // Sync cell dimensions for cursor position calculation
                        renderer.update_cell_dimensions(
                            cell_renderer.cell_width(),
                            cell_renderer.cell_height(),
                            window_padding,
                        );
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
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

        // Create cursor shader renderer if configured (separate from background shader)
        let (cursor_shader_renderer, initial_cursor_shader_path) = if cursor_shader_enabled {
            if let Some(shader_path) = cursor_shader_path {
                let path = crate::config::Config::shader_path(shader_path);
                match CustomShaderRenderer::new(
                    cell_renderer.device(),
                    cell_renderer.queue(),
                    cell_renderer.surface_format(),
                    &path,
                    size.width,
                    size.height,
                    cursor_shader_animation,
                    cursor_shader_animation_speed,
                    window_opacity,
                    1.0,  // Text opacity (cursor shader always uses 1.0)
                    true, // Full content mode (cursor shader always uses full content)
                ) {
                    Ok(mut renderer) => {
                        // Sync cell dimensions for cursor position calculation
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
            } else {
                (None, None)
            }
        } else {
            (None, None)
        };

        Ok(Self {
            cell_renderer,
            graphics_renderer,
            sixel_graphics: Vec::new(),
            egui_renderer,
            custom_shader_renderer,
            custom_shader_path: initial_shader_path,
            cursor_shader_renderer,
            cursor_shader_path: initial_cursor_shader_path,
            size,
            dirty: true, // Start dirty to ensure initial render
            debug_text: None,
        })
    }

    /// Resize the renderer and recalculate grid dimensions based on padding/font metrics
    pub fn resize(&mut self, new_size: PhysicalSize<u32>) -> (usize, usize) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.dirty = true; // Mark dirty on resize
            let result = self.cell_renderer.resize(new_size.width, new_size.height);

            // Update graphics renderer cell dimensions
            self.graphics_renderer.update_cell_dimensions(
                self.cell_renderer.cell_width(),
                self.cell_renderer.cell_height(),
                self.cell_renderer.window_padding(),
            );

            // Update custom shader renderer dimensions
            if let Some(ref mut custom_shader) = self.custom_shader_renderer {
                custom_shader.resize(self.cell_renderer.device(), new_size.width, new_size.height);
                // Sync cell dimensions for cursor position calculation
                custom_shader.update_cell_dimensions(
                    self.cell_renderer.cell_width(),
                    self.cell_renderer.cell_height(),
                    self.cell_renderer.window_padding(),
                );
            }

            // Update cursor shader renderer dimensions
            if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
                cursor_shader.resize(self.cell_renderer.device(), new_size.width, new_size.height);
                // Sync cell dimensions for cursor position calculation
                cursor_shader.update_cell_dimensions(
                    self.cell_renderer.cell_width(),
                    self.cell_renderer.cell_height(),
                    self.cell_renderer.window_padding(),
                );
            }

            return result;
        }

        self.cell_renderer.grid_size()
    }

    /// Update scale factor and resize so the PTY grid matches the new DPI.
    pub fn handle_scale_factor_change(
        &mut self,
        scale_factor: f64,
        new_size: PhysicalSize<u32>,
    ) -> (usize, usize) {
        self.cell_renderer.update_scale_factor(scale_factor);
        self.resize(new_size)
    }

    /// Update the terminal cells
    pub fn update_cells(&mut self, cells: &[Cell]) {
        self.cell_renderer.update_cells(cells);
        self.dirty = true; // Mark dirty when cells change
    }

    /// Update cursor position and style for geometric rendering
    pub fn update_cursor(
        &mut self,
        position: (usize, usize),
        opacity: f32,
        style: par_term_emu_core_rust::cursor::CursorStyle,
    ) {
        self.cell_renderer.update_cursor(position, opacity, style);
        self.dirty = true;
    }

    /// Clear cursor (hide it)
    pub fn clear_cursor(&mut self) {
        self.cell_renderer.clear_cursor();
        self.dirty = true;
    }

    /// Update scrollbar state
    ///
    /// # Arguments
    /// * `scroll_offset` - Current scroll offset (0 = at bottom)
    /// * `visible_lines` - Number of lines visible on screen
    /// * `total_lines` - Total number of lines including scrollback
    pub fn update_scrollbar(
        &mut self,
        scroll_offset: usize,
        visible_lines: usize,
        total_lines: usize,
    ) {
        self.cell_renderer
            .update_scrollbar(scroll_offset, visible_lines, total_lines);
        self.dirty = true; // Mark dirty when scrollbar changes
    }

    /// Set the visual bell flash intensity
    ///
    /// # Arguments
    /// * `intensity` - Flash intensity from 0.0 (no flash) to 1.0 (full white flash)
    pub fn set_visual_bell_intensity(&mut self, intensity: f32) {
        self.cell_renderer.set_visual_bell_intensity(intensity);
        if intensity > 0.0 {
            self.dirty = true; // Mark dirty when flash is active
        }
    }

    /// Update window opacity in real-time
    pub fn update_opacity(&mut self, opacity: f32) {
        self.cell_renderer.update_opacity(opacity);

        // Propagate to custom shader renderer if present
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.set_opacity(opacity);
        }

        self.dirty = true;
    }

    /// Update cursor color for cell rendering
    pub fn update_cursor_color(&mut self, color: [u8; 3]) {
        self.cell_renderer.update_cursor_color(color);
        self.dirty = true;
    }

    /// Update window padding in real-time without full renderer rebuild
    /// Returns Some((cols, rows)) if grid size changed and terminal needs resize
    pub fn update_window_padding(&mut self, padding: f32) -> Option<(usize, usize)> {
        let result = self.cell_renderer.update_window_padding(padding);
        self.dirty = true;
        result
    }

    /// Enable/disable background image and reload if needed
    pub fn set_background_image_enabled(
        &mut self,
        enabled: bool,
        path: Option<&str>,
        mode: crate::config::BackgroundImageMode,
        opacity: f32,
    ) {
        let path = if enabled { path } else { None };
        self.cell_renderer.set_background_image(path, mode, opacity);
        self.dirty = true;
    }

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
                    1.0,  // Text opacity (cursor shader always uses 1.0)
                    true, // Full content mode (cursor shader always uses full content)
                ) {
                    Ok(mut renderer) => {
                        // Sync cell dimensions for cursor position calculation
                        renderer.update_cell_dimensions(
                            self.cell_renderer.cell_width(),
                            self.cell_renderer.cell_height(),
                            self.cell_renderer.window_padding(),
                        );
                        log::info!(
                            "Cursor shader enabled at runtime: {}",
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
                        log::error!("{}", error_msg);
                        Err(error_msg)
                    }
                }
            }
            _ => {
                if self.cursor_shader_renderer.is_some() {
                    log::info!("Cursor shader disabled at runtime");
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

    /// Update scrollbar appearance in real-time
    pub fn update_scrollbar_appearance(
        &mut self,
        width: f32,
        thumb_color: [f32; 4],
        track_color: [f32; 4],
    ) {
        self.cell_renderer
            .update_scrollbar_appearance(width, thumb_color, track_color);
        self.dirty = true;
    }

    /// Update scrollbar position (left/right) in real-time
    #[allow(dead_code)]
    pub fn update_scrollbar_position(&mut self, position: &str) {
        self.cell_renderer.update_scrollbar_position(position);
        self.dirty = true;
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
                ) {
                    Ok(mut renderer) => {
                        // Sync cell dimensions for cursor position calculation
                        renderer.update_cell_dimensions(
                            self.cell_renderer.cell_width(),
                            self.cell_renderer.cell_height(),
                            self.cell_renderer.window_padding(),
                        );
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

    /// Update background image opacity in real-time
    pub fn update_background_image_opacity(&mut self, opacity: f32) {
        self.cell_renderer.update_background_image_opacity(opacity);
        self.dirty = true;
    }

    /// Update graphics textures (Sixel, iTerm2, Kitty)
    ///
    /// # Arguments
    /// * `graphics` - Graphics from the terminal with RGBA data
    /// * `view_scroll_offset` - Current view scroll offset (0 = viewing current content)
    /// * `scrollback_len` - Total lines in scrollback buffer
    /// * `visible_rows` - Number of visible rows in terminal
    #[allow(dead_code)]
    pub fn update_graphics(
        &mut self,
        graphics: &[par_term_emu_core_rust::graphics::TerminalGraphic],
        view_scroll_offset: usize,
        scrollback_len: usize,
        visible_rows: usize,
    ) -> Result<()> {
        // Clear old graphics list
        self.sixel_graphics.clear();

        // Calculate the view window in absolute terms
        // total_lines = scrollback_len + visible_rows
        // When scroll_offset = 0, we view lines [scrollback_len, scrollback_len + visible_rows)
        // When scroll_offset > 0, we view earlier lines
        let total_lines = scrollback_len + visible_rows;
        let view_end = total_lines.saturating_sub(view_scroll_offset);
        let view_start = view_end.saturating_sub(visible_rows);

        // Process each graphic
        for graphic in graphics {
            // Use the unique ID from the graphic (stable across position changes)
            let id = graphic.id;
            let (col, row) = graphic.position;

            // Calculate screen row based on whether this is a scrollback graphic or current
            let screen_row: isize = if let Some(sb_row) = graphic.scrollback_row {
                // Scrollback graphic: sb_row is absolute index in scrollback
                // Screen row = sb_row - view_start
                sb_row as isize - view_start as isize
            } else {
                // Current graphic: position is relative to visible area
                // Absolute position = scrollback_len + row - scroll_offset_rows
                // This keeps the graphic at its original absolute position as scrollback grows
                let absolute_row = scrollback_len.saturating_sub(graphic.scroll_offset_rows) + row;

                debug_trace!(
                    "RENDERER",
                    "CALC: scrollback_len={}, row={}, scroll_offset_rows={}, absolute_row={}, view_start={}, screen_row={}",
                    scrollback_len,
                    row,
                    graphic.scroll_offset_rows,
                    absolute_row,
                    view_start,
                    absolute_row as isize - view_start as isize
                );

                absolute_row as isize - view_start as isize
            };

            debug_log!(
                "RENDERER",
                "Graphics update: id={}, protocol={:?}, pos=({},{}), screen_row={}, scrollback_row={:?}, scroll_offset_rows={}, size={}x{}, view=[{},{})",
                id,
                graphic.protocol,
                col,
                row,
                screen_row,
                graphic.scrollback_row,
                graphic.scroll_offset_rows,
                graphic.width,
                graphic.height,
                view_start,
                view_end
            );

            // Create or update texture in cache
            self.graphics_renderer.get_or_create_texture(
                self.cell_renderer.device(),
                self.cell_renderer.queue(),
                id,
                &graphic.pixels, // RGBA pixel data (Arc<Vec<u8>>)
                graphic.width as u32,
                graphic.height as u32,
            )?;

            // Add to render list with position and dimensions
            // Calculate size in cells (rounding up to cover all affected cells)
            let width_cells =
                ((graphic.width as f32 / self.cell_renderer.cell_width()).ceil() as usize).max(1);
            let height_cells =
                ((graphic.height as f32 / self.cell_renderer.cell_height()).ceil() as usize).max(1);

            // Calculate effective clip rows based on screen position
            // If screen_row < 0, we need to clip that many rows from the top
            // If screen_row >= 0, no clipping needed (we can see the full graphic)
            let effective_clip_rows = if screen_row < 0 {
                (-screen_row) as usize
            } else {
                0
            };

            self.sixel_graphics.push((
                id,
                screen_row, // row position (can be negative if scrolled off top)
                col,        // col position
                width_cells,
                height_cells,
                1.0,                 // Full opacity by default
                effective_clip_rows, // Rows to clip from top for partial rendering
            ));
        }

        if !graphics.is_empty() {
            self.dirty = true; // Mark dirty when graphics change
        }

        Ok(())
    }

    /// Check if animation requires continuous rendering
    ///
    /// Returns true if shader animation is enabled or a cursor trail animation
    /// might still be in progress.
    pub fn needs_continuous_render(&self) -> bool {
        let custom_needs = self
            .custom_shader_renderer
            .as_ref()
            .is_some_and(|r| r.animation_enabled() || r.cursor_needs_animation());
        let cursor_needs = self
            .cursor_shader_renderer
            .as_ref()
            .is_some_and(|r| r.animation_enabled() || r.cursor_needs_animation());
        custom_needs || cursor_needs
    }

    /// Render a frame with optional egui overlay
    /// Returns true if rendering was performed, false if skipped
    pub fn render(
        &mut self,
        egui_data: Option<(egui::FullOutput, &egui::Context)>,
        force_egui_opaque: bool,
        show_scrollbar: bool,
    ) -> Result<bool> {
        // Custom shader animation forces continuous rendering
        let force_render = self.needs_continuous_render();

        if !self.dirty && egui_data.is_none() && !force_render {
            // Skip rendering if nothing has changed
            return Ok(false);
        }

        // Check if shaders are enabled
        let has_custom_shader = self.custom_shader_renderer.is_some();
        let has_cursor_shader = self.cursor_shader_renderer.is_some();

        // Cell renderer renders terminal content
        let t1 = std::time::Instant::now();
        let surface_texture = if has_custom_shader {
            // Render terminal to intermediate texture for background shader
            self.cell_renderer.render_to_texture(
                self.custom_shader_renderer
                    .as_ref()
                    .unwrap()
                    .intermediate_texture_view(),
            )?
        } else if has_cursor_shader {
            // Render terminal to intermediate texture for cursor shader
            self.cell_renderer.render_to_texture(
                self.cursor_shader_renderer
                    .as_ref()
                    .unwrap()
                    .intermediate_texture_view(),
            )?
        } else {
            // Render directly to surface
            self.cell_renderer.render(show_scrollbar)?
        };
        let cell_render_time = t1.elapsed();

        // Apply background custom shader if enabled
        let t_custom = std::time::Instant::now();
        let custom_shader_time = if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            if has_cursor_shader {
                // Background shader renders to cursor shader's intermediate texture
                custom_shader.render(
                    self.cell_renderer.device(),
                    self.cell_renderer.queue(),
                    self.cursor_shader_renderer
                        .as_ref()
                        .unwrap()
                        .intermediate_texture_view(),
                )?;
            } else {
                // Background shader renders directly to surface
                let surface_view = surface_texture
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                custom_shader.render(
                    self.cell_renderer.device(),
                    self.cell_renderer.queue(),
                    &surface_view,
                )?;

                // Render overlays (scrollbar, visual bell) on top after shader
                self.cell_renderer
                    .render_overlays(&surface_texture, show_scrollbar)?;
            }
            t_custom.elapsed()
        } else {
            std::time::Duration::ZERO
        };

        // Apply cursor shader if enabled
        let t_cursor = std::time::Instant::now();
        let cursor_shader_time = if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            log::trace!("Rendering cursor shader");
            let surface_view = surface_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            cursor_shader.render(
                self.cell_renderer.device(),
                self.cell_renderer.queue(),
                &surface_view,
            )?;

            // Render overlays (scrollbar, visual bell) on top after cursor shader
            self.cell_renderer
                .render_overlays(&surface_texture, show_scrollbar)?;
            t_cursor.elapsed()
        } else {
            std::time::Duration::ZERO
        };

        // Render sixel graphics on top of cells
        let t2 = std::time::Instant::now();
        if !self.sixel_graphics.is_empty() {
            self.render_sixel_graphics(&surface_texture)?;
        }
        let sixel_render_time = t2.elapsed();

        // Render egui overlay if provided
        let t3 = std::time::Instant::now();
        if let Some((egui_output, egui_ctx)) = egui_data {
            self.render_egui(&surface_texture, egui_output, egui_ctx, force_egui_opaque)?;
        }
        let egui_render_time = t3.elapsed();

        // Present the surface texture - THIS IS WHERE VSYNC WAIT HAPPENS
        let t4 = std::time::Instant::now();
        surface_texture.present();
        let present_time = t4.elapsed();

        // Log timing breakdown
        let total = cell_render_time
            + custom_shader_time
            + cursor_shader_time
            + sixel_render_time
            + egui_render_time
            + present_time;
        if present_time.as_millis() > 10 || total.as_millis() > 10 {
            log::info!(
                "RENDER_BREAKDOWN: CellRender={:.2}ms BgShader={:.2}ms CursorShader={:.2}ms Sixel={:.2}ms Egui={:.2}ms PRESENT={:.2}ms Total={:.2}ms",
                cell_render_time.as_secs_f64() * 1000.0,
                custom_shader_time.as_secs_f64() * 1000.0,
                cursor_shader_time.as_secs_f64() * 1000.0,
                sixel_render_time.as_secs_f64() * 1000.0,
                egui_render_time.as_secs_f64() * 1000.0,
                present_time.as_secs_f64() * 1000.0,
                total.as_secs_f64() * 1000.0
            );
        }

        // Clear dirty flag after successful render
        self.dirty = false;

        Ok(true)
    }

    /// Render sixel graphics on top of terminal cells
    fn render_sixel_graphics(&mut self, surface_texture: &wgpu::SurfaceTexture) -> Result<()> {
        use wgpu::TextureViewDescriptor;

        // Create view of the surface texture
        let view = surface_texture
            .texture
            .create_view(&TextureViewDescriptor::default());

        // Create command encoder for sixel rendering
        let mut encoder =
            self.cell_renderer
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("sixel encoder"),
                });

        // Create render pass
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("sixel render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Don't clear - render on top of terminal
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Render all sixel graphics
            self.graphics_renderer.render(
                self.cell_renderer.device(),
                self.cell_renderer.queue(),
                &mut render_pass,
                &self.sixel_graphics,
                self.size.width as f32,
                self.size.height as f32,
            )?;
        } // render_pass dropped here

        // Submit sixel commands
        self.cell_renderer
            .queue()
            .submit(std::iter::once(encoder.finish()));

        Ok(())
    }

    /// Render egui overlay on top of the terminal
    fn render_egui(
        &mut self,
        surface_texture: &wgpu::SurfaceTexture,
        egui_output: egui::FullOutput,
        egui_ctx: &egui::Context,
        force_opaque: bool,
    ) -> Result<()> {
        use wgpu::TextureViewDescriptor;

        // Create view of the surface texture
        let view = surface_texture
            .texture
            .create_view(&TextureViewDescriptor::default());

        // Create command encoder for egui
        let mut encoder =
            self.cell_renderer
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("egui encoder"),
                });

        // Convert egui output to screen descriptor
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.size.width, self.size.height],
            pixels_per_point: egui_output.pixels_per_point,
        };

        // Update egui textures
        for (id, image_delta) in &egui_output.textures_delta.set {
            self.egui_renderer.update_texture(
                self.cell_renderer.device(),
                self.cell_renderer.queue(),
                *id,
                image_delta,
            );
        }

        // Tessellate egui shapes into paint jobs
        let mut paint_jobs = egui_ctx.tessellate(egui_output.shapes, egui_output.pixels_per_point);

        // If requested, force all egui vertices to full opacity so UI stays solid
        if force_opaque {
            for job in paint_jobs.iter_mut() {
                match &mut job.primitive {
                    egui::epaint::Primitive::Mesh(mesh) => {
                        for v in mesh.vertices.iter_mut() {
                            v.color[3] = 255;
                        }
                    }
                    egui::epaint::Primitive::Callback(_) => {}
                }
            }
        }

        // Update egui buffers
        self.egui_renderer.update_buffers(
            self.cell_renderer.device(),
            self.cell_renderer.queue(),
            &mut encoder,
            &paint_jobs,
            &screen_descriptor,
        );

        // Render egui on top of the terminal content
        {
            let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Don't clear - render on top of terminal
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Convert to 'static lifetime as required by egui_renderer.render()
            let mut render_pass = render_pass.forget_lifetime();

            self.egui_renderer
                .render(&mut render_pass, &paint_jobs, &screen_descriptor);
        } // render_pass dropped here

        // Submit egui commands
        self.cell_renderer
            .queue()
            .submit(std::iter::once(encoder.finish()));

        // Free egui textures
        for id in &egui_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }

        Ok(())
    }

    /// Get the current size
    pub fn size(&self) -> PhysicalSize<u32> {
        self.size
    }

    /// Get the current grid dimensions (columns, rows)
    pub fn grid_size(&self) -> (usize, usize) {
        self.cell_renderer.grid_size()
    }

    /// Get cell width in pixels
    pub fn cell_width(&self) -> f32 {
        self.cell_renderer.cell_width()
    }

    /// Get cell height in pixels
    pub fn cell_height(&self) -> f32 {
        self.cell_renderer.cell_height()
    }

    /// Get window padding in pixels
    pub fn window_padding(&self) -> f32 {
        self.cell_renderer.window_padding()
    }

    /// Check if a point (in pixel coordinates) is within the scrollbar bounds
    ///
    /// # Arguments
    /// * `x` - X coordinate in pixels (from left edge)
    /// * `y` - Y coordinate in pixels (from top edge)
    pub fn scrollbar_contains_point(&self, x: f32, y: f32) -> bool {
        self.cell_renderer.scrollbar_contains_point(x, y)
    }

    /// Get the scrollbar thumb bounds (top Y, height) in pixels
    pub fn scrollbar_thumb_bounds(&self) -> Option<(f32, f32)> {
        self.cell_renderer.scrollbar_thumb_bounds()
    }

    /// Check if an X coordinate is within the scrollbar track
    pub fn scrollbar_track_contains_x(&self, x: f32) -> bool {
        self.cell_renderer.scrollbar_track_contains_x(x)
    }

    /// Convert a mouse Y position to a scroll offset
    ///
    /// # Arguments
    /// * `mouse_y` - Mouse Y coordinate in pixels (from top edge)
    ///
    /// # Returns
    /// The scroll offset corresponding to the mouse position, or None if scrollbar is not visible
    pub fn scrollbar_mouse_y_to_scroll_offset(&self, mouse_y: f32) -> Option<usize> {
        self.cell_renderer
            .scrollbar_mouse_y_to_scroll_offset(mouse_y)
    }

    /// Check if the renderer needs to be redrawn
    #[allow(dead_code)]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Mark the renderer as dirty, forcing a redraw on next render call
    #[allow(dead_code)]
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Clear all cached sixel textures
    #[allow(dead_code)]
    pub fn clear_sixel_cache(&mut self) {
        self.graphics_renderer.clear_cache();
        self.sixel_graphics.clear();
        self.dirty = true;
    }

    /// Get the number of cached sixel textures
    #[allow(dead_code)]
    pub fn sixel_cache_size(&self) -> usize {
        self.graphics_renderer.cache_size()
    }

    /// Remove a specific sixel texture from cache
    #[allow(dead_code)]
    pub fn remove_sixel_texture(&mut self, id: u64) {
        self.graphics_renderer.remove_texture(id);
        self.sixel_graphics
            .retain(|(gid, _, _, _, _, _, _)| *gid != id);
        self.dirty = true;
    }

    /// Set debug overlay text to be rendered
    #[allow(dead_code)]
    #[allow(dead_code)]
    pub fn render_debug_overlay(&mut self, text: &str) {
        self.debug_text = Some(text.to_string());
        self.dirty = true; // Mark dirty to ensure debug overlay renders
    }

    /// Reconfigure the surface (call when surface becomes outdated or lost)
    /// This typically happens when dragging the window between displays
    pub fn reconfigure_surface(&mut self) {
        self.cell_renderer.reconfigure_surface();
        self.dirty = true;
    }

    /// Clear the glyph cache to force re-rasterization
    /// Useful after display changes where font rendering may differ
    pub fn clear_glyph_cache(&mut self) {
        self.cell_renderer.clear_glyph_cache();
        self.dirty = true;
    }
}
