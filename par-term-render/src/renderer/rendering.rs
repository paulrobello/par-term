// ARC-009: `take_screenshot` and its shader chain moved to `renderer/screenshot.rs`.
// If this file needs cutting again, the next seams are:
//
//   split_layout.rs   — Split-pane geometry (render_split_panes_with_data)
//   separator_draw.rs — Separator-mark draw calls; QA-001 and QA-008 affect this area
//
// Tracking: Issue ARC-009 in AUDIT.md.

// ARC-004: `render_split_panes` used to end every pane in its own `queue.submit`.
// Panes are now built in one pass and drawn from a single command encoder — one
// render pass per pane, one submit per pane batch — so pane submits per frame went
// from N to 1. The exception is auto-dim without full-content mode, the one
// configuration that renders the panes twice: once into the shader's intermediate
// texture as a content mask, then again onto the content view. That is 2.
//
// That was unsafe until three single-instance GPU resources were made per-pane,
// each of which had been correct only because a submit separated one pane's write
// from the next's:
//
//   1. bg_instance_buffer / text_instance_buffer — every pane rebuilt them at
//      offset 0. Panes now suballocate: `begin_pane_batch` resets the cursors,
//      each build appends at them, and `emit_three_phase_draw_calls` takes
//      absolute ranges. The buffers stay bound at offset 0, so no vertex-buffer
//      offset alignment is involved and there is still one draw ordering.
//   2. bg_state.pane_bg_uniform_cache — was keyed by image path while the uniform
//      carries pane position and size, so two panes sharing an image aliased even
//      before batching. Now keyed by pane index, with the path stored in the entry
//      so the bind group is rebuilt only when a pane's image changes.
//   3. The scrollbar's thumb, track and mark uniforms (scrollbar.rs) — one set for
//      the whole renderer. Now one slot per pane. Hit testing stays
//      single-instance and follows the *focused* pane rather than whichever pane
//      was updated last.
//
// Capacity: `max_bg_instances` is sized for one full-window grid, which is not
// enough once panes are simultaneously resident — each pane also needs a viewport
// fill quad, its own separator rows and its own cursor overlay slots.
// `begin_pane_batch` sums `pane_instance_capacity` over the frame's panes, keeps
// the single-grid requirement as a floor, and grows the buffers when needed. The
// emitters' bounds guards drop instances silently, so a build that reaches the cap
// logs an error (once per allocation) instead of quietly losing content.
//
// Not measured: verifying the frame-rate effect needs a full workspace build, which
// this change was not able to run. No speedup is claimed.

use crate::cell_renderer::{PaneViewport, pane_instance_capacity};
use anyhow::Result;

use super::{
    DividerRenderInfo, PaneDividerSettings, PaneRenderInfo, PaneTitleInfo, Renderer, SeparatorMark,
    fill_visible_separator_marks,
};

// This file contains the multi-pane frame-level helper `render_split_panes`.

fn should_populate_terminal_intermediate_texture(
    full_content_mode: bool,
    auto_dim_under_text: bool,
    auto_dim_strength: f32,
) -> bool {
    full_content_mode || (auto_dim_under_text && auto_dim_strength > 0.0)
}

/// Per-batch settings for [`Renderer::prepare_and_draw_panes`].
///
/// The capacities are the summed `pane_instance_capacity` of the panes in the
/// batch; the two flags depend on which target the batch is being drawn to.
struct PaneBatchSettings {
    bg_capacity: usize,
    text_capacity: usize,
    skip_background_image: bool,
    fill_default_bg_cells: bool,
}

/// The pane content of one composited frame.
///
/// QA-011: this is the input [`Renderer::composite_panes`] needs, and it is all
/// an offscreen capture can supply. [`SplitPanesRenderParams`] adds the two
/// surface-only concerns — the egui overlay and its opacity override — which
/// have no meaning off-surface: `egui::FullOutput` is produced once per frame by
/// the live egui pass and consumed by value there, so a capture taken between
/// frames has none to hand over.
pub struct PaneCaptureParams<'a> {
    pub panes: &'a [PaneRenderInfo<'a>],
    pub dividers: &'a [DividerRenderInfo],
    pub pane_titles: &'a [PaneTitleInfo],
    pub focused_viewport: Option<&'a PaneViewport>,
    pub divider_settings: &'a PaneDividerSettings,
}

/// Parameters for [`Renderer::render_split_panes`].
pub struct SplitPanesRenderParams<'a> {
    pub panes: &'a [PaneRenderInfo<'a>],
    pub dividers: &'a [DividerRenderInfo],
    pub pane_titles: &'a [PaneTitleInfo],
    pub focused_viewport: Option<&'a PaneViewport>,
    pub divider_settings: &'a PaneDividerSettings,
    pub egui_data: Option<(egui::FullOutput, &'a egui::Context)>,
    pub force_egui_opaque: bool,
}

impl Renderer {
    /// Update one pane's scrollbar slot.
    ///
    /// Hit-test geometry is single-instance and follows the focused pane, so a
    /// focused pane without a scrollbar has to drop it rather than leave another
    /// pane's bounds in place.
    fn update_pane_scrollbar(&mut self, index: usize, pane: &PaneRenderInfo<'_>) {
        if pane.show_scrollbar {
            let total_lines = pane.scrollback_len + pane.grid_size.1;
            self.cell_renderer.update_scrollbar_for_pane(
                index,
                pane.scroll_offset,
                pane.grid_size.1,
                total_lines,
                &pane.marks,
                &pane.viewport,
            );
        } else if pane.viewport.focused {
            self.cell_renderer.scrollbar.clear_hit_state();
        }
    }

    /// Build every pane's instances and scrollbar slot, then draw the whole batch
    /// to `target_view` from one command encoder (ARC-004).
    fn prepare_and_draw_panes(
        &mut self,
        target_view: &wgpu::TextureView,
        panes: &[PaneRenderInfo<'_>],
        settings: PaneBatchSettings,
    ) -> Result<()> {
        self.cell_renderer
            .begin_pane_batch(settings.bg_capacity, settings.text_capacity);

        // `scratch` is declared outside the loop so its capacity is preserved
        // across iterations, avoiding a per-pane heap allocation.
        let mut scratch: Vec<SeparatorMark> = Vec::new();
        let mut prepared = Vec::with_capacity(panes.len());
        for (index, pane) in panes.iter().enumerate() {
            self.update_pane_scrollbar(index, pane);
            fill_visible_separator_marks(
                &mut scratch,
                &pane.marks,
                pane.scrollback_len,
                pane.scroll_offset,
                pane.grid_size.1,
            );
            prepared.push(self.cell_renderer.prepare_pane(
                index,
                crate::cell_renderer::PaneRenderViewParams {
                    viewport: &pane.viewport,
                    cells: pane.cells,
                    cols: pane.grid_size.0,
                    rows: pane.grid_size.1,
                    cursor_pos: pane.cursor_pos,
                    cursor_opacity: pane.cursor_opacity,
                    show_scrollbar: pane.show_scrollbar,
                    clear_first: false, // Don't clear - the target was already cleared
                    skip_background_image: settings.skip_background_image,
                    fill_default_bg_cells: settings.fill_default_bg_cells,
                    separator_marks: &scratch,
                    pane_background: pane.background.as_ref(),
                },
            )?);
        }

        self.cell_renderer
            .draw_prepared_panes(target_view, &prepared);
        Ok(())
    }

    /// Load any per-pane background textures that aren't cached yet.
    ///
    /// Kept out of [`Renderer::composite_panes`] so the live path still runs it
    /// *before* acquiring the swapchain drawable — the first frame using a pane
    /// background reads it from disk, and holding the drawable across that read
    /// would stall presentation.
    fn preload_pane_backgrounds(&mut self, panes: &[PaneRenderInfo<'_>]) {
        for pane in panes.iter() {
            if let Some(ref bg) = pane.background
                && let Some(ref path) = bg.image_path
                && let Err(e) = self.cell_renderer.load_pane_background(path)
            {
                log::error!("Failed to load pane background '{}': {}", path, e);
            }
        }
    }

    /// Composite one frame of pane content onto `final_view`.
    ///
    /// Covers the shader chain, every pane's cells and inline graphics, dividers,
    /// pane titles, the visual bell and the focus indicator, ending with the
    /// cursor-shader composite onto `final_view`.
    ///
    /// QA-011: deliberately excludes the surface-only tail (egui overlay,
    /// opaque-alpha stamp, `present`) and the `dirty` bookkeeping, so
    /// [`Renderer::take_screenshot`] can reuse it — a capture must render even
    /// when the renderer is clean, and must not consume the live path's dirty
    /// flag. Callers must run [`Renderer::preload_pane_backgrounds`] first.
    fn composite_panes(
        &mut self,
        p: PaneCaptureParams<'_>,
        final_view: &wgpu::TextureView,
    ) -> Result<()> {
        let PaneCaptureParams {
            panes,
            dividers,
            pane_titles,
            focused_viewport,
            divider_settings,
        } = p;

        let has_custom_shader = self.custom_shader_renderer.is_some();
        // Only use cursor shader if it's enabled and not disabled for alt screen
        let use_cursor_shader =
            self.cursor_shader_renderer.is_some() && !self.cursor_shader_disabled_for_alt_screen;

        // Instance-buffer capacity this frame's panes need, all resident at once.
        let (batch_bg_capacity, batch_text_capacity) =
            panes.iter().fold((0usize, 0usize), |(bg, text), pane| {
                let (pane_bg, pane_text) =
                    pane_instance_capacity(pane.grid_size.0, pane.grid_size.1);
                (bg + pane_bg, text + pane_text)
            });

        // When cursor shader is active, render all content to its intermediate texture.
        // The cursor shader will then composite the result onto `final_view`.
        let cursor_intermediate: Option<wgpu::TextureView> = if use_cursor_shader {
            Some(
                self.cursor_shader_renderer
                    .as_ref()
                    .ok_or_else(|| {
                        crate::error::RenderError::ShaderUnavailable(
                            "cursor_shader_renderer unavailable (GPU device loss?)".into(),
                        )
                    })?
                    .intermediate_texture_view()
                    .clone(),
            )
        } else {
            None
        };
        // Content render target: cursor shader intermediate (if active) or the
        // final target directly
        let content_view = cursor_intermediate.as_ref().unwrap_or(final_view);

        // Clear color for content rendering. When cursor shader will apply opacity,
        // use non-premultiplied color so opacity isn't applied twice.
        let opacity = self.cell_renderer.window_opacity as f64;
        let clear_color = if self.cell_renderer.pipelines.bg_image_bind_group.is_some() {
            wgpu::Color::TRANSPARENT
        } else if use_cursor_shader {
            // Cursor shader applies opacity — use full-opacity background
            wgpu::Color {
                r: self.cell_renderer.background_color[0] as f64,
                g: self.cell_renderer.background_color[1] as f64,
                b: self.cell_renderer.background_color[2] as f64,
                a: 1.0,
            }
        } else {
            wgpu::Color {
                r: self.cell_renderer.background_color[0] as f64 * opacity,
                g: self.cell_renderer.background_color[1] as f64 * opacity,
                b: self.cell_renderer.background_color[2] as f64 * opacity,
                a: opacity,
            }
        };

        // Determine if the shader needs terminal pixels in iChannel4.
        // Full-content mode processes terminal content directly; auto-dim uses the same
        // texture as a content mask so it can dim only beneath text/content pixels.
        let (full_content_mode, populate_terminal_intermediate_texture) = self
            .custom_shader_renderer
            .as_ref()
            .map(|s| {
                let full_content_mode = s.full_content_mode();
                (
                    full_content_mode,
                    should_populate_terminal_intermediate_texture(
                        full_content_mode,
                        s.auto_dim_under_text,
                        s.auto_dim_strength,
                    ),
                )
            })
            .unwrap_or((false, false));

        // Render pane content to the shader's intermediate texture BEFORE running the
        // shader when it needs terminal pixels via iChannel4.
        // This must happen outside the `custom_shader_renderer` mutable borrow scope
        // because rendering panes requires `&mut self`.
        if populate_terminal_intermediate_texture {
            let custom_shader = self.custom_shader_renderer.as_mut().ok_or_else(|| {
                crate::error::RenderError::ShaderUnavailable(
                    "custom_shader_renderer unavailable for iChannel4 content (GPU device loss?)"
                        .into(),
                )
            })?;
            custom_shader.clear_intermediate_texture(
                self.cell_renderer.device(),
                self.cell_renderer.queue(),
            );
            let intermediate_view = custom_shader.intermediate_texture_view().clone();

            // Render each pane's content to the intermediate texture.
            // Scrollbar geometry is updated per-pane so unfocused panes can also
            // show their own scrollbar.
            self.prepare_and_draw_panes(
                &intermediate_view,
                panes,
                PaneBatchSettings {
                    bg_capacity: batch_bg_capacity,
                    text_capacity: batch_text_capacity,
                    skip_background_image: true, // Shader handles background
                    fill_default_bg_cells: false, // Shader shows through default-bg cells
                },
            )?;

            // Render inline graphics to intermediate so shader can process them
            for pane in panes.iter() {
                if !pane.graphics.is_empty() || !pane.virtual_placements.is_empty() {
                    self.render_pane_sixel_graphics(
                        &intermediate_view,
                        &pane.viewport,
                        &pane.graphics,
                        pane.scroll_offset,
                        pane.scrollback_len,
                        pane.grid_size.1,
                        pane.cells,
                        pane.grid_size.0,
                        &pane.virtual_placements,
                    )?;
                }
            }
        }

        // If custom shader is enabled, render it to the content target
        // (the shader's render pass will handle clearing the target)
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            if !populate_terminal_intermediate_texture {
                // Background-only mode without auto-dim: clear intermediate texture
                // (shader doesn't need terminal content, panes will be rendered on top)
                custom_shader.clear_intermediate_texture(
                    self.cell_renderer.device(),
                    self.cell_renderer.queue(),
                );
            }

            // Render shader effect. When cursor shader is chained, render to cursor
            // shader's intermediate without applying opacity (cursor shader will do it).
            // When no cursor shader, render directly to surface with opacity applied.
            custom_shader.render_with_clear_color(
                self.cell_renderer.device(),
                self.cell_renderer.queue(),
                content_view,
                !use_cursor_shader, // Apply opacity only when not chaining to cursor shader
                clear_color,
            )?;
        } else {
            // No custom shader - just clear the content target with background color
            let mut encoder = self.cell_renderer.device().create_command_encoder(
                &wgpu::CommandEncoderDescriptor {
                    label: Some("split pane clear encoder"),
                },
            );

            {
                let _clear_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("surface clear pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: content_view,
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
            }

            self.cell_renderer
                .queue()
                .submit(std::iter::once(encoder.finish()));
        }

        // Render background image (full-screen, after shader but before panes)
        // Skip if custom shader is handling the background.
        // Also skip if any pane has a per-pane background configured -
        // per-pane backgrounds are rendered individually in render_pane_to_view.
        let any_pane_has_background = panes.iter().any(|p| p.background.is_some());
        let has_background_image = if !has_custom_shader && !any_pane_has_background {
            self.cell_renderer
                .render_background_only(content_view, false)?
        } else {
            false
        };

        // In full content mode, panes were already rendered to the shader's intermediate
        // texture and the shader output includes the processed terminal content.
        // Skip re-rendering panes to the content view.
        if !full_content_mode {
            // Render each pane's content (skip background image since we rendered it full-screen).
            // Scrollbar geometry is updated per-pane so unfocused panes can also
            // show their own scrollbar.
            self.prepare_and_draw_panes(
                content_view,
                panes,
                PaneBatchSettings {
                    bg_capacity: batch_bg_capacity,
                    text_capacity: batch_text_capacity,
                    skip_background_image: has_background_image || has_custom_shader,
                    // Only fill gaps in bg-image mode; shader shows through
                    fill_default_bg_cells: has_background_image,
                },
            )?;

            // Render inline graphics (Sixel/iTerm2/Kitty) for each pane, clipped to its bounds
            for pane in panes {
                if !pane.graphics.is_empty() || !pane.virtual_placements.is_empty() {
                    self.render_pane_sixel_graphics(
                        content_view,
                        &pane.viewport,
                        &pane.graphics,
                        pane.scroll_offset,
                        pane.scrollback_len,
                        pane.grid_size.1,
                        pane.cells,
                        pane.grid_size.0,
                        &pane.virtual_placements,
                    )?;
                }
            }
        }

        // Render dividers between panes
        if !dividers.is_empty() {
            self.render_dividers(content_view, dividers, divider_settings)?;
        }

        // Render pane title bars (background + text)
        if !pane_titles.is_empty() {
            self.render_pane_titles(content_view, pane_titles)?;
        }

        // Render visual bell overlay (fullscreen flash)
        if self.cell_renderer.visual_bell_intensity > 0.0 {
            let uniforms: [f32; 8] = [
                -1.0,                                     // position.x (NDC left)
                -1.0,                                     // position.y (NDC bottom)
                2.0,                                      // size.x (full width in NDC)
                2.0,                                      // size.y (full height in NDC)
                self.cell_renderer.visual_bell_color[0],  // color.r
                self.cell_renderer.visual_bell_color[1],  // color.g
                self.cell_renderer.visual_bell_color[2],  // color.b
                self.cell_renderer.visual_bell_intensity, // color.a (intensity)
            ];
            self.cell_renderer.queue().write_buffer(
                &self.cell_renderer.buffers.visual_bell_uniform_buffer,
                0,
                bytemuck::cast_slice(&uniforms),
            );

            let mut encoder = self.cell_renderer.device().create_command_encoder(
                &wgpu::CommandEncoderDescriptor {
                    label: Some("visual bell encoder"),
                },
            );
            {
                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("visual bell pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: content_view,
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
                render_pass.set_pipeline(&self.cell_renderer.pipelines.visual_bell_pipeline);
                render_pass.set_bind_group(
                    0,
                    &self.cell_renderer.pipelines.visual_bell_bind_group,
                    &[],
                );
                render_pass.draw(0..4, 0..1); // 4 vertices = triangle strip quad
            }
            self.cell_renderer
                .queue()
                .submit(std::iter::once(encoder.finish()));
        }

        // Render focus indicator around focused pane (only if multiple panes)
        if panes.len() > 1
            && let Some(viewport) = focused_viewport
        {
            self.render_focus_indicator(content_view, viewport, divider_settings)?;
        }

        // Apply cursor shader if active: composite content to the final target
        if use_cursor_shader {
            self.cursor_shader_renderer
                .as_mut()
                .ok_or_else(|| crate::error::RenderError::ShaderUnavailable(
                    "cursor_shader_renderer unavailable during final composite (GPU device loss?)".into(),
                ))?
                .render(
                    self.cell_renderer.device(),
                    self.cell_renderer.queue(),
                    final_view,
                    true, // Apply opacity - final render to the target
                )?;
        }

        Ok(())
    }

    /// Render split panes with dividers and focus indicator
    ///
    /// This is the main entry point for rendering a split pane layout.
    /// It handles:
    /// 1. Clearing the surface
    /// 2. Rendering each pane's content
    /// 3. Rendering dividers between panes
    /// 4. Rendering focus indicator around the focused pane
    /// 5. Rendering egui overlay if provided
    /// 6. Presenting the surface
    ///
    /// Steps 1–4 are shared with [`Renderer::take_screenshot`] via
    /// [`Renderer::composite_panes`]; this method adds the surface acquisition,
    /// the egui overlay, presentation and the `dirty` bookkeeping.
    ///
    /// # Arguments
    /// * `panes` - List of panes to render with their viewport info
    /// * `dividers` - List of dividers between panes with hover state
    /// * `focused_viewport` - Viewport of the focused pane (for focus indicator)
    /// * `divider_settings` - Settings for divider and focus indicator appearance
    /// * `egui_data` - Optional egui overlay data
    /// * `force_egui_opaque` - Force egui to render at full opacity
    ///
    /// # Returns
    /// `true` if rendering was performed, `false` if skipped
    pub fn render_split_panes(&mut self, params: SplitPanesRenderParams<'_>) -> Result<bool> {
        let SplitPanesRenderParams {
            panes,
            dividers,
            pane_titles,
            focused_viewport,
            divider_settings,
            egui_data,
            force_egui_opaque,
        } = params;
        // Check if we need to render
        let force_render = self.needs_continuous_render();
        if !self.dirty && !force_render && egui_data.is_none() {
            return Ok(false);
        }

        self.preload_pane_backgrounds(panes);

        // Get the surface texture
        let surface_texture = match self.cell_renderer.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            other => {
                if let Some((mut output, _)) = egui_data {
                    output.textures_delta.clear();
                }
                return Err(crate::error::RenderError::Surface(format!("{other:?}")).into());
            }
        };
        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        if let Err(error) = self.composite_panes(
            PaneCaptureParams {
                panes,
                dividers,
                pane_titles,
                focused_viewport,
                divider_settings,
            },
            &surface_view,
        ) {
            if let Some((mut output, _)) = egui_data {
                output.textures_delta.clear();
            }
            return Err(error);
        }

        // Render egui overlay if provided
        if let Some((egui_output, egui_ctx)) = egui_data {
            self.render_egui(&surface_texture, egui_output, egui_ctx, force_egui_opaque)?;
        }

        // Ensure opaque surface when window_opacity == 1.0 (skipped for transparent windows)
        self.cell_renderer.render_opaque_alpha(&surface_texture)?;

        // Present the surface
        self.cell_renderer.queue().present(surface_texture);

        self.dirty = false;
        Ok(true)
    }

    /// Composite a frame of pane content into an offscreen target (QA-011).
    ///
    /// Used by [`Renderer::take_screenshot`]; kept next to the live path so the
    /// two stay in step.
    pub(super) fn composite_panes_offscreen(
        &mut self,
        params: PaneCaptureParams<'_>,
        target_view: &wgpu::TextureView,
    ) -> Result<()> {
        self.preload_pane_backgrounds(params.panes);
        self.composite_panes(params, target_view)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn background_shader_auto_dim_requires_terminal_intermediate_texture() {
        assert!(should_populate_terminal_intermediate_texture(
            false, true, 0.35
        ));
    }

    #[test]
    fn full_content_mode_requires_terminal_intermediate_texture() {
        assert!(should_populate_terminal_intermediate_texture(
            true, false, 0.0
        ));
    }

    #[test]
    fn background_only_shader_without_auto_dim_skips_terminal_intermediate_texture() {
        assert!(!should_populate_terminal_intermediate_texture(
            false, false, 0.35
        ));
        assert!(!should_populate_terminal_intermediate_texture(
            false, true, 0.0
        ));
    }
}
