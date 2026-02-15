use super::block_chars;
use super::{BackgroundInstance, Cell, CellRenderer, PaneViewport, RowCacheEntry, TextInstance};
use crate::renderer::SeparatorMark;
use crate::text_shaper::ShapingOptions;
use anyhow::Result;

impl CellRenderer {
    pub fn render(
        &mut self,
        _show_scrollbar: bool,
        pane_background: Option<&crate::pane::PaneBackground>,
    ) -> Result<wgpu::SurfaceTexture> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.build_instance_buffers()?;

        // Pre-create per-pane background bind group if needed (must happen before render pass)
        // This supports pane 0 background in single-pane (no splits) mode.
        let pane_bg_resources = if !self.bg_is_solid_color {
            if let Some(pane_bg) = pane_background {
                if let Some(ref path) = pane_bg.image_path {
                    self.pane_bg_cache.get(path.as_str()).map(|entry| {
                        self.create_pane_bg_bind_group(
                            entry,
                            0.0, // pane_x: full window starts at 0
                            0.0, // pane_y: full window starts at 0
                            self.config.width as f32,
                            self.config.height as f32,
                            pane_bg.mode,
                            pane_bg.opacity,
                        )
                    })
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render encoder"),
            });

        // Determine clear color and whether to use bg_image pipeline:
        // - Solid color mode: use clear color directly (same as Default mode for proper transparency)
        // - Image mode: use TRANSPARENT clear, let bg_image_pipeline handle background
        // - Default mode: use theme background with window_opacity
        // - Per-pane bg: use TRANSPARENT clear, render pane bg before global bg
        let has_pane_bg = pane_bg_resources.is_some();
        let (clear_color, use_bg_image_pipeline) = if has_pane_bg {
            // Per-pane background: use transparent clear, pane bg will be rendered first
            (wgpu::Color::TRANSPARENT, false)
        } else if self.bg_is_solid_color {
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

            // Render per-pane background for single-pane mode (pane 0)
            if let Some((ref bind_group, _)) = pane_bg_resources {
                render_pass.set_pipeline(&self.bg_image_pipeline);
                render_pass.set_bind_group(0, bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                render_pass.draw(0..4, 0..1);
            }

            // Render global background image if present (not used for solid color or pane bg mode)
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

    /// Render only the background (image or solid color) to a view.
    ///
    /// This is useful for split pane rendering where the background should be
    /// rendered once full-screen before rendering each pane's cells on top.
    ///
    /// # Arguments
    /// * `target_view` - The texture view to render to
    /// * `clear_first` - If true, clear the surface before rendering
    ///
    /// # Returns
    /// `true` if a background image was rendered, `false` if only clear color was used
    pub fn render_background_only(
        &self,
        target_view: &wgpu::TextureView,
        clear_first: bool,
    ) -> Result<bool> {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("background only encoder"),
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

        let load_op = if clear_first {
            wgpu::LoadOp::Clear(clear_color)
        } else {
            wgpu::LoadOp::Load
        };

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("background only render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target_view,
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

            // Render background image if present
            if use_bg_image_pipeline && let Some(ref bg_bind_group) = self.bg_image_bind_group {
                render_pass.set_pipeline(&self.bg_image_pipeline);
                render_pass.set_bind_group(0, bg_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                render_pass.draw(0..4, 0..1);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        Ok(use_bg_image_pipeline)
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

                    // Check for cursor at this position, accounting for unfocused state
                    let cursor_visible = self.cursor_opacity > 0.0
                        && !self.cursor_hidden_for_shader
                        && self.cursor_pos.1 == row
                        && self.cursor_pos.0 == col;

                    // Handle unfocused cursor visibility
                    let has_cursor = if cursor_visible && !self.is_focused {
                        match self.unfocused_cursor_style {
                            crate::config::UnfocusedCursorStyle::Hidden => false,
                            crate::config::UnfocusedCursorStyle::Hollow
                            | crate::config::UnfocusedCursorStyle::Same => true,
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
                    let mut bg_color = [
                        cell.bg_color[0] as f32 / 255.0,
                        cell.bg_color[1] as f32 / 255.0,
                        cell.bg_color[2] as f32 / 255.0,
                        bg_alpha,
                    ];

                    // Handle cursor at this position
                    if has_cursor && self.cursor_opacity > 0.0 {
                        use par_term_emu_core_rust::cursor::CursorStyle;

                        // Check if we should render hollow cursor (unfocused hollow style)
                        let render_hollow = !self.is_focused
                            && self.unfocused_cursor_style
                                == crate::config::UnfocusedCursorStyle::Hollow;

                        match self.cursor_style {
                            CursorStyle::SteadyBlock | CursorStyle::BlinkingBlock => {
                                if render_hollow {
                                    // Hollow cursor: don't fill the cell, outline will be added later
                                    // Keep original background color
                                } else {
                                    // Solid block cursor
                                    for (bg, &cursor) in
                                        bg_color.iter_mut().take(3).zip(&self.cursor_color)
                                    {
                                        *bg = *bg * (1.0 - self.cursor_opacity)
                                            + cursor * self.cursor_opacity;
                                    }
                                    bg_color[3] = bg_color[3].max(self.cursor_opacity);
                                }
                            }
                            _ => {}
                        }
                        // Cursor cell can't be merged, render it alone
                        let x0 = self.window_padding
                            + self.content_offset_x
                            + col as f32 * self.cell_width;
                        let x1 = self.window_padding
                            + self.content_offset_x
                            + (col + 1) as f32 * self.cell_width;
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
                    let x0 = self.window_padding
                        + self.content_offset_x
                        + start_col as f32 * self.cell_width;
                    let x1 = self.window_padding
                        + self.content_offset_x
                        + (start_col + run_length) as f32 * self.cell_width;
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
                let natural_line_height = self.font_ascent + self.font_descent + self.font_leading;
                let vertical_padding = (self.cell_height - natural_line_height).max(0.0) / 2.0;
                let baseline_y_unrounded = self.window_padding
                    + self.content_offset_y
                    + (row as f32 * self.cell_height)
                    + vertical_padding
                    + self.font_ascent;

                // Check if this row has the cursor and it's a visible block cursor
                // (for cursor text color override)
                let cursor_is_block_on_this_row = {
                    use par_term_emu_core_rust::cursor::CursorStyle;
                    self.cursor_pos.1 == row
                        && self.cursor_opacity > 0.0
                        && !self.cursor_hidden_for_shader
                        && matches!(
                            self.cursor_style,
                            CursorStyle::SteadyBlock | CursorStyle::BlinkingBlock
                        )
                        && (self.is_focused
                            || self.unfocused_cursor_style
                                == crate::config::UnfocusedCursorStyle::Same)
                };

                let mut current_col = 0usize;
                for (grapheme, bold, italic, fg_color, bg_color, is_spacer, is_wide) in cell_data {
                    if is_spacer || grapheme == " " {
                        x_offset += self.cell_width;
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
                        if cursor_is_block_on_this_row && current_col == self.cursor_pos.0 {
                            if let Some(cursor_text) = self.cursor_text_color {
                                [cursor_text[0], cursor_text[1], cursor_text[2], text_alpha]
                            } else {
                                // Auto-contrast: use cursor color as a starting point
                                // Simple inversion: if cursor is bright, use dark text; if dark, use bright
                                let cursor_brightness = (self.cursor_color[0]
                                    + self.cursor_color[1]
                                    + self.cursor_color[2])
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
                                [
                                    bg_color[0] as f32 / 255.0,
                                    bg_color[1] as f32 / 255.0,
                                    bg_color[2] as f32 / 255.0,
                                    1.0,
                                ]
                            } else {
                                // Use terminal default background
                                [
                                    self.background_color[0],
                                    self.background_color[1],
                                    self.background_color[2],
                                    1.0,
                                ]
                            };

                            let base_fg = [
                                fg_color[0] as f32 / 255.0,
                                fg_color[1] as f32 / 255.0,
                                fg_color[2] as f32 / 255.0,
                                text_alpha,
                            ];

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
                                self.cell_width * 2.0
                            } else {
                                self.cell_width
                            };
                            let x0 =
                                (self.window_padding + self.content_offset_x + x_offset).round();
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
                                        color: render_fg_color,
                                        is_colored: 0,
                                    });
                                }
                                x_offset += self.cell_width;
                                current_col += 1;
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
                                    color: render_fg_color,
                                    is_colored: 0,
                                });

                                x_offset += self.cell_width;
                                current_col += 1;
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
                            let x0 =
                                (self.window_padding + self.content_offset_x + x_offset).round();
                            let x1 =
                                (self.window_padding + self.content_offset_x + x_offset + char_w)
                                    .round();
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

                            // Position glyph relative to snapped cell top-left.
                            // Round the scaled baseline position once, then subtract
                            // the integer bearing_y. This ensures all glyphs on a row
                            // share the same rounded baseline, with bearing offsets
                            // applied exactly (no scale_y on bearing avoids rounding
                            // artifacts between glyphs with different bearings).
                            let baseline_offset = baseline_y_unrounded
                                - (self.window_padding
                                    + self.content_offset_y
                                    + row as f32 * self.cell_height);
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
                                color: render_fg_color,
                                is_colored: if info.is_colored { 1 } else { 0 },
                            });
                        }
                    }
                    x_offset += self.cell_width;
                    current_col += 1;
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

        // Write cursor-related overlays to extra slots at the end of bg_instances
        // Slot layout: [0] cursor overlay (beam/underline), [1] guide, [2] shadow, [3-6] boost glow, [7-10] hollow outline
        let base_overlay_index = self.cols * self.rows;
        let mut overlay_instances = vec![
            BackgroundInstance {
                position: [0.0, 0.0],
                size: [0.0, 0.0],
                color: [0.0, 0.0, 0.0, 0.0],
            };
            10
        ];

        // Check if cursor should be visible
        let cursor_visible = self.cursor_opacity > 0.0
            && !self.cursor_hidden_for_shader
            && (self.is_focused
                || self.unfocused_cursor_style != crate::config::UnfocusedCursorStyle::Hidden);

        // Calculate cursor pixel positions
        let cursor_col = self.cursor_pos.0;
        let cursor_row = self.cursor_pos.1;
        let cursor_x0 =
            self.window_padding + self.content_offset_x + cursor_col as f32 * self.cell_width;
        let cursor_x1 = cursor_x0 + self.cell_width;
        let cursor_y0 =
            self.window_padding + self.content_offset_y + cursor_row as f32 * self.cell_height;
        let cursor_y1 = cursor_y0 + self.cell_height;

        // Slot 0: Cursor overlay (beam/underline) - handled by existing cursor_overlay
        overlay_instances[0] = self.cursor_overlay.unwrap_or(BackgroundInstance {
            position: [0.0, 0.0],
            size: [0.0, 0.0],
            color: [0.0, 0.0, 0.0, 0.0],
        });

        // Slot 1: Cursor guide (horizontal line spanning full width at cursor row)
        if cursor_visible && self.cursor_guide_enabled {
            let guide_x0 = self.window_padding + self.content_offset_x;
            let guide_x1 = self.config.width as f32 - self.window_padding;
            overlay_instances[1] = BackgroundInstance {
                position: [
                    guide_x0 / self.config.width as f32 * 2.0 - 1.0,
                    1.0 - (cursor_y0 / self.config.height as f32 * 2.0),
                ],
                size: [
                    (guide_x1 - guide_x0) / self.config.width as f32 * 2.0,
                    (cursor_y1 - cursor_y0) / self.config.height as f32 * 2.0,
                ],
                color: self.cursor_guide_color,
            };
        }

        // Slot 2: Cursor shadow (offset rectangle behind cursor)
        if cursor_visible && self.cursor_shadow_enabled {
            let shadow_x0 = cursor_x0 + self.cursor_shadow_offset[0];
            let shadow_y0 = cursor_y0 + self.cursor_shadow_offset[1];
            overlay_instances[2] = BackgroundInstance {
                position: [
                    shadow_x0 / self.config.width as f32 * 2.0 - 1.0,
                    1.0 - (shadow_y0 / self.config.height as f32 * 2.0),
                ],
                size: [
                    self.cell_width / self.config.width as f32 * 2.0,
                    self.cell_height / self.config.height as f32 * 2.0,
                ],
                color: self.cursor_shadow_color,
            };
        }

        // Slot 3: Cursor boost glow (larger rectangle around cursor with low opacity)
        if cursor_visible && self.cursor_boost > 0.0 {
            let glow_expand = 4.0 * self.scale_factor * self.cursor_boost; // Expand by up to 4 logical pixels
            let glow_x0 = cursor_x0 - glow_expand;
            let glow_y0 = cursor_y0 - glow_expand;
            let glow_w = self.cell_width + glow_expand * 2.0;
            let glow_h = self.cell_height + glow_expand * 2.0;
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
                    self.cursor_boost_color[0],
                    self.cursor_boost_color[1],
                    self.cursor_boost_color[2],
                    self.cursor_boost * 0.3 * self.cursor_opacity, // Max 30% alpha
                ],
            };
        }

        // Slots 4-7: Hollow cursor outline (4 thin rectangles forming a border)
        // Rendered when unfocused with hollow style and block cursor
        let render_hollow = cursor_visible
            && !self.is_focused
            && self.unfocused_cursor_style == crate::config::UnfocusedCursorStyle::Hollow;

        if render_hollow {
            use par_term_emu_core_rust::cursor::CursorStyle;
            let is_block = matches!(
                self.cursor_style,
                CursorStyle::SteadyBlock | CursorStyle::BlinkingBlock
            );

            if is_block {
                let border_width = 2.0; // 2 pixel border
                let color = [
                    self.cursor_color[0],
                    self.cursor_color[1],
                    self.cursor_color[2],
                    self.cursor_opacity,
                ];

                // Top border
                overlay_instances[4] = BackgroundInstance {
                    position: [
                        cursor_x0 / self.config.width as f32 * 2.0 - 1.0,
                        1.0 - (cursor_y0 / self.config.height as f32 * 2.0),
                    ],
                    size: [
                        self.cell_width / self.config.width as f32 * 2.0,
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
                        self.cell_width / self.config.width as f32 * 2.0,
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
                        (self.cell_height - border_width * 2.0) / self.config.height as f32 * 2.0,
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
                        (self.cell_height - border_width * 2.0) / self.config.height as f32 * 2.0,
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
            &self.bg_instance_buffer,
            (base_overlay_index * std::mem::size_of::<BackgroundInstance>()) as u64,
            bytemuck::cast_slice(&overlay_instances),
        );

        // Write command separator line instances after cursor overlay slots
        let separator_base = self.cols * self.rows + 10;
        let mut separator_instances = vec![
            BackgroundInstance {
                position: [0.0, 0.0],
                size: [0.0, 0.0],
                color: [0.0, 0.0, 0.0, 0.0],
            };
            self.rows
        ];

        if self.command_separator_enabled {
            let width_f = self.config.width as f32;
            let height_f = self.config.height as f32;
            for &(screen_row, exit_code, custom_color) in &self.visible_separator_marks {
                if screen_row < self.rows {
                    let x0 = self.window_padding + self.content_offset_x;
                    let x1 = width_f - self.window_padding;
                    let y0 = self.window_padding
                        + self.content_offset_y
                        + screen_row as f32 * self.cell_height;
                    let color = self.separator_color(exit_code, custom_color, 1.0);
                    separator_instances[screen_row] = BackgroundInstance {
                        position: [x0 / width_f * 2.0 - 1.0, 1.0 - (y0 / height_f * 2.0)],
                        size: [
                            (x1 - x0) / width_f * 2.0,
                            self.command_separator_thickness / height_f * 2.0,
                        ],
                        color,
                    };
                }
            }
        }

        for (i, instance) in separator_instances.iter().enumerate() {
            if separator_base + i < self.max_bg_instances {
                self.bg_instances[separator_base + i] = *instance;
            }
        }
        let separator_byte_offset = separator_base * std::mem::size_of::<BackgroundInstance>();
        let separator_byte_count =
            separator_instances.len() * std::mem::size_of::<BackgroundInstance>();
        if separator_byte_offset + separator_byte_count
            <= self.max_bg_instances * std::mem::size_of::<BackgroundInstance>()
        {
            self.queue.write_buffer(
                &self.bg_instance_buffer,
                separator_byte_offset as u64,
                bytemuck::cast_slice(&separator_instances),
            );
        }

        Ok(())
    }

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
        pane_background: Option<&crate::pane::PaneBackground>,
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
            self.pane_bg_cache.get(path.as_str()).map(|entry| {
                self.create_pane_bg_bind_group(
                    entry,
                    viewport.x,
                    viewport.y,
                    viewport.width,
                    viewport.height,
                    pane_bg.mode,
                    pane_bg.opacity,
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
            let clear_color = if self.bg_is_solid_color {
                wgpu::Color {
                    r: self.solid_bg_color[0] as f64
                        * self.window_opacity as f64
                        * viewport.opacity as f64,
                    g: self.solid_bg_color[1] as f64
                        * self.window_opacity as f64
                        * viewport.opacity as f64,
                    b: self.solid_bg_color[2] as f64
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
                render_pass.set_pipeline(&self.bg_image_pipeline);
                render_pass.set_bind_group(0, bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                render_pass.draw(0..4, 0..1);
            }

            // Render cell backgrounds
            render_pass.set_pipeline(&self.bg_pipeline);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.bg_instance_buffer.slice(..));
            render_pass.draw(0..4, 0..self.max_bg_instances as u32);

            // Render text
            render_pass.set_pipeline(&self.text_pipeline);
            render_pass.set_bind_group(0, &self.text_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.text_instance_buffer.slice(..));
            render_pass.draw(0..4, 0..self.max_text_instances as u32);

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
            enable_ligatures: self.enable_ligatures,
            enable_kerning: self.enable_kerning,
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
                let is_default_bg = (cell.bg_color[0] as f32 / 255.0 - self.background_color[0])
                    .abs()
                    < 0.001
                    && (cell.bg_color[1] as f32 / 255.0 - self.background_color[1]).abs() < 0.001
                    && (cell.bg_color[2] as f32 / 255.0 - self.background_color[2]).abs() < 0.001;

                // Check for cursor at this position
                let has_cursor = cursor_pos.is_some_and(|(cx, cy)| cx == col && cy == row)
                    && cursor_opacity > 0.0
                    && !self.cursor_hidden_for_shader;

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
                let mut bg_color = [
                    cell.bg_color[0] as f32 / 255.0,
                    cell.bg_color[1] as f32 / 255.0,
                    cell.bg_color[2] as f32 / 255.0,
                    pane_alpha,
                ];

                // Handle cursor at this position
                if has_cursor {
                    use par_term_emu_core_rust::cursor::CursorStyle;
                    match self.cursor_style {
                        CursorStyle::SteadyBlock | CursorStyle::BlinkingBlock => {
                            for (bg, &cursor) in bg_color.iter_mut().take(3).zip(&self.cursor_color)
                            {
                                *bg = *bg * (1.0 - cursor_opacity) + cursor * cursor_opacity;
                            }
                            bg_color[3] = bg_color[3].max(cursor_opacity * opacity_multiplier);
                        }
                        _ => {}
                    }

                    // Cursor cell can't be merged
                    let x0 = content_x + col as f32 * self.cell_width;
                    let y0 = content_y + row as f32 * self.cell_height;
                    let x1 = x0 + self.cell_width;
                    let y1 = y0 + self.cell_height;

                    if bg_index < self.max_bg_instances {
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
                let x0 = content_x + start_col as f32 * self.cell_width;
                let x1 = content_x + (start_col + run_length) as f32 * self.cell_width;
                let y0 = content_y + row as f32 * self.cell_height;
                let y1 = y0 + self.cell_height;

                if bg_index < self.max_bg_instances {
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
            let natural_line_height = self.font_ascent + self.font_descent + self.font_leading;
            let vertical_padding = (self.cell_height - natural_line_height).max(0.0) / 2.0;
            let baseline_y =
                content_y + (row as f32 * self.cell_height) + vertical_padding + self.font_ascent;

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
                        self.cell_width * 2.0
                    } else {
                        self.cell_width
                    };
                    let x0 = content_x + col_idx as f32 * self.cell_width;
                    let y0 = content_y + row as f32 * self.cell_height;

                    let fg_color = [
                        cell.fg_color[0] as f32 / 255.0,
                        cell.fg_color[1] as f32 / 255.0,
                        cell.fg_color[2] as f32 / 255.0,
                        text_alpha,
                    ];

                    // Try box drawing geometry first
                    let aspect_ratio = self.cell_height / char_w;
                    if let Some(box_geo) = block_chars::get_box_drawing_geometry(ch, aspect_ratio) {
                        for segment in &box_geo.segments {
                            let rect = segment.to_pixel_rect(x0, y0, char_w, self.cell_height);

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

                            if text_index < self.max_text_instances {
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
                                        self.solid_pixel_offset.0 as f32 / 2048.0,
                                        self.solid_pixel_offset.1 as f32 / 2048.0,
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
                        let rect = geo_block.to_pixel_rect(x0, y0, char_w, self.cell_height);

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

                        if text_index < self.max_text_instances {
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
                                    self.solid_pixel_offset.0 as f32 / 2048.0,
                                    self.solid_pixel_offset.1 as f32 / 2048.0,
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

                // Regular glyph rendering
                let glyph_result = if chars.len() > 1 {
                    self.font_manager
                        .find_grapheme_glyph(&cell.grapheme, cell.bold, cell.italic)
                } else {
                    self.font_manager.find_glyph(ch, cell.bold, cell.italic)
                };

                if let Some((font_idx, glyph_id)) = glyph_result {
                    let cache_key = ((font_idx as u64) << 32) | (glyph_id as u64);
                    let info = if self.glyph_cache.contains_key(&cache_key) {
                        self.lru_remove(cache_key);
                        self.lru_push_front(cache_key);
                        self.glyph_cache.get(&cache_key).unwrap().clone()
                    } else if let Some(raster) = self.rasterize_glyph(font_idx, glyph_id) {
                        let info = self.upload_glyph(cache_key, &raster);
                        self.glyph_cache.insert(cache_key, info.clone());
                        self.lru_push_front(cache_key);
                        info
                    } else {
                        continue;
                    };

                    let char_w = if cell.wide_char {
                        self.cell_width * 2.0
                    } else {
                        self.cell_width
                    };
                    let x0 = content_x + col_idx as f32 * self.cell_width;
                    let y0 = content_y + row as f32 * self.cell_height;
                    let x1 = x0 + char_w;
                    let y1 = y0 + self.cell_height;

                    let cell_w = x1 - x0;
                    let cell_h = y1 - y0;
                    let scale_x = cell_w / char_w;
                    let scale_y = cell_h / self.cell_height;

                    let baseline_offset = baseline_y - (content_y + row as f32 * self.cell_height);
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

                    let fg_color = [
                        cell.fg_color[0] as f32 / 255.0,
                        cell.fg_color[1] as f32 / 255.0,
                        cell.fg_color[2] as f32 / 255.0,
                        text_alpha,
                    ];

                    if text_index < self.max_text_instances {
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
        if self.command_separator_enabled && !separator_marks.is_empty() {
            let width_f = self.config.width as f32;
            let height_f = self.config.height as f32;
            let opacity_multiplier = viewport.opacity;
            for &(screen_row, exit_code, custom_color) in separator_marks {
                if screen_row < rows && bg_index < self.max_bg_instances {
                    let x0 = content_x;
                    let x1 = content_x + cols as f32 * self.cell_width;
                    let y0 = content_y + screen_row as f32 * self.cell_height;
                    let color = self.separator_color(exit_code, custom_color, opacity_multiplier);
                    self.bg_instances[bg_index] = BackgroundInstance {
                        position: [x0 / width_f * 2.0 - 1.0, 1.0 - (y0 / height_f * 2.0)],
                        size: [
                            (x1 - x0) / width_f * 2.0,
                            self.command_separator_thickness / height_f * 2.0,
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
            &self.bg_instance_buffer,
            0,
            bytemuck::cast_slice(&self.bg_instances),
        );
        self.queue.write_buffer(
            &self.text_instance_buffer,
            0,
            bytemuck::cast_slice(&self.text_instances),
        );

        Ok(())
    }
}
