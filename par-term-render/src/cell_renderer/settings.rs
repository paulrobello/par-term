use par_term_config::{SeparatorMark, color_tuple_to_f32_a, color_u8_to_f32};

use super::{CellRenderer, PaneViewport};

impl CellRenderer {
    pub fn update_scrollbar(
        &mut self,
        scroll_offset: usize,
        visible_lines: usize,
        total_lines: usize,
        marks: &[par_term_config::ScrollbackMark],
    ) {
        let right_inset = self.grid.content_inset_right + self.grid.egui_right_inset;
        self.scrollbar.update(
            &self.queue,
            crate::scrollbar::ScrollbarUpdateParams {
                scroll_offset,
                visible_lines,
                total_lines,
                window_width: self.config.width,
                window_height: self.config.height,
                content_offset_y: self.grid.content_offset_y,
                content_inset_bottom: self.grid.content_inset_bottom + self.grid.egui_bottom_inset,
                content_inset_right: right_inset,
                marks,
            },
        );
    }

    /// Update scrollbar state constrained to a specific pane's bounds.
    ///
    /// Converts the pane viewport (pixel bounds) into the inset parameters
    /// that `Scrollbar::update` expects, so the track and thumb are confined
    /// to the pane instead of spanning the full window.
    pub fn update_scrollbar_for_pane(
        &mut self,
        scroll_offset: usize,
        visible_lines: usize,
        total_lines: usize,
        marks: &[par_term_config::ScrollbackMark],
        viewport: &PaneViewport,
    ) {
        let win_w = self.config.width as f32;
        let win_h = self.config.height as f32;

        // Top inset: space above the pane
        let pane_content_offset_y = viewport.y;

        // Bottom inset: space below the pane + existing egui bottom inset
        let pane_bottom_inset =
            (win_h - (viewport.y + viewport.height)).max(0.0) + self.grid.egui_bottom_inset;

        // Right inset: space to the right of the pane + existing egui/panel right inset
        let pane_right_inset = (win_w - (viewport.x + viewport.width)).max(0.0)
            + self.grid.content_inset_right
            + self.grid.egui_right_inset;

        self.scrollbar.update(
            &self.queue,
            crate::scrollbar::ScrollbarUpdateParams {
                scroll_offset,
                visible_lines,
                total_lines,
                window_width: self.config.width,
                window_height: self.config.height,
                content_offset_y: pane_content_offset_y,
                content_inset_bottom: pane_bottom_inset,
                content_inset_right: pane_right_inset,
                marks,
            },
        );
    }

    pub fn set_visual_bell_intensity(&mut self, intensity: f32) {
        self.visual_bell_intensity = intensity;
    }

    pub fn update_opacity(&mut self, opacity: f32) {
        self.window_opacity = opacity;
        // update_bg_image_uniforms() multiplies bg_image_opacity by window_opacity,
        // so both images and solid colors respect window transparency
        self.update_bg_image_uniforms(None);
    }

    /// Set whether transparency affects only default background cells.
    /// When true, non-default (colored) backgrounds remain opaque for readability.
    pub fn set_transparency_affects_only_default_background(&mut self, value: bool) {
        if self.transparency_affects_only_default_background != value {
            log::info!(
                "transparency_affects_only_default_background: {} -> {} (window_opacity={})",
                self.transparency_affects_only_default_background,
                value,
                self.window_opacity
            );
            self.transparency_affects_only_default_background = value;
            // Mark all rows dirty to re-render with new transparency behavior
            self.dirty_rows.fill(true);
        }
    }

    /// Set whether text should always be rendered at full opacity.
    /// When true, text remains opaque regardless of window transparency settings.
    pub fn set_keep_text_opaque(&mut self, value: bool) {
        if self.keep_text_opaque != value {
            log::info!(
                "keep_text_opaque: {} -> {} (window_opacity={}, transparency_affects_only_default_bg={})",
                self.keep_text_opaque,
                value,
                self.window_opacity,
                self.transparency_affects_only_default_background
            );
            self.keep_text_opaque = value;
            // Mark all rows dirty to re-render with new text opacity behavior
            self.dirty_rows.fill(true);
        }
    }

    pub fn set_link_underline_style(&mut self, style: par_term_config::LinkUnderlineStyle) {
        if self.link_underline_style != style {
            self.link_underline_style = style;
            self.dirty_rows.fill(true);
        }
    }

    /// Update command separator settings from config
    pub fn update_command_separator(
        &mut self,
        enabled: bool,
        thickness: f32,
        opacity: f32,
        exit_color: bool,
        color: [u8; 3],
    ) {
        self.separator.enabled = enabled;
        self.separator.thickness = thickness;
        self.separator.opacity = opacity;
        self.separator.exit_color = exit_color;
        self.separator.color = color_u8_to_f32(color);
    }

    /// Set the visible separator marks for the current frame.
    /// Returns `true` if the marks changed.
    pub fn set_separator_marks(&mut self, marks: Vec<SeparatorMark>) -> bool {
        if self.separator.visible_marks != marks {
            self.separator.visible_marks = marks;
            return true;
        }
        false
    }

    /// Set the gutter indicator data for the current frame.
    ///
    /// Each entry is `(screen_row, [r, g, b, a])` for the gutter background.
    pub fn set_gutter_indicators(&mut self, indicators: Vec<(usize, [f32; 4])>) {
        self.gutter_indicators = indicators;
    }

    /// Compute separator color based on exit code and settings
    pub(crate) fn separator_color(
        &self,
        exit_code: Option<i32>,
        custom_color: Option<(u8, u8, u8)>,
        opacity_mult: f32,
    ) -> [f32; 4] {
        let alpha = self.separator.opacity * opacity_mult;
        // Custom color from trigger marks takes priority
        if let Some((r, g, b)) = custom_color {
            return color_tuple_to_f32_a(r, g, b, alpha);
        }
        if self.separator.exit_color {
            match exit_code {
                Some(0) => [0.3, 0.75, 0.3, alpha],   // Green for success
                Some(_) => [0.85, 0.25, 0.25, alpha], // Red for failure
                None => [0.5, 0.5, 0.5, alpha],       // Gray for unknown
            }
        } else {
            [
                self.separator.color[0],
                self.separator.color[1],
                self.separator.color[2],
                alpha,
            ]
        }
    }

    pub fn update_scrollbar_appearance(
        &mut self,
        width: f32,
        thumb_color: [f32; 4],
        track_color: [f32; 4],
    ) {
        self.scrollbar
            .update_appearance(width, thumb_color, track_color);
    }

    pub fn update_scrollbar_position(&mut self, position: &str) {
        self.scrollbar.update_position(position);
    }

    pub fn scrollbar_contains_point(&self, x: f32, y: f32) -> bool {
        self.scrollbar.contains_point(x, y)
    }

    pub fn scrollbar_thumb_bounds(&self) -> Option<(f32, f32)> {
        self.scrollbar.thumb_bounds()
    }

    pub fn scrollbar_track_contains_x(&self, x: f32) -> bool {
        self.scrollbar.track_contains_x(x)
    }

    pub fn scrollbar_mouse_y_to_scroll_offset(&self, mouse_y: f32) -> Option<usize> {
        self.scrollbar.mouse_y_to_scroll_offset(mouse_y)
    }

    /// Find a scrollbar mark at the given mouse position for tooltip display.
    /// Returns the mark if mouse is within `tolerance` pixels of a mark.
    pub fn scrollbar_mark_at_position(
        &self,
        mouse_x: f32,
        mouse_y: f32,
        tolerance: f32,
    ) -> Option<&par_term_config::ScrollbackMark> {
        self.scrollbar.mark_at_position(mouse_x, mouse_y, tolerance)
    }
}
