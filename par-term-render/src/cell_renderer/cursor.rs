use par_term_config::{color_u8_to_f32, color_u8x4_to_f32};

use super::{BackgroundInstance, CellRenderer};

/// Cursor position, style, colors, and visual enhancement settings.
pub(crate) struct CursorState {
    pub(crate) pos: (usize, usize),
    pub(crate) opacity: f32,
    pub(crate) style: par_term_emu_core_rust::cursor::CursorStyle,
    /// Separate cursor instance for beam/underline styles (rendered as overlay)
    pub(crate) overlay: Option<BackgroundInstance>,
    /// Cursor color [R, G, B] as floats (0.0-1.0)
    pub(crate) color: [f32; 3],
    /// Text color under block cursor [R, G, B] as floats (0.0-1.0), or None for auto-contrast
    pub(crate) text_color: Option<[f32; 3]>,
    /// Hide cursor when cursor shader is active (let shader handle cursor rendering)
    pub(crate) hidden_for_shader: bool,
    /// Enable cursor guide (horizontal line at cursor row)
    pub(crate) guide_enabled: bool,
    /// Cursor guide color [R, G, B, A] as floats (0.0-1.0)
    pub(crate) guide_color: [f32; 4],
    /// Enable cursor shadow
    pub(crate) shadow_enabled: bool,
    /// Cursor shadow color [R, G, B, A] as floats (0.0-1.0)
    pub(crate) shadow_color: [f32; 4],
    /// Cursor shadow offset in pixels [x, y]
    pub(crate) shadow_offset: [f32; 2],
    /// Cursor shadow blur radius (not fully supported yet, but stores config)
    pub(crate) shadow_blur: f32,
    /// Cursor boost (glow) intensity (0.0-1.0)
    pub(crate) boost: f32,
    /// Cursor boost glow color [R, G, B] as floats (0.0-1.0)
    pub(crate) boost_color: [f32; 3],
    /// Unfocused cursor style (hollow, same, hidden)
    pub(crate) unfocused_style: par_term_config::UnfocusedCursorStyle,
}

impl CellRenderer {
    /// Update cursor position, opacity and style. Returns `true` if anything changed.
    pub fn update_cursor(
        &mut self,
        pos: (usize, usize),
        opacity: f32,
        style: par_term_emu_core_rust::cursor::CursorStyle,
    ) -> bool {
        if self.cursor.pos != pos || self.cursor.opacity != opacity || self.cursor.style != style {
            self.dirty_rows[self.cursor.pos.1.min(self.grid.rows - 1)] = true;
            self.cursor.pos = pos;
            self.cursor.opacity = opacity;
            self.cursor.style = style;
            self.dirty_rows[self.cursor.pos.1.min(self.grid.rows - 1)] = true;

            // Compute cursor overlay for beam/underline styles
            use par_term_emu_core_rust::cursor::CursorStyle;
            self.cursor.overlay = if opacity > 0.0 {
                let col = pos.0;
                let row = pos.1;
                let x0 = (self.grid.window_padding
                    + self.grid.content_offset_x
                    + col as f32 * self.grid.cell_width)
                    .round();
                let x1 = (self.grid.window_padding
                    + self.grid.content_offset_x
                    + (col + 1) as f32 * self.grid.cell_width)
                    .round();
                let y0 = (self.grid.window_padding
                    + self.grid.content_offset_y
                    + row as f32 * self.grid.cell_height)
                    .round();
                let y1 = (self.grid.window_padding
                    + self.grid.content_offset_y
                    + (row + 1) as f32 * self.grid.cell_height)
                    .round();

                match style {
                    CursorStyle::SteadyBlock | CursorStyle::BlinkingBlock => None,
                    CursorStyle::SteadyBar | CursorStyle::BlinkingBar => Some(BackgroundInstance {
                        position: [
                            x0 / self.config.width as f32 * 2.0 - 1.0,
                            1.0 - (y0 / self.config.height as f32 * 2.0),
                        ],
                        size: [
                            2.0 / self.config.width as f32 * 2.0,
                            (y1 - y0) / self.config.height as f32 * 2.0,
                        ],
                        color: [
                            self.cursor.color[0],
                            self.cursor.color[1],
                            self.cursor.color[2],
                            opacity,
                        ],
                    }),
                    CursorStyle::SteadyUnderline | CursorStyle::BlinkingUnderline => {
                        Some(BackgroundInstance {
                            position: [
                                x0 / self.config.width as f32 * 2.0 - 1.0,
                                1.0 - ((y1 - 2.0) / self.config.height as f32 * 2.0),
                            ],
                            size: [
                                (x1 - x0) / self.config.width as f32 * 2.0,
                                2.0 / self.config.height as f32 * 2.0,
                            ],
                            color: [
                                self.cursor.color[0],
                                self.cursor.color[1],
                                self.cursor.color[2],
                                opacity,
                            ],
                        })
                    }
                }
            } else {
                None
            };
            return true;
        }
        false
    }

    pub fn clear_cursor(&mut self) -> bool {
        let pos = self.cursor.pos;
        let style = self.cursor.style;
        self.update_cursor(pos, 0.0, style)
    }

    /// Update cursor color
    pub fn update_cursor_color(&mut self, color: [u8; 3]) {
        self.cursor.color = color_u8_to_f32(color);
        self.dirty_rows[self.cursor.pos.1.min(self.grid.rows - 1)] = true;
    }

    /// Update cursor text color (color of text under block cursor)
    pub fn update_cursor_text_color(&mut self, color: Option<[u8; 3]>) {
        self.cursor.text_color = color.map(color_u8_to_f32);
        self.dirty_rows[self.cursor.pos.1.min(self.grid.rows - 1)] = true;
    }

    /// Set whether cursor should be hidden when cursor shader is active.
    /// Returns `true` if the value changed.
    pub fn set_cursor_hidden_for_shader(&mut self, hidden: bool) -> bool {
        if self.cursor.hidden_for_shader != hidden {
            self.cursor.hidden_for_shader = hidden;
            self.dirty_rows[self.cursor.pos.1.min(self.grid.rows - 1)] = true;
            return true;
        }
        false
    }

    /// Set window focus state (affects unfocused cursor rendering).
    /// Returns `true` if the value changed.
    pub fn set_focused(&mut self, focused: bool) -> bool {
        if self.is_focused != focused {
            self.is_focused = focused;
            self.dirty_rows[self.cursor.pos.1.min(self.grid.rows - 1)] = true;
            return true;
        }
        false
    }

    /// Update cursor guide settings
    pub fn update_cursor_guide(&mut self, enabled: bool, color: [u8; 4]) {
        self.cursor.guide_enabled = enabled;
        self.cursor.guide_color = color_u8x4_to_f32(color);
        if enabled {
            self.dirty_rows[self.cursor.pos.1.min(self.grid.rows - 1)] = true;
        }
    }

    /// Update cursor shadow settings
    pub fn update_cursor_shadow(
        &mut self,
        enabled: bool,
        color: [u8; 4],
        offset: [f32; 2],
        blur: f32,
    ) {
        self.cursor.shadow_enabled = enabled;
        self.cursor.shadow_color = color_u8x4_to_f32(color);
        self.cursor.shadow_offset = offset;
        self.cursor.shadow_blur = blur;
        if enabled {
            self.dirty_rows[self.cursor.pos.1.min(self.grid.rows - 1)] = true;
        }
    }

    /// Update cursor boost settings
    pub fn update_cursor_boost(&mut self, intensity: f32, color: [u8; 3]) {
        self.cursor.boost = intensity.clamp(0.0, 1.0);
        self.cursor.boost_color = color_u8_to_f32(color);
        if intensity > 0.0 {
            self.dirty_rows[self.cursor.pos.1.min(self.grid.rows - 1)] = true;
        }
    }

    /// Update unfocused cursor style
    pub fn update_unfocused_cursor_style(&mut self, style: par_term_config::UnfocusedCursorStyle) {
        self.cursor.unfocused_style = style;
        if !self.is_focused {
            self.dirty_rows[self.cursor.pos.1.min(self.grid.rows - 1)] = true;
        }
    }
}
