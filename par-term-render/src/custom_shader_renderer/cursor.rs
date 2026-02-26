//! Cursor tracking methods for Ghostty-compatible shader effects.
//!
//! This module provides cursor position tracking and style-based dimension
//! calculations for shader-based cursor animations like trails and glows.

use par_term_config::color_u8_to_f32_a;
use par_term_emu_core_rust::cursor::CursorStyle;

use super::CustomShaderRenderer;

impl CustomShaderRenderer {
    /// Update cursor position and appearance for shader effects
    ///
    /// This method tracks cursor movement and records the time of change,
    /// enabling Ghostty-compatible cursor trail effects and animations.
    ///
    /// # Arguments
    /// * `col` - Cursor column position (0-based)
    /// * `row` - Cursor row position (0-based)
    /// * `opacity` - Cursor opacity (0.0 = invisible, 1.0 = fully visible)
    /// * `cursor_color` - Cursor RGBA color
    /// * `style` - Cursor style (Block, Beam, Underline)
    pub fn update_cursor(
        &mut self,
        col: usize,
        row: usize,
        opacity: f32,
        cursor_color: [f32; 4],
        style: CursorStyle,
    ) {
        let new_pos = (col, row);
        let style_changed = style != self.current_cursor_style;
        let pos_changed = new_pos != self.current_cursor_pos;

        if pos_changed || style_changed {
            // Store previous state before updating
            self.previous_cursor_pos = self.current_cursor_pos;
            self.previous_cursor_opacity = self.current_cursor_opacity;
            self.previous_cursor_color = self.current_cursor_color;
            self.previous_cursor_style = self.current_cursor_style;
            self.current_cursor_pos = new_pos;
            self.current_cursor_style = style;

            // Record time of change (same timebase as iTime)
            self.cursor_change_time = if self.animation_enabled {
                self.start_time.elapsed().as_secs_f32() * self.animation_speed.max(0.0)
            } else {
                0.0
            };

            if pos_changed {
                log::trace!(
                    "Cursor moved: ({}, {}) -> ({}, {}), change_time={:.3}",
                    self.previous_cursor_pos.0,
                    self.previous_cursor_pos.1,
                    col,
                    row,
                    self.cursor_change_time
                );
            }
        }
        self.current_cursor_opacity = opacity;
        self.current_cursor_color = cursor_color;
    }

    /// Update cell dimensions for cursor pixel position calculation
    ///
    /// # Arguments
    /// * `cell_width` - Cell width in pixels
    /// * `cell_height` - Cell height in pixels
    /// * `padding` - Window padding in pixels
    pub fn update_cell_dimensions(&mut self, cell_width: f32, cell_height: f32, padding: f32) {
        self.cursor_cell_width = cell_width;
        self.cursor_cell_height = cell_height;
        self.cursor_window_padding = padding;
    }

    /// Set vertical content offset (e.g., tab bar height)
    pub fn set_content_offset_y(&mut self, offset: f32) {
        self.cursor_content_offset_y = offset;
    }

    /// Set horizontal content offset (e.g., tab bar on left)
    pub fn set_content_offset_x(&mut self, offset: f32) {
        self.cursor_content_offset_x = offset;
    }

    /// Set display scale factor for DPI-aware cursor sizing
    pub fn set_scale_factor(&mut self, scale_factor: f32) {
        self.scale_factor = scale_factor;
    }

    /// Convert cursor cell coordinates to pixel coordinates
    ///
    /// Returns (x, y) in pixels from top-left corner of the window.
    pub(super) fn cursor_to_pixels(&self, col: usize, row: usize) -> (f32, f32) {
        let x = self.cursor_window_padding
            + self.cursor_content_offset_x
            + (col as f32 * self.cursor_cell_width);
        let y = self.cursor_window_padding
            + self.cursor_content_offset_y
            + (row as f32 * self.cursor_cell_height);
        (x, y)
    }

    /// Get cursor width in pixels based on cursor style.
    /// Returns physical pixels (cell dimensions are already in physical pixels).
    pub(super) fn cursor_width_for_style(&self, style: CursorStyle, scale_factor: f32) -> f32 {
        match style {
            // Block cursor: full cell width
            CursorStyle::SteadyBlock | CursorStyle::BlinkingBlock => self.cursor_cell_width,
            // Beam/Bar cursor: thin vertical line (2 logical pixels, scaled)
            CursorStyle::SteadyBar | CursorStyle::BlinkingBar => 2.0 * scale_factor,
            // Underline cursor: full cell width
            CursorStyle::SteadyUnderline | CursorStyle::BlinkingUnderline => self.cursor_cell_width,
        }
    }

    /// Get cursor height in pixels based on cursor style.
    /// Returns physical pixels (cell dimensions are already in physical pixels).
    pub(super) fn cursor_height_for_style(&self, style: CursorStyle, scale_factor: f32) -> f32 {
        match style {
            // Block cursor: full cell height
            CursorStyle::SteadyBlock | CursorStyle::BlinkingBlock => self.cursor_cell_height,
            // Beam/Bar cursor: full cell height
            CursorStyle::SteadyBar | CursorStyle::BlinkingBar => self.cursor_cell_height,
            // Underline cursor: thin horizontal line (2 logical pixels, scaled)
            CursorStyle::SteadyUnderline | CursorStyle::BlinkingUnderline => 2.0 * scale_factor,
        }
    }

    /// Check if cursor animation might need continuous rendering
    ///
    /// Returns true if a cursor trail animation is likely still in progress
    /// (within 1 second of the last cursor movement).
    pub fn cursor_needs_animation(&self) -> bool {
        if self.animation_enabled {
            let current_time =
                self.start_time.elapsed().as_secs_f32() * self.animation_speed.max(0.0);
            // Allow 1 second for cursor trail animations to complete
            (current_time - self.cursor_change_time) < 1.0
        } else {
            false
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
        self.cursor_shader_color = color_u8_to_f32_a(color, 1.0);
        self.cursor_trail_duration = trail_duration.max(0.0);
        self.cursor_glow_radius = glow_radius.max(0.0);
        self.cursor_glow_intensity = glow_intensity.clamp(0.0, 1.0);
    }
}
