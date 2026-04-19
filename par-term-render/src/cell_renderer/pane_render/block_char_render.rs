/// Geometric rendering of block/box-drawing characters via the text pipeline.
///
/// All characters handled here are emitted as solid-color quads using the atlas
/// solid-pixel offset, bypassing the glyph rasterizer. This eliminates sub-pixel
/// seams that appear when mixing the background pipeline (for cell fills) with
/// atlas glyphs for characters like ▄/▀.
///
/// The public entry point is `CellRenderer::render_block_char_geometrically`.
use super::super::block_chars;
use super::super::{CellRenderer, TextInstance};
use super::ATLAS_SIZE;
use par_term_config::{Cell, color_u8x4_rgb_to_f32_a};

/// Parameters for `CellRenderer::render_block_char_geometrically`.
pub(super) struct BlockCharRenderParams<'a> {
    /// The cell to render (provides bg/fg color, wide_char flag, etc.).
    pub cell: &'a Cell,
    /// The unicode scalar of the primary character (`cell.grapheme.chars().next()`).
    pub ch: char,
    /// Number of characters in the grapheme (1 = single char; stops at 3).
    pub grapheme_len: usize,
    /// Pixel x position of the left edge of the cell.
    pub x0_pixel: f32,
    /// Pixel y position of the top edge of the cell (snapped).
    pub y0_pixel: f32,
    /// Pixel y position of the bottom edge of the cell (snapped).
    pub y1_pixel: f32,
    /// Foreground color with alpha already applied (RGBA [0,1]).
    pub render_fg_color: [f32; 4],
    /// Combined text alpha (window opacity * pane dimming).
    pub text_alpha: f32,
    /// Current write index into `CellRenderer::text_instances`.
    pub text_index: usize,
}

impl CellRenderer {
    /// Attempt to render `ch` as a geometric block character.
    ///
    /// If `ch` is a box-drawing, half-block, or block-element character that
    /// can be rendered geometrically, the appropriate `TextInstance`(s) are
    /// written into `self.text_instances` starting at `params.text_index`,
    /// and the updated index is returned as `Some(new_text_index)`.
    ///
    /// Returns `None` if the character is not a supported geometric type,
    /// in which case the caller should fall through to the atlas-glyph path.
    ///
    /// The caller must `continue` the per-cell loop when `Some(_)` is returned.
    pub(super) fn render_block_char_geometrically(
        &mut self,
        params: BlockCharRenderParams<'_>,
    ) -> Option<usize> {
        let BlockCharRenderParams {
            cell,
            ch,
            grapheme_len,
            x0_pixel: x0,
            y0_pixel: y0,
            y1_pixel: y1,
            render_fg_color,
            text_alpha,
            mut text_index,
        } = params;

        let char_type = block_chars::classify_char(ch);
        if grapheme_len != 1 || !block_chars::should_render_geometrically(char_type) {
            return None;
        }

        let char_w = if cell.wide_char {
            self.grid.cell_width * 2.0
        } else {
            self.grid.cell_width
        };
        let snapped_cell_height = y1 - y0;

        let solid_tex_offset = [
            self.atlas.solid_pixel_offset.0 as f32 / ATLAS_SIZE,
            self.atlas.solid_pixel_offset.1 as f32 / ATLAS_SIZE,
        ];
        let solid_tex_size = [1.0 / ATLAS_SIZE, 1.0 / ATLAS_SIZE];

        // --- Box drawing geometry ---
        let aspect_ratio = snapped_cell_height / char_w;
        if let Some(box_geo) = block_chars::get_box_drawing_geometry(ch, aspect_ratio) {
            for segment in &box_geo.segments {
                let rect = segment
                    .to_pixel_rect(x0, y0, char_w, snapped_cell_height)
                    .snap_to_pixels();

                // 1 px extension for seamless lines at cell boundaries.
                let extension = 1.0;
                let ext_x = if segment.x <= 0.01 { extension } else { 0.0 };
                let ext_y = if segment.y <= 0.01 { extension } else { 0.0 };
                let ext_w = if segment.x + segment.width >= 0.99 {
                    extension
                } else {
                    0.0
                };
                let ext_h = if segment.y + segment.height >= 0.99 {
                    extension
                } else {
                    0.0
                };

                if text_index < self.buffers.max_text_instances {
                    self.text_instances[text_index] = TextInstance {
                        position: [
                            (rect.x - ext_x) / self.config.width as f32 * 2.0 - 1.0,
                            1.0 - ((rect.y - ext_y) / self.config.height as f32 * 2.0),
                        ],
                        size: [
                            (rect.width + ext_x + ext_w) / self.config.width as f32 * 2.0,
                            (rect.height + ext_y + ext_h) / self.config.height as f32 * 2.0,
                        ],
                        tex_offset: solid_tex_offset,
                        tex_size: solid_tex_size,
                        color: render_fg_color,
                        is_colored: 0,
                    };
                    text_index += 1;
                }
            }
            return Some(text_index);
        }

        // --- Half-block characters (▄/▀) ---
        // Both halves are rendered through the text pipeline to avoid cross-pipeline seams.
        if ch == '\u{2584}' || ch == '\u{2580}' {
            let x1 = x0 + char_w;
            let cell_w = x1 - x0;
            let y_mid = y0 + self.grid.cell_height / 2.0;

            let bg_half_color = color_u8x4_rgb_to_f32_a(cell.bg_color, text_alpha);
            let (top_color, bottom_color) = if ch == '\u{2584}' {
                (bg_half_color, render_fg_color) // ▄: top=bg, bottom=fg
            } else {
                (render_fg_color, bg_half_color) // ▀: top=fg, bottom=bg
            };

            // Top half: [y0, y_mid)
            if text_index < self.buffers.max_text_instances {
                self.text_instances[text_index] = TextInstance {
                    position: [
                        x0 / self.config.width as f32 * 2.0 - 1.0,
                        1.0 - (y0 / self.config.height as f32 * 2.0),
                    ],
                    size: [
                        cell_w / self.config.width as f32 * 2.0,
                        (y_mid - y0) / self.config.height as f32 * 2.0,
                    ],
                    tex_offset: solid_tex_offset,
                    tex_size: solid_tex_size,
                    color: top_color,
                    is_colored: 0,
                };
                text_index += 1;
            }

            // Bottom half: [y_mid, y1)
            if text_index < self.buffers.max_text_instances {
                self.text_instances[text_index] = TextInstance {
                    position: [
                        x0 / self.config.width as f32 * 2.0 - 1.0,
                        1.0 - (y_mid / self.config.height as f32 * 2.0),
                    ],
                    size: [
                        cell_w / self.config.width as f32 * 2.0,
                        (y1 - y_mid) / self.config.height as f32 * 2.0,
                    ],
                    tex_offset: solid_tex_offset,
                    tex_size: solid_tex_size,
                    color: bottom_color,
                    is_colored: 0,
                };
                text_index += 1;
            }
            return Some(text_index);
        }

        // --- Geometric shape rectangles (filled squares/rectangles, U+25A0–U+25FF) ---
        // Aspect-ratio-aware filled shapes like ■ ▪ ▬ ▮ ◼ ◾. Outline/hollow shapes
        // return None and fall through to the font path, where the Symbol/Geometric
        // scale-to-fill branch in pane_render centers and fills them.
        if let Some(rect) =
            block_chars::get_geometric_shape_rect(ch, x0, y0, char_w, snapped_cell_height)
        {
            if text_index < self.buffers.max_text_instances {
                self.text_instances[text_index] = TextInstance {
                    position: [
                        rect.x / self.config.width as f32 * 2.0 - 1.0,
                        1.0 - (rect.y / self.config.height as f32 * 2.0),
                    ],
                    size: [
                        rect.width / self.config.width as f32 * 2.0,
                        rect.height / self.config.height as f32 * 2.0,
                    ],
                    tex_offset: solid_tex_offset,
                    tex_size: solid_tex_size,
                    color: render_fg_color,
                    is_colored: 0,
                };
                text_index += 1;
            }
            return Some(text_index);
        }

        // --- Block element geometry ---
        if let Some(geo_block) = block_chars::get_geometric_block(ch) {
            let rect = geo_block.to_pixel_rect(x0, y0, char_w, self.grid.cell_height);

            // 1 px extension to prevent gaps at cell edges.
            let extension = 1.0;
            let ext_x = if geo_block.x == 0.0 { extension } else { 0.0 };
            let ext_y = if geo_block.y == 0.0 { extension } else { 0.0 };
            let ext_w = if geo_block.x + geo_block.width >= 1.0 {
                extension
            } else {
                0.0
            };
            let ext_h = if geo_block.y + geo_block.height >= 1.0 {
                extension
            } else {
                0.0
            };

            if text_index < self.buffers.max_text_instances {
                self.text_instances[text_index] = TextInstance {
                    position: [
                        (rect.x - ext_x) / self.config.width as f32 * 2.0 - 1.0,
                        1.0 - ((rect.y - ext_y) / self.config.height as f32 * 2.0),
                    ],
                    size: [
                        (rect.width + ext_x + ext_w) / self.config.width as f32 * 2.0,
                        (rect.height + ext_y + ext_h) / self.config.height as f32 * 2.0,
                    ],
                    tex_offset: solid_tex_offset,
                    tex_size: solid_tex_size,
                    color: render_fg_color,
                    is_colored: 0,
                };
                text_index += 1;
            }
            return Some(text_index);
        }

        // Not a supported geometric block character.
        None
    }
}
