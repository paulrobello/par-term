use super::instance_buffers::{
    COLOR_COMPONENT_EPSILON, CURSOR_BOOST_MAX_ALPHA, CURSOR_OVERLAY_SLOTS, GUTTER_WIDTH_CELLS,
    HOLLOW_CURSOR_BORDER_PX,
};
use super::{BackgroundInstance, Cell, CellRenderer};
use par_term_config::{color_u8x4_rgb_to_f32, color_u8x4_rgb_to_f32_a};

impl CellRenderer {
    /// Build background instances for a single row, populating `self.scratch_row_bg`.
    ///
    /// Uses RLE to merge consecutive cells with the same background color, eliminating
    /// seams between adjacent same-colored cells. The scratch buffer is cleared on entry
    /// and padded to `self.grid.cols` entries on return.
    pub(crate) fn build_row_bg_instances(&mut self, row: usize, row_cells: &[Cell]) {
        let mut col = 0;
        while col < row_cells.len() {
            let cell = &row_cells[col];
            let bg_f = color_u8x4_rgb_to_f32(cell.bg_color);
            let is_default_bg = (bg_f[0] - self.background_color[0]).abs()
                < COLOR_COMPONENT_EPSILON
                && (bg_f[1] - self.background_color[1]).abs() < COLOR_COMPONENT_EPSILON
                && (bg_f[2] - self.background_color[2]).abs() < COLOR_COMPONENT_EPSILON;

            // Check for cursor at this position, accounting for unfocused state
            let cursor_visible = self.cursor.opacity > 0.0
                && !self.cursor.hidden_for_shader
                && self.cursor.pos.1 == row
                && self.cursor.pos.0 == col;

            // Handle unfocused cursor visibility
            let has_cursor = if cursor_visible && !self.is_focused {
                match self.cursor.unfocused_style {
                    par_term_config::UnfocusedCursorStyle::Hidden => false,
                    par_term_config::UnfocusedCursorStyle::Hollow
                    | par_term_config::UnfocusedCursorStyle::Same => true,
                }
            } else {
                cursor_visible
            };

            if is_default_bg && !has_cursor {
                col += 1;
                continue;
            }

            // Calculate background color with alpha
            let bg_alpha = if self.transparency_affects_only_default_background && !is_default_bg {
                1.0
            } else {
                self.window_opacity
            };
            let mut bg_color = color_u8x4_rgb_to_f32_a(cell.bg_color, bg_alpha);

            // Handle cursor at this position
            if has_cursor && self.cursor.opacity > 0.0 {
                use par_term_emu_core_rust::cursor::CursorStyle;

                // Check if we should render hollow cursor (unfocused hollow style)
                let render_hollow = !self.is_focused
                    && self.cursor.unfocused_style == par_term_config::UnfocusedCursorStyle::Hollow;

                match self.cursor.style {
                    CursorStyle::SteadyBlock | CursorStyle::BlinkingBlock => {
                        if render_hollow {
                            // Hollow cursor: don't fill the cell, outline will be added later
                            // Keep original background color
                        } else {
                            // Solid block cursor
                            for (bg, &cursor) in bg_color.iter_mut().take(3).zip(&self.cursor.color)
                            {
                                *bg = *bg * (1.0 - self.cursor.opacity)
                                    + cursor * self.cursor.opacity;
                            }
                            bg_color[3] = bg_color[3].max(self.cursor.opacity);
                        }
                    }
                    _ => {}
                }
                // Cursor cell can't be merged, render it alone
                let x0 = self.grid.window_padding
                    + self.grid.content_offset_x
                    + col as f32 * self.grid.cell_width;
                let x1 = self.grid.window_padding
                    + self.grid.content_offset_x
                    + (col + 1) as f32 * self.grid.cell_width;
                let y0 = self.grid.window_padding
                    + self.grid.content_offset_y
                    + row as f32 * self.grid.cell_height;
                let y1 = y0 + self.grid.cell_height;
                self.scratch_row_bg.push(BackgroundInstance {
                    position: [
                        x0 / self.config.width as f32 * 2.0 - 1.0,
                        1.0 - (y0 / self.config.height as f32 * 2.0),
                    ],
                    size: [
                        (x1 - x0) / self.config.width as f32 * 2.0,
                        (y1 - y0) / self.config.height as f32 * 2.0,
                    ],
                    color: bg_color,
                });
                col += 1;
                continue;
            }

            // RLE: Find run of consecutive cells with same background color
            let start_col = col;
            let run_color = cell.bg_color;
            col += 1;
            while col < row_cells.len() {
                let next_cell = &row_cells[col];
                let next_has_cursor = self.cursor.opacity > 0.0
                    && !self.cursor.hidden_for_shader
                    && self.cursor.pos.1 == row
                    && self.cursor.pos.0 == col;
                // Stop run if color differs or cursor is here
                if next_cell.bg_color != run_color || next_has_cursor {
                    break;
                }
                col += 1;
            }
            let run_length = col - start_col;

            // Create single quad spanning entire run (no per-cell rounding)
            let x0 = self.grid.window_padding
                + self.grid.content_offset_x
                + start_col as f32 * self.grid.cell_width;
            let x1 = self.grid.window_padding
                + self.grid.content_offset_x
                + (start_col + run_length) as f32 * self.grid.cell_width;
            let y0 = self.grid.window_padding
                + self.grid.content_offset_y
                + row as f32 * self.grid.cell_height;
            let y1 = y0 + self.grid.cell_height;

            self.scratch_row_bg.push(BackgroundInstance {
                position: [
                    x0 / self.config.width as f32 * 2.0 - 1.0,
                    1.0 - (y0 / self.config.height as f32 * 2.0),
                ],
                size: [
                    (x1 - x0) / self.config.width as f32 * 2.0,
                    (y1 - y0) / self.config.height as f32 * 2.0,
                ],
                color: bg_color,
            });
        }

        // Pad row_bg to expected size with empty instances
        // (RLE creates fewer instances than cells, but buffer expects cols entries)
        while self.scratch_row_bg.len() < self.grid.cols {
            self.scratch_row_bg.push(BackgroundInstance {
                position: [0.0, 0.0],
                size: [0.0, 0.0],
                color: [0.0, 0.0, 0.0, 0.0],
            });
        }
    }

    /// Build cursor overlay background instances for the CURSOR_OVERLAY_SLOTS slots.
    ///
    /// Returns a `Vec` of exactly `CURSOR_OVERLAY_SLOTS` entries. Slots that are not
    /// active carry a zero-sized transparent instance so the GPU draw call is a no-op
    /// for those entries.
    ///
    /// Slot layout:
    ///   [0] cursor overlay (beam/underline, from `cursor.overlay`)
    ///   [1] cursor guide line
    ///   [2] cursor shadow
    ///   [3] cursor boost glow
    ///   [4-7] hollow cursor outline (top, bottom, left, right)
    ///   [8-9] reserved (zero)
    pub(crate) fn build_cursor_overlay_instances(&self) -> Vec<BackgroundInstance> {
        let mut overlay_instances = vec![
            BackgroundInstance {
                position: [0.0, 0.0],
                size: [0.0, 0.0],
                color: [0.0, 0.0, 0.0, 0.0],
            };
            CURSOR_OVERLAY_SLOTS
        ];

        // Check if cursor should be visible
        let cursor_visible = self.cursor.opacity > 0.0
            && !self.cursor.hidden_for_shader
            && (self.is_focused
                || self.cursor.unfocused_style != par_term_config::UnfocusedCursorStyle::Hidden);

        // Calculate cursor pixel positions
        let cursor_col = self.cursor.pos.0;
        let cursor_row = self.cursor.pos.1;
        let cursor_x0 = self.grid.window_padding
            + self.grid.content_offset_x
            + cursor_col as f32 * self.grid.cell_width;
        let cursor_x1 = cursor_x0 + self.grid.cell_width;
        let cursor_y0 = self.grid.window_padding
            + self.grid.content_offset_y
            + cursor_row as f32 * self.grid.cell_height;
        let cursor_y1 = cursor_y0 + self.grid.cell_height;

        // Slot 0: Cursor overlay (beam/underline) - handled by existing cursor_overlay
        overlay_instances[0] = self.cursor.overlay.unwrap_or(BackgroundInstance {
            position: [0.0, 0.0],
            size: [0.0, 0.0],
            color: [0.0, 0.0, 0.0, 0.0],
        });

        // Slot 1: Cursor guide (horizontal line spanning full width at cursor row)
        if cursor_visible && self.cursor.guide_enabled {
            let guide_x0 = self.grid.window_padding + self.grid.content_offset_x;
            let guide_x1 =
                self.config.width as f32 - self.grid.window_padding - self.grid.content_inset_right;
            overlay_instances[1] = BackgroundInstance {
                position: [
                    guide_x0 / self.config.width as f32 * 2.0 - 1.0,
                    1.0 - (cursor_y0 / self.config.height as f32 * 2.0),
                ],
                size: [
                    (guide_x1 - guide_x0) / self.config.width as f32 * 2.0,
                    (cursor_y1 - cursor_y0) / self.config.height as f32 * 2.0,
                ],
                color: self.cursor.guide_color,
            };
        }

        // Slot 2: Cursor shadow (offset rectangle behind cursor)
        if cursor_visible && self.cursor.shadow_enabled {
            let shadow_x0 = cursor_x0 + self.cursor.shadow_offset[0];
            let shadow_y0 = cursor_y0 + self.cursor.shadow_offset[1];
            overlay_instances[2] = BackgroundInstance {
                position: [
                    shadow_x0 / self.config.width as f32 * 2.0 - 1.0,
                    1.0 - (shadow_y0 / self.config.height as f32 * 2.0),
                ],
                size: [
                    self.grid.cell_width / self.config.width as f32 * 2.0,
                    self.grid.cell_height / self.config.height as f32 * 2.0,
                ],
                color: self.cursor.shadow_color,
            };
        }

        // Slot 3: Cursor boost glow (larger rectangle around cursor with low opacity)
        if cursor_visible && self.cursor.boost > 0.0 {
            let glow_expand = 4.0 * self.scale_factor * self.cursor.boost; // Expand by up to 4 logical pixels
            let glow_x0 = cursor_x0 - glow_expand;
            let glow_y0 = cursor_y0 - glow_expand;
            let glow_w = self.grid.cell_width + glow_expand * 2.0;
            let glow_h = self.grid.cell_height + glow_expand * 2.0;
            overlay_instances[3] = BackgroundInstance {
                position: [
                    glow_x0 / self.config.width as f32 * 2.0 - 1.0,
                    1.0 - (glow_y0 / self.config.height as f32 * 2.0),
                ],
                size: [
                    glow_w / self.config.width as f32 * 2.0,
                    glow_h / self.config.height as f32 * 2.0,
                ],
                color: [
                    self.cursor.boost_color[0],
                    self.cursor.boost_color[1],
                    self.cursor.boost_color[2],
                    self.cursor.boost * CURSOR_BOOST_MAX_ALPHA * self.cursor.opacity,
                ],
            };
        }

        // Slots 4-7: Hollow cursor outline (4 thin rectangles forming a border)
        // Rendered when unfocused with hollow style and block cursor
        let render_hollow = cursor_visible
            && !self.is_focused
            && self.cursor.unfocused_style == par_term_config::UnfocusedCursorStyle::Hollow;

        if render_hollow {
            use par_term_emu_core_rust::cursor::CursorStyle;
            let is_block = matches!(
                self.cursor.style,
                CursorStyle::SteadyBlock | CursorStyle::BlinkingBlock
            );

            if is_block {
                let border_width = HOLLOW_CURSOR_BORDER_PX;
                let color = [
                    self.cursor.color[0],
                    self.cursor.color[1],
                    self.cursor.color[2],
                    self.cursor.opacity,
                ];

                // Top border
                overlay_instances[4] = BackgroundInstance {
                    position: [
                        cursor_x0 / self.config.width as f32 * 2.0 - 1.0,
                        1.0 - (cursor_y0 / self.config.height as f32 * 2.0),
                    ],
                    size: [
                        self.grid.cell_width / self.config.width as f32 * 2.0,
                        border_width / self.config.height as f32 * 2.0,
                    ],
                    color,
                };

                // Bottom border
                overlay_instances[5] = BackgroundInstance {
                    position: [
                        cursor_x0 / self.config.width as f32 * 2.0 - 1.0,
                        1.0 - ((cursor_y1 - border_width) / self.config.height as f32 * 2.0),
                    ],
                    size: [
                        self.grid.cell_width / self.config.width as f32 * 2.0,
                        border_width / self.config.height as f32 * 2.0,
                    ],
                    color,
                };

                // Left border
                overlay_instances[6] = BackgroundInstance {
                    position: [
                        cursor_x0 / self.config.width as f32 * 2.0 - 1.0,
                        1.0 - ((cursor_y0 + border_width) / self.config.height as f32 * 2.0),
                    ],
                    size: [
                        border_width / self.config.width as f32 * 2.0,
                        (self.grid.cell_height - border_width * 2.0) / self.config.height as f32
                            * 2.0,
                    ],
                    color,
                };

                // Right border
                overlay_instances[7] = BackgroundInstance {
                    position: [
                        (cursor_x1 - border_width) / self.config.width as f32 * 2.0 - 1.0,
                        1.0 - ((cursor_y0 + border_width) / self.config.height as f32 * 2.0),
                    ],
                    size: [
                        border_width / self.config.width as f32 * 2.0,
                        (self.grid.cell_height - border_width * 2.0) / self.config.height as f32
                            * 2.0,
                    ],
                    color,
                };
            }
        }

        overlay_instances
    }

    /// Build separator line background instances (one per row).
    ///
    /// Returns a `Vec` of exactly `self.grid.rows` entries. Inactive rows carry
    /// a zero-sized transparent instance.
    pub(crate) fn build_separator_instances(&self) -> Vec<BackgroundInstance> {
        let mut separator_instances = vec![
            BackgroundInstance {
                position: [0.0, 0.0],
                size: [0.0, 0.0],
                color: [0.0, 0.0, 0.0, 0.0],
            };
            self.grid.rows
        ];

        if self.separator.enabled {
            let width_f = self.config.width as f32;
            let height_f = self.config.height as f32;
            for &(screen_row, exit_code, custom_color) in &self.separator.visible_marks {
                if screen_row < self.grid.rows {
                    let x0 = self.grid.window_padding + self.grid.content_offset_x;
                    let x1 = width_f - self.grid.window_padding - self.grid.content_inset_right;
                    let y0 = self.grid.window_padding
                        + self.grid.content_offset_y
                        + screen_row as f32 * self.grid.cell_height;
                    let color = self.separator_color(exit_code, custom_color, 1.0);
                    separator_instances[screen_row] = BackgroundInstance {
                        position: [x0 / width_f * 2.0 - 1.0, 1.0 - (y0 / height_f * 2.0)],
                        size: [
                            (x1 - x0) / width_f * 2.0,
                            self.separator.thickness / height_f * 2.0,
                        ],
                        color,
                    };
                }
            }
        }

        separator_instances
    }

    /// Build gutter indicator background instances (one per row).
    ///
    /// Returns a `Vec` of exactly `self.grid.rows` entries. Rows without a gutter
    /// indicator carry a zero-sized transparent instance.
    pub(crate) fn build_gutter_instances(&self) -> Vec<BackgroundInstance> {
        let mut gutter_instances = vec![
            BackgroundInstance {
                position: [0.0, 0.0],
                size: [0.0, 0.0],
                color: [0.0, 0.0, 0.0, 0.0],
            };
            self.grid.rows
        ];

        if !self.gutter_indicators.is_empty() {
            let width_f = self.config.width as f32;
            let height_f = self.config.height as f32;
            for &(screen_row, color) in &self.gutter_indicators {
                if screen_row < self.grid.rows {
                    let x0 = self.grid.window_padding + self.grid.content_offset_x;
                    let x1 = x0 + GUTTER_WIDTH_CELLS * self.grid.cell_width;
                    let y0 = self.grid.window_padding
                        + self.grid.content_offset_y
                        + screen_row as f32 * self.grid.cell_height;
                    gutter_instances[screen_row] = BackgroundInstance {
                        position: [x0 / width_f * 2.0 - 1.0, 1.0 - (y0 / height_f * 2.0)],
                        size: [
                            (x1 - x0) / width_f * 2.0,
                            self.grid.cell_height / height_f * 2.0,
                        ],
                        color,
                    };
                }
            }
        }

        gutter_instances
    }
}
