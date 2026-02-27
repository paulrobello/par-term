use super::block_chars;
use super::{BackgroundInstance, CellRenderer, RowCacheEntry, TextInstance};
use anyhow::Result;
use par_term_config::{color_u8x4_rgb_to_f32, color_u8x4_rgb_to_f32_a};
use par_term_fonts::text_shaper::ShapingOptions;

impl CellRenderer {
    pub(crate) fn build_instance_buffers(&mut self) -> Result<()> {
        let _shaping_options = ShapingOptions {
            enable_ligatures: self.font.enable_ligatures,
            enable_kerning: self.font.enable_kerning,
            ..Default::default()
        };

        for row in 0..self.grid.rows {
            if self.dirty_rows[row] || self.row_cache[row].is_none() {
                let start = row * self.grid.cols;
                let end = (row + 1) * self.grid.cols;
                let row_cells = &self.cells[start..end];

                self.scratch_row_bg.clear();
                self.scratch_row_text.clear();

                // Background - use RLE to merge consecutive cells with same color (like iTerm2)
                // This eliminates seams between adjacent same-colored cells
                let mut col = 0;
                while col < row_cells.len() {
                    let cell = &row_cells[col];
                    let bg_f = color_u8x4_rgb_to_f32(cell.bg_color);
                    let is_default_bg = (bg_f[0] - self.background_color[0]).abs() < 0.001
                        && (bg_f[1] - self.background_color[1]).abs() < 0.001
                        && (bg_f[2] - self.background_color[2]).abs() < 0.001;

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
                    let bg_alpha =
                        if self.transparency_affects_only_default_background && !is_default_bg {
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
                            && self.cursor.unfocused_style
                                == par_term_config::UnfocusedCursorStyle::Hollow;

                        match self.cursor.style {
                            CursorStyle::SteadyBlock | CursorStyle::BlinkingBlock => {
                                if render_hollow {
                                    // Hollow cursor: don't fill the cell, outline will be added later
                                    // Keep original background color
                                } else {
                                    // Solid block cursor
                                    for (bg, &cursor) in
                                        bg_color.iter_mut().take(3).zip(&self.cursor.color)
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

                // Text
                let mut x_offset = 0.0;
                #[allow(clippy::type_complexity)]
                let cell_data: Vec<(
                    String,
                    bool,
                    bool,
                    [u8; 4],
                    [u8; 4],
                    bool,
                    bool,
                )> = row_cells
                    .iter()
                    .map(|c| {
                        (
                            c.grapheme.clone(),
                            c.bold,
                            c.italic,
                            c.fg_color,
                            c.bg_color,
                            c.wide_char_spacer,
                            c.wide_char,
                        )
                    })
                    .collect();

                // Dynamic baseline calculation based on font metrics
                let natural_line_height =
                    self.font.font_ascent + self.font.font_descent + self.font.font_leading;
                let vertical_padding = (self.grid.cell_height - natural_line_height).max(0.0) / 2.0;
                let baseline_y_unrounded = self.grid.window_padding
                    + self.grid.content_offset_y
                    + (row as f32 * self.grid.cell_height)
                    + vertical_padding
                    + self.font.font_ascent;

                // Check if this row has the cursor and it's a visible block cursor
                // (for cursor text color override)
                let cursor_is_block_on_this_row = {
                    use par_term_emu_core_rust::cursor::CursorStyle;
                    self.cursor.pos.1 == row
                        && self.cursor.opacity > 0.0
                        && !self.cursor.hidden_for_shader
                        && matches!(
                            self.cursor.style,
                            CursorStyle::SteadyBlock | CursorStyle::BlinkingBlock
                        )
                        && (self.is_focused
                            || self.cursor.unfocused_style
                                == par_term_config::UnfocusedCursorStyle::Same)
                };

                let mut current_col = 0usize;
                for (grapheme, bold, italic, fg_color, bg_color, is_spacer, is_wide) in cell_data {
                    if is_spacer || grapheme == " " {
                        x_offset += self.grid.cell_width;
                        current_col += 1;
                        continue;
                    }

                    // Compute text alpha - force opaque if keep_text_opaque is enabled,
                    // otherwise use window opacity so text becomes transparent with the window
                    let text_alpha = if self.keep_text_opaque {
                        1.0
                    } else {
                        self.window_opacity
                    };

                    // Determine text color - use cursor_text_color if this is the cursor position
                    // with a block cursor, otherwise use the cell's foreground color
                    let render_fg_color: [f32; 4] =
                        if cursor_is_block_on_this_row && current_col == self.cursor.pos.0 {
                            if let Some(cursor_text) = self.cursor.text_color {
                                [cursor_text[0], cursor_text[1], cursor_text[2], text_alpha]
                            } else {
                                // Auto-contrast: use cursor color as a starting point
                                // Simple inversion: if cursor is bright, use dark text; if dark, use bright
                                let cursor_brightness = (self.cursor.color[0]
                                    + self.cursor.color[1]
                                    + self.cursor.color[2])
                                    / 3.0;
                                if cursor_brightness > 0.5 {
                                    [0.0, 0.0, 0.0, text_alpha] // Dark text on bright cursor
                                } else {
                                    [1.0, 1.0, 1.0, text_alpha] // Bright text on dark cursor
                                }
                            }
                        } else {
                            // Determine the effective background color for contrast calculation
                            // If the cell has a non-default bg, use that; otherwise use terminal background
                            let effective_bg = if bg_color[3] > 0 {
                                // Cell has explicit background
                                color_u8x4_rgb_to_f32_a(bg_color, 1.0)
                            } else {
                                // Use terminal default background
                                [
                                    self.background_color[0],
                                    self.background_color[1],
                                    self.background_color[2],
                                    1.0,
                                ]
                            };

                            let base_fg = color_u8x4_rgb_to_f32_a(fg_color, text_alpha);

                            // Apply minimum contrast adjustment if enabled
                            self.ensure_minimum_contrast(base_fg, effective_bg)
                        };

                    let chars: Vec<char> = grapheme.chars().collect();
                    #[allow(clippy::collapsible_if)]
                    if let Some(ch) = chars.first() {
                        // Classify the character for rendering optimization
                        // Only classify based on first char for block drawing detection
                        let char_type = block_chars::classify_char(*ch);

                        // Check if we should render this character geometrically
                        // (only for single-char graphemes that are block drawing chars)
                        if chars.len() == 1 && block_chars::should_render_geometrically(char_type) {
                            let char_w = if is_wide {
                                self.grid.cell_width * 2.0
                            } else {
                                self.grid.cell_width
                            };
                            let x0 =
                                (self.grid.window_padding + self.grid.content_offset_x + x_offset)
                                    .round();
                            let y0 = (self.grid.window_padding
                                + self.grid.content_offset_y
                                + row as f32 * self.grid.cell_height)
                                .round();

                            // Try box drawing geometry first (for lines, corners, junctions)
                            // Pass aspect ratio so vertical lines have same visual thickness as horizontal
                            let aspect_ratio = self.grid.cell_height / char_w;
                            if let Some(box_geo) =
                                block_chars::get_box_drawing_geometry(*ch, aspect_ratio)
                            {
                                for segment in &box_geo.segments {
                                    let rect = segment
                                        .to_pixel_rect(x0, y0, char_w, self.grid.cell_height)
                                        .snap_to_pixels();

                                    // Extend segments that touch cell edges
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

                                    let final_x = rect.x - ext_x;
                                    let final_y = rect.y - ext_y;
                                    let final_w = rect.width + ext_x + ext_w;
                                    let final_h = rect.height + ext_y + ext_h;

                                    self.scratch_row_text.push(TextInstance {
                                        position: [
                                            final_x / self.config.width as f32 * 2.0 - 1.0,
                                            1.0 - (final_y / self.config.height as f32 * 2.0),
                                        ],
                                        size: [
                                            final_w / self.config.width as f32 * 2.0,
                                            final_h / self.config.height as f32 * 2.0,
                                        ],
                                        tex_offset: [
                                            self.atlas.solid_pixel_offset.0 as f32 / 2048.0,
                                            self.atlas.solid_pixel_offset.1 as f32 / 2048.0,
                                        ],
                                        tex_size: [1.0 / 2048.0, 1.0 / 2048.0],
                                        color: render_fg_color,
                                        is_colored: 0,
                                    });
                                }
                                x_offset += self.grid.cell_width;
                                current_col += 1;
                                continue;
                            }

                            // Try block element geometry (for solid blocks, half blocks, etc.)
                            if let Some(geo_block) = block_chars::get_geometric_block(*ch) {
                                let rect =
                                    geo_block.to_pixel_rect(x0, y0, char_w, self.grid.cell_height);

                                // Add small extension to prevent gaps (1 pixel overlap)
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

                                let final_x = rect.x - ext_x;
                                let final_y = rect.y - ext_y;
                                let final_w = rect.width + ext_x + ext_w;
                                let final_h = rect.height + ext_y + ext_h;

                                // Render as a colored rectangle using the solid white pixel in atlas
                                // This goes through the text pipeline with foreground color
                                self.scratch_row_text.push(TextInstance {
                                    position: [
                                        final_x / self.config.width as f32 * 2.0 - 1.0,
                                        1.0 - (final_y / self.config.height as f32 * 2.0),
                                    ],
                                    size: [
                                        final_w / self.config.width as f32 * 2.0,
                                        final_h / self.config.height as f32 * 2.0,
                                    ],
                                    // Use solid white pixel from atlas
                                    tex_offset: [
                                        self.atlas.solid_pixel_offset.0 as f32 / 2048.0,
                                        self.atlas.solid_pixel_offset.1 as f32 / 2048.0,
                                    ],
                                    tex_size: [1.0 / 2048.0, 1.0 / 2048.0],
                                    color: render_fg_color,
                                    is_colored: 0,
                                });

                                x_offset += self.grid.cell_width;
                                current_col += 1;
                                continue;
                            }

                            // Try geometric shape (aspect-ratio-aware squares, rectangles)
                            if let Some(rect) = block_chars::get_geometric_shape_rect(
                                *ch,
                                x0,
                                y0,
                                char_w,
                                self.grid.cell_height,
                            ) {
                                self.scratch_row_text.push(TextInstance {
                                    position: [
                                        rect.x / self.config.width as f32 * 2.0 - 1.0,
                                        1.0 - (rect.y / self.config.height as f32 * 2.0),
                                    ],
                                    size: [
                                        rect.width / self.config.width as f32 * 2.0,
                                        rect.height / self.config.height as f32 * 2.0,
                                    ],
                                    tex_offset: [
                                        self.atlas.solid_pixel_offset.0 as f32 / 2048.0,
                                        self.atlas.solid_pixel_offset.1 as f32 / 2048.0,
                                    ],
                                    tex_size: [1.0 / 2048.0, 1.0 / 2048.0],
                                    color: render_fg_color,
                                    is_colored: 0,
                                });

                                x_offset += self.grid.cell_width;
                                current_col += 1;
                                continue;
                            }
                        }

                        // Check if this character should be rendered as a monochrome symbol
                        // (dingbats, etc.) rather than colorful emoji.
                        // Also handle symbol + VS16 (U+FE0F emoji presentation selector):
                        // in terminal contexts, symbols should remain monochrome even with VS16.
                        let (force_monochrome, base_char) = if chars.len() == 1 {
                            (super::atlas::should_render_as_symbol(*ch), *ch)
                        } else if chars.len() == 2
                            && chars[1] == '\u{FE0F}'
                            && super::atlas::should_render_as_symbol(chars[0])
                        {
                            // Symbol + VS16: strip VS16 and render base char as monochrome
                            (true, chars[0])
                        } else {
                            (false, *ch)
                        };

                        // Use grapheme-aware glyph lookup for multi-character sequences
                        // (flags, emoji with skin tones, ZWJ sequences, combining chars).
                        // When force_monochrome strips VS16, use single-char lookup instead.
                        let mut glyph_result = if force_monochrome || chars.len() == 1 {
                            self.font_manager.find_glyph(base_char, bold, italic)
                        } else {
                            self.font_manager
                                .find_grapheme_glyph(&grapheme, bold, italic)
                        };

                        // Try to find a renderable glyph. Some fonts (e.g., Apple Color
                        // Emoji) have charmap entries for characters but produce empty
                        // outlines. When rasterization fails, retry with alternative fonts.
                        let mut excluded_fonts: Vec<usize> = Vec::new();
                        let resolved_info = loop {
                            match glyph_result {
                                Some((font_idx, glyph_id)) => {
                                    let cache_key = ((font_idx as u64) << 32) | (glyph_id as u64);
                                    if self.atlas.glyph_cache.contains_key(&cache_key) {
                                        self.lru_remove(cache_key);
                                        self.lru_push_front(cache_key);
                                        break Some(
                                            self.atlas.glyph_cache.get(&cache_key).expect("Glyph cache entry must exist after contains_key check").clone(),
                                        );
                                    } else if let Some(raster) =
                                        self.rasterize_glyph(font_idx, glyph_id, force_monochrome)
                                    {
                                        let info = self.upload_glyph(cache_key, &raster);
                                        self.atlas.glyph_cache.insert(cache_key, info.clone());
                                        self.lru_push_front(cache_key);
                                        break Some(info);
                                    } else {
                                        // Rasterization failed — try next font
                                        excluded_fonts.push(font_idx);
                                        glyph_result = self.font_manager.find_glyph_excluding(
                                            base_char,
                                            bold,
                                            italic,
                                            &excluded_fonts,
                                        );
                                        continue;
                                    }
                                }
                                None => break None,
                            }
                        };

                        // Last resort: if monochrome rendering failed across all fonts
                        // (no font has vector outlines for this character), retry with
                        // colored emoji rendering. Characters like ✨ only exist in
                        // Apple Color Emoji — rendering them colored is better than
                        // rendering nothing.
                        let resolved_info = if resolved_info.is_none() && force_monochrome {
                            let mut glyph_result2 =
                                self.font_manager.find_glyph(base_char, bold, italic);
                            loop {
                                match glyph_result2 {
                                    Some((font_idx, glyph_id)) => {
                                        let cache_key = ((font_idx as u64) << 32)
                                            | (glyph_id as u64)
                                            | (1u64 << 63); // different cache key for colored
                                        if let Some(raster) =
                                            self.rasterize_glyph(font_idx, glyph_id, false)
                                        {
                                            let info = self.upload_glyph(cache_key, &raster);
                                            self.atlas.glyph_cache.insert(cache_key, info.clone());
                                            self.lru_push_front(cache_key);
                                            break Some(info);
                                        } else {
                                            glyph_result2 = self.font_manager.find_glyph_excluding(
                                                base_char,
                                                bold,
                                                italic,
                                                &[font_idx],
                                            );
                                            continue;
                                        }
                                    }
                                    None => break None,
                                }
                            }
                        } else {
                            resolved_info
                        };

                        let info = match resolved_info {
                            Some(info) => info,
                            None => {
                                x_offset += self.grid.cell_width;
                                continue;
                            }
                        };

                        let char_w = if is_wide {
                            self.grid.cell_width * 2.0
                        } else {
                            self.grid.cell_width
                        };
                        let x0 = (self.grid.window_padding + self.grid.content_offset_x + x_offset)
                            .round();
                        let x1 = (self.grid.window_padding
                            + self.grid.content_offset_x
                            + x_offset
                            + char_w)
                            .round();
                        let y0 = (self.grid.window_padding
                            + self.grid.content_offset_y
                            + row as f32 * self.grid.cell_height)
                            .round();
                        let y1 = (self.grid.window_padding
                            + self.grid.content_offset_y
                            + (row + 1) as f32 * self.grid.cell_height)
                            .round();

                        let cell_w = x1 - x0;
                        let cell_h = y1 - y0;

                        let scale_x = cell_w / char_w;
                        let scale_y = cell_h / self.grid.cell_height;

                        // Position glyph relative to snapped cell top-left.
                        // Round the scaled baseline position once, then subtract
                        // the integer bearing_y. This ensures all glyphs on a row
                        // share the same rounded baseline, with bearing offsets
                        // applied exactly (no scale_y on bearing avoids rounding
                        // artifacts between glyphs with different bearings).
                        let baseline_offset = baseline_y_unrounded
                            - (self.grid.window_padding
                                + self.grid.content_offset_y
                                + row as f32 * self.grid.cell_height);
                        let glyph_left = x0 + (info.bearing_x * scale_x).round();
                        let baseline_in_cell = (baseline_offset * scale_y).round();
                        let glyph_top = y0 + baseline_in_cell - info.bearing_y;

                        let render_w = info.width as f32 * scale_x;
                        let render_h = info.height as f32 * scale_y;

                        // For block characters that need font rendering (box drawing, etc.),
                        // apply snapping to cell boundaries with sub-pixel extension.
                        // Only apply to single-char graphemes (multi-char are never block chars)
                        let (final_left, final_top, final_w, final_h) = if chars.len() == 1
                            && block_chars::should_snap_to_boundaries(char_type)
                        {
                            // Snap threshold of 3 pixels, extension of 0.5 pixels
                            block_chars::snap_glyph_to_cell(
                                glyph_left, glyph_top, render_w, render_h, x0, y0, x1, y1, 3.0, 0.5,
                            )
                        } else {
                            (glyph_left, glyph_top, render_w, render_h)
                        };

                        self.scratch_row_text.push(TextInstance {
                            position: [
                                final_left / self.config.width as f32 * 2.0 - 1.0,
                                1.0 - (final_top / self.config.height as f32 * 2.0),
                            ],
                            size: [
                                final_w / self.config.width as f32 * 2.0,
                                final_h / self.config.height as f32 * 2.0,
                            ],
                            tex_offset: [info.x as f32 / 2048.0, info.y as f32 / 2048.0],
                            tex_size: [info.width as f32 / 2048.0, info.height as f32 / 2048.0],
                            color: render_fg_color,
                            is_colored: if info.is_colored { 1 } else { 0 },
                        });
                    }
                    x_offset += self.grid.cell_width;
                    current_col += 1;
                }

                // Underlines: emit thin rectangle(s) at the bottom of each underlined cell
                {
                    let underline_thickness = (self.grid.cell_height * 0.07).max(1.0).round();
                    let tex_offset = [
                        self.atlas.solid_pixel_offset.0 as f32 / 2048.0,
                        self.atlas.solid_pixel_offset.1 as f32 / 2048.0,
                    ];
                    let tex_size = [1.0 / 2048.0, 1.0 / 2048.0];
                    let y0 = self.grid.window_padding
                        + self.grid.content_offset_y
                        + (row + 1) as f32 * self.grid.cell_height
                        - underline_thickness;
                    let ndc_y = 1.0 - (y0 / self.config.height as f32 * 2.0);
                    let ndc_h = underline_thickness / self.config.height as f32 * 2.0;
                    let is_stipple =
                        self.link_underline_style == par_term_config::LinkUnderlineStyle::Stipple;
                    // Stipple: 2px on, 2px off pattern
                    let stipple_on = 2.0_f32;
                    let stipple_off = 2.0_f32;
                    let stipple_period = stipple_on + stipple_off;

                    for col_idx in 0..self.grid.cols {
                        let cell = &self.cells[start + col_idx];
                        if !cell.underline || self.scratch_row_text.len() >= self.grid.cols * 2 {
                            continue;
                        }
                        let text_alpha = if self.keep_text_opaque {
                            1.0
                        } else {
                            self.window_opacity
                        };
                        let fg = color_u8x4_rgb_to_f32_a(cell.fg_color, text_alpha);
                        let cell_x0 = self.grid.window_padding
                            + self.grid.content_offset_x
                            + col_idx as f32 * self.grid.cell_width;

                        if is_stipple {
                            // Emit alternating dot segments across the cell width
                            let mut px = 0.0;
                            while px < self.grid.cell_width && self.scratch_row_text.len() < self.grid.cols * 2 {
                                let seg_w = stipple_on.min(self.grid.cell_width - px);
                                let x = cell_x0 + px;
                                self.scratch_row_text.push(TextInstance {
                                    position: [x / self.config.width as f32 * 2.0 - 1.0, ndc_y],
                                    size: [seg_w / self.config.width as f32 * 2.0, ndc_h],
                                    tex_offset,
                                    tex_size,
                                    color: fg,
                                    is_colored: 0,
                                });
                                px += stipple_period;
                            }
                        } else {
                            self.scratch_row_text.push(TextInstance {
                                position: [cell_x0 / self.config.width as f32 * 2.0 - 1.0, ndc_y],
                                size: [
                                    self.grid.cell_width / self.config.width as f32 * 2.0,
                                    ndc_h,
                                ],
                                tex_offset,
                                tex_size,
                                color: fg,
                                is_colored: 0,
                            });
                        }
                    }
                }

                // Update CPU-side buffers
                let bg_start = row * self.grid.cols;
                self.bg_instances[bg_start..bg_start + self.grid.cols].copy_from_slice(&self.scratch_row_bg);

                let text_start = row * self.grid.cols * 2;
                // Clear row text segment first
                for i in 0..(self.grid.cols * 2) {
                    self.text_instances[text_start + i].size = [0.0, 0.0];
                }
                // Copy new text instances
                let text_count = self.scratch_row_text.len().min(self.grid.cols * 2);
                self.text_instances[text_start..text_start + text_count]
                    .copy_from_slice(&self.scratch_row_text[..text_count]);

                // Update GPU-side buffers incrementally
                self.queue.write_buffer(
                    &self.buffers.bg_instance_buffer,
                    (bg_start * std::mem::size_of::<BackgroundInstance>()) as u64,
                    bytemuck::cast_slice(&self.scratch_row_bg),
                );
                self.queue.write_buffer(
                    &self.buffers.text_instance_buffer,
                    (text_start * std::mem::size_of::<TextInstance>()) as u64,
                    bytemuck::cast_slice(
                        &self.text_instances[text_start..text_start + self.grid.cols * 2],
                    ),
                );

                self.row_cache[row] = Some(RowCacheEntry {});
                self.dirty_rows[row] = false;
            }
        }

        // Write cursor-related overlays to extra slots at the end of bg_instances
        // Slot layout: [0] cursor overlay (beam/underline), [1] guide, [2] shadow, [3-6] boost glow, [7-10] hollow outline
        let base_overlay_index = self.grid.cols * self.grid.rows;
        let mut overlay_instances = vec![
            BackgroundInstance {
                position: [0.0, 0.0],
                size: [0.0, 0.0],
                color: [0.0, 0.0, 0.0, 0.0],
            };
            10
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
                    self.cursor.boost * 0.3 * self.cursor.opacity, // Max 30% alpha
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
                let border_width = 2.0; // 2 pixel border
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

        // Write all overlay instances to GPU buffer
        for (i, instance) in overlay_instances.iter().enumerate() {
            self.bg_instances[base_overlay_index + i] = *instance;
        }
        self.queue.write_buffer(
            &self.buffers.bg_instance_buffer,
            (base_overlay_index * std::mem::size_of::<BackgroundInstance>()) as u64,
            bytemuck::cast_slice(&overlay_instances),
        );

        // Write command separator line instances after cursor overlay slots
        let separator_base = self.grid.cols * self.grid.rows + 10;
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

        for (i, instance) in separator_instances.iter().enumerate() {
            if separator_base + i < self.buffers.max_bg_instances {
                self.bg_instances[separator_base + i] = *instance;
            }
        }
        let separator_byte_offset = separator_base * std::mem::size_of::<BackgroundInstance>();
        let separator_byte_count =
            separator_instances.len() * std::mem::size_of::<BackgroundInstance>();
        if separator_byte_offset + separator_byte_count
            <= self.buffers.max_bg_instances * std::mem::size_of::<BackgroundInstance>()
        {
            self.queue.write_buffer(
                &self.buffers.bg_instance_buffer,
                separator_byte_offset as u64,
                bytemuck::cast_slice(&separator_instances),
            );
        }

        // Write gutter indicator background instances after separator slots
        let gutter_base = separator_base + self.grid.rows;
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
                    let x1 = x0 + 2.0 * self.grid.cell_width; // gutter_width = 2 columns
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

        for (i, instance) in gutter_instances.iter().enumerate() {
            if gutter_base + i < self.buffers.max_bg_instances {
                self.bg_instances[gutter_base + i] = *instance;
            }
        }
        let gutter_byte_offset = gutter_base * std::mem::size_of::<BackgroundInstance>();
        let gutter_byte_count = gutter_instances.len() * std::mem::size_of::<BackgroundInstance>();
        if gutter_byte_offset + gutter_byte_count
            <= self.buffers.max_bg_instances * std::mem::size_of::<BackgroundInstance>()
        {
            self.queue.write_buffer(
                &self.buffers.bg_instance_buffer,
                gutter_byte_offset as u64,
                bytemuck::cast_slice(&gutter_instances),
            );
        }

        // Update actual instance counts for draw calls
        // Layout: [0..cols*rows] cells + [cols*rows..cols*rows+10] overlays + [cols*rows+10..+rows] separators + [cols*rows+10+rows..+rows] gutters
        self.buffers.actual_bg_instances =
            self.grid.cols * self.grid.rows + 10 + self.grid.rows + self.grid.rows;
        self.buffers.actual_text_instances = self.grid.cols * self.grid.rows * 2;

        Ok(())
    }
}
