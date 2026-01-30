use super::block_chars;
use super::{BackgroundInstance, CellRenderer, RowCacheEntry, TextInstance};
use crate::text_shaper::ShapingOptions;
use anyhow::Result;

impl CellRenderer {
    pub fn render(&mut self, show_scrollbar: bool) -> Result<wgpu::SurfaceTexture> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.build_instance_buffers()?;

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render encoder"),
            });

        // Determine clear color and whether to use bg_image pipeline:
        // - Solid color mode: use clear color directly (same as Default mode for proper transparency)
        // - Image mode: use TRANSPARENT clear, let bg_image_pipeline handle background
        // - Default mode: use theme background with window_opacity
        let (clear_color, use_bg_image_pipeline) = if self.bg_is_solid_color {
            // Solid color mode: use clear color directly for proper window transparency
            // This works the same as Default mode - LoadOp::Clear sets alpha correctly
            debug_info!(
                "BACKGROUND",
                "Solid color mode: RGB({:.3}, {:.3}, {:.3}) * opacity {:.3}",
                self.solid_bg_color[0],
                self.solid_bg_color[1],
                self.solid_bg_color[2],
                self.window_opacity
            );
            (
                wgpu::Color {
                    r: self.solid_bg_color[0] as f64 * self.window_opacity as f64,
                    g: self.solid_bg_color[1] as f64 * self.window_opacity as f64,
                    b: self.solid_bg_color[2] as f64 * self.window_opacity as f64,
                    a: self.window_opacity as f64,
                },
                false,
            )
        } else if self.bg_image_bind_group.is_some() {
            // Image mode: use TRANSPARENT, let bg_image_pipeline handle background
            (wgpu::Color::TRANSPARENT, true)
        } else {
            // Default mode: use theme background with window_opacity
            (
                wgpu::Color {
                    r: self.background_color[0] as f64 * self.window_opacity as f64,
                    g: self.background_color[1] as f64 * self.window_opacity as f64,
                    b: self.background_color[2] as f64 * self.window_opacity as f64,
                    a: self.window_opacity as f64,
                },
                false,
            )
        };

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Render background image if present (not used for solid color mode)
            if use_bg_image_pipeline && let Some(ref bg_bind_group) = self.bg_image_bind_group {
                render_pass.set_pipeline(&self.bg_image_pipeline);
                render_pass.set_bind_group(0, bg_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                render_pass.draw(0..4, 0..1);
            }

            render_pass.set_pipeline(&self.bg_pipeline);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.bg_instance_buffer.slice(..));
            render_pass.draw(0..4, 0..self.max_bg_instances as u32);

            render_pass.set_pipeline(&self.text_pipeline);
            render_pass.set_bind_group(0, &self.text_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.text_instance_buffer.slice(..));
            render_pass.draw(0..4, 0..self.max_text_instances as u32);

            if show_scrollbar {
                self.scrollbar.render(&mut render_pass);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        Ok(output)
    }

    /// Render terminal content to an intermediate texture for shader processing.
    ///
    /// # Arguments
    /// * `target_view` - The texture view to render to
    /// * `skip_background_image` - If true, skip rendering the background image. Use this when
    ///   a custom shader will handle the background image via iChannel0 instead.
    ///
    /// Note: Solid color backgrounds are NOT rendered here. For cursor shaders, the solid color
    /// is passed to the shader's render function as the clear color instead.
    pub fn render_to_texture(
        &mut self,
        target_view: &wgpu::TextureView,
        skip_background_image: bool,
    ) -> Result<wgpu::SurfaceTexture> {
        let output = self.surface.get_current_texture()?;
        self.build_instance_buffers()?;

        // Only render background IMAGE to intermediate texture (not solid color).
        // Solid colors are handled by the shader's clear color for proper compositing.
        let render_background_image =
            !skip_background_image && !self.bg_is_solid_color && self.bg_image_bind_group.is_some();
        let saved_window_opacity = self.window_opacity;

        if render_background_image {
            // Temporarily set window_opacity to 1.0 for the background render
            // The shader wrapper will apply window_opacity at the end
            self.window_opacity = 1.0;
            self.update_bg_image_uniforms();
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render to texture encoder"),
            });

        // Always clear with TRANSPARENT for intermediate textures
        let clear_color = wgpu::Color::TRANSPARENT;

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Render background IMAGE (not solid color) via bg_image_pipeline at full opacity
            if render_background_image && let Some(ref bg_bind_group) = self.bg_image_bind_group {
                debug_info!(
                    "BACKGROUND",
                    "render_to_texture: bg_image_pipeline (image, window_opacity={:.3} applied by shader)",
                    saved_window_opacity
                );
                render_pass.set_pipeline(&self.bg_image_pipeline);
                render_pass.set_bind_group(0, bg_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                render_pass.draw(0..4, 0..1);
            }

            render_pass.set_pipeline(&self.bg_pipeline);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.bg_instance_buffer.slice(..));
            render_pass.draw(0..4, 0..self.max_bg_instances as u32);

            render_pass.set_pipeline(&self.text_pipeline);
            render_pass.set_bind_group(0, &self.text_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.text_instance_buffer.slice(..));
            render_pass.draw(0..4, 0..self.max_text_instances as u32);
        }

        self.queue.submit(std::iter::once(encoder.finish()));

        // Restore window_opacity and update uniforms
        if render_background_image {
            self.window_opacity = saved_window_opacity;
            self.update_bg_image_uniforms();
        }

        Ok(output)
    }

    /// Render terminal content to a view for screenshots.
    /// This renders without requiring the surface texture.
    pub fn render_to_view(&self, target_view: &wgpu::TextureView) -> Result<()> {
        // Note: We don't rebuild instance buffers here since this is typically called
        // right after a normal render, and the buffers should already be up to date.

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("screenshot render encoder"),
            });

        // Determine clear color and whether to use bg_image pipeline
        let (clear_color, use_bg_image_pipeline) = if self.bg_is_solid_color {
            (
                wgpu::Color {
                    r: self.solid_bg_color[0] as f64 * self.window_opacity as f64,
                    g: self.solid_bg_color[1] as f64 * self.window_opacity as f64,
                    b: self.solid_bg_color[2] as f64 * self.window_opacity as f64,
                    a: self.window_opacity as f64,
                },
                false,
            )
        } else if self.bg_image_bind_group.is_some() {
            (wgpu::Color::TRANSPARENT, true)
        } else {
            (
                wgpu::Color {
                    r: self.background_color[0] as f64 * self.window_opacity as f64,
                    g: self.background_color[1] as f64 * self.window_opacity as f64,
                    b: self.background_color[2] as f64 * self.window_opacity as f64,
                    a: self.window_opacity as f64,
                },
                false,
            )
        };

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("screenshot render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Render background image if present
            if use_bg_image_pipeline && let Some(ref bg_bind_group) = self.bg_image_bind_group {
                render_pass.set_pipeline(&self.bg_image_pipeline);
                render_pass.set_bind_group(0, bg_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                render_pass.draw(0..4, 0..1);
            }

            render_pass.set_pipeline(&self.bg_pipeline);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.bg_instance_buffer.slice(..));
            render_pass.draw(0..4, 0..self.max_bg_instances as u32);

            render_pass.set_pipeline(&self.text_pipeline);
            render_pass.set_bind_group(0, &self.text_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.text_instance_buffer.slice(..));
            render_pass.draw(0..4, 0..self.max_text_instances as u32);

            // Render scrollbar
            self.scrollbar.render(&mut render_pass);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        Ok(())
    }

    pub fn render_overlays(
        &mut self,
        surface_texture: &wgpu::SurfaceTexture,
        show_scrollbar: bool,
    ) -> Result<()> {
        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("overlay encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("overlay pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            if show_scrollbar {
                self.scrollbar.render(&mut render_pass);
            }

            if self.visual_bell_intensity > 0.0 {
                // Visual bell logic
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        Ok(())
    }

    pub(crate) fn build_instance_buffers(&mut self) -> Result<()> {
        let _shaping_options = ShapingOptions {
            enable_ligatures: self.enable_ligatures,
            enable_kerning: self.enable_kerning,
            ..Default::default()
        };

        for row in 0..self.rows {
            if self.dirty_rows[row] || self.row_cache[row].is_none() {
                let start = row * self.cols;
                let end = (row + 1) * self.cols;
                let row_cells = &self.cells[start..end];

                let mut row_bg = Vec::with_capacity(self.cols);
                let mut row_text = Vec::with_capacity(self.cols);

                // Background - use RLE to merge consecutive cells with same color (like iTerm2)
                // This eliminates seams between adjacent same-colored cells
                let mut col = 0;
                while col < row_cells.len() {
                    let cell = &row_cells[col];
                    let is_default_bg =
                        (cell.bg_color[0] as f32 / 255.0 - self.background_color[0]).abs() < 0.001
                            && (cell.bg_color[1] as f32 / 255.0 - self.background_color[1]).abs()
                                < 0.001
                            && (cell.bg_color[2] as f32 / 255.0 - self.background_color[2]).abs()
                                < 0.001;

                    let has_cursor = self.cursor_opacity > 0.0
                        && !self.cursor_hidden_for_shader
                        && self.cursor_pos.1 == row
                        && self.cursor_pos.0 == col;

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
                    let mut bg_color = [
                        cell.bg_color[0] as f32 / 255.0,
                        cell.bg_color[1] as f32 / 255.0,
                        cell.bg_color[2] as f32 / 255.0,
                        bg_alpha,
                    ];

                    // Handle cursor at this position
                    if has_cursor && self.cursor_opacity > 0.0 {
                        use par_term_emu_core_rust::cursor::CursorStyle;
                        match self.cursor_style {
                            CursorStyle::SteadyBlock | CursorStyle::BlinkingBlock => {
                                for (bg, &cursor) in
                                    bg_color.iter_mut().take(3).zip(&self.cursor_color)
                                {
                                    *bg = *bg * (1.0 - self.cursor_opacity)
                                        + cursor * self.cursor_opacity;
                                }
                                bg_color[3] = bg_color[3].max(self.cursor_opacity);
                            }
                            _ => {}
                        }
                        // Cursor cell can't be merged, render it alone
                        let x0 = self.window_padding + col as f32 * self.cell_width;
                        let x1 = self.window_padding + (col + 1) as f32 * self.cell_width;
                        let y0 = self.window_padding
                            + self.content_offset_y
                            + row as f32 * self.cell_height;
                        let y1 = y0 + self.cell_height;
                        row_bg.push(BackgroundInstance {
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
                        let next_has_cursor = self.cursor_opacity > 0.0
                            && !self.cursor_hidden_for_shader
                            && self.cursor_pos.1 == row
                            && self.cursor_pos.0 == col;
                        // Stop run if color differs or cursor is here
                        if next_cell.bg_color != run_color || next_has_cursor {
                            break;
                        }
                        col += 1;
                    }
                    let run_length = col - start_col;

                    // Create single quad spanning entire run (no per-cell rounding)
                    let x0 = self.window_padding + start_col as f32 * self.cell_width;
                    let x1 =
                        self.window_padding + (start_col + run_length) as f32 * self.cell_width;
                    let y0 =
                        self.window_padding + self.content_offset_y + row as f32 * self.cell_height;
                    let y1 = y0 + self.cell_height;

                    row_bg.push(BackgroundInstance {
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
                while row_bg.len() < self.cols {
                    row_bg.push(BackgroundInstance {
                        position: [0.0, 0.0],
                        size: [0.0, 0.0],
                        color: [0.0, 0.0, 0.0, 0.0],
                    });
                }

                // Text
                let mut x_offset = 0.0;
                let cell_data: Vec<(String, bool, bool, [u8; 4], bool, bool)> = row_cells
                    .iter()
                    .map(|c| {
                        (
                            c.grapheme.clone(),
                            c.bold,
                            c.italic,
                            c.fg_color,
                            c.wide_char_spacer,
                            c.wide_char,
                        )
                    })
                    .collect();

                // Dynamic baseline calculation based on font metrics
                let natural_line_height = self.font_ascent + self.font_descent + self.font_leading;
                let vertical_padding = (self.cell_height - natural_line_height).max(0.0) / 2.0;
                let baseline_y_unrounded = self.window_padding
                    + self.content_offset_y
                    + (row as f32 * self.cell_height)
                    + vertical_padding
                    + self.font_ascent;

                for (grapheme, bold, italic, fg_color, is_spacer, is_wide) in cell_data {
                    if is_spacer || grapheme == " " {
                        x_offset += self.cell_width;
                        continue;
                    }

                    // Compute text alpha - force opaque if keep_text_opaque is enabled,
                    // otherwise use window opacity so text becomes transparent with the window
                    let text_alpha = if self.keep_text_opaque {
                        1.0
                    } else {
                        self.window_opacity
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
                                self.cell_width * 2.0
                            } else {
                                self.cell_width
                            };
                            let x0 = (self.window_padding + x_offset).round();
                            let y0 = (self.window_padding
                                + self.content_offset_y
                                + row as f32 * self.cell_height)
                                .round();

                            // Try box drawing geometry first (for lines, corners, junctions)
                            // Pass aspect ratio so vertical lines have same visual thickness as horizontal
                            let aspect_ratio = self.cell_height / char_w;
                            if let Some(box_geo) =
                                block_chars::get_box_drawing_geometry(*ch, aspect_ratio)
                            {
                                for segment in &box_geo.segments {
                                    let rect =
                                        segment.to_pixel_rect(x0, y0, char_w, self.cell_height);

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

                                    row_text.push(TextInstance {
                                        position: [
                                            final_x / self.config.width as f32 * 2.0 - 1.0,
                                            1.0 - (final_y / self.config.height as f32 * 2.0),
                                        ],
                                        size: [
                                            final_w / self.config.width as f32 * 2.0,
                                            final_h / self.config.height as f32 * 2.0,
                                        ],
                                        tex_offset: [
                                            self.solid_pixel_offset.0 as f32 / 2048.0,
                                            self.solid_pixel_offset.1 as f32 / 2048.0,
                                        ],
                                        tex_size: [1.0 / 2048.0, 1.0 / 2048.0],
                                        color: [
                                            fg_color[0] as f32 / 255.0,
                                            fg_color[1] as f32 / 255.0,
                                            fg_color[2] as f32 / 255.0,
                                            text_alpha,
                                        ],
                                        is_colored: 0,
                                    });
                                }
                                x_offset += self.cell_width;
                                continue;
                            }

                            // Try block element geometry (for solid blocks, half blocks, etc.)
                            if let Some(geo_block) = block_chars::get_geometric_block(*ch) {
                                let rect =
                                    geo_block.to_pixel_rect(x0, y0, char_w, self.cell_height);

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
                                row_text.push(TextInstance {
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
                                        self.solid_pixel_offset.0 as f32 / 2048.0,
                                        self.solid_pixel_offset.1 as f32 / 2048.0,
                                    ],
                                    tex_size: [1.0 / 2048.0, 1.0 / 2048.0],
                                    color: [
                                        fg_color[0] as f32 / 255.0,
                                        fg_color[1] as f32 / 255.0,
                                        fg_color[2] as f32 / 255.0,
                                        text_alpha,
                                    ],
                                    is_colored: 0,
                                });

                                x_offset += self.cell_width;
                                continue;
                            }
                        }

                        // Use grapheme-aware glyph lookup for multi-character sequences
                        // (flags, emoji with skin tones, ZWJ sequences, combining chars)
                        let glyph_result = if chars.len() > 1 {
                            self.font_manager
                                .find_grapheme_glyph(&grapheme, bold, italic)
                        } else {
                            self.font_manager.find_glyph(*ch, bold, italic)
                        };

                        if let Some((font_idx, glyph_id)) = glyph_result {
                            let cache_key = ((font_idx as u64) << 32) | (glyph_id as u64);
                            let info = if self.glyph_cache.contains_key(&cache_key) {
                                // Move to front of LRU
                                self.lru_remove(cache_key);
                                self.lru_push_front(cache_key);
                                self.glyph_cache.get(&cache_key).unwrap().clone()
                            } else if let Some(raster) = self.rasterize_glyph(font_idx, glyph_id) {
                                let info = self.upload_glyph(cache_key, &raster);
                                self.glyph_cache.insert(cache_key, info.clone());
                                self.lru_push_front(cache_key);
                                info
                            } else {
                                x_offset += self.cell_width;
                                continue;
                            };

                            let char_w = if is_wide {
                                self.cell_width * 2.0
                            } else {
                                self.cell_width
                            };
                            let x0 = (self.window_padding + x_offset).round();
                            let x1 = (self.window_padding + x_offset + char_w).round();
                            let y0 = (self.window_padding
                                + self.content_offset_y
                                + row as f32 * self.cell_height)
                                .round();
                            let y1 = (self.window_padding
                                + self.content_offset_y
                                + (row + 1) as f32 * self.cell_height)
                                .round();

                            let cell_w = x1 - x0;
                            let cell_h = y1 - y0;

                            let scale_x = cell_w / char_w;
                            let scale_y = cell_h / self.cell_height;

                            // Position glyph relative to snapped cell top-left
                            let baseline_offset = baseline_y_unrounded
                                - (self.window_padding
                                    + self.content_offset_y
                                    + row as f32 * self.cell_height);
                            let glyph_left = x0 + (info.bearing_x * scale_x).round();
                            let glyph_top =
                                y0 + ((baseline_offset - info.bearing_y) * scale_y).round();

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
                                    glyph_left, glyph_top, render_w, render_h, x0, y0, x1, y1, 3.0,
                                    0.5,
                                )
                            } else {
                                (glyph_left, glyph_top, render_w, render_h)
                            };

                            row_text.push(TextInstance {
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
                                color: [
                                    fg_color[0] as f32 / 255.0,
                                    fg_color[1] as f32 / 255.0,
                                    fg_color[2] as f32 / 255.0,
                                    text_alpha,
                                ],
                                is_colored: if info.is_colored { 1 } else { 0 },
                            });
                        }
                    }
                    x_offset += self.cell_width;
                }

                // Update CPU-side buffers
                let bg_start = row * self.cols;
                self.bg_instances[bg_start..bg_start + self.cols].copy_from_slice(&row_bg);

                let text_start = row * self.cols * 2;
                // Clear row text segment first
                for i in 0..(self.cols * 2) {
                    self.text_instances[text_start + i].size = [0.0, 0.0];
                }
                // Copy new text instances
                let text_count = row_text.len().min(self.cols * 2);
                self.text_instances[text_start..text_start + text_count]
                    .copy_from_slice(&row_text[..text_count]);

                // Update GPU-side buffers incrementally
                self.queue.write_buffer(
                    &self.bg_instance_buffer,
                    (bg_start * std::mem::size_of::<BackgroundInstance>()) as u64,
                    bytemuck::cast_slice(&row_bg),
                );
                self.queue.write_buffer(
                    &self.text_instance_buffer,
                    (text_start * std::mem::size_of::<TextInstance>()) as u64,
                    bytemuck::cast_slice(
                        &self.text_instances[text_start..text_start + self.cols * 2],
                    ),
                );

                self.row_cache[row] = Some(RowCacheEntry {});
                self.dirty_rows[row] = false;
            }
        }

        // Write cursor overlay to the last slot of bg_instances (for beam/underline cursors)
        let cursor_overlay_index = self.cols * self.rows;
        let cursor_overlay_instance = self.cursor_overlay.unwrap_or(BackgroundInstance {
            position: [0.0, 0.0],
            size: [0.0, 0.0],
            color: [0.0, 0.0, 0.0, 0.0],
        });
        self.bg_instances[cursor_overlay_index] = cursor_overlay_instance;
        self.queue.write_buffer(
            &self.bg_instance_buffer,
            (cursor_overlay_index * std::mem::size_of::<BackgroundInstance>()) as u64,
            bytemuck::cast_slice(&[cursor_overlay_instance]),
        );

        Ok(())
    }
}
