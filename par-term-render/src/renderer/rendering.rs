use crate::cell_renderer::PaneViewport;
use anyhow::Result;

use super::{
    DividerRenderInfo, PaneDividerSettings, PaneRenderInfo, PaneTitleInfo, Renderer,
    compute_visible_separator_marks,
};

impl Renderer {
    /// Render a frame with optional egui overlay
    /// Returns true if rendering was performed, false if skipped
    pub fn render(
        &mut self,
        egui_data: Option<(egui::FullOutput, &egui::Context)>,
        force_egui_opaque: bool,
        show_scrollbar: bool,
        pane_background: Option<&par_term_config::PaneBackground>,
    ) -> Result<bool> {
        // Custom shader animation forces continuous rendering
        let force_render = self.needs_continuous_render();

        // Fast path: when nothing changed, render cells from cached buffers + egui overlay
        // This skips expensive shader passes, sixel uploads, etc.
        if !self.dirty && !force_render {
            if let Some((egui_output, egui_ctx)) = egui_data {
                let surface_texture = self.cell_renderer.render(show_scrollbar, pane_background)?;
                self.cell_renderer
                    .render_overlays(&surface_texture, show_scrollbar)?;
                self.render_egui(&surface_texture, egui_output, egui_ctx, force_egui_opaque)?;
                surface_texture.present();
                return Ok(true);
            }
            return Ok(false);
        }

        // Check if shaders are enabled
        let has_custom_shader = self.custom_shader_renderer.is_some();
        // Only use cursor shader if it's enabled and not disabled for alt screen
        let use_cursor_shader =
            self.cursor_shader_renderer.is_some() && !self.cursor_shader_disabled_for_alt_screen;

        // Cell renderer renders terminal content
        let t1 = std::time::Instant::now();
        let surface_texture = if has_custom_shader {
            // When custom shader is enabled, always skip rendering background image
            // to the intermediate texture. The shader controls the background:
            // - If user wants background image in shader, enable use_background_as_channel0
            // - Otherwise, the shader's own effects provide the background
            // This prevents the background image from being treated as "terminal content"
            // and passed through unchanged by the shader.

            // Render terminal to intermediate texture for background shader
            self.cell_renderer.render_to_texture(
                self.custom_shader_renderer
                    .as_ref()
                    .expect("Custom shader renderer must be Some when use_custom_shader is true")
                    .intermediate_texture_view(),
                true, // Always skip background image - shader handles background
            )?
        } else if use_cursor_shader {
            // Render terminal to intermediate texture for cursor shader
            // Skip background image - it will be handled via iBackgroundColor uniform
            // or passed as iChannel0. This ensures proper opacity handling.
            self.cell_renderer.render_to_texture(
                self.cursor_shader_renderer
                    .as_ref()
                    .expect("Cursor shader renderer must be Some when use_cursor_shader is true")
                    .intermediate_texture_view(),
                true, // Skip background image - shader handles it
            )?
        } else {
            // Render directly to surface (no shaders, or cursor shader disabled for alt screen)
            // Note: scrollbar is rendered separately after egui so it appears on top
            self.cell_renderer.render(show_scrollbar, pane_background)?
        };
        let cell_render_time = t1.elapsed();

        // Apply background custom shader if enabled
        let t_custom = std::time::Instant::now();
        let custom_shader_time = if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            if use_cursor_shader {
                // Background shader renders to cursor shader's intermediate texture
                // Don't apply opacity here - cursor shader will apply it when rendering to surface
                custom_shader.render(
                    self.cell_renderer.device(),
                    self.cell_renderer.queue(),
                    self.cursor_shader_renderer
                        .as_ref()
                        .expect("Cursor shader renderer must be Some when use_cursor_shader is true")
                        .intermediate_texture_view(),
                    false, // Don't apply opacity - cursor shader will do it
                )?;
            } else {
                // Background shader renders directly to surface
                // (cursor shader disabled for alt screen or not configured)
                let surface_view = surface_texture
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                custom_shader.render(
                    self.cell_renderer.device(),
                    self.cell_renderer.queue(),
                    &surface_view,
                    true, // Apply opacity - this is the final render
                )?;
            }
            t_custom.elapsed()
        } else {
            std::time::Duration::ZERO
        };

        // Apply cursor shader if enabled (skip when alt screen is active for TUI apps)
        let t_cursor = std::time::Instant::now();
        let cursor_shader_time = if use_cursor_shader {
            log::trace!("Rendering cursor shader");
            let cursor_shader = self.cursor_shader_renderer.as_mut().expect("Cursor shader renderer must be Some when use_cursor_shader is true");
            let surface_view = surface_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());

            cursor_shader.render(
                self.cell_renderer.device(),
                self.cell_renderer.queue(),
                &surface_view,
                true, // Apply opacity - this is the final render to surface
            )?;
            t_cursor.elapsed()
        } else {
            if self.cursor_shader_disabled_for_alt_screen {
                log::trace!("Skipping cursor shader - alt screen active");
            }
            std::time::Duration::ZERO
        };

        // Render sixel graphics on top of cells
        let t2 = std::time::Instant::now();
        if !self.sixel_graphics.is_empty() {
            self.render_sixel_graphics(&surface_texture)?;
        }
        let sixel_render_time = t2.elapsed();

        // Render overlays (scrollbar, visual bell) BEFORE egui so that modal
        // dialogs (egui) render on top of the scrollbar. The scrollbar track
        // already accounts for status bar inset via content_inset_bottom.
        self.cell_renderer
            .render_overlays(&surface_texture, show_scrollbar)?;

        // Render egui overlay if provided
        let t3 = std::time::Instant::now();
        if let Some((egui_output, egui_ctx)) = egui_data {
            self.render_egui(&surface_texture, egui_output, egui_ctx, force_egui_opaque)?;
        }
        let egui_render_time = t3.elapsed();

        // Present the surface texture - THIS IS WHERE VSYNC WAIT HAPPENS
        let t4 = std::time::Instant::now();
        surface_texture.present();
        let present_time = t4.elapsed();

        // Log timing breakdown
        let total = cell_render_time
            + custom_shader_time
            + cursor_shader_time
            + sixel_render_time
            + egui_render_time
            + present_time;
        if present_time.as_millis() > 10 || total.as_millis() > 10 {
            log::info!(
                "[RENDER] RENDER_BREAKDOWN: CellRender={:.2}ms BgShader={:.2}ms CursorShader={:.2}ms Sixel={:.2}ms Egui={:.2}ms PRESENT={:.2}ms Total={:.2}ms",
                cell_render_time.as_secs_f64() * 1000.0,
                custom_shader_time.as_secs_f64() * 1000.0,
                cursor_shader_time.as_secs_f64() * 1000.0,
                sixel_render_time.as_secs_f64() * 1000.0,
                egui_render_time.as_secs_f64() * 1000.0,
                present_time.as_secs_f64() * 1000.0,
                total.as_secs_f64() * 1000.0
            );
        }

        // Clear dirty flag after successful render
        self.dirty = false;

        Ok(true)
    }

    /// Render multiple panes to the surface
    ///
    /// This method renders each pane's content to its viewport region,
    /// handling focus indicators and inactive pane dimming.
    ///
    /// # Arguments
    /// * `panes` - List of panes to render with their viewport info
    /// * `egui_data` - Optional egui overlay data
    /// * `force_egui_opaque` - Force egui to render at full opacity
    ///
    /// # Returns
    /// `true` if rendering was performed, `false` if skipped
    pub fn render_panes(
        &mut self,
        panes: &[PaneRenderInfo<'_>],
        egui_data: Option<(egui::FullOutput, &egui::Context)>,
        force_egui_opaque: bool,
    ) -> Result<bool> {
        // Check if we need to render
        let force_render = self.needs_continuous_render();
        if !self.dirty && !force_render && egui_data.is_none() {
            return Ok(false);
        }

        // Get the surface texture
        let surface_texture = self.cell_renderer.surface.get_current_texture()?;
        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Clear the surface first with the background color (respecting solid color mode)
        {
            let mut encoder = self.cell_renderer.device().create_command_encoder(
                &wgpu::CommandEncoderDescriptor {
                    label: Some("pane clear encoder"),
                },
            );

            let opacity = self.cell_renderer.window_opacity as f64;
            let clear_color = if self.cell_renderer.bg_state.bg_is_solid_color {
                wgpu::Color {
                    r: self.cell_renderer.bg_state.solid_bg_color[0] as f64 * opacity,
                    g: self.cell_renderer.bg_state.solid_bg_color[1] as f64 * opacity,
                    b: self.cell_renderer.bg_state.solid_bg_color[2] as f64 * opacity,
                    a: opacity,
                }
            } else {
                wgpu::Color {
                    r: self.cell_renderer.background_color[0] as f64 * opacity,
                    g: self.cell_renderer.background_color[1] as f64 * opacity,
                    b: self.cell_renderer.background_color[2] as f64 * opacity,
                    a: opacity,
                }
            };

            {
                let _clear_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("surface clear pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &surface_view,
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
            }

            self.cell_renderer
                .queue()
                .submit(std::iter::once(encoder.finish()));
        }

        // Render background image first (full-screen, before panes)
        let has_background_image = self
            .cell_renderer
            .render_background_only(&surface_view, false)?;

        // Render each pane (skip background image since we rendered it full-screen)
        for pane in panes {
            let separator_marks = compute_visible_separator_marks(
                &pane.marks,
                pane.scrollback_len,
                pane.scroll_offset,
                pane.grid_size.1,
            );
            self.cell_renderer.render_pane_to_view(
                &surface_view,
                &pane.viewport,
                pane.cells,
                pane.grid_size.0,
                pane.grid_size.1,
                pane.cursor_pos,
                pane.cursor_opacity,
                pane.show_scrollbar,
                false,                // Don't clear - we already cleared the surface
                has_background_image, // Skip background image if already rendered full-screen
                &separator_marks,
                pane.background.as_ref(),
            )?;
        }

        // Render egui overlay if provided
        if let Some((egui_output, egui_ctx)) = egui_data {
            self.render_egui(&surface_texture, egui_output, egui_ctx, force_egui_opaque)?;
        }

        // Present the surface
        surface_texture.present();

        self.dirty = false;
        Ok(true)
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
    #[allow(dead_code, clippy::too_many_arguments)]
    pub fn render_split_panes(
        &mut self,
        panes: &[PaneRenderInfo<'_>],
        dividers: &[DividerRenderInfo],
        pane_titles: &[PaneTitleInfo],
        focused_viewport: Option<&PaneViewport>,
        divider_settings: &PaneDividerSettings,
        egui_data: Option<(egui::FullOutput, &egui::Context)>,
        force_egui_opaque: bool,
    ) -> Result<bool> {
        // Check if we need to render
        let force_render = self.needs_continuous_render();
        if !self.dirty && !force_render && egui_data.is_none() {
            return Ok(false);
        }

        let has_custom_shader = self.custom_shader_renderer.is_some();

        // Pre-load any per-pane background textures that aren't cached yet
        for pane in panes.iter() {
            if let Some(ref bg) = pane.background
                && let Some(ref path) = bg.image_path
                && let Err(e) = self.cell_renderer.load_pane_background(path)
            {
                log::error!("Failed to load pane background '{}': {}", path, e);
            }
        }

        // Get the surface texture
        let surface_texture = self.cell_renderer.surface.get_current_texture()?;
        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Clear the surface with background color (respecting solid color mode)
        let opacity = self.cell_renderer.window_opacity as f64;
        let clear_color = if self.cell_renderer.bg_state.bg_is_solid_color {
            wgpu::Color {
                r: self.cell_renderer.bg_state.solid_bg_color[0] as f64 * opacity,
                g: self.cell_renderer.bg_state.solid_bg_color[1] as f64 * opacity,
                b: self.cell_renderer.bg_state.solid_bg_color[2] as f64 * opacity,
                a: opacity,
            }
        } else {
            wgpu::Color {
                r: self.cell_renderer.background_color[0] as f64 * opacity,
                g: self.cell_renderer.background_color[1] as f64 * opacity,
                b: self.cell_renderer.background_color[2] as f64 * opacity,
                a: opacity,
            }
        };

        // If custom shader is enabled, render it with the background clear color
        // (the shader's render pass will handle clearing the surface)
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            // Clear the intermediate texture to remove any old single-pane content
            // This prevents the shader from displaying stale terminal content
            custom_shader.clear_intermediate_texture(
                self.cell_renderer.device(),
                self.cell_renderer.queue(),
            );

            // Render shader effect to surface with background color as clear
            // Don't apply opacity here - pane cells will blend on top
            custom_shader.render_with_clear_color(
                self.cell_renderer.device(),
                self.cell_renderer.queue(),
                &surface_view,
                false, // Don't apply opacity - let pane rendering handle it
                clear_color,
            )?;
        } else {
            // No custom shader - just clear the surface with background color
            let mut encoder = self.cell_renderer.device().create_command_encoder(
                &wgpu::CommandEncoderDescriptor {
                    label: Some("split pane clear encoder"),
                },
            );

            {
                let _clear_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("surface clear pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &surface_view,
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
                .render_background_only(&surface_view, false)?
        } else {
            false
        };

        // Update scrollbar state for the focused pane before rendering.
        // In single-pane mode this is done in the main render loop; in split mode
        // we must do it here, constrained to the pane's pixel bounds, so the
        // track and thumb appear inside the focused pane rather than spanning
        // the full window height/width.
        for pane in panes.iter() {
            if pane.viewport.focused && pane.show_scrollbar {
                let total_lines = pane.scrollback_len + pane.grid_size.1;
                let new_state = (pane.scroll_offset, pane.grid_size.1, total_lines);
                if new_state != self.last_scrollbar_state {
                    self.last_scrollbar_state = new_state;
                    self.cell_renderer.update_scrollbar_for_pane(
                        pane.scroll_offset,
                        pane.grid_size.1,
                        total_lines,
                        &pane.marks,
                        &pane.viewport,
                    );
                }
                break;
            }
        }

        // Render each pane's content (skip background image since we rendered it full-screen)
        for pane in panes {
            let separator_marks = compute_visible_separator_marks(
                &pane.marks,
                pane.scrollback_len,
                pane.scroll_offset,
                pane.grid_size.1,
            );
            self.cell_renderer.render_pane_to_view(
                &surface_view,
                &pane.viewport,
                pane.cells,
                pane.grid_size.0,
                pane.grid_size.1,
                pane.cursor_pos,
                pane.cursor_opacity,
                pane.show_scrollbar,
                false, // Don't clear - we already cleared the surface
                has_background_image || has_custom_shader, // Skip background if already rendered
                &separator_marks,
                pane.background.as_ref(),
            )?;
        }

        // Render inline graphics (Sixel/iTerm2/Kitty) for each pane, clipped to its bounds
        for pane in panes {
            if !pane.graphics.is_empty() {
                self.render_pane_sixel_graphics(
                    &surface_view,
                    &pane.viewport,
                    &pane.graphics,
                    pane.scroll_offset,
                    pane.scrollback_len,
                    pane.grid_size.1,
                )?;
            }
        }

        // Render dividers between panes
        if !dividers.is_empty() {
            self.render_dividers(&surface_view, dividers, divider_settings)?;
        }

        // Render pane title bars (background + text)
        if !pane_titles.is_empty() {
            self.render_pane_titles(&surface_view, pane_titles)?;
        }

        // Render focus indicator around focused pane (only if multiple panes)
        if panes.len() > 1
            && let Some(viewport) = focused_viewport
        {
            self.render_focus_indicator(&surface_view, viewport, divider_settings)?;
        }

        // Render egui overlay if provided
        if let Some((egui_output, egui_ctx)) = egui_data {
            self.render_egui(&surface_texture, egui_output, egui_ctx, force_egui_opaque)?;
        }

        // Present the surface
        surface_texture.present();

        self.dirty = false;
        Ok(true)
    }

    /// Render pane dividers on top of pane content
    ///
    /// This should be called after rendering pane content but before egui.
    ///
    /// # Arguments
    /// * `surface_view` - The texture view to render to
    /// * `dividers` - List of dividers to render with hover state
    /// * `settings` - Divider appearance settings
    pub fn render_dividers(
        &mut self,
        surface_view: &wgpu::TextureView,
        dividers: &[DividerRenderInfo],
        settings: &PaneDividerSettings,
    ) -> Result<()> {
        if dividers.is_empty() {
            return Ok(());
        }

        // Build divider instances using the cell renderer's background pipeline
        // We reuse the bg_instances buffer for dividers
        let mut instances = Vec::with_capacity(dividers.len() * 3); // Extra capacity for multi-rect styles

        let w = self.size.width as f32;
        let h = self.size.height as f32;

        for divider in dividers {
            let color = if divider.hovered {
                settings.hover_color
            } else {
                settings.divider_color
            };

            use par_term_config::DividerStyle;
            match settings.divider_style {
                DividerStyle::Solid => {
                    let x_ndc = divider.x / w * 2.0 - 1.0;
                    let y_ndc = 1.0 - (divider.y / h * 2.0);
                    let w_ndc = divider.width / w * 2.0;
                    let h_ndc = divider.height / h * 2.0;

                    instances.push(crate::cell_renderer::types::BackgroundInstance {
                        position: [x_ndc, y_ndc],
                        size: [w_ndc, h_ndc],
                        color: [color[0], color[1], color[2], 1.0],
                    });
                }
                DividerStyle::Double => {
                    // Two parallel lines with a visible gap between them
                    let is_horizontal = divider.width > divider.height;
                    let thickness = if is_horizontal {
                        divider.height
                    } else {
                        divider.width
                    };

                    if thickness >= 4.0 {
                        // Enough space for two 1px lines with visible gap
                        if is_horizontal {
                            // Top line
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [divider.x / w * 2.0 - 1.0, 1.0 - (divider.y / h * 2.0)],
                                size: [divider.width / w * 2.0, 1.0 / h * 2.0],
                                color: [color[0], color[1], color[2], 1.0],
                            });
                            // Bottom line (gap in between shows background)
                            let bottom_y = divider.y + divider.height - 1.0;
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [divider.x / w * 2.0 - 1.0, 1.0 - (bottom_y / h * 2.0)],
                                size: [divider.width / w * 2.0, 1.0 / h * 2.0],
                                color: [color[0], color[1], color[2], 1.0],
                            });
                        } else {
                            // Left line
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [divider.x / w * 2.0 - 1.0, 1.0 - (divider.y / h * 2.0)],
                                size: [1.0 / w * 2.0, divider.height / h * 2.0],
                                color: [color[0], color[1], color[2], 1.0],
                            });
                            // Right line
                            let right_x = divider.x + divider.width - 1.0;
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [right_x / w * 2.0 - 1.0, 1.0 - (divider.y / h * 2.0)],
                                size: [1.0 / w * 2.0, divider.height / h * 2.0],
                                color: [color[0], color[1], color[2], 1.0],
                            });
                        }
                    } else {
                        // Divider too thin for double lines — render centered 1px line
                        // (visibly thinner than Solid to differentiate)
                        if is_horizontal {
                            let center_y = divider.y + (divider.height - 1.0) / 2.0;
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [divider.x / w * 2.0 - 1.0, 1.0 - (center_y / h * 2.0)],
                                size: [divider.width / w * 2.0, 1.0 / h * 2.0],
                                color: [color[0], color[1], color[2], 1.0],
                            });
                        } else {
                            let center_x = divider.x + (divider.width - 1.0) / 2.0;
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [center_x / w * 2.0 - 1.0, 1.0 - (divider.y / h * 2.0)],
                                size: [1.0 / w * 2.0, divider.height / h * 2.0],
                                color: [color[0], color[1], color[2], 1.0],
                            });
                        }
                    }
                }
                DividerStyle::Dashed => {
                    // Dashed line effect using segments
                    let is_horizontal = divider.width > divider.height;
                    let dash_len: f32 = 6.0;
                    let gap_len: f32 = 4.0;

                    if is_horizontal {
                        let mut x = divider.x;
                        while x < divider.x + divider.width {
                            let seg_w = dash_len.min(divider.x + divider.width - x);
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [x / w * 2.0 - 1.0, 1.0 - (divider.y / h * 2.0)],
                                size: [seg_w / w * 2.0, divider.height / h * 2.0],
                                color: [color[0], color[1], color[2], 1.0],
                            });
                            x += dash_len + gap_len;
                        }
                    } else {
                        let mut y = divider.y;
                        while y < divider.y + divider.height {
                            let seg_h = dash_len.min(divider.y + divider.height - y);
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [divider.x / w * 2.0 - 1.0, 1.0 - (y / h * 2.0)],
                                size: [divider.width / w * 2.0, seg_h / h * 2.0],
                                color: [color[0], color[1], color[2], 1.0],
                            });
                            y += dash_len + gap_len;
                        }
                    }
                }
                DividerStyle::Shadow => {
                    // Beveled/embossed effect — all rendering stays within divider bounds
                    // Highlight on top/left edge, shadow on bottom/right edge
                    let is_horizontal = divider.width > divider.height;
                    let thickness = if is_horizontal {
                        divider.height
                    } else {
                        divider.width
                    };

                    // Brighter highlight color
                    let highlight = [
                        (color[0] + 0.3).min(1.0),
                        (color[1] + 0.3).min(1.0),
                        (color[2] + 0.3).min(1.0),
                        1.0,
                    ];
                    // Darker shadow color
                    let shadow = [(color[0] * 0.3), (color[1] * 0.3), (color[2] * 0.3), 1.0];

                    if thickness >= 3.0 {
                        // 3+ px: highlight line / main body / shadow line
                        let edge = 1.0_f32;
                        if is_horizontal {
                            // Top highlight
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [divider.x / w * 2.0 - 1.0, 1.0 - (divider.y / h * 2.0)],
                                size: [divider.width / w * 2.0, edge / h * 2.0],
                                color: highlight,
                            });
                            // Main body (middle portion)
                            let body_y = divider.y + edge;
                            let body_h = divider.height - edge * 2.0;
                            if body_h > 0.0 {
                                instances.push(crate::cell_renderer::types::BackgroundInstance {
                                    position: [divider.x / w * 2.0 - 1.0, 1.0 - (body_y / h * 2.0)],
                                    size: [divider.width / w * 2.0, body_h / h * 2.0],
                                    color: [color[0], color[1], color[2], 1.0],
                                });
                            }
                            // Bottom shadow
                            let shadow_y = divider.y + divider.height - edge;
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [divider.x / w * 2.0 - 1.0, 1.0 - (shadow_y / h * 2.0)],
                                size: [divider.width / w * 2.0, edge / h * 2.0],
                                color: shadow,
                            });
                        } else {
                            // Left highlight
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [divider.x / w * 2.0 - 1.0, 1.0 - (divider.y / h * 2.0)],
                                size: [edge / w * 2.0, divider.height / h * 2.0],
                                color: highlight,
                            });
                            // Main body
                            let body_x = divider.x + edge;
                            let body_w = divider.width - edge * 2.0;
                            if body_w > 0.0 {
                                instances.push(crate::cell_renderer::types::BackgroundInstance {
                                    position: [body_x / w * 2.0 - 1.0, 1.0 - (divider.y / h * 2.0)],
                                    size: [body_w / w * 2.0, divider.height / h * 2.0],
                                    color: [color[0], color[1], color[2], 1.0],
                                });
                            }
                            // Right shadow
                            let shadow_x = divider.x + divider.width - edge;
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [shadow_x / w * 2.0 - 1.0, 1.0 - (divider.y / h * 2.0)],
                                size: [edge / w * 2.0, divider.height / h * 2.0],
                                color: shadow,
                            });
                        }
                    } else {
                        // 2px or less: top/left half highlight, bottom/right half shadow
                        if is_horizontal {
                            let half = (divider.height / 2.0).max(1.0);
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [divider.x / w * 2.0 - 1.0, 1.0 - (divider.y / h * 2.0)],
                                size: [divider.width / w * 2.0, half / h * 2.0],
                                color: highlight,
                            });
                            let bottom_y = divider.y + half;
                            let bottom_h = divider.height - half;
                            if bottom_h > 0.0 {
                                instances.push(crate::cell_renderer::types::BackgroundInstance {
                                    position: [
                                        divider.x / w * 2.0 - 1.0,
                                        1.0 - (bottom_y / h * 2.0),
                                    ],
                                    size: [divider.width / w * 2.0, bottom_h / h * 2.0],
                                    color: shadow,
                                });
                            }
                        } else {
                            let half = (divider.width / 2.0).max(1.0);
                            instances.push(crate::cell_renderer::types::BackgroundInstance {
                                position: [divider.x / w * 2.0 - 1.0, 1.0 - (divider.y / h * 2.0)],
                                size: [half / w * 2.0, divider.height / h * 2.0],
                                color: highlight,
                            });
                            let right_x = divider.x + half;
                            let right_w = divider.width - half;
                            if right_w > 0.0 {
                                instances.push(crate::cell_renderer::types::BackgroundInstance {
                                    position: [
                                        right_x / w * 2.0 - 1.0,
                                        1.0 - (divider.y / h * 2.0),
                                    ],
                                    size: [right_w / w * 2.0, divider.height / h * 2.0],
                                    color: shadow,
                                });
                            }
                        }
                    }
                }
            }
        }

        // Write instances to GPU buffer
        self.cell_renderer.queue().write_buffer(
            &self.cell_renderer.buffers.bg_instance_buffer,
            0,
            bytemuck::cast_slice(&instances),
        );

        // Render dividers
        let mut encoder =
            self.cell_renderer
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("divider render encoder"),
                });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("divider render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Don't clear - render on top
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.cell_renderer.pipelines.bg_pipeline);
            render_pass.set_vertex_buffer(0, self.cell_renderer.buffers.vertex_buffer.slice(..));
            render_pass
                .set_vertex_buffer(1, self.cell_renderer.buffers.bg_instance_buffer.slice(..));
            render_pass.draw(0..4, 0..instances.len() as u32);
        }

        self.cell_renderer
            .queue()
            .submit(std::iter::once(encoder.finish()));
        Ok(())
    }

    /// Render focus indicator around a pane
    ///
    /// This draws a colored border around the focused pane to highlight it.
    ///
    /// # Arguments
    /// * `surface_view` - The texture view to render to
    /// * `viewport` - The focused pane's viewport
    /// * `settings` - Divider/focus settings
    pub fn render_focus_indicator(
        &mut self,
        surface_view: &wgpu::TextureView,
        viewport: &PaneViewport,
        settings: &PaneDividerSettings,
    ) -> Result<()> {
        if !settings.show_focus_indicator {
            return Ok(());
        }

        let border_w = settings.focus_width;
        let color = [
            settings.focus_color[0],
            settings.focus_color[1],
            settings.focus_color[2],
            1.0,
        ];

        // Create 4 border rectangles (top, bottom, left, right)
        let instances = vec![
            // Top border
            crate::cell_renderer::types::BackgroundInstance {
                position: [
                    viewport.x / self.size.width as f32 * 2.0 - 1.0,
                    1.0 - (viewport.y / self.size.height as f32 * 2.0),
                ],
                size: [
                    viewport.width / self.size.width as f32 * 2.0,
                    border_w / self.size.height as f32 * 2.0,
                ],
                color,
            },
            // Bottom border
            crate::cell_renderer::types::BackgroundInstance {
                position: [
                    viewport.x / self.size.width as f32 * 2.0 - 1.0,
                    1.0 - ((viewport.y + viewport.height - border_w) / self.size.height as f32
                        * 2.0),
                ],
                size: [
                    viewport.width / self.size.width as f32 * 2.0,
                    border_w / self.size.height as f32 * 2.0,
                ],
                color,
            },
            // Left border (between top and bottom)
            crate::cell_renderer::types::BackgroundInstance {
                position: [
                    viewport.x / self.size.width as f32 * 2.0 - 1.0,
                    1.0 - ((viewport.y + border_w) / self.size.height as f32 * 2.0),
                ],
                size: [
                    border_w / self.size.width as f32 * 2.0,
                    (viewport.height - border_w * 2.0) / self.size.height as f32 * 2.0,
                ],
                color,
            },
            // Right border (between top and bottom)
            crate::cell_renderer::types::BackgroundInstance {
                position: [
                    (viewport.x + viewport.width - border_w) / self.size.width as f32 * 2.0 - 1.0,
                    1.0 - ((viewport.y + border_w) / self.size.height as f32 * 2.0),
                ],
                size: [
                    border_w / self.size.width as f32 * 2.0,
                    (viewport.height - border_w * 2.0) / self.size.height as f32 * 2.0,
                ],
                color,
            },
        ];

        // Write instances to GPU buffer
        self.cell_renderer.queue().write_buffer(
            &self.cell_renderer.buffers.bg_instance_buffer,
            0,
            bytemuck::cast_slice(&instances),
        );

        // Render focus indicator
        let mut encoder =
            self.cell_renderer
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("focus indicator encoder"),
                });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("focus indicator pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Don't clear - render on top
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.cell_renderer.pipelines.bg_pipeline);
            render_pass.set_vertex_buffer(0, self.cell_renderer.buffers.vertex_buffer.slice(..));
            render_pass
                .set_vertex_buffer(1, self.cell_renderer.buffers.bg_instance_buffer.slice(..));
            render_pass.draw(0..4, 0..instances.len() as u32);
        }

        self.cell_renderer
            .queue()
            .submit(std::iter::once(encoder.finish()));
        Ok(())
    }

    /// Render pane title bars (background rectangles + text)
    ///
    /// Title bars are rendered on top of pane content and dividers.
    /// Each title bar consists of a colored background rectangle and centered text.
    pub fn render_pane_titles(
        &mut self,
        surface_view: &wgpu::TextureView,
        titles: &[PaneTitleInfo],
    ) -> Result<()> {
        if titles.is_empty() {
            return Ok(());
        }

        let width = self.size.width as f32;
        let height = self.size.height as f32;

        // Phase 1: Render title bar backgrounds
        let mut bg_instances = Vec::with_capacity(titles.len());
        for title in titles {
            let x_ndc = title.x / width * 2.0 - 1.0;
            let y_ndc = 1.0 - (title.y / height * 2.0);
            let w_ndc = title.width / width * 2.0;
            let h_ndc = title.height / height * 2.0;

            // Title bar must be fully opaque (alpha=1.0) to cover the background.
            // Differentiate focused/unfocused by lightening/darkening the color.
            let brightness = if title.focused { 1.0 } else { 0.7 };

            bg_instances.push(crate::cell_renderer::types::BackgroundInstance {
                position: [x_ndc, y_ndc],
                size: [w_ndc, h_ndc],
                color: [
                    title.bg_color[0] * brightness,
                    title.bg_color[1] * brightness,
                    title.bg_color[2] * brightness,
                    1.0, // Always fully opaque
                ],
            });
        }

        // Write background instances to GPU buffer
        self.cell_renderer.queue().write_buffer(
            &self.cell_renderer.buffers.bg_instance_buffer,
            0,
            bytemuck::cast_slice(&bg_instances),
        );

        // Render title backgrounds
        let mut encoder =
            self.cell_renderer
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("pane title bg encoder"),
                });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("pane title bg pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
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

            render_pass.set_pipeline(&self.cell_renderer.pipelines.bg_pipeline);
            render_pass.set_vertex_buffer(0, self.cell_renderer.buffers.vertex_buffer.slice(..));
            render_pass
                .set_vertex_buffer(1, self.cell_renderer.buffers.bg_instance_buffer.slice(..));
            render_pass.draw(0..4, 0..bg_instances.len() as u32);
        }

        self.cell_renderer
            .queue()
            .submit(std::iter::once(encoder.finish()));

        // Phase 2: Render title text using glyph atlas
        let mut text_instances = Vec::new();
        let baseline_y = self.cell_renderer.font.font_ascent;

        for title in titles {
            let title_text = &title.title;
            if title_text.is_empty() {
                continue;
            }

            // Calculate starting X position (centered in title bar with left padding)
            let padding_x = 8.0;
            let mut x_pos = title.x + padding_x;
            let y_base = title.y + (title.height - self.cell_renderer.grid.cell_height) / 2.0;

            let text_color = [
                title.text_color[0],
                title.text_color[1],
                title.text_color[2],
                if title.focused { 1.0 } else { 0.8 },
            ];

            // Truncate title if it would overflow the title bar
            let max_chars =
                ((title.width - padding_x * 2.0) / self.cell_renderer.grid.cell_width) as usize;
            let display_text: String = if title_text.len() > max_chars && max_chars > 3 {
                let truncated: String = title_text.chars().take(max_chars - 1).collect();
                format!("{}\u{2026}", truncated) // ellipsis
            } else {
                title_text.clone()
            };

            for ch in display_text.chars() {
                if x_pos >= title.x + title.width - padding_x {
                    break;
                }

                if let Some((font_idx, glyph_id)) =
                    self.cell_renderer.font_manager.find_glyph(ch, false, false)
                {
                    let cache_key = ((font_idx as u64) << 32) | (glyph_id as u64);
                    // Check if this character should be rendered as a monochrome symbol
                    let force_monochrome = crate::cell_renderer::atlas::should_render_as_symbol(ch);
                    let info = if self
                        .cell_renderer
                        .atlas
                        .glyph_cache
                        .contains_key(&cache_key)
                    {
                        self.cell_renderer.lru_remove(cache_key);
                        self.cell_renderer.lru_push_front(cache_key);
                        self.cell_renderer
                            .atlas
                            .glyph_cache
                            .get(&cache_key)
                            .expect("Glyph cache entry must exist after contains_key check")
                            .clone()
                    } else if let Some(raster) =
                        self.cell_renderer
                            .rasterize_glyph(font_idx, glyph_id, force_monochrome)
                    {
                        let info = self.cell_renderer.upload_glyph(cache_key, &raster);
                        self.cell_renderer
                            .atlas
                            .glyph_cache
                            .insert(cache_key, info.clone());
                        self.cell_renderer.lru_push_front(cache_key);
                        info
                    } else {
                        x_pos += self.cell_renderer.grid.cell_width;
                        continue;
                    };

                    let glyph_left = x_pos + info.bearing_x;
                    let glyph_top = y_base + (baseline_y - info.bearing_y);

                    text_instances.push(crate::cell_renderer::types::TextInstance {
                        position: [
                            glyph_left / width * 2.0 - 1.0,
                            1.0 - (glyph_top / height * 2.0),
                        ],
                        size: [
                            info.width as f32 / width * 2.0,
                            info.height as f32 / height * 2.0,
                        ],
                        tex_offset: [info.x as f32 / 2048.0, info.y as f32 / 2048.0],
                        tex_size: [info.width as f32 / 2048.0, info.height as f32 / 2048.0],
                        color: text_color,
                        is_colored: if info.is_colored { 1 } else { 0 },
                    });
                }

                x_pos += self.cell_renderer.grid.cell_width;
            }
        }

        if text_instances.is_empty() {
            return Ok(());
        }

        // Write text instances to GPU buffer
        self.cell_renderer.queue().write_buffer(
            &self.cell_renderer.buffers.text_instance_buffer,
            0,
            bytemuck::cast_slice(&text_instances),
        );

        // Render title text
        let mut encoder =
            self.cell_renderer
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("pane title text encoder"),
                });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("pane title text pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: surface_view,
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

            render_pass.set_pipeline(&self.cell_renderer.pipelines.text_pipeline);
            render_pass.set_bind_group(0, &self.cell_renderer.pipelines.text_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.cell_renderer.buffers.vertex_buffer.slice(..));
            render_pass
                .set_vertex_buffer(1, self.cell_renderer.buffers.text_instance_buffer.slice(..));
            render_pass.draw(0..4, 0..text_instances.len() as u32);
        }

        self.cell_renderer
            .queue()
            .submit(std::iter::once(encoder.finish()));

        Ok(())
    }

    /// Render egui overlay on top of the terminal
    fn render_egui(
        &mut self,
        surface_texture: &wgpu::SurfaceTexture,
        egui_output: egui::FullOutput,
        egui_ctx: &egui::Context,
        force_opaque: bool,
    ) -> Result<()> {
        use wgpu::TextureViewDescriptor;

        // Create view of the surface texture
        let view = surface_texture
            .texture
            .create_view(&TextureViewDescriptor::default());

        // Create command encoder for egui
        let mut encoder =
            self.cell_renderer
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("egui encoder"),
                });

        // Convert egui output to screen descriptor
        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.size.width, self.size.height],
            pixels_per_point: egui_output.pixels_per_point,
        };

        // Update egui textures
        for (id, image_delta) in &egui_output.textures_delta.set {
            self.egui_renderer.update_texture(
                self.cell_renderer.device(),
                self.cell_renderer.queue(),
                *id,
                image_delta,
            );
        }

        // Tessellate egui shapes into paint jobs
        let mut paint_jobs = egui_ctx.tessellate(egui_output.shapes, egui_output.pixels_per_point);

        // If requested, force all egui vertices to full opacity so UI stays solid
        if force_opaque {
            for job in paint_jobs.iter_mut() {
                match &mut job.primitive {
                    egui::epaint::Primitive::Mesh(mesh) => {
                        for v in mesh.vertices.iter_mut() {
                            v.color[3] = 255;
                        }
                    }
                    egui::epaint::Primitive::Callback(_) => {}
                }
            }
        }

        // Update egui buffers
        self.egui_renderer.update_buffers(
            self.cell_renderer.device(),
            self.cell_renderer.queue(),
            &mut encoder,
            &paint_jobs,
            &screen_descriptor,
        );

        // Render egui on top of the terminal content
        {
            let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Don't clear - render on top of terminal
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Convert to 'static lifetime as required by egui_renderer.render()
            let mut render_pass = render_pass.forget_lifetime();

            self.egui_renderer
                .render(&mut render_pass, &paint_jobs, &screen_descriptor);
        } // render_pass dropped here

        // Submit egui commands
        self.cell_renderer
            .queue()
            .submit(std::iter::once(encoder.finish()));

        // Free egui textures
        for id in &egui_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }

        Ok(())
    }
}
