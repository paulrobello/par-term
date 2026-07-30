//! Phase 2 of pane rendering: per-row text instance generation.
//!
//! Provides [`CellRenderer::emit_row_text`], which appends this row's glyph quads and
//! underline rectangles to `self.text_instances` starting at `text_index` and returns
//! the updated index. Underlines are emitted through the same text pipeline, so they
//! belong to this phase rather than to the background or cursor-overlay phases.
//!
//! Geometric block/box-drawing characters short-circuit into `block_char_render.rs`;
//! everything else resolves through the shared atlas font-fallback helper.

use super::super::instance_buffers::{
    STIPPLE_OFF_PX, STIPPLE_ON_PX, UNDERLINE_HEIGHT_RATIO, compute_cursor_text_color,
};
use super::super::{Cell, CellRenderer, TextInstance, block_chars};
use super::ATLAS_SIZE;
use super::block_char_render::BlockCharRenderParams;
use par_term_config::color_u8x4_rgb_to_f32_a;

/// Parameters for [`CellRenderer::emit_row_text`].
pub(super) struct RowTextParams<'a> {
    /// The cells of this row (already sliced out of the pane's cell buffer).
    pub row_cells: &'a [Cell],
    /// The pane's full cell buffer (the underline pass indexes it via `row_start`).
    pub cells: &'a [Cell],
    /// Index of this row's first cell within `cells`.
    pub row_start: usize,
    /// Number of columns in the cell grid.
    pub cols: usize,
    /// Zero-based row index within the pane.
    pub row: usize,
    /// Pixel X of the content area origin (left edge after padding).
    pub content_x: f32,
    /// Pixel Y of the content area origin (top edge after padding).
    pub content_y: f32,
    /// Cursor position (col, row) within this pane, or None if no cursor.
    pub cursor_pos: Option<(usize, usize)>,
    /// Cursor blink opacity (0.0 = hidden, 1.0 = fully visible).
    pub cursor_opacity: f32,
    /// Pane dimming factor applied on top of window opacity.
    pub opacity_multiplier: f32,
}

impl CellRenderer {
    /// Append this row's glyph and underline instances to `self.text_instances`.
    ///
    /// Returns the updated `text_index` after the row's instances have been appended.
    pub(super) fn emit_row_text(&mut self, p: RowTextParams<'_>, mut text_index: usize) -> usize {
        let RowTextParams {
            row_cells,
            cells,
            row_start,
            cols,
            row,
            content_x,
            content_y,
            cursor_pos,
            cursor_opacity,
            opacity_multiplier,
        } = p;

        // Text rendering
        let natural_line_height =
            self.font.font_ascent + self.font.font_descent + self.font.font_leading;
        let vertical_padding = (self.grid.cell_height - natural_line_height).max(0.0) / 2.0;
        let baseline_y = content_y
            + (row as f32 * self.grid.cell_height)
            + vertical_padding
            + self.font.font_ascent;

        // Compute text alpha - force opaque if keep_text_opaque is enabled
        let text_alpha = if self.keep_text_opaque {
            opacity_multiplier // Only apply pane dimming, not window transparency
        } else {
            self.window_opacity * opacity_multiplier
        };

        // Check if this row has the cursor and it's a visible block cursor
        // (for cursor text color override in split-pane rendering)
        let cursor_is_block_on_this_row = {
            use par_term_emu_core_rust::cursor::CursorStyle;
            cursor_pos.is_some_and(|(_, cy)| cy == row)
                && cursor_opacity > 0.0
                && !self.cursor.hidden_for_shader
                && matches!(
                    self.cursor.style,
                    CursorStyle::SteadyBlock | CursorStyle::BlinkingBlock
                )
                && (self.is_focused
                    || self.cursor.unfocused_style == par_term_config::UnfocusedCursorStyle::Same)
        };

        for (col_idx, cell) in row_cells.iter().enumerate() {
            if cell.wide_char_spacer || cell.grapheme == " " {
                continue;
            }

            // Avoid Vec<char> allocation: use iterator-based char access.
            let Some(ch) = cell.grapheme.chars().next() else {
                continue;
            };

            // Kitty Unicode placeholder cells render as images (handled by
            // the graphics path), not as glyphs. Most fonts have no glyph
            // for U+10EEEE so falling through here would draw tofu where
            // an image is supposed to appear.
            if ch == '\u{10EEEE}' {
                continue;
            }
            let second_char = cell.grapheme.chars().nth(1);
            // grapheme_len is 1, 2, or "more than 2" — we stop counting at 3.
            let grapheme_len = match second_char {
                None => 1usize,
                Some(_) => {
                    if cell.grapheme.chars().nth(2).is_none() {
                        2
                    } else {
                        3
                    }
                }
            };

            // Determine text color - apply cursor_text_color (or auto-contrast) when the
            // block cursor is on this cell, otherwise use the cell's foreground color.
            let render_fg_color: [f32; 4] =
                if cursor_is_block_on_this_row && cursor_pos.is_some_and(|(cx, _)| cx == col_idx) {
                    compute_cursor_text_color(self.cursor.color, self.cursor.text_color, text_alpha)
                } else {
                    color_u8x4_rgb_to_f32_a(cell.fg_color, text_alpha)
                };

            // Classify character for block/box-drawing detection and glyph snapping.
            let char_type = block_chars::classify_char(ch);

            // Attempt geometric rendering for block/box-drawing characters.
            // See block_char_render.rs for the full implementation.
            let block_x0 = (content_x + col_idx as f32 * self.grid.cell_width).round();
            let block_y0 = (content_y + row as f32 * self.grid.cell_height).round();
            let block_y1 = (content_y + (row + 1) as f32 * self.grid.cell_height).round();
            if let Some(new_idx) = self.render_block_char_geometrically(BlockCharRenderParams {
                cell,
                ch,
                grapheme_len,
                x0_pixel: block_x0,
                y0_pixel: block_y0,
                y1_pixel: block_y1,
                render_fg_color,
                text_alpha,
                text_index,
            }) {
                text_index = new_idx;
                continue;
            }

            // Check if this character should be rendered as a monochrome symbol.
            // Also handle symbol + VS16 (U+FE0F): strip VS16, render monochrome.
            let (force_monochrome, base_char) = if grapheme_len == 1 {
                (super::super::atlas::should_render_as_symbol(ch), ch)
            } else if grapheme_len == 2
                && second_char == Some('\u{FE0F}')
                && super::super::atlas::should_render_as_symbol(ch)
            {
                // Symbol + VS16: strip VS16 and render base char as monochrome
                (true, ch)
            } else {
                (false, ch)
            };

            // Resolve a renderable glyph via the shared font-fallback helper (ARC-004 / QA-003).
            // This replaces the duplicated excluded_fonts/get_or_rasterize_glyph loop
            // that previously existed in both pane_render/mod.rs and text_instance_builder.rs.
            let resolved_info = self.resolve_glyph_with_fallback(
                base_char,
                &cell.grapheme,
                cell.bold,
                cell.italic,
                force_monochrome,
            );

            if let Some(info) = resolved_info {
                let char_w = if cell.wide_char {
                    self.grid.cell_width * 2.0
                } else {
                    self.grid.cell_width
                };
                let x0 = content_x + col_idx as f32 * self.grid.cell_width;
                let y0 = content_y + row as f32 * self.grid.cell_height;
                let x1 = x0 + char_w;
                let y1 = y0 + self.grid.cell_height;

                let cell_w = x1 - x0;
                let cell_h = y1 - y0;
                let scale_x = cell_w / char_w;
                let scale_y = cell_h / self.grid.cell_height;

                let baseline_offset = baseline_y - (content_y + row as f32 * self.grid.cell_height);
                let glyph_left = x0 + (info.bearing_x * scale_x).round();
                let baseline_in_cell = (baseline_offset * scale_y).round();
                let glyph_top = y0 + baseline_in_cell - info.bearing_y;

                let render_w = info.width as f32 * scale_x;
                let render_h = info.height as f32 * scale_y;

                let (final_left, final_top, final_w, final_h) = if grapheme_len == 1
                    && matches!(
                        char_type,
                        block_chars::BlockCharType::Symbol | block_chars::BlockCharType::Geometric
                    ) {
                    let height_scale = cell_h / render_h;
                    let width_scale = cell_w / render_w;
                    let symbol_scale = height_scale.min(width_scale).max(1.0);
                    let final_w = render_w * symbol_scale;
                    let final_h = render_h * symbol_scale;
                    (
                        x0 + (cell_w - final_w) / 2.0,
                        y0 + (cell_h - final_h) / 2.0,
                        final_w,
                        final_h,
                    )
                } else if grapheme_len == 1 && block_chars::should_snap_to_boundaries(char_type) {
                    block_chars::snap_glyph_to_cell(block_chars::SnapGlyphParams {
                        glyph_left,
                        glyph_top,
                        render_w,
                        render_h,
                        cell_x0: x0,
                        cell_y0: y0,
                        cell_x1: x1,
                        cell_y1: y1,
                        snap_threshold: 3.0,
                        extension: 0.5,
                    })
                } else {
                    (glyph_left, glyph_top, render_w, render_h)
                };

                if text_index < self.buffers.max_text_instances {
                    self.text_instances[text_index] = TextInstance {
                        position: [
                            final_left / self.config.width as f32 * 2.0 - 1.0,
                            1.0 - (final_top / self.config.height as f32 * 2.0),
                        ],
                        size: [
                            final_w / self.config.width as f32 * 2.0,
                            final_h / self.config.height as f32 * 2.0,
                        ],
                        tex_offset: [info.x as f32 / ATLAS_SIZE, info.y as f32 / ATLAS_SIZE],
                        tex_size: [
                            info.width as f32 / ATLAS_SIZE,
                            info.height as f32 / ATLAS_SIZE,
                        ],
                        color: render_fg_color,
                        is_colored: if info.is_colored { 1 } else { 0 },
                    };
                    text_index += 1;
                }
            }
        }

        // Underlines: emit a thin rectangle at the bottom of each underlined cell.
        // Mirrors the logic in text_instance_builder.rs but uses pane-local coordinates.
        {
            let underline_thickness = (self.grid.cell_height * UNDERLINE_HEIGHT_RATIO)
                .max(1.0)
                .round();
            let tex_offset = [
                self.atlas.solid_pixel_offset.0 as f32 / ATLAS_SIZE,
                self.atlas.solid_pixel_offset.1 as f32 / ATLAS_SIZE,
            ];
            let tex_size = [1.0 / ATLAS_SIZE, 1.0 / ATLAS_SIZE];
            let y0 = content_y + (row + 1) as f32 * self.grid.cell_height - underline_thickness;
            let ndc_y = 1.0 - (y0 / self.config.height as f32 * 2.0);
            let ndc_h = underline_thickness / self.config.height as f32 * 2.0;
            let is_stipple =
                self.link_underline_style == par_term_config::LinkUnderlineStyle::Stipple;
            let stipple_period = STIPPLE_ON_PX + STIPPLE_OFF_PX;

            for col_idx in 0..cols {
                if row_start + col_idx >= cells.len() {
                    break;
                }
                let cell = &cells[row_start + col_idx];
                if !cell.underline {
                    continue;
                }
                let fg = color_u8x4_rgb_to_f32_a(cell.fg_color, text_alpha);
                let cell_x0 = content_x + col_idx as f32 * self.grid.cell_width;

                if is_stipple {
                    let mut px = 0.0;
                    while px < self.grid.cell_width && text_index < self.buffers.max_text_instances
                    {
                        let seg_w = STIPPLE_ON_PX.min(self.grid.cell_width - px);
                        let x = cell_x0 + px;
                        self.text_instances[text_index] = TextInstance {
                            position: [x / self.config.width as f32 * 2.0 - 1.0, ndc_y],
                            size: [seg_w / self.config.width as f32 * 2.0, ndc_h],
                            tex_offset,
                            tex_size,
                            color: fg,
                            is_colored: 0,
                        };
                        text_index += 1;
                        px += stipple_period;
                    }
                } else if text_index < self.buffers.max_text_instances {
                    self.text_instances[text_index] = TextInstance {
                        position: [cell_x0 / self.config.width as f32 * 2.0 - 1.0, ndc_y],
                        size: [self.grid.cell_width / self.config.width as f32 * 2.0, ndc_h],
                        tex_offset,
                        tex_size,
                        color: fg,
                        is_colored: 0,
                    };
                    text_index += 1;
                }
            }
        }

        text_index
    }
}
