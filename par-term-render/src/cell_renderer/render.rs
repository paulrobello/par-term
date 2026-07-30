use super::CellRenderer;
use anyhow::Result;
use std::ops::Range;

/// Absolute instance ranges for one three-phase cell draw.
///
/// ARC-004: the pane path suballocates the shared instance buffers, so a pane's
/// instances no longer start at index 0. Every range here is absolute into
/// `bg_instance_buffer` / `text_instance_buffer`, which lets several panes stay
/// resident at once and be drawn from a single command encoder.
pub(crate) struct ThreePhaseRanges {
    /// Phase 1 — cell background quads (includes the pane's viewport fill quad).
    pub bg: Range<u32>,
    /// Phase 1b — separator / gutter quads that live *after* the cursor overlays.
    /// Empty on the pane path, which packs separators before the overlays.
    pub extra_bg: Range<u32>,
    /// Phase 2 — glyph and underline quads.
    pub text: Range<u32>,
    /// Phase 3 — cursor overlays, drawn last so they sit on top of the glyphs.
    pub cursor_overlays: Range<u32>,
}

impl CellRenderer {
    /// Emit the standard 3-phase draw calls into an existing render pass.
    ///
    /// This is the single source of truth for the cell rendering draw call sequence.
    /// Background images / pane backgrounds must be drawn by the caller before this.
    ///
    /// **Phase 1**: Cell backgrounds
    /// **Phase 1b**: Separators / gutter — skipped when the range is empty
    ///   (the pane path packs these before the cursor overlays)
    /// **Phase 2**: Text glyphs
    /// **Phase 3**: Cursor overlays — must stay last, or beam and underline
    ///   cursors are hidden under the glyphs drawn in phase 2.
    pub(crate) fn emit_three_phase_draw_calls(
        &self,
        render_pass: &mut wgpu::RenderPass<'_>,
        ranges: &ThreePhaseRanges,
    ) {
        // Phase 1: cell backgrounds (before text)
        render_pass.set_pipeline(&self.pipelines.bg_pipeline);
        render_pass.set_vertex_buffer(0, self.buffers.vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, self.buffers.bg_instance_buffer.slice(..));
        render_pass.draw(0..4, ranges.bg.clone());

        // Phase 1b: separator + gutter overlays (before text, background elements)
        if !ranges.extra_bg.is_empty() {
            render_pass.draw(0..4, ranges.extra_bg.clone());
        }

        // Phase 2: text (on top of cell backgrounds)
        render_pass.set_pipeline(&self.pipelines.text_pipeline);
        render_pass.set_bind_group(0, &self.pipelines.text_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.buffers.vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, self.buffers.text_instance_buffer.slice(..));
        render_pass.draw(0..4, ranges.text.clone());

        // Phase 3: cursor overlays (beam/underline bar + hollow outline) ON TOP of text
        if !ranges.cursor_overlays.is_empty() {
            render_pass.set_pipeline(&self.pipelines.bg_pipeline);
            render_pass.set_vertex_buffer(0, self.buffers.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.buffers.bg_instance_buffer.slice(..));
            render_pass.draw(0..4, ranges.cursor_overlays.clone());
        }
    }

    /// Ranges for the single-grid layout that `build_instance_buffers` writes.
    ///
    /// Layout: `[0..cols*rows]` cells, `[cols*rows..+CURSOR_OVERLAY_SLOTS]` cursor
    /// overlays, then separators and gutter indicators up to `actual_bg_instances`.
    fn single_grid_ranges(&self) -> ThreePhaseRanges {
        let cursor_overlay_start = (self.grid.cols * self.grid.rows) as u32;
        let cursor_overlay_end =
            cursor_overlay_start + super::instance_buffers::CURSOR_OVERLAY_SLOTS as u32;
        let bg_end = (self.buffers.actual_bg_instances as u32).max(cursor_overlay_end);
        ThreePhaseRanges {
            bg: 0..cursor_overlay_start,
            extra_bg: cursor_overlay_end..bg_end,
            text: 0..self.buffers.actual_text_instances as u32,
            cursor_overlays: cursor_overlay_start..cursor_overlay_end,
        }
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
    ///
    /// QA-011: this used to acquire a `SurfaceTexture` it never drew to and never
    /// presented. The caller dropped it, which returns the drawable to the swapchain
    /// unpresented and can stall the next real frame's acquire. Nothing here targets
    /// the surface — `target_view` is the only render target — so the acquire is gone.
    pub fn render_to_texture(
        &mut self,
        target_view: &wgpu::TextureView,
        skip_background_image: bool,
    ) -> Result<()> {
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
                multiview_mask: None,
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

            self.emit_three_phase_draw_calls(&mut render_pass, &self.single_grid_ranges());
        }

        self.queue.submit(std::iter::once(encoder.finish()));

        // Restore the uniforms to use the actual window_opacity now that the intermediate
        // texture has been submitted.  No state mutation occurred above — self.window_opacity
        // was never changed — so we simply write the real value back into the buffer.
        if render_background_image {
            self.update_bg_image_uniforms(None);
        }

        Ok(())
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
                multiview_mask: None,
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
    ///
    /// QA-011: this used to skip the rebuild on the theory that a normal render had
    /// just left the buffers up to date. It has not been true since the pane path
    /// became the only live path: the pane builder writes pane-relative geometry at
    /// pane offsets, while the draw ranges below assume the single-grid layout, so
    /// the capture drew stale bytes from an older frame. Rebuilding makes the buffer
    /// contents and the ranges agree — the same thing `render_to_texture` does for
    /// the shader-active branch.
    pub fn render_to_view(&mut self, target_view: &wgpu::TextureView) -> Result<()> {
        self.build_instance_buffers()?;

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
                multiview_mask: None,
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

            self.emit_three_phase_draw_calls(&mut render_pass, &self.single_grid_ranges());

            // Render scrollbar (slot 0 — the single-grid path's slot)
            self.scrollbar.render(&mut render_pass, 0);
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
                multiview_mask: None,
            });

            if show_scrollbar {
                // Slot 0 — the single-grid path's slot.
                self.scrollbar.render(&mut render_pass, 0);
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
        // Checked before creating the view, not only inside the delegate: this runs
        // once per frame, and a transparent window would otherwise allocate and drop
        // a `TextureView` on the hot path for a call that does nothing.
        if self.window_opacity < 1.0 {
            return Ok(());
        }
        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.render_opaque_alpha_to_view(&view)
    }

    /// [`CellRenderer::render_opaque_alpha`] against an arbitrary target view.
    ///
    /// QA-011: the offscreen screenshot target needs the same alpha stamp as the
    /// surface, or a capture of an opaque window reads back semi-transparent
    /// wherever anti-aliased text reduced alpha.
    pub fn render_opaque_alpha_to_view(&self, view: &wgpu::TextureView) -> Result<()> {
        if self.window_opacity < 1.0 {
            return Ok(());
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("opaque alpha encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("opaque alpha pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
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
                multiview_mask: None,
            });

            render_pass.set_pipeline(&self.pipelines.opaque_alpha_pipeline);
            render_pass.draw(0..3, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        Ok(())
    }
}
