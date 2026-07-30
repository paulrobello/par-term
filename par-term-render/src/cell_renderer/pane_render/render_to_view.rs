//! Single-pane surface rendering entry point.
//!
//! Provides [`CellRenderer::render_pane_to_view`], which builds this pane's instance
//! buffers, prepares any per-pane background image bind group, and issues the pane's
//! render pass (per-pane background image → 3-phase cell draw → optional scrollbar).

use super::super::CellRenderer;
use super::{PaneInstanceBuildParams, PaneRenderViewParams};
use anyhow::Result;

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
    pub fn render_pane_to_view(
        &mut self,
        surface_view: &wgpu::TextureView,
        p: PaneRenderViewParams<'_>,
    ) -> Result<()> {
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
        // Build instance buffers for this pane's cells.
        // Returns cursor_overlay_start: the bg_instance index where cursor overlays begin.
        // Used for 3-phase rendering (bgs → text → cursor overlays).
        let cursor_overlay_start = self.build_pane_instance_buffers(PaneInstanceBuildParams {
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
        let has_pane_bg = if let Some(pane_bg) = pane_background
            && let Some(ref path) = pane_bg.image_path
            && self.bg_state.pane_bg_cache.contains_key(path.as_str())
        {
            self.prepare_pane_bg_bind_group(
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
            true
        } else {
            false
        };

        // Retrieve cached path for use in the render pass (must be done before borrow in pass).
        let pane_bg_path: Option<String> = if has_pane_bg {
            pane_background
                .and_then(|pb| pb.image_path.as_ref())
                .map(|p| p.to_string())
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
                multiview_mask: None,
            });

            // Set scissor rect to clip rendering to pane bounds
            let (sx, sy, sw, sh) = viewport.to_scissor_rect();
            render_pass.set_scissor_rect(sx, sy, sw, sh);

            // Render per-pane background image within scissor rect.
            // Per-pane backgrounds are explicit user overrides and always render,
            // even when a custom shader or global background is active.
            if let Some(ref path) = pane_bg_path
                && let Some(cached) = self.bg_state.pane_bg_uniform_cache.get(path.as_str())
            {
                render_pass.set_pipeline(&self.pipelines.bg_image_pipeline);
                render_pass.set_bind_group(0, &cached.bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.buffers.vertex_buffer.slice(..));
                render_pass.draw(0..4, 0..1);
            }

            self.emit_three_phase_draw_calls(
                &mut render_pass,
                cursor_overlay_start as u32,
                self.buffers.actual_bg_instances as u32,
            );

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
}
