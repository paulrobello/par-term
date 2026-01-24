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

                    let x0 = (self.window_padding + col as f32 * self.cell_width).round();
                    let x1 = (self.window_padding + (col + 1) as f32 * self.cell_width).round();
                    let y0 = (self.window_padding + row as f32 * self.cell_height).round();
                    let y1 = (self.window_padding + (row + 1) as f32 * self.cell_height).round();

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
                            let mut glyph_left = x0 + (info.bearing_x * scale_x).round();
                            let mut glyph_top =
                                y0 + ((baseline_offset - info.bearing_y) * scale_y).round();

                            let mut render_w = info.width as f32 * scale_x;
                            let mut render_h = info.height as f32 * scale_y;

                            // Special case: for box drawing and block elements, ensure they fill the cell
                            // if they are close to the edges to avoid 1px gaps.
                            let char_code = *ch as u32;
                            let is_block_char = (0x2500..=0x259F).contains(&char_code)
                                || (0xE0A0..=0xE0D4).contains(&char_code)
                                || (0x25A0..=0x25FF).contains(&char_code); // Geometric shapes

                            if is_block_char {
                                // Snap to left/right cell boundaries
                                if (glyph_left - x0).abs() < 3.0 {
                                    let right = glyph_left + render_w;
                                    glyph_left = x0;
                                    render_w = (right - x0).max(render_w);
                                }
                                if (x1 - (glyph_left + render_w)).abs() < 3.0 {
                                    render_w = x1 - glyph_left;
                                }

                                // Snap to top/bottom cell boundaries
                                if (glyph_top - y0).abs() < 3.0 {
                                    let bottom = glyph_top + render_h;
                                    glyph_top = y0;
                                    render_h = (bottom - y0).max(render_h);
                                }
                                if (y1 - (glyph_top + render_h)).abs() < 3.0 {
                                    render_h = y1 - glyph_top;
                                }

                                // For half-blocks and quadrants, also snap to middle boundaries
                                let cx = (x0 + x1) / 2.0;
                                let cy = (y0 + y1) / 2.0;

                                // Vertical middle snap
                                if (glyph_top + render_h - cy).abs() < 2.0 {
                                    render_h = cy - glyph_top;
                                } else if (glyph_top - cy).abs() < 2.0 {
                                    let bottom = glyph_top + render_h;
                                    glyph_top = cy;
                                    render_h = bottom - cy;
                                }

                                // Horizontal middle snap
                                if (glyph_left + render_w - cx).abs() < 2.0 {
                                    render_w = cx - glyph_left;
                                } else if (glyph_left - cx).abs() < 2.0 {
                                    let right = glyph_left + render_w;
                                    glyph_left = cx;
                                    render_w = right - cx;
                                }
                            }

                            row_text.push(TextInstance {
                                position: [
                                    glyph_left / self.config.width as f32 * 2.0 - 1.0,
                                    1.0 - (glyph_top / self.config.height as f32 * 2.0),
                                ],
                                size: [
                                    render_w / self.config.width as f32 * 2.0,
                                    render_h / self.config.height as f32 * 2.0,
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
