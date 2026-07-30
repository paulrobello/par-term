//! Pane surface rendering entry points.
//!
//! ARC-004: rendering a pane is split into two phases.
//!
//! * [`CellRenderer::prepare_pane`] does everything that needs `&mut self` — building
//!   the pane's instances into its own slice of the shared buffers, updating its
//!   scrollbar slot, and preparing its background-image bind group — and returns the
//!   absolute draw ranges as a [`PreparedPane`].
//! * [`CellRenderer::draw_prepared_panes`] takes `&self`, and turns a whole frame's
//!   worth of prepared panes into **one** command encoder with one render pass per
//!   pane, submitted once.
//!
//! The phases have to stay separate: the draw phase holds a render pass borrowed
//! from the encoder, so it cannot allocate or write anything. Every write is queued
//! before the single submit, which is exactly why each pane needs its own instance
//! range, scrollbar slot, and background uniform — one submit no longer separates
//! one pane's writes from the next's.

use super::super::CellRenderer;
use super::super::render::ThreePhaseRanges;
use super::{PaneInstanceBuildParams, PaneRenderViewParams};
use anyhow::Result;

/// One pane's GPU state for the frame, ready to be drawn.
pub(crate) struct PreparedPane {
    /// Scissor rect clipping the pane to its viewport.
    scissor: (u32, u32, u32, u32),
    /// `Some(color)` clears the pane's region before drawing; `None` loads it.
    clear_color: Option<wgpu::Color>,
    /// Key into `bg_state.pane_bg_uniform_cache` when this pane has a background image.
    pane_bg_index: Option<usize>,
    /// Absolute instance ranges for the three-phase cell draw.
    ranges: ThreePhaseRanges,
    /// Scrollbar slot to draw after the cells, if this pane shows one.
    scrollbar_slot: Option<usize>,
}

impl CellRenderer {
    /// Build one pane's GPU state for this frame.
    ///
    /// `pane_index` identifies the pane within the current batch and keys both its
    /// scrollbar slot and its background-image uniform. Callers must have started
    /// the batch with [`CellRenderer::begin_pane_batch`] and must pass indices
    /// `0..n` in order.
    ///
    /// # Arguments
    /// * `viewport` - The pane's viewport (position, size, focus state, opacity)
    /// * `cells` - The cells to render (should match viewport grid size)
    /// * `cols` / `rows` - The pane's cell grid dimensions
    /// * `cursor_pos` - Cursor position (col, row) within this pane, or None if no cursor
    /// * `cursor_opacity` - Cursor opacity (0.0 = hidden, 1.0 = fully visible)
    /// * `show_scrollbar` - Whether to render the scrollbar for this pane
    /// * `clear_first` - If true, clears the viewport region before rendering
    /// * `skip_background_image` - If true, skip rendering the background image. Use this
    ///   when the background image has already been rendered full-screen (for split panes).
    pub(crate) fn prepare_pane(
        &mut self,
        pane_index: usize,
        p: PaneRenderViewParams<'_>,
    ) -> Result<PreparedPane> {
        let PaneRenderViewParams {
            viewport,
            cells,
            cols,
            rows,
            cursor_pos,
            cursor_opacity,
            show_scrollbar,
            clear_first,
            skip_background_image,
            fill_default_bg_cells,
            separator_marks,
            pane_background,
        } = p;
        // Build this pane's slice of the shared instance buffers. The returned
        // ranges are absolute, so several panes stay resident at once.
        let ranges = self.build_pane_instance_buffers(PaneInstanceBuildParams {
            viewport,
            cells,
            cols,
            rows,
            cursor_pos,
            cursor_opacity,
            skip_solid_background: skip_background_image,
            fill_default_bg_cells,
            separator_marks,
        })?;

        // Pre-update per-pane background uniform buffer and bind group if needed (must happen
        // before the render pass). Buffers are allocated once and reused across frames.
        // Per-pane backgrounds are explicit user overrides and always prepared, even when a
        // custom shader or global background would normally be skipped.
        let pane_bg_index = if let Some(pane_bg) = pane_background
            && let Some(ref path) = pane_bg.image_path
            && self.bg_state.pane_bg_cache.contains_key(path.as_str())
        {
            self.prepare_pane_bg_bind_group(
                pane_index,
                path.as_str(),
                super::super::background::PaneBgBindGroupParams {
                    pane_x: viewport.x,
                    pane_y: viewport.y,
                    pane_width: viewport.width,
                    pane_height: viewport.height,
                    mode: pane_bg.mode,
                    opacity: pane_bg.opacity,
                    darken: pane_bg.darken,
                },
            );
            Some(pane_index)
        } else {
            None
        };

        // Determine load operation and clear color
        let clear_color = clear_first.then(|| {
            let base: [f32; 3] = if self.bg_state.bg_is_solid_color {
                self.bg_state.solid_bg_color
            } else {
                [
                    self.background_color[0],
                    self.background_color[1],
                    self.background_color[2],
                ]
            };
            let alpha = self.window_opacity as f64 * viewport.opacity as f64;
            wgpu::Color {
                r: base[0] as f64 * alpha,
                g: base[1] as f64 * alpha,
                b: base[2] as f64 * alpha,
                a: alpha,
            }
        });

        Ok(PreparedPane {
            scissor: viewport.to_scissor_rect(),
            clear_color,
            pane_bg_index,
            ranges: ThreePhaseRanges {
                bg: ranges.bg.start as u32..ranges.bg.end as u32,
                // The pane layout packs separators before the cursor overlays, so
                // there is nothing after them to draw in phase 1b.
                extra_bg: 0..0,
                text: ranges.text.start as u32..ranges.text.end as u32,
                cursor_overlays: ranges.cursor_overlays.start as u32
                    ..ranges.cursor_overlays.end as u32,
            },
            scrollbar_slot: show_scrollbar.then_some(pane_index),
        })
    }

    /// Draw a frame's prepared panes to `surface_view` from a single command encoder.
    ///
    /// One render pass per pane keeps each pane's scissor rect scoped to its own
    /// pass; passes run in order and load the target, so the result is identical to
    /// the per-pane submits this replaces, minus N-1 submits per frame.
    pub(crate) fn draw_prepared_panes(
        &self,
        surface_view: &wgpu::TextureView,
        panes: &[PreparedPane],
    ) {
        if panes.is_empty() {
            return;
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("pane batch encoder"),
            });

        for pane in panes {
            let load = match pane.clear_color {
                Some(color) => wgpu::LoadOp::Clear(color),
                None => wgpu::LoadOp::Load,
            };

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("pane render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            // Set scissor rect to clip rendering to pane bounds
            let (sx, sy, sw, sh) = pane.scissor;
            render_pass.set_scissor_rect(sx, sy, sw, sh);

            // Render per-pane background image within scissor rect.
            // Per-pane backgrounds are explicit user overrides and always render,
            // even when a custom shader or global background is active.
            if let Some(index) = pane.pane_bg_index
                && let Some(cached) = self.bg_state.pane_bg_uniform_cache.get(&index)
            {
                render_pass.set_pipeline(&self.pipelines.bg_image_pipeline);
                render_pass.set_bind_group(0, &cached.bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.buffers.vertex_buffer.slice(..));
                render_pass.draw(0..4, 0..1);
            }

            self.emit_three_phase_draw_calls(&mut render_pass, &pane.ranges);

            // Render scrollbar if requested (uses its own scissor rect internally)
            if let Some(slot) = pane.scrollbar_slot {
                // Reset scissor to full surface for scrollbar
                render_pass.set_scissor_rect(0, 0, self.config.width, self.config.height);
                self.scrollbar.render(&mut render_pass, slot);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Render a single pane's content within a viewport to an existing surface texture.
    ///
    /// Convenience wrapper over [`CellRenderer::prepare_pane`] +
    /// [`CellRenderer::draw_prepared_panes`] for callers that render one pane on its
    /// own. Multi-pane frames should drive the two phases directly so the whole
    /// frame shares one encoder.
    pub fn render_pane_to_view(
        &mut self,
        surface_view: &wgpu::TextureView,
        p: PaneRenderViewParams<'_>,
    ) -> Result<()> {
        let (bg, text) = super::pane_instance_capacity(p.cols, p.rows);
        self.begin_pane_batch(bg, text);
        let prepared = self.prepare_pane(0, p)?;
        self.draw_prepared_panes(surface_view, std::slice::from_ref(&prepared));
        Ok(())
    }
}
