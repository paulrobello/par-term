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

        // Pre-update per-pane background uniform buffer and bind group if needed (must happen
        // before render pass). Buffers are allocated once and reused across frames.
        // This supports pane 0 background in single-pane (no splits) mode.
        let pane_bg_path: Option<String> = if !self.bg_state.bg_is_solid_color {
            if let Some(pane_bg) = pane_background {
                if let Some(ref path) = pane_bg.image_path
                    && self.bg_state.pane_bg_cache.contains_key(path.as_str())
                {
                    self.prepare_pane_bg_bind_group(
                        path.as_str(),
                        crate::cell_renderer::background::PaneBgBindGroupParams {
                            pane_x: 0.0, // full window starts at 0
                            pane_y: 0.0, // full window starts at 0
                            pane_width: self.config.width as f32,
                            pane_height: self.config.height as f32,
                            mode: pane_bg.mode,
                            opacity: pane_bg.opacity,
                            darken: pane_bg.darken,
                        },
                    );
                    Some(path.to_string())
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
        // - Per-pane bg: use TRANSPARENT clear, render pane bg before global bg
        // - Any mode with bg_image_bind_group (Color, Image, Default): use TRANSPARENT clear,
        //   let bg_image_pipeline render a full-screen opaque quad (prevents macOS alpha artifacts)
        // - Fallback (no bind group): use theme background with window_opacity as clear color
        let has_pane_bg = pane_bg_path.is_some();
        let (clear_color, use_bg_image_pipeline) = if has_pane_bg {
            // Per-pane background: use transparent clear, pane bg will be rendered first
            (wgpu::Color::TRANSPARENT, false)
        } else if self.pipelines.bg_image_bind_group.is_some() {
            // Use bg_image_pipeline for ALL modes with a texture (Image, Color, Default).
            // A full-screen opaque quad ensures complete pixel coverage, preventing
            // macOS per-pixel alpha transparency artifacts from LoadOp::Clear alone.
            (wgpu::Color::TRANSPARENT, true)
        } else {
            // Fallback: no texture available - use theme background with window_opacity
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
            if let Some(ref path) = pane_bg_path
                && let Some(cached) = self.bg_state.pane_bg_uniform_cache.get(path.as_str())
            {
                render_pass.set_pipeline(&self.pipelines.bg_image_pipeline);
                render_pass.set_bind_group(0, &cached.bind_group, &[]);
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

            // Phase 1: cell backgrounds (before text)
            let cell_bg_end = (self.grid.cols * self.grid.rows) as u32;
            let cursor_overlay_end =
                cell_bg_end + super::instance_buffers::CURSOR_OVERLAY_SLOTS as u32;
            render_pass.set_pipeline(&self.pipelines.bg_pipeline);
            render_pass.set_vertex_buffer(0, self.buffers.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.buffers.bg_instance_buffer.slice(..));
            render_pass.draw(0..4, 0..cell_bg_end);

            // Phase 1b: separator + gutter overlays (before text, background elements)
            if cursor_overlay_end < self.buffers.actual_bg_instances as u32 {
                render_pass.draw(
                    0..4,
                    cursor_overlay_end..self.buffers.actual_bg_instances as u32,
                );
            }

            // Phase 2: text (on top of cell backgrounds)
            render_pass.set_pipeline(&self.pipelines.text_pipeline);
            render_pass.set_bind_group(0, &self.pipelines.text_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.buffers.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.buffers.text_instance_buffer.slice(..));
            render_pass.draw(0..4, 0..self.buffers.actual_text_instances as u32);

            // Phase 3: cursor overlays (beam/underline bar + hollow outline) ON TOP of text
            render_pass.set_pipeline(&self.pipelines.bg_pipeline);
            render_pass.set_vertex_buffer(0, self.buffers.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.buffers.bg_instance_buffer.slice(..));
            render_pass.draw(0..4, cell_bg_end..cursor_overlay_end);
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

        // Render background to intermediate texture via bg_image_pipeline when available.
        // This covers all modes (Image, Color, Default) with a full-screen opaque quad.
        let render_background_image =
            !skip_background_image && self.pipelines.bg_image_bind_group.is_some();

        if render_background_image {
            // Pass Some(1.0) to render the background image at full opacity for this
            // intermediate texture; the shader wrapper will apply window_opacity at the end.
            // This avoids temporarily mutating self.window_opacity (which could be skipped
            // on restoration if an early return via `?` fires after this point).
            self.update_bg_image_uniforms(Some(1.0));
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
                render_pass.set_pipeline(&self.pipelines.bg_image_pipeline);
                render_pass.set_bind_group(0, bg_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.buffers.vertex_buffer.slice(..));
                render_pass.draw(0..4, 0..1);
            }

            // Phase 1: cell backgrounds (before text)
            let cell_bg_end = (self.grid.cols * self.grid.rows) as u32;
            let cursor_overlay_end =
                cell_bg_end + super::instance_buffers::CURSOR_OVERLAY_SLOTS as u32;
            render_pass.set_pipeline(&self.pipelines.bg_pipeline);
            render_pass.set_vertex_buffer(0, self.buffers.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.buffers.bg_instance_buffer.slice(..));
            render_pass.draw(0..4, 0..cell_bg_end);

            // Phase 1b: separator + gutter overlays (before text, background elements)
            if cursor_overlay_end < self.buffers.actual_bg_instances as u32 {
                render_pass.draw(
                    0..4,
                    cursor_overlay_end..self.buffers.actual_bg_instances as u32,
                );
            }

            // Phase 2: text (on top of cell backgrounds)
            render_pass.set_pipeline(&self.pipelines.text_pipeline);
            render_pass.set_bind_group(0, &self.pipelines.text_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.buffers.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.buffers.text_instance_buffer.slice(..));
            render_pass.draw(0..4, 0..self.buffers.actual_text_instances as u32);

            // Phase 3: cursor overlays (beam/underline bar + hollow outline) ON TOP of text
            render_pass.set_pipeline(&self.pipelines.bg_pipeline);
            render_pass.set_vertex_buffer(0, self.buffers.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.buffers.bg_instance_buffer.slice(..));
            render_pass.draw(0..4, cell_bg_end..cursor_overlay_end);
        }

        self.queue.submit(std::iter::once(encoder.finish()));

        // Restore the uniforms to use the actual window_opacity now that the intermediate
        // texture has been submitted.  No state mutation occurred above — self.window_opacity
        // was never changed — so we simply write the real value back into the buffer.
        if render_background_image {
            self.update_bg_image_uniforms(None);
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

        // Use bg_image_pipeline when a bind group exists (Image, Color, or Default modes).
        // This renders a full-screen opaque quad, preventing macOS alpha artifacts.
        let use_bg_image_pipeline = self.pipelines.bg_image_bind_group.is_some();
        let clear_color = if use_bg_image_pipeline {
            wgpu::Color::TRANSPARENT
        } else {
            wgpu::Color {
                r: self.background_color[0] as f64 * self.window_opacity as f64,
                g: self.background_color[1] as f64 * self.window_opacity as f64,
                b: self.background_color[2] as f64 * self.window_opacity as f64,
                a: self.window_opacity as f64,
            }
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

            // Render background via bg_image_pipeline (full-screen opaque quad)
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

        // Use bg_image_pipeline when a bind group exists (Image, Color, or Default modes).
        let use_bg_image_pipeline = self.pipelines.bg_image_bind_group.is_some();
        let clear_color = if use_bg_image_pipeline {
            wgpu::Color::TRANSPARENT
        } else {
            wgpu::Color {
                r: self.background_color[0] as f64 * self.window_opacity as f64,
                g: self.background_color[1] as f64 * self.window_opacity as f64,
                b: self.background_color[2] as f64 * self.window_opacity as f64,
                a: self.window_opacity as f64,
            }
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

            // Render background via bg_image_pipeline (full-screen opaque quad)
            if use_bg_image_pipeline
                && let Some(ref bg_bind_group) = self.pipelines.bg_image_bind_group
            {
                render_pass.set_pipeline(&self.pipelines.bg_image_pipeline);
                render_pass.set_bind_group(0, bg_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.buffers.vertex_buffer.slice(..));
                render_pass.draw(0..4, 0..1);
            }

            // Phase 1: cell backgrounds (before text)
            let cell_bg_end = (self.grid.cols * self.grid.rows) as u32;
            let cursor_overlay_end =
                cell_bg_end + super::instance_buffers::CURSOR_OVERLAY_SLOTS as u32;
            render_pass.set_pipeline(&self.pipelines.bg_pipeline);
            render_pass.set_vertex_buffer(0, self.buffers.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.buffers.bg_instance_buffer.slice(..));
            render_pass.draw(0..4, 0..cell_bg_end);

            // Phase 1b: separator + gutter overlays (before text, background elements)
            if cursor_overlay_end < self.buffers.actual_bg_instances as u32 {
                render_pass.draw(
                    0..4,
                    cursor_overlay_end..self.buffers.actual_bg_instances as u32,
                );
            }

            // Phase 2: text (on top of cell backgrounds)
            render_pass.set_pipeline(&self.pipelines.text_pipeline);
            render_pass.set_bind_group(0, &self.pipelines.text_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.buffers.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.buffers.text_instance_buffer.slice(..));
            render_pass.draw(0..4, 0..self.buffers.actual_text_instances as u32);

            // Phase 3: cursor overlays (beam/underline bar + hollow outline) ON TOP of text
            render_pass.set_pipeline(&self.pipelines.bg_pipeline);
            render_pass.set_vertex_buffer(0, self.buffers.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.buffers.bg_instance_buffer.slice(..));
            render_pass.draw(0..4, cell_bg_end..cursor_overlay_end);

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
                // Update visual bell uniform buffer with fullscreen quad params
                // Layout: position (vec2) + size (vec2) + color (vec4) = 32 bytes
                let uniforms: [f32; 8] = [
                    -1.0,                       // position.x (NDC left)
                    -1.0,                       // position.y (NDC bottom)
                    2.0,                        // size.x (full width in NDC)
                    2.0,                        // size.y (full height in NDC)
                    self.visual_bell_color[0],  // color.r
                    self.visual_bell_color[1],  // color.g
                    self.visual_bell_color[2],  // color.b
                    self.visual_bell_intensity, // color.a (intensity)
                ];
                self.queue.write_buffer(
                    &self.buffers.visual_bell_uniform_buffer,
                    0,
                    bytemuck::cast_slice(&uniforms),
                );

                render_pass.set_pipeline(&self.pipelines.visual_bell_pipeline);
                render_pass.set_bind_group(0, &self.pipelines.visual_bell_bind_group, &[]);
                render_pass.draw(0..4, 0..1); // 4 vertices = triangle strip quad
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        Ok(())
    }

    /// Stamp alpha=1.0 over the entire surface without modifying RGB values.
    ///
    /// On macOS with `CompositeAlphaMode::PreMultiplied`, any framebuffer pixel with
    /// alpha < 1.0 becomes translucent through to the desktop. Multiple rendering
    /// passes (anti-aliased text, overlay compositing) can inadvertently reduce alpha.
    /// This single full-screen triangle guarantees an opaque surface.
    ///
    /// Skipped when `window_opacity < 1.0` so that user-configured transparency works.
    pub fn render_opaque_alpha(&self, surface_texture: &wgpu::SurfaceTexture) -> Result<()> {
        if self.window_opacity < 1.0 {
            return Ok(());
        }

        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("opaque alpha encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("opaque alpha pass"),
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

            render_pass.set_pipeline(&self.pipelines.opaque_alpha_pipeline);
            render_pass.draw(0..3, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        Ok(())
    }
}
