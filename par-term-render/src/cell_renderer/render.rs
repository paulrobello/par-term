use super::CellRenderer;
use anyhow::Result;
use std::ops::Range;

/// Background instance slots reserved for cursor overlays.
///
/// Re-exported here because `instance_buffers` is a private module and the slot
/// count is part of the buffer layout this module defines. Anything checking the
/// layout must read it from here rather than restating `10`.
pub use super::instance_buffers::CURSOR_OVERLAY_SLOTS;

/// Which GPU pipeline a phase draws with.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CellPipeline {
    /// The background-quad pipeline, fed from `bg_instance_buffer`.
    Background,
    /// The glyph pipeline, fed from `text_instance_buffer`.
    Text,
}

/// One phase of the three-phase cell draw, in the order it must be emitted.
///
/// The ordering constraint is the reason this enum exists: cursor overlays are
/// drawn with the background pipeline but must be emitted *after* the text, or a
/// beam or underline cursor ends up hidden underneath the glyphs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CellDrawPhase {
    /// Phase 1 — cell background quads.
    Background,
    /// Phase 1b — separator / gutter quads that sit after the cursor overlays.
    ExtraBackground,
    /// Phase 2 — glyph and underline quads.
    Text,
    /// Phase 3 — cursor overlays.
    CursorOverlays,
}

impl CellDrawPhase {
    /// The pipeline this phase draws with.
    pub fn pipeline(self) -> CellPipeline {
        match self {
            Self::Background | Self::ExtraBackground | Self::CursorOverlays => {
                CellPipeline::Background
            }
            Self::Text => CellPipeline::Text,
        }
    }
}

/// Instance-buffer layout of the single-grid (offscreen) path.
///
/// One background buffer holds four back-to-back regions. Four call sites used to
/// restate this arithmetic — the builder's write offsets, the draw ranges, the
/// capacity check and the initial allocation — so a change to one could silently
/// disagree with the others. They all read it from here now.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct SingleGridLayout {
    /// One quad per cell, in row-major order.
    pub cells: Range<usize>,
    /// [`CURSOR_OVERLAY_SLOTS`] slots, written whether or not a cursor is shown.
    pub cursor_overlays: Range<usize>,
    /// One command-separator line slot per row.
    pub separators: Range<usize>,
    /// One gutter-indicator slot per row.
    pub gutters: Range<usize>,
}

impl SingleGridLayout {
    /// The layout for a `cols` × `rows` grid.
    pub fn new(cols: usize, rows: usize) -> Self {
        let cells = 0..cols * rows;
        let cursor_overlays = cells.end..cells.end + CURSOR_OVERLAY_SLOTS;
        let separators = cursor_overlays.end..cursor_overlays.end + rows;
        let gutters = separators.end..separators.end + rows;
        Self {
            cells,
            cursor_overlays,
            separators,
            gutters,
        }
    }

    /// Total background instances the layout occupies.
    pub fn bg_instances(&self) -> usize {
        self.gutters.end
    }
}

/// Absolute instance ranges for one three-phase cell draw.
///
/// ARC-004: the pane path suballocates the shared instance buffers, so a pane's
/// instances no longer start at index 0. Every range here is absolute into
/// `bg_instance_buffer` / `text_instance_buffer`, which lets several panes stay
/// resident at once and be drawn from a single command encoder.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ThreePhaseRanges {
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

impl ThreePhaseRanges {
    /// The phases in the order they must be drawn, with the range each covers.
    ///
    /// This is the single source of truth for the ordering.
    /// [`CellRenderer::emit_three_phase_draw_calls`] walks exactly this sequence,
    /// so the order cannot be changed there without changing it here — which is
    /// what makes the invariant testable without a GPU.
    pub fn draw_sequence(&self) -> [(CellDrawPhase, Range<u32>); 4] {
        [
            (CellDrawPhase::Background, self.bg.clone()),
            (CellDrawPhase::ExtraBackground, self.extra_bg.clone()),
            (CellDrawPhase::Text, self.text.clone()),
            (CellDrawPhase::CursorOverlays, self.cursor_overlays.clone()),
        ]
    }

    /// Draw ranges for the single-grid layout `build_instance_buffers` writes.
    ///
    /// `actual_bg_instances` is clamped up to the end of the overlay slots so a
    /// grid that has not been built yet cannot produce an inverted range.
    pub fn for_single_grid(
        layout: &SingleGridLayout,
        actual_bg_instances: usize,
        actual_text_instances: usize,
    ) -> Self {
        let overlay_end = layout.cursor_overlays.end;
        let bg_end = actual_bg_instances.max(overlay_end);
        Self {
            bg: layout.cells.start as u32..layout.cells.end as u32,
            extra_bg: overlay_end as u32..bg_end as u32,
            text: 0..actual_text_instances as u32,
            cursor_overlays: layout.cursor_overlays.start as u32..overlay_end as u32,
        }
    }

    /// Draw ranges for one pane's slice of the shared instance buffers.
    ///
    /// The pane builder packs separators *before* the cursor overlays and emits
    /// the overlays immediately after its background run, so phase 1b is empty
    /// and the overlays are contiguous with `bg`.
    pub fn for_pane(bg: Range<usize>, cursor_overlays: Range<usize>, text: Range<usize>) -> Self {
        debug_assert_eq!(
            bg.end, cursor_overlays.start,
            "a pane's cursor overlays must directly follow its background run"
        );
        Self {
            bg: bg.start as u32..bg.end as u32,
            extra_bg: 0..0,
            text: text.start as u32..text.end as u32,
            cursor_overlays: cursor_overlays.start as u32..cursor_overlays.end as u32,
        }
    }
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
    ///
    /// The order is not written out here: this walks
    /// [`ThreePhaseRanges::draw_sequence`], which is what a test can check
    /// without a GPU. Empty ranges are skipped, and pipeline state is set only
    /// when the phase changes which pipeline it needs.
    pub(crate) fn emit_three_phase_draw_calls(
        &self,
        render_pass: &mut wgpu::RenderPass<'_>,
        ranges: &ThreePhaseRanges,
    ) {
        let mut bound: Option<CellPipeline> = None;

        for (phase, range) in ranges.draw_sequence() {
            if range.is_empty() {
                continue;
            }

            let pipeline = phase.pipeline();
            if bound != Some(pipeline) {
                match pipeline {
                    CellPipeline::Background => {
                        render_pass.set_pipeline(&self.pipelines.bg_pipeline);
                        render_pass.set_vertex_buffer(0, self.buffers.vertex_buffer.slice(..));
                        render_pass.set_vertex_buffer(1, self.buffers.bg_instance_buffer.slice(..));
                    }
                    CellPipeline::Text => {
                        render_pass.set_pipeline(&self.pipelines.text_pipeline);
                        render_pass.set_bind_group(0, &self.pipelines.text_bind_group, &[]);
                        render_pass.set_vertex_buffer(0, self.buffers.vertex_buffer.slice(..));
                        render_pass
                            .set_vertex_buffer(1, self.buffers.text_instance_buffer.slice(..));
                    }
                }
                bound = Some(pipeline);
            }

            render_pass.draw(0..4, range);
        }
    }

    /// Ranges for the single-grid layout that `build_instance_buffers` writes.
    fn single_grid_ranges(&self) -> ThreePhaseRanges {
        ThreePhaseRanges::for_single_grid(
            &SingleGridLayout::new(self.grid.cols, self.grid.rows),
            self.buffers.actual_bg_instances,
            self.buffers.actual_text_instances,
        )
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Position of a phase within the draw sequence.
    fn phase_index(ranges: &ThreePhaseRanges, phase: CellDrawPhase) -> usize {
        ranges
            .draw_sequence()
            .iter()
            .position(|(candidate, _)| *candidate == phase)
            .unwrap_or_else(|| panic!("{phase:?} is missing from the draw sequence"))
    }

    /// A pane's ranges as `build_pane_instance_buffers` returns them: the viewport
    /// fill, its cell backgrounds and separators, then the overlays, then the
    /// pane's text region in the other buffer.
    fn pane_ranges(bg_base: usize, bg_len: usize, overlays: usize) -> ThreePhaseRanges {
        let bg = bg_base..bg_base + bg_len;
        ThreePhaseRanges::for_pane(bg.clone(), bg.end..bg.end + overlays, 40..90)
    }

    /// The invariant: cursor overlays are emitted after the text, or a beam or
    /// underline cursor is hidden under the glyphs.
    ///
    /// `emit_three_phase_draw_calls` walks `draw_sequence()`, so this holds it to
    /// the order the GPU actually receives without needing an adapter.
    #[test]
    fn cursor_overlays_are_drawn_after_text() {
        let single_grid = ThreePhaseRanges::for_single_grid(
            &SingleGridLayout::new(80, 24),
            80 * 24 + CURSOR_OVERLAY_SLOTS + 24 + 24,
            80 * 24 * 2,
        );
        let pane = pane_ranges(0, 1 + 80 * 24, CURSOR_OVERLAY_SLOTS);

        for ranges in [single_grid, pane] {
            assert!(
                phase_index(&ranges, CellDrawPhase::CursorOverlays)
                    > phase_index(&ranges, CellDrawPhase::Text),
                "cursor overlays must be emitted after the text: {ranges:?}"
            );
        }
    }

    /// Backgrounds come first, and phase 1b stays on the background side of the
    /// text so separators and gutters cannot cover a glyph.
    #[test]
    fn backgrounds_are_drawn_before_text() {
        let ranges = ThreePhaseRanges::for_single_grid(
            &SingleGridLayout::new(10, 4),
            SingleGridLayout::new(10, 4).bg_instances(),
            80,
        );
        let text = phase_index(&ranges, CellDrawPhase::Text);
        assert!(phase_index(&ranges, CellDrawPhase::Background) < text);
        assert!(phase_index(&ranges, CellDrawPhase::ExtraBackground) < text);
    }

    /// Overlays are drawn with the background pipeline but after the text, which
    /// is exactly why the phase cannot be folded back into phase 1.
    #[test]
    fn overlays_use_the_background_pipeline_in_a_later_phase() {
        assert_eq!(
            CellDrawPhase::CursorOverlays.pipeline(),
            CellDrawPhase::Background.pipeline()
        );
        assert_ne!(
            CellDrawPhase::CursorOverlays.pipeline(),
            CellDrawPhase::Text.pipeline()
        );
    }

    /// Every phase appears exactly once, so no draw is silently dropped or doubled.
    #[test]
    fn the_sequence_covers_each_phase_once() {
        let ranges = pane_ranges(7, 30, CURSOR_OVERLAY_SLOTS);
        let sequence = ranges.draw_sequence();
        for phase in [
            CellDrawPhase::Background,
            CellDrawPhase::ExtraBackground,
            CellDrawPhase::Text,
            CellDrawPhase::CursorOverlays,
        ] {
            assert_eq!(
                sequence
                    .iter()
                    .filter(|(candidate, _)| *candidate == phase)
                    .count(),
                1,
                "{phase:?} appears more than once"
            );
        }
    }

    /// The four single-grid regions tile the background buffer without gaps or
    /// overlaps, in the order the builder writes them.
    #[test]
    fn the_single_grid_regions_tile_the_background_buffer() {
        let (cols, rows) = (80usize, 24usize);
        let layout = SingleGridLayout::new(cols, rows);

        assert_eq!(layout.cells, 0..cols * rows);
        assert_eq!(layout.cursor_overlays.start, layout.cells.end);
        assert_eq!(layout.cursor_overlays.len(), CURSOR_OVERLAY_SLOTS);
        assert_eq!(layout.separators.start, layout.cursor_overlays.end);
        assert_eq!(layout.separators.len(), rows);
        assert_eq!(layout.gutters.start, layout.separators.end);
        assert_eq!(layout.gutters.len(), rows);
        assert_eq!(
            layout.bg_instances(),
            cols * rows + CURSOR_OVERLAY_SLOTS + rows + rows
        );
    }

    /// The cell quads and the overlays must not address the same instances, or
    /// the overlays would redraw cell backgrounds over the text.
    #[test]
    fn single_grid_overlays_do_not_overlap_the_cells() {
        let layout = SingleGridLayout::new(80, 24);
        let ranges = ThreePhaseRanges::for_single_grid(&layout, layout.bg_instances(), 3840);

        assert!(ranges.bg.end <= ranges.cursor_overlays.start);
        assert!(ranges.extra_bg.start >= ranges.cursor_overlays.end);
        assert_eq!(ranges.cursor_overlays.len(), CURSOR_OVERLAY_SLOTS);
    }

    /// A grid whose instances have never been built reports zero used instances;
    /// phase 1b must come out empty rather than inverted.
    #[test]
    fn an_unbuilt_single_grid_yields_an_empty_extra_phase() {
        let layout = SingleGridLayout::new(80, 24);
        let ranges = ThreePhaseRanges::for_single_grid(&layout, 0, 0);
        assert!(ranges.extra_bg.is_empty());
        assert!(ranges.extra_bg.start <= ranges.extra_bg.end);
    }

    /// Panes suballocate, so their ranges are absolute and phase 1b is unused:
    /// the pane builder packs separators before the overlays.
    #[test]
    fn a_pane_packs_its_overlays_directly_after_its_backgrounds() {
        let ranges = pane_ranges(500, 120, CURSOR_OVERLAY_SLOTS);

        assert_eq!(ranges.bg, 500..620);
        assert_eq!(ranges.cursor_overlays.start, ranges.bg.end);
        assert!(
            ranges.extra_bg.is_empty(),
            "the pane path has nothing to draw after its overlays"
        );
    }

    /// A pane with no cursor emits no overlay instances; the empty range must be
    /// skipped rather than issuing a zero-instance draw.
    #[test]
    fn a_pane_without_a_cursor_has_an_empty_overlay_phase() {
        let ranges = pane_ranges(0, 60, 0);
        let sequence = ranges.draw_sequence();
        let drawn: Vec<CellDrawPhase> = sequence
            .iter()
            .filter(|(_, range)| !range.is_empty())
            .map(|(phase, _)| *phase)
            .collect();
        assert_eq!(drawn, vec![CellDrawPhase::Background, CellDrawPhase::Text]);
    }

    /// With a background shader active the pane skips its viewport fill, and a
    /// screenful of default-background cells emits no background quads either —
    /// so phase 1 is empty while the cursor overlays are not.
    ///
    /// The emitter binds pipeline state lazily on the first non-empty phase, so
    /// this is the configuration where an overlay would be the first thing drawn.
    /// It must still come after the text.
    #[test]
    fn a_shaded_pane_with_no_background_quads_still_draws_overlays_last() {
        let ranges = ThreePhaseRanges::for_pane(300..300, 300..300 + CURSOR_OVERLAY_SLOTS, 90..140);
        assert!(ranges.bg.is_empty(), "the shader path emits no bg quads");

        let sequence = ranges.draw_sequence();
        let drawn: Vec<CellDrawPhase> = sequence
            .iter()
            .filter(|(_, range)| !range.is_empty())
            .map(|(phase, _)| *phase)
            .collect();
        assert_eq!(
            drawn,
            vec![CellDrawPhase::Text, CellDrawPhase::CursorOverlays],
            "the overlays must follow the text even when nothing precedes them"
        );

        // The two phases need different pipelines, so the emitter rebinds
        // between them rather than reusing whatever the caller left bound.
        assert_ne!(
            CellDrawPhase::Text.pipeline(),
            CellDrawPhase::CursorOverlays.pipeline()
        );
    }
}
