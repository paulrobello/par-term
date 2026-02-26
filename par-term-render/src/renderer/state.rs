use crate::cell_renderer::Cell;
use anyhow::Result;
use par_term_config::SeparatorMark;
use par_term_config::color_u8_to_f32;

use super::Renderer;

impl Renderer {
    pub fn update_cells(&mut self, cells: &[Cell]) {
        if self.cell_renderer.update_cells(cells) {
            self.dirty = true;
        }
    }

    /// Clear all cells in the renderer.
    /// Call this when switching tabs to ensure a clean slate.
    pub fn clear_all_cells(&mut self) {
        self.cell_renderer.clear_all_cells();
        self.dirty = true;
    }

    /// Update cursor position and style for geometric rendering
    pub fn update_cursor(
        &mut self,
        position: (usize, usize),
        opacity: f32,
        style: par_term_emu_core_rust::cursor::CursorStyle,
    ) {
        if self.cell_renderer.update_cursor(position, opacity, style) {
            self.dirty = true;
        }
    }

    /// Clear cursor (hide it)
    pub fn clear_cursor(&mut self) {
        if self.cell_renderer.clear_cursor() {
            self.dirty = true;
        }
    }

    /// Update scrollbar state
    ///
    /// # Arguments
    /// * `scroll_offset` - Current scroll offset (0 = at bottom)
    /// * `visible_lines` - Number of lines visible on screen
    /// * `total_lines` - Total number of lines including scrollback
    /// * `marks` - Scrollback marks for visualization on the scrollbar
    pub fn update_scrollbar(
        &mut self,
        scroll_offset: usize,
        visible_lines: usize,
        total_lines: usize,
        marks: &[par_term_config::ScrollbackMark],
    ) {
        let new_state = (scroll_offset, visible_lines, total_lines);
        if new_state == self.last_scrollbar_state {
            return;
        }
        self.last_scrollbar_state = new_state;
        self.cell_renderer
            .update_scrollbar(scroll_offset, visible_lines, total_lines, marks);
        self.dirty = true;
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

        // Propagate to cursor shader renderer if present
        if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            cursor_shader.set_opacity(opacity);
        }

        self.dirty = true;
    }

    /// Update cursor color for cell rendering
    pub fn update_cursor_color(&mut self, color: [u8; 3]) {
        self.cell_renderer.update_cursor_color(color);
        self.dirty = true;
    }

    /// Update cursor text color (color of text under block cursor)
    pub fn update_cursor_text_color(&mut self, color: Option<[u8; 3]>) {
        self.cell_renderer.update_cursor_text_color(color);
        self.dirty = true;
    }

    /// Set whether cursor should be hidden when cursor shader is active
    pub fn set_cursor_hidden_for_shader(&mut self, hidden: bool) {
        if self.cell_renderer.set_cursor_hidden_for_shader(hidden) {
            self.dirty = true;
        }
    }

    /// Set window focus state (affects unfocused cursor rendering)
    pub fn set_focused(&mut self, focused: bool) {
        if self.cell_renderer.set_focused(focused) {
            self.dirty = true;
        }
    }

    /// Update cursor guide settings
    pub fn update_cursor_guide(&mut self, enabled: bool, color: [u8; 4]) {
        self.cell_renderer.update_cursor_guide(enabled, color);
        self.dirty = true;
    }

    /// Update cursor shadow settings.
    /// Offset and blur are in logical pixels and will be scaled to physical pixels internally.
    pub fn update_cursor_shadow(
        &mut self,
        enabled: bool,
        color: [u8; 4],
        offset: [f32; 2],
        blur: f32,
    ) {
        let scale = self.cell_renderer.scale_factor;
        let physical_offset = [offset[0] * scale, offset[1] * scale];
        let physical_blur = blur * scale;
        self.cell_renderer
            .update_cursor_shadow(enabled, color, physical_offset, physical_blur);
        self.dirty = true;
    }

    /// Update cursor boost settings
    pub fn update_cursor_boost(&mut self, intensity: f32, color: [u8; 3]) {
        self.cell_renderer.update_cursor_boost(intensity, color);
        self.dirty = true;
    }

    /// Update unfocused cursor style
    pub fn update_unfocused_cursor_style(&mut self, style: par_term_config::UnfocusedCursorStyle) {
        self.cell_renderer.update_unfocused_cursor_style(style);
        self.dirty = true;
    }

    /// Update command separator settings from config.
    /// Thickness is in logical pixels and will be scaled to physical pixels internally.
    pub fn update_command_separator(
        &mut self,
        enabled: bool,
        logical_thickness: f32,
        opacity: f32,
        exit_color: bool,
        color: [u8; 3],
    ) {
        let physical_thickness = logical_thickness * self.cell_renderer.scale_factor;
        self.cell_renderer.update_command_separator(
            enabled,
            physical_thickness,
            opacity,
            exit_color,
            color,
        );
        self.dirty = true;
    }

    /// Set the visible separator marks for the current frame (single-pane path)
    pub fn set_separator_marks(&mut self, marks: Vec<SeparatorMark>) {
        if self.cell_renderer.set_separator_marks(marks) {
            self.dirty = true;
        }
    }

    /// Set gutter indicator data for the current frame (single-pane path).
    pub fn set_gutter_indicators(&mut self, indicators: Vec<(usize, [f32; 4])>) {
        self.cell_renderer.set_gutter_indicators(indicators);
        self.dirty = true;
    }

    /// Set whether transparency affects only default background cells.
    /// When true, non-default (colored) backgrounds remain opaque for readability.
    pub fn set_transparency_affects_only_default_background(&mut self, value: bool) {
        self.cell_renderer
            .set_transparency_affects_only_default_background(value);
        self.dirty = true;
    }

    /// Set whether text should always be rendered at full opacity.
    /// When true, text remains opaque regardless of window transparency settings.
    pub fn set_keep_text_opaque(&mut self, value: bool) {
        self.cell_renderer.set_keep_text_opaque(value);

        // Also propagate to custom shader renderer if present
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.set_keep_text_opaque(value);
        }

        // And to cursor shader renderer if present
        if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            cursor_shader.set_keep_text_opaque(value);
        }

        self.dirty = true;
    }

    pub fn set_link_underline_style(&mut self, style: par_term_config::LinkUnderlineStyle) {
        self.cell_renderer.set_link_underline_style(style);
        self.dirty = true;
    }

    /// Set whether cursor shader should be disabled due to alt screen being active
    ///
    /// When alt screen is active (e.g., vim, htop, less), cursor shader effects
    /// are disabled since TUI applications typically have their own cursor handling.
    pub fn set_cursor_shader_disabled_for_alt_screen(&mut self, disabled: bool) {
        if self.cursor_shader_disabled_for_alt_screen != disabled {
            log::debug!("[cursor-shader] Alt-screen disable set to {}", disabled);
            self.cursor_shader_disabled_for_alt_screen = disabled;
        } else {
            self.cursor_shader_disabled_for_alt_screen = disabled;
        }
    }

    /// Update window padding in real-time without full renderer rebuild.
    /// Accepts logical pixels (from config); scales to physical pixels internally.
    /// Returns Some((cols, rows)) if grid size changed and terminal needs resize.
    pub fn update_window_padding(&mut self, logical_padding: f32) -> Option<(usize, usize)> {
        let physical_padding = logical_padding * self.cell_renderer.scale_factor;
        let result = self.cell_renderer.update_window_padding(physical_padding);
        // Update graphics renderer padding
        self.graphics_renderer.update_cell_dimensions(
            self.cell_renderer.cell_width(),
            self.cell_renderer.cell_height(),
            physical_padding,
        );
        // Update custom shader renderer padding
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.update_cell_dimensions(
                self.cell_renderer.cell_width(),
                self.cell_renderer.cell_height(),
                physical_padding,
            );
        }
        // Update cursor shader renderer padding
        if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            cursor_shader.update_cell_dimensions(
                self.cell_renderer.cell_width(),
                self.cell_renderer.cell_height(),
                physical_padding,
            );
        }
        self.dirty = true;
        result
    }

    /// Enable/disable background image and reload if needed
    pub fn set_background_image_enabled(
        &mut self,
        enabled: bool,
        path: Option<&str>,
        mode: par_term_config::BackgroundImageMode,
        opacity: f32,
    ) {
        let path = if enabled { path } else { None };
        self.cell_renderer.set_background_image(path, mode, opacity);

        // Sync background texture to custom shader if it's using background as channel0
        self.sync_background_texture_to_shader();

        self.dirty = true;
    }

    /// Set background based on mode (Default, Color, or Image).
    ///
    /// This unified method handles all background types and syncs with shaders.
    pub fn set_background(
        &mut self,
        mode: par_term_config::BackgroundMode,
        color: [u8; 3],
        image_path: Option<&str>,
        image_mode: par_term_config::BackgroundImageMode,
        image_opacity: f32,
        image_enabled: bool,
    ) {
        self.cell_renderer.set_background(
            mode,
            color,
            image_path,
            image_mode,
            image_opacity,
            image_enabled,
        );

        // Sync background texture to custom shader if it's using background as channel0
        self.sync_background_texture_to_shader();

        // Sync background to shaders for proper compositing
        let is_solid_color = matches!(mode, par_term_config::BackgroundMode::Color);
        let is_image_mode = matches!(mode, par_term_config::BackgroundMode::Image);
        let normalized_color = color_u8_to_f32(color);

        // Sync to cursor shader
        if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            // When background shader is enabled and chained into cursor shader,
            // don't give cursor shader its own background - background shader handles it
            let has_background_shader = self.custom_shader_renderer.is_some();

            if has_background_shader {
                // Background shader handles the background, cursor shader just passes through
                cursor_shader.set_background_color([0.0, 0.0, 0.0], false);
                cursor_shader.set_background_texture(self.cell_renderer.device(), None);
                cursor_shader.update_use_background_as_channel0(self.cell_renderer.device(), false);
            } else {
                cursor_shader.set_background_color(normalized_color, is_solid_color);

                // For image mode, pass background image as iChannel0
                if is_image_mode && image_enabled {
                    let bg_texture = self.cell_renderer.get_background_as_channel_texture();
                    cursor_shader.set_background_texture(self.cell_renderer.device(), bg_texture);
                    cursor_shader
                        .update_use_background_as_channel0(self.cell_renderer.device(), true);
                } else {
                    // Clear background texture when not in image mode
                    cursor_shader.set_background_texture(self.cell_renderer.device(), None);
                    cursor_shader
                        .update_use_background_as_channel0(self.cell_renderer.device(), false);
                }
            }
        }

        // Sync to custom shader
        // Note: We don't pass is_solid_color=true to custom shaders because
        // that would replace the shader output with a solid color, making the
        // shader invisible. Custom shaders handle their own background.
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.set_background_color(normalized_color, false);
        }

        self.dirty = true;
    }

    /// Update scrollbar appearance in real-time.
    /// Width is in logical pixels and will be scaled to physical pixels internally.
    pub fn update_scrollbar_appearance(
        &mut self,
        logical_width: f32,
        thumb_color: [f32; 4],
        track_color: [f32; 4],
    ) {
        let physical_width = logical_width * self.cell_renderer.scale_factor;
        self.cell_renderer
            .update_scrollbar_appearance(physical_width, thumb_color, track_color);
        self.dirty = true;
    }

    /// Update scrollbar position (left/right) in real-time
    pub fn update_scrollbar_position(&mut self, position: &str) {
        self.cell_renderer.update_scrollbar_position(position);
        self.dirty = true;
    }

    /// Update background image opacity in real-time
    pub fn update_background_image_opacity(&mut self, opacity: f32) {
        self.cell_renderer.update_background_image_opacity(opacity);
        self.dirty = true;
    }

    /// Load a per-pane background image into the texture cache.
    /// Delegates to CellRenderer::load_pane_background.
    pub fn load_pane_background(&mut self, path: &str) -> Result<bool, crate::error::RenderError> {
        self.cell_renderer.load_pane_background(path)
    }

    /// Update inline image scaling mode (nearest vs linear filtering).
    ///
    /// Recreates the GPU sampler and clears the texture cache so images
    /// are re-rendered with the new filter mode.
    pub fn update_image_scaling_mode(&mut self, scaling_mode: par_term_config::ImageScalingMode) {
        self.graphics_renderer
            .update_scaling_mode(self.cell_renderer.device(), scaling_mode);
        self.dirty = true;
    }

    /// Update whether inline images preserve their aspect ratio.
    pub fn update_image_preserve_aspect_ratio(&mut self, preserve: bool) {
        self.graphics_renderer.set_preserve_aspect_ratio(preserve);
        self.dirty = true;
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

}
