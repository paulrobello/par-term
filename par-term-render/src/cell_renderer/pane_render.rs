use super::block_chars;
use super::{BackgroundInstance, Cell, CellRenderer, PaneViewport, TextInstance};
use anyhow::Result;
use par_term_config::{SeparatorMark, color_u8x4_rgb_to_f32, color_u8x4_rgb_to_f32_a};
use par_term_fonts::text_shaper::ShapingOptions;

impl CellRenderer {
    /// Render a single pane's content within a viewport to an existing surface texture
    ///
    /// This method renders cells to a specific region of the render target,
    /// using a GPU scissor rect to clip to the pane bounds.
    ///
    /// # Arguments
    /// * `surface_view` - The texture view to render to
    /// * `viewport` - The pane's viewport (position, size, focus state, opacity)
    /// * `cells` - The cells to render (should match viewport grid size)
    /// * `cols` - Number of columns in the cell grid
    /// * `rows` - Number of rows in the cell grid
    /// * `cursor_pos` - Cursor position (col, row) within this pane, or None if no cursor
    /// * `cursor_opacity` - Cursor opacity (0.0 = hidden, 1.0 = fully visible)
    /// * `show_scrollbar` - Whether to render the scrollbar for this pane
    /// * `clear_first` - If true, clears the viewport region before rendering
    /// * `skip_background_image` - If true, skip rendering the background image. Use this
    ///   when the background image has already been rendered full-screen (for split panes).
    #[allow(dead_code, clippy::too_many_arguments)]
    pub fn render_pane_to_view(
        &mut self,
        surface_view: &wgpu::TextureView,
        viewport: &PaneViewport,
        cells: &[Cell],
        cols: usize,
        rows: usize,
        cursor_pos: Option<(usize, usize)>,
        cursor_opacity: f32,
        show_scrollbar: bool,
        clear_first: bool,
        skip_background_image: bool,
        separator_marks: &[SeparatorMark],
        pane_background: Option<&par_term_config::PaneBackground>,
    ) -> Result<()> {
        // Build instance buffers for this pane's cells
        // Skip solid background fill if background (shader/image) was already rendered full-screen
        self.build_pane_instance_buffers(
            viewport,
            cells,
            cols,
            rows,
            cursor_pos,
            cursor_opacity,
            skip_background_image,
            separator_marks,
        )?;

        // Pre-create per-pane background bind group if needed (must happen before render pass).
        // Per-pane backgrounds are explicit user overrides and always created,
        // even when a custom shader or global background would normally be skipped.
        let pane_bg_resources = if let Some(pane_bg) = pane_background
            && let Some(ref path) = pane_bg.image_path
        {
            self.bg_state.pane_bg_cache.get(path.as_str()).map(|entry| {
                self.create_pane_bg_bind_group(
                    entry,
                    viewport.x,
                    viewport.y,
                    viewport.width,
                    viewport.height,
                    pane_bg.mode,
                    pane_bg.opacity,
                    pane_bg.darken,
                )
            })
        } else {
            None
        };

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("pane render encoder"),
            });

        // Determine load operation and clear color
        let load_op = if clear_first {
            let clear_color = if self.bg_state.bg_is_solid_color {
                wgpu::Color {
                    r: self.bg_state.solid_bg_color[0] as f64
                        * self.window_opacity as f64
                        * viewport.opacity as f64,
                    g: self.bg_state.solid_bg_color[1] as f64
                        * self.window_opacity as f64
                        * viewport.opacity as f64,
                    b: self.bg_state.solid_bg_color[2] as f64
                        * self.window_opacity as f64
                        * viewport.opacity as f64,
                    a: self.window_opacity as f64 * viewport.opacity as f64,
                }
            } else {
                wgpu::Color {
                    r: self.background_color[0] as f64
                        * self.window_opacity as f64
                        * viewport.opacity as f64,
                    g: self.background_color[1] as f64
                        * self.window_opacity as f64
                        * viewport.opacity as f64,
                    b: self.background_color[2] as f64
                        * self.window_opacity as f64
                        * viewport.opacity as f64,
                    a: self.window_opacity as f64 * viewport.opacity as f64,
                }
            };
            wgpu::LoadOp::Clear(clear_color)
        } else {
            wgpu::LoadOp::Load
        };

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("pane render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: load_op,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Set scissor rect to clip rendering to pane bounds
            let (sx, sy, sw, sh) = viewport.to_scissor_rect();
            render_pass.set_scissor_rect(sx, sy, sw, sh);

            // Render per-pane background image within scissor rect.
            // Per-pane backgrounds are explicit user overrides and always render,
            // even when a custom shader or global background is active.
            if let Some((ref bind_group, ref _buf)) = pane_bg_resources {
                render_pass.set_pipeline(&self.pipelines.bg_image_pipeline);
                render_pass.set_bind_group(0, bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.buffers.vertex_buffer.slice(..));
                render_pass.draw(0..4, 0..1);
            }

            // Render cell backgrounds
            render_pass.set_pipeline(&self.pipelines.bg_pipeline);
            render_pass.set_vertex_buffer(0, self.buffers.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.buffers.bg_instance_buffer.slice(..));
            render_pass.draw(0..4, 0..self.buffers.max_bg_instances as u32);

            // Render text
            render_pass.set_pipeline(&self.pipelines.text_pipeline);
            render_pass.set_bind_group(0, &self.pipelines.text_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.buffers.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.buffers.text_instance_buffer.slice(..));
            render_pass.draw(0..4, 0..self.buffers.max_text_instances as u32);

            // Render scrollbar if requested (uses its own scissor rect internally)
            if show_scrollbar {
                // Reset scissor to full surface for scrollbar
                render_pass.set_scissor_rect(0, 0, self.config.width, self.config.height);
                self.scrollbar.render(&mut render_pass);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        Ok(())
    }

    /// Build instance buffers for a pane's cells with viewport offset
    ///
    /// This is similar to `build_instance_buffers` but adjusts all positions
    /// to be relative to the viewport origin.
    ///
    /// # Arguments
    /// * `skip_solid_background` - If true, skip adding a solid background fill for the viewport.
    ///   Use when a custom shader or background image was already rendered full-screen.
    #[allow(clippy::too_many_arguments)]
    fn build_pane_instance_buffers(
        &mut self,
        viewport: &PaneViewport,
        cells: &[Cell],
        cols: usize,
        rows: usize,
        cursor_pos: Option<(usize, usize)>,
        cursor_opacity: f32,
        skip_solid_background: bool,
        separator_marks: &[SeparatorMark],
    ) -> Result<()> {
        let _shaping_options = ShapingOptions {
            enable_ligatures: self.font.enable_ligatures,
            enable_kerning: self.font.enable_kerning,
            ..Default::default()
        };

        // Clear previous instance buffers
        for instance in &mut self.bg_instances {
            instance.size = [0.0, 0.0];
            instance.color = [0.0, 0.0, 0.0, 0.0];
        }

        // Add a background rectangle covering the entire pane viewport (unless skipped)
        // This ensures the pane has a proper background even when cells are skipped.
        // Skip when a custom shader or background image was already rendered full-screen.
        let bg_start_index = if !skip_solid_background && !self.bg_instances.is_empty() {
            let bg_color = self.background_color;
            let opacity = self.window_opacity * viewport.opacity;
            self.bg_instances[0] = super::types::BackgroundInstance {
                position: [viewport.x, viewport.y],
                size: [viewport.width, viewport.height],
                color: [
                    bg_color[0] * opacity,
                    bg_color[1] * opacity,
                    bg_color[2] * opacity,
                    opacity,
                ],
            };
            1 // Start cell backgrounds at index 1
        } else {
            0 // Start cell backgrounds at index 0 (no viewport fill)
        };

        for instance in &mut self.text_instances {
            instance.size = [0.0, 0.0];
        }

        // Start at bg_start_index (1 if viewport fill was added, 0 otherwise)
        let mut bg_index = bg_start_index;
        let mut text_index = 0;

        // Content offset - positions are relative to content area (with padding applied)
        let (content_x, content_y) = viewport.content_origin();
        let opacity_multiplier = viewport.opacity;

        for row in 0..rows {
            let row_start = row * cols;
            let row_end = (row + 1) * cols;
            if row_start >= cells.len() {
                break;
            }
            let row_cells = &cells[row_start..row_end.min(cells.len())];

            // Background - use RLE to merge consecutive cells with same color
            let mut col = 0;
            while col < row_cells.len() {
                let cell = &row_cells[col];
                let bg_f = color_u8x4_rgb_to_f32(cell.bg_color);
                let is_default_bg = (bg_f[0] - self.background_color[0]).abs() < 0.001
                    && (bg_f[1] - self.background_color[1]).abs() < 0.001
                    && (bg_f[2] - self.background_color[2]).abs() < 0.001;

                // Check for cursor at this position
                let has_cursor = cursor_pos.is_some_and(|(cx, cy)| cx == col && cy == row)
                    && cursor_opacity > 0.0
                    && !self.cursor.hidden_for_shader;

                if is_default_bg && !has_cursor {
                    col += 1;
                    continue;
                }

                // Calculate background color with alpha and pane opacity
                let bg_alpha =
                    if self.transparency_affects_only_default_background && !is_default_bg {
                        1.0
                    } else {
                        self.window_opacity
                    };
                let pane_alpha = bg_alpha * opacity_multiplier;
                let mut bg_color = color_u8x4_rgb_to_f32_a(cell.bg_color, pane_alpha);

                // Handle cursor at this position
                if has_cursor {
                    use par_term_emu_core_rust::cursor::CursorStyle;
                    match self.cursor.style {
                        CursorStyle::SteadyBlock | CursorStyle::BlinkingBlock => {
                            for (bg, &cursor) in bg_color.iter_mut().take(3).zip(&self.cursor.color)
                            {
                                *bg = *bg * (1.0 - cursor_opacity) + cursor * cursor_opacity;
                            }
                            bg_color[3] = bg_color[3].max(cursor_opacity * opacity_multiplier);
                        }
                        _ => {}
                    }

                    // Cursor cell can't be merged
                    let x0 = content_x + col as f32 * self.grid.cell_width;
                    let y0 = content_y + row as f32 * self.grid.cell_height;
                    let x1 = x0 + self.grid.cell_width;
                    let y1 = y0 + self.grid.cell_height;

                    if bg_index < self.buffers.max_bg_instances {
                        self.bg_instances[bg_index] = BackgroundInstance {
                            position: [
                                x0 / self.config.width as f32 * 2.0 - 1.0,
                                1.0 - (y0 / self.config.height as f32 * 2.0),
                            ],
                            size: [
                                (x1 - x0) / self.config.width as f32 * 2.0,
                                (y1 - y0) / self.config.height as f32 * 2.0,
                            ],
                            color: bg_color,
                        };
                        bg_index += 1;
                    }
                    col += 1;
                    continue;
                }

                // RLE: Find run of consecutive cells with same background color
                let start_col = col;
                let run_color = cell.bg_color;
                col += 1;
                while col < row_cells.len() {
                    let next_cell = &row_cells[col];
                    let next_has_cursor = cursor_pos.is_some_and(|(cx, cy)| cx == col && cy == row)
                        && cursor_opacity > 0.0;
                    if next_cell.bg_color != run_color || next_has_cursor {
                        break;
                    }
                    col += 1;
                }
                let run_length = col - start_col;

                // Create single quad spanning entire run
                let x0 = content_x + start_col as f32 * self.grid.cell_width;
                let x1 = content_x + (start_col + run_length) as f32 * self.grid.cell_width;
                let y0 = content_y + row as f32 * self.grid.cell_height;
                let y1 = y0 + self.grid.cell_height;

                if bg_index < self.buffers.max_bg_instances {
                    self.bg_instances[bg_index] = BackgroundInstance {
                        position: [
                            x0 / self.config.width as f32 * 2.0 - 1.0,
                            1.0 - (y0 / self.config.height as f32 * 2.0),
                        ],
                        size: [
                            (x1 - x0) / self.config.width as f32 * 2.0,
                            (y1 - y0) / self.config.height as f32 * 2.0,
                        ],
                        color: bg_color,
                    };
                    bg_index += 1;
                }
            }

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

            for (col_idx, cell) in row_cells.iter().enumerate() {
                if cell.wide_char_spacer || cell.grapheme == " " {
                    continue;
                }

                let chars: Vec<char> = cell.grapheme.chars().collect();
                if chars.is_empty() {
                    continue;
                }

                let ch = chars[0];

                // Check for block characters that should be rendered geometrically
                let char_type = block_chars::classify_char(ch);
                if chars.len() == 1 && block_chars::should_render_geometrically(char_type) {
                    let char_w = if cell.wide_char {
                        self.grid.cell_width * 2.0
                    } else {
                        self.grid.cell_width
                    };
                    let x0 = content_x + col_idx as f32 * self.grid.cell_width;
                    let y0 = content_y + row as f32 * self.grid.cell_height;

                    let fg_color = color_u8x4_rgb_to_f32_a(cell.fg_color, text_alpha);

                    // Try box drawing geometry first
                    let aspect_ratio = self.grid.cell_height / char_w;
                    if let Some(box_geo) = block_chars::get_box_drawing_geometry(ch, aspect_ratio) {
                        for segment in &box_geo.segments {
                            let rect = segment
                                .to_pixel_rect(x0, y0, char_w, self.grid.cell_height)
                                .snap_to_pixels();

                            // Extension for seamless lines
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

                            if text_index < self.buffers.max_text_instances {
                                self.text_instances[text_index] = TextInstance {
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
                                    color: fg_color,
                                    is_colored: 0,
                                };
                                text_index += 1;
                            }
                        }
                        continue;
                    }

                    // Try block element geometry
                    if let Some(geo_block) = block_chars::get_geometric_block(ch) {
                        let rect = geo_block.to_pixel_rect(x0, y0, char_w, self.grid.cell_height);

                        // Extension for seamless blocks
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

                        if text_index < self.buffers.max_text_instances {
                            self.text_instances[text_index] = TextInstance {
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
                                color: fg_color,
                                is_colored: 0,
                            };
                            text_index += 1;
                        }
                        continue;
                    }
                }

                // Check if this character should be rendered as a monochrome symbol.
                // Also handle symbol + VS16 (U+FE0F): strip VS16, render monochrome.
                let (force_monochrome, base_char) = if chars.len() == 1 {
                    (super::atlas::should_render_as_symbol(ch), ch)
                } else if chars.len() == 2
                    && chars[1] == '\u{FE0F}'
                    && super::atlas::should_render_as_symbol(chars[0])
                {
                    (true, chars[0])
                } else {
                    (false, ch)
                };

                // Regular glyph rendering — use single-char lookup when force_monochrome
                // strips VS16, otherwise grapheme-aware lookup for multi-char sequences.
                let mut glyph_result = if force_monochrome || chars.len() == 1 {
                    self.font_manager
                        .find_glyph(base_char, cell.bold, cell.italic)
                } else {
                    self.font_manager
                        .find_grapheme_glyph(&cell.grapheme, cell.bold, cell.italic)
                };

                // Try to find a renderable glyph with font fallback for failures.
                let mut excluded_fonts: Vec<usize> = Vec::new();
                let resolved_info = loop {
                    match glyph_result {
                        Some((font_idx, glyph_id)) => {
                            let cache_key = ((font_idx as u64) << 32) | (glyph_id as u64);
                            if self.atlas.glyph_cache.contains_key(&cache_key) {
                                self.lru_remove(cache_key);
                                self.lru_push_front(cache_key);
                                break Some(
                                    self.atlas
                                        .glyph_cache
                                        .get(&cache_key)
                                        .expect(
                                            "Glyph cache entry must exist after contains_key check",
                                        )
                                        .clone(),
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
                                    cell.bold,
                                    cell.italic,
                                    &excluded_fonts,
                                );
                                continue;
                            }
                        }
                        None => break None,
                    }
                };

                // Last resort: colored emoji when no font has vector outlines
                let resolved_info = if resolved_info.is_none() && force_monochrome {
                    let mut glyph_result2 =
                        self.font_manager
                            .find_glyph(base_char, cell.bold, cell.italic);
                    loop {
                        match glyph_result2 {
                            Some((font_idx, glyph_id)) => {
                                let cache_key =
                                    ((font_idx as u64) << 32) | (glyph_id as u64) | (1u64 << 63);
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
                                        cell.bold,
                                        cell.italic,
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

                    let baseline_offset =
                        baseline_y - (content_y + row as f32 * self.grid.cell_height);
                    let glyph_left = x0 + (info.bearing_x * scale_x).round();
                    let baseline_in_cell = (baseline_offset * scale_y).round();
                    let glyph_top = y0 + baseline_in_cell - info.bearing_y;

                    let render_w = info.width as f32 * scale_x;
                    let render_h = info.height as f32 * scale_y;

                    let (final_left, final_top, final_w, final_h) =
                        if chars.len() == 1 && block_chars::should_snap_to_boundaries(char_type) {
                            block_chars::snap_glyph_to_cell(
                                glyph_left, glyph_top, render_w, render_h, x0, y0, x1, y1, 3.0, 0.5,
                            )
                        } else {
                            (glyph_left, glyph_top, render_w, render_h)
                        };

                    let fg_color = color_u8x4_rgb_to_f32_a(cell.fg_color, text_alpha);

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
                            tex_offset: [info.x as f32 / 2048.0, info.y as f32 / 2048.0],
                            tex_size: [info.width as f32 / 2048.0, info.height as f32 / 2048.0],
                            color: fg_color,
                            is_colored: if info.is_colored { 1 } else { 0 },
                        };
                        text_index += 1;
                    }
                }
            }
        }

        // Inject command separator line instances for split panes
        if self.separator.enabled && !separator_marks.is_empty() {
            let width_f = self.config.width as f32;
            let height_f = self.config.height as f32;
            let opacity_multiplier = viewport.opacity;
            for &(screen_row, exit_code, custom_color) in separator_marks {
                if screen_row < rows && bg_index < self.buffers.max_bg_instances {
                    let x0 = content_x;
                    let x1 = content_x + cols as f32 * self.grid.cell_width;
                    let y0 = content_y + screen_row as f32 * self.grid.cell_height;
                    let color = self.separator_color(exit_code, custom_color, opacity_multiplier);
                    self.bg_instances[bg_index] = BackgroundInstance {
                        position: [x0 / width_f * 2.0 - 1.0, 1.0 - (y0 / height_f * 2.0)],
                        size: [
                            (x1 - x0) / width_f * 2.0,
                            self.separator.thickness / height_f * 2.0,
                        ],
                        color,
                    };
                    bg_index += 1;
                }
            }
        }
        let _ = bg_index; // suppress unused warning

        // Upload instance buffers to GPU
        self.queue.write_buffer(
            &self.buffers.bg_instance_buffer,
            0,
            bytemuck::cast_slice(&self.bg_instances),
        );
        self.queue.write_buffer(
            &self.buffers.text_instance_buffer,
            0,
            bytemuck::cast_slice(&self.text_instances),
        );

        Ok(())
    }
}
