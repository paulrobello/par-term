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

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: self.background_color[0] as f64,
                            g: self.background_color[1] as f64,
                            b: self.background_color[2] as f64,
                            a: self.window_opacity as f64,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            if let Some(ref bg_bind_group) = self.bg_image_bind_group {
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

    pub fn render_to_texture(
        &mut self,
        target_view: &wgpu::TextureView,
    ) -> Result<wgpu::SurfaceTexture> {
        let output = self.surface.get_current_texture()?;
        self.build_instance_buffers()?;

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render to texture encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            if let Some(ref bg_bind_group) = self.bg_image_bind_group {
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
        Ok(output)
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

                // Background
                for (col, cell) in row_cells.iter().enumerate() {
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
                        row_bg.push(BackgroundInstance {
                            position: [0.0, 0.0],
                            size: [0.0, 0.0],
                            color: [0.0, 0.0, 0.0, 0.0],
                        });
                        continue;
                    }

                    let bg_color = [
                        cell.bg_color[0] as f32 / 255.0,
                        cell.bg_color[1] as f32 / 255.0,
                        cell.bg_color[2] as f32 / 255.0,
                        cell.bg_color[3] as f32 / 255.0,
                    ];

                    let x0 = self.window_padding + col as f32 * self.cell_width;
                    let x1 = self.window_padding + (col + 1) as f32 * self.cell_width;
                    let y0 = self.window_padding + row as f32 * self.cell_height;
                    let y1 = self.window_padding + (row + 1) as f32 * self.cell_height;

                    // Extend cell backgrounds by 0.5 pixels on ALL sides to eliminate seams
                    // Adjacent cells will overlap by 1 pixel total, ensuring no gaps
                    let bg_overlap = 0.5;
                    let x0 = (x0 - bg_overlap).floor();
                    let x1 = (x1 + bg_overlap).ceil();
                    let y0 = (y0 - bg_overlap).floor();
                    let y1 = (y1 + bg_overlap).ceil();

                    // Geometric cursor rendering based on cursor style
                    // For block cursor, blend into cell background; for others, add overlay later
                    let mut final_bg_color = bg_color;
                    if has_cursor && self.cursor_opacity > 0.0 {
                        use par_term_emu_core_rust::cursor::CursorStyle;
                        match self.cursor_style {
                            // Block cursor: blend cursor color into background
                            CursorStyle::SteadyBlock | CursorStyle::BlinkingBlock => {
                                for (bg, &cursor) in
                                    final_bg_color.iter_mut().take(3).zip(&self.cursor_color)
                                {
                                    *bg = *bg * (1.0 - self.cursor_opacity)
                                        + cursor * self.cursor_opacity;
                                }
                                final_bg_color[3] = final_bg_color[3].max(self.cursor_opacity);
                            }
                            // Beam/Bar and Underline: handled separately in cursor_instance
                            _ => {}
                        }
                    }

                    // Add cell background
                    row_bg.push(BackgroundInstance {
                        position: [
                            x0 / self.config.width as f32 * 2.0 - 1.0,
                            1.0 - (y0 / self.config.height as f32 * 2.0),
                        ],
                        size: [
                            (x1 - x0) / self.config.width as f32 * 2.0,
                            (y1 - y0) / self.config.height as f32 * 2.0,
                        ],
                        color: final_bg_color,
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
                    + (row as f32 * self.cell_height)
                    + vertical_padding
                    + self.font_ascent;

                for (grapheme, bold, italic, fg_color, is_spacer, is_wide) in cell_data {
                    if is_spacer || grapheme == " " {
                        x_offset += self.cell_width;
                        continue;
                    }

                    let chars: Vec<char> = grapheme.chars().collect();
                    #[allow(clippy::collapsible_if)]
                    if let Some(ch) = chars.first() {
                        // Classify the character for rendering optimization
                        let char_type = block_chars::classify_char(*ch);

                        // Check if we should render this character geometrically
                        if block_chars::should_render_geometrically(char_type) {
                            let char_w = if is_wide {
                                self.cell_width * 2.0
                            } else {
                                self.cell_width
                            };
                            let x0 = (self.window_padding + x_offset).round();
                            let y0 = (self.window_padding + row as f32 * self.cell_height).round();

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
                                            fg_color[3] as f32 / 255.0,
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
                                        fg_color[3] as f32 / 255.0,
                                    ],
                                    is_colored: 0,
                                });

                                x_offset += self.cell_width;
                                continue;
                            }
                        }

                        if let Some((font_idx, glyph_id)) =
                            self.font_manager.find_glyph(*ch, bold, italic)
                        {
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
                            let y0 = (self.window_padding + row as f32 * self.cell_height).round();
                            let y1 =
                                (self.window_padding + (row + 1) as f32 * self.cell_height).round();

                            let cell_w = x1 - x0;
                            let cell_h = y1 - y0;

                            let scale_x = cell_w / char_w;
                            let scale_y = cell_h / self.cell_height;

                            // Position glyph relative to snapped cell top-left
                            let baseline_offset = baseline_y_unrounded
                                - (self.window_padding + row as f32 * self.cell_height);
                            let glyph_left = x0 + (info.bearing_x * scale_x).round();
                            let glyph_top =
                                y0 + ((baseline_offset - info.bearing_y) * scale_y).round();

                            let render_w = info.width as f32 * scale_x;
                            let render_h = info.height as f32 * scale_y;

                            // For block characters that need font rendering (box drawing, etc.),
                            // apply snapping to cell boundaries with sub-pixel extension
                            let (final_left, final_top, final_w, final_h) =
                                if block_chars::should_snap_to_boundaries(char_type) {
                                    // Snap threshold of 3 pixels, extension of 0.5 pixels
                                    block_chars::snap_glyph_to_cell(
                                        glyph_left, glyph_top, render_w, render_h, x0, y0, x1, y1,
                                        3.0, 0.5,
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
                                    fg_color[3] as f32 / 255.0,
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
