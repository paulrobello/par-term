use super::CellRenderer;
use anyhow::Result;

impl CellRenderer {
    pub fn render(
        &mut self,
        _show_scrollbar: bool,
        pane_background: Option<&par_term_config::PaneBackground>,
    ) -> Result<wgpu::SurfaceTexture> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.build_instance_buffers()?;

        // Pre-create per-pane background bind group if needed (must happen before render pass)
        // This supports pane 0 background in single-pane (no splits) mode.
        let pane_bg_resources = if !self.bg_state.bg_is_solid_color {
            if let Some(pane_bg) = pane_background {
                if let Some(ref path) = pane_bg.image_path {
                    self.bg_state.pane_bg_cache.get(path.as_str()).map(|entry| {
                        self.create_pane_bg_bind_group(
                            entry,
                            0.0, // pane_x: full window starts at 0
                            0.0, // pane_y: full window starts at 0
                            self.config.width as f32,
                            self.config.height as f32,
                            pane_bg.mode,
                            pane_bg.opacity,
                            pane_bg.darken,
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
        } else if self.bg_state.bg_is_solid_color {
            // Solid color mode: use clear color directly for proper window transparency
            // This works the same as Default mode - LoadOp::Clear sets alpha correctly
            log::info!(
                "[BACKGROUND] Solid color mode: RGB({:.3}, {:.3}, {:.3}) * opacity {:.3}",
                self.bg_state.solid_bg_color[0],
                self.bg_state.solid_bg_color[1],
                self.bg_state.solid_bg_color[2],
                self.window_opacity
            );
            (
                wgpu::Color {
                    r: self.bg_state.solid_bg_color[0] as f64 * self.window_opacity as f64,
                    g: self.bg_state.solid_bg_color[1] as f64 * self.window_opacity as f64,
                    b: self.bg_state.solid_bg_color[2] as f64 * self.window_opacity as f64,
                    a: self.window_opacity as f64,
                },
                false,
            )
        } else if self.pipelines.bg_image_bind_group.is_some() {
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
                render_pass.set_pipeline(&self.pipelines.bg_image_pipeline);
                render_pass.set_bind_group(0, bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.buffers.vertex_buffer.slice(..));
                render_pass.draw(0..4, 0..1);
            }

            // Render global background image if present (not used for solid color or pane bg mode)
            if use_bg_image_pipeline
                && let Some(ref bg_bind_group) = self.pipelines.bg_image_bind_group
            {
                render_pass.set_pipeline(&self.pipelines.bg_image_pipeline);
                render_pass.set_bind_group(0, bg_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.buffers.vertex_buffer.slice(..));
                render_pass.draw(0..4, 0..1);
            }

            render_pass.set_pipeline(&self.pipelines.bg_pipeline);
            render_pass.set_vertex_buffer(0, self.buffers.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.buffers.bg_instance_buffer.slice(..));
            render_pass.draw(0..4, 0..self.buffers.actual_bg_instances as u32);

            render_pass.set_pipeline(&self.pipelines.text_pipeline);
            render_pass.set_bind_group(0, &self.pipelines.text_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.buffers.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.buffers.text_instance_buffer.slice(..));
            render_pass.draw(0..4, 0..self.buffers.actual_text_instances as u32);
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
        let render_background_image = !skip_background_image
            && !self.bg_state.bg_is_solid_color
            && self.pipelines.bg_image_bind_group.is_some();
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
            if render_background_image
                && let Some(ref bg_bind_group) = self.pipelines.bg_image_bind_group
            {
                log::info!(
                    "[BACKGROUND] render_to_texture: bg_image_pipeline (image, window_opacity={:.3} applied by shader)",
                    saved_window_opacity
                );
                render_pass.set_pipeline(&self.pipelines.bg_image_pipeline);
                render_pass.set_bind_group(0, bg_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.buffers.vertex_buffer.slice(..));
                render_pass.draw(0..4, 0..1);
            }

            render_pass.set_pipeline(&self.pipelines.bg_pipeline);
            render_pass.set_vertex_buffer(0, self.buffers.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.buffers.bg_instance_buffer.slice(..));
            render_pass.draw(0..4, 0..self.buffers.actual_bg_instances as u32);

            render_pass.set_pipeline(&self.pipelines.text_pipeline);
            render_pass.set_bind_group(0, &self.pipelines.text_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.buffers.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.buffers.text_instance_buffer.slice(..));
            render_pass.draw(0..4, 0..self.buffers.actual_text_instances as u32);
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
        let (clear_color, use_bg_image_pipeline) = if self.bg_state.bg_is_solid_color {
            (
                wgpu::Color {
                    r: self.bg_state.solid_bg_color[0] as f64 * self.window_opacity as f64,
                    g: self.bg_state.solid_bg_color[1] as f64 * self.window_opacity as f64,
                    b: self.bg_state.solid_bg_color[2] as f64 * self.window_opacity as f64,
                    a: self.window_opacity as f64,
                },
                false,
            )
        } else if self.pipelines.bg_image_bind_group.is_some() {
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
            if use_bg_image_pipeline
                && let Some(ref bg_bind_group) = self.pipelines.bg_image_bind_group
            {
                render_pass.set_pipeline(&self.pipelines.bg_image_pipeline);
                render_pass.set_bind_group(0, bg_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.buffers.vertex_buffer.slice(..));
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
        let (clear_color, use_bg_image_pipeline) = if self.bg_state.bg_is_solid_color {
            (
                wgpu::Color {
                    r: self.bg_state.solid_bg_color[0] as f64 * self.window_opacity as f64,
                    g: self.bg_state.solid_bg_color[1] as f64 * self.window_opacity as f64,
                    b: self.bg_state.solid_bg_color[2] as f64 * self.window_opacity as f64,
                    a: self.window_opacity as f64,
                },
                false,
            )
        } else if self.pipelines.bg_image_bind_group.is_some() {
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
            if use_bg_image_pipeline
                && let Some(ref bg_bind_group) = self.pipelines.bg_image_bind_group
            {
                render_pass.set_pipeline(&self.pipelines.bg_image_pipeline);
                render_pass.set_bind_group(0, bg_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.buffers.vertex_buffer.slice(..));
                render_pass.draw(0..4, 0..1);
            }

            render_pass.set_pipeline(&self.pipelines.bg_pipeline);
            render_pass.set_vertex_buffer(0, self.buffers.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.buffers.bg_instance_buffer.slice(..));
            render_pass.draw(0..4, 0..self.buffers.actual_bg_instances as u32);

            render_pass.set_pipeline(&self.pipelines.text_pipeline);
            render_pass.set_bind_group(0, &self.pipelines.text_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.buffers.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.buffers.text_instance_buffer.slice(..));
            render_pass.draw(0..4, 0..self.buffers.actual_text_instances as u32);

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
        // Early return if no overlays to render - avoid creating empty command buffers
        if !show_scrollbar && self.visual_bell_intensity <= 0.0 {
            return Ok(());
        }

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
}
