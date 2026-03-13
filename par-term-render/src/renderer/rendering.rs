// ARC-009 TODO: This file is 705 lines (limit: 800 — approaching threshold). When it
// exceeds 800 lines, extract into renderer/ siblings:
//
//   split_layout.rs  — Split-pane geometry calculations (render_split_panes_with_data)
//   separator_draw.rs — compute_visible_separator_marks + draw calls (see also QA-001,
//                       QA-008 which affect this area)
//
// Tracking: Issue ARC-009 in AUDIT.md.

use crate::cell_renderer::PaneViewport;
use anyhow::Result;

use super::{
    DividerRenderInfo, PaneDividerSettings, PaneRenderInfo, PaneTitleInfo, Renderer, SeparatorMark,
    fill_visible_separator_marks,
};

// This file contains the multi-pane frame-level helper `render_split_panes` and `take_screenshot`.

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

        let has_custom_shader = self.custom_shader_renderer.is_some();
        // Only use cursor shader if it's enabled and not disabled for alt screen
        let use_cursor_shader =
            self.cursor_shader_renderer.is_some() && !self.cursor_shader_disabled_for_alt_screen;

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

        // When cursor shader is active, render all content to its intermediate texture.
        // The cursor shader will then composite the result onto the surface.
        let cursor_intermediate: Option<wgpu::TextureView> = if use_cursor_shader {
            Some(
                self.cursor_shader_renderer
                    .as_ref()
                    .expect("cursor_shader_renderer must be Some when use_cursor_shader is true")
                    .intermediate_texture_view()
                    .clone(),
            )
        } else {
            None
        };
        // Content render target: cursor shader intermediate (if active) or surface directly
        let content_view = cursor_intermediate.as_ref().unwrap_or(&surface_view);

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

        // Determine if custom shader uses full content mode (shader processes terminal content)
        let full_content_mode = self
            .custom_shader_renderer
            .as_ref()
            .is_some_and(|s| s.full_content_mode());

        // Full content mode: render pane content to the shader's intermediate texture
        // BEFORE running the shader, so it can process terminal content via iChannel4.
        // This must happen outside the `custom_shader_renderer` mutable borrow scope
        // because rendering panes requires `&mut self`.
        if full_content_mode {
            let custom_shader = self.custom_shader_renderer.as_mut()
                .expect("custom_shader_renderer must be Some when full_content_mode is true");
            custom_shader.clear_intermediate_texture(
                self.cell_renderer.device(),
                self.cell_renderer.queue(),
            );
            let intermediate_view = custom_shader.intermediate_texture_view().clone();

            // Update scrollbar state before rendering panes to intermediate
            for pane in panes.iter() {
                if pane.viewport.focused && pane.show_scrollbar {
                    let total_lines = pane.scrollback_len + pane.grid_size.1;
                    let new_state = (
                        pane.scroll_offset,
                        pane.grid_size.1,
                        total_lines,
                        pane.marks.len(),
                        self.cell_renderer.config.width,
                        self.cell_renderer.config.height,
                        pane.viewport.x.to_bits(),
                        pane.viewport.y.to_bits(),
                        pane.viewport.width.to_bits(),
                        pane.viewport.height.to_bits(),
                    );
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

            // Render each pane's content to the intermediate texture.
            // `scratch` is declared outside the loop so its capacity is preserved
            // across iterations, avoiding a per-pane heap allocation.
            let mut scratch: Vec<SeparatorMark> = Vec::new();
            for pane in panes.iter() {
                fill_visible_separator_marks(
                    &mut scratch,
                    &pane.marks,
                    pane.scrollback_len,
                    pane.scroll_offset,
                    pane.grid_size.1,
                );
                self.cell_renderer.render_pane_to_view(
                    &intermediate_view,
                    crate::cell_renderer::PaneRenderViewParams {
                        viewport: &pane.viewport,
                        cells: pane.cells,
                        cols: pane.grid_size.0,
                        rows: pane.grid_size.1,
                        cursor_pos: pane.cursor_pos,
                        cursor_opacity: pane.cursor_opacity,
                        show_scrollbar: pane.show_scrollbar,
                        clear_first: false,
                        skip_background_image: true, // Shader handles background
                        fill_default_bg_cells: false, // Shader shows through default-bg cells
                        separator_marks: &scratch,
                        pane_background: pane.background.as_ref(),
                    },
                )?;
            }

            // Render inline graphics to intermediate so shader can process them
            for pane in panes.iter() {
                if !pane.graphics.is_empty() {
                    self.render_pane_sixel_graphics(
                        &intermediate_view,
                        &pane.viewport,
                        &pane.graphics,
                        pane.scroll_offset,
                        pane.scrollback_len,
                        pane.grid_size.1,
                    )?;
                }
            }
        }

        // If custom shader is enabled, render it to the content target
        // (the shader's render pass will handle clearing the target)
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            if !full_content_mode {
                // Background-only mode: clear intermediate texture (shader doesn't
                // need terminal content, panes will be rendered on top)
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
            // Update scrollbar state for the focused pane before rendering.
            // In single-pane mode this is done in the main render loop; in split mode
            // we must do it here, constrained to the pane's pixel bounds, so the
            // track and thumb appear inside the focused pane rather than spanning
            // the full window height/width.
            for pane in panes.iter() {
                if pane.viewport.focused && pane.show_scrollbar {
                    let total_lines = pane.scrollback_len + pane.grid_size.1;
                    let new_state = (
                        pane.scroll_offset,
                        pane.grid_size.1,
                        total_lines,
                        pane.marks.len(),
                        self.cell_renderer.config.width,
                        self.cell_renderer.config.height,
                        // Include pane viewport bounds so splits/resizes trigger
                        // a scrollbar geometry update (viewport changes position/size).
                        pane.viewport.x.to_bits(),
                        pane.viewport.y.to_bits(),
                        pane.viewport.width.to_bits(),
                        pane.viewport.height.to_bits(),
                    );
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

            // Render each pane's content (skip background image since we rendered it full-screen).
            // `scratch` is declared outside the loop so its capacity is preserved
            // across iterations, avoiding a per-pane heap allocation.
            let mut scratch: Vec<SeparatorMark> = Vec::new();
            for pane in panes {
                fill_visible_separator_marks(
                    &mut scratch,
                    &pane.marks,
                    pane.scrollback_len,
                    pane.scroll_offset,
                    pane.grid_size.1,
                );
                self.cell_renderer.render_pane_to_view(
                    content_view,
                    crate::cell_renderer::PaneRenderViewParams {
                        viewport: &pane.viewport,
                        cells: pane.cells,
                        cols: pane.grid_size.0,
                        rows: pane.grid_size.1,
                        cursor_pos: pane.cursor_pos,
                        cursor_opacity: pane.cursor_opacity,
                        show_scrollbar: pane.show_scrollbar,
                        clear_first: false, // Don't clear - we already cleared the surface
                        skip_background_image: has_background_image || has_custom_shader,
                        fill_default_bg_cells: has_background_image, // Only fill gaps in bg-image mode; shader shows through
                        separator_marks: &scratch,
                        pane_background: pane.background.as_ref(),
                    },
                )?;
            }

            // Render inline graphics (Sixel/iTerm2/Kitty) for each pane, clipped to its bounds
            for pane in panes {
                if !pane.graphics.is_empty() {
                    self.render_pane_sixel_graphics(
                        content_view,
                        &pane.viewport,
                        &pane.graphics,
                        pane.scroll_offset,
                        pane.scrollback_len,
                        pane.grid_size.1,
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

        // Apply cursor shader if active: composite content to surface
        if use_cursor_shader {
            self.cursor_shader_renderer
                .as_mut()
                .expect("cursor_shader_renderer must be Some when use_cursor_shader is true")
                .render(
                self.cell_renderer.device(),
                self.cell_renderer.queue(),
                &surface_view,
                true, // Apply opacity - final render to surface
            )?;
        }

        // Render egui overlay if provided
        if let Some((egui_output, egui_ctx)) = egui_data {
            self.render_egui(&surface_texture, egui_output, egui_ctx, force_egui_opaque)?;
        }

        // Ensure opaque surface when window_opacity == 1.0 (skipped for transparent windows)
        self.cell_renderer.render_opaque_alpha(&surface_texture)?;

        // Present the surface
        surface_texture.present();

        self.dirty = false;
        Ok(true)
    }

    /// Take a screenshot of the current terminal content
    /// Returns an RGBA image that can be saved to disk
    ///
    /// This captures the fully composited output including shader effects.
    pub fn take_screenshot(&mut self) -> Result<image::RgbaImage, crate::error::RenderError> {
        log::info!(
            "take_screenshot: Starting screenshot capture ({}x{})",
            self.size.width,
            self.size.height
        );

        let width = self.size.width;
        let height = self.size.height;
        // Use the same format as the surface to match pipeline expectations
        let format = self.cell_renderer.surface_format();
        log::info!("take_screenshot: Using texture format {:?}", format);

        // Create a texture to render the final composited output to (with COPY_SRC for reading back)
        let screenshot_texture =
            self.cell_renderer
                .device()
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("screenshot texture"),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                });

        let screenshot_view =
            screenshot_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Render the full composited frame (cells + shaders + overlays)
        log::info!("take_screenshot: Rendering composited frame...");

        // Check if shaders are enabled
        let has_custom_shader = self.custom_shader_renderer.is_some();
        let use_cursor_shader =
            self.cursor_shader_renderer.is_some() && !self.cursor_shader_disabled_for_alt_screen;

        if has_custom_shader {
            // Render cells to the custom shader's intermediate texture
            let intermediate_view = self
                .custom_shader_renderer
                .as_ref()
                .expect("Custom shader renderer must be Some when has_custom_shader is true")
                .intermediate_texture_view()
                .clone();
            self.cell_renderer
                .render_to_texture(&intermediate_view, true)
                .map_err(|e| {
                    crate::error::RenderError::ScreenshotMap(format!("Render failed: {:#}", e))
                })?;

            if use_cursor_shader {
                // Background shader renders to cursor shader's intermediate texture
                let cursor_intermediate = self
                    .cursor_shader_renderer
                    .as_ref()
                    .expect("Cursor shader renderer must be Some when use_cursor_shader is true")
                    .intermediate_texture_view()
                    .clone();
                self.custom_shader_renderer
                    .as_mut()
                    .expect("Custom shader renderer must be Some when has_custom_shader is true")
                    .render(
                        self.cell_renderer.device(),
                        self.cell_renderer.queue(),
                        &cursor_intermediate,
                        false,
                    )
                    .map_err(|e| {
                        crate::error::RenderError::ScreenshotMap(format!("Render failed: {:#}", e))
                    })?;
                // Cursor shader renders to screenshot texture
                self.cursor_shader_renderer
                    .as_mut()
                    .expect("Cursor shader renderer must be Some when use_cursor_shader is true")
                    .render(
                        self.cell_renderer.device(),
                        self.cell_renderer.queue(),
                        &screenshot_view,
                        true,
                    )
                    .map_err(|e| {
                        crate::error::RenderError::ScreenshotMap(format!("Render failed: {:#}", e))
                    })?;
            } else {
                // Background shader renders directly to screenshot texture
                self.custom_shader_renderer
                    .as_mut()
                    .expect("Custom shader renderer must be Some when has_custom_shader is true")
                    .render(
                        self.cell_renderer.device(),
                        self.cell_renderer.queue(),
                        &screenshot_view,
                        true,
                    )
                    .map_err(|e| {
                        crate::error::RenderError::ScreenshotMap(format!("Render failed: {:#}", e))
                    })?;
            }
        } else if use_cursor_shader {
            // Render cells to cursor shader's intermediate texture
            let cursor_intermediate = self
                .cursor_shader_renderer
                .as_ref()
                .expect("Cursor shader renderer must be Some when use_cursor_shader is true")
                .intermediate_texture_view()
                .clone();
            self.cell_renderer
                .render_to_texture(&cursor_intermediate, true)
                .map_err(|e| {
                    crate::error::RenderError::ScreenshotMap(format!("Render failed: {:#}", e))
                })?;
            // Cursor shader renders to screenshot texture
            self.cursor_shader_renderer
                .as_mut()
                .expect("Cursor shader renderer must be Some when use_cursor_shader is true")
                .render(
                    self.cell_renderer.device(),
                    self.cell_renderer.queue(),
                    &screenshot_view,
                    true,
                )
                .map_err(|e| {
                    crate::error::RenderError::ScreenshotMap(format!("Render failed: {:#}", e))
                })?;
        } else {
            // No shaders - render directly to screenshot texture
            self.cell_renderer
                .render_to_view(&screenshot_view)
                .map_err(|e| {
                    crate::error::RenderError::ScreenshotMap(format!("Render failed: {:#}", e))
                })?;
        }

        log::info!("take_screenshot: Render complete");

        // Get device and queue references for buffer operations
        let device = self.cell_renderer.device();
        let queue = self.cell_renderer.queue();

        // Create buffer for reading back the texture
        let bytes_per_pixel = 4u32;
        let unpadded_bytes_per_row = width * bytes_per_pixel;
        // wgpu requires rows to be aligned to 256 bytes
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = unpadded_bytes_per_row.div_ceil(align) * align;
        let buffer_size = (padded_bytes_per_row * height) as u64;

        let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("screenshot buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // Copy texture to buffer
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("screenshot encoder"),
        });

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &screenshot_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &output_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        queue.submit(std::iter::once(encoder.finish()));
        log::info!("take_screenshot: Texture copy submitted");

        // Map the buffer and read the data
        let buffer_slice = output_buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });

        // Wait for GPU to finish
        log::info!("take_screenshot: Waiting for GPU...");
        if let Err(e) = device.poll(wgpu::PollType::wait_indefinitely()) {
            log::warn!("take_screenshot: GPU poll returned error: {:?}", e);
        }
        log::info!("take_screenshot: GPU poll complete, waiting for buffer map...");
        rx.recv()
            .map_err(|e| {
                crate::error::RenderError::ScreenshotMap(format!(
                    "Failed to receive map result: {}",
                    e
                ))
            })?
            .map_err(|e| {
                crate::error::RenderError::ScreenshotMap(format!("Failed to map buffer: {:?}", e))
            })?;
        log::info!("take_screenshot: Buffer mapped successfully");

        // Read the data
        let data = buffer_slice.get_mapped_range();
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);

        // Check if format is BGRA (needs swizzle) or RGBA (direct copy)
        let is_bgra = matches!(
            format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        );

        // Copy data row by row (to handle padding)
        for y in 0..height {
            let row_start = (y * padded_bytes_per_row) as usize;
            let row_end = row_start + (width * bytes_per_pixel) as usize;
            let row = &data[row_start..row_end];

            if is_bgra {
                // Convert BGRA to RGBA
                for chunk in row.chunks(4) {
                    pixels.push(chunk[2]); // R (was B)
                    pixels.push(chunk[1]); // G
                    pixels.push(chunk[0]); // B (was R)
                    pixels.push(chunk[3]); // A
                }
            } else {
                // Already RGBA, direct copy
                pixels.extend_from_slice(row);
            }
        }

        drop(data);
        output_buffer.unmap();

        // Create image
        image::RgbaImage::from_raw(width, height, pixels)
            .ok_or(crate::error::RenderError::ScreenshotImageAssembly)
    }
}
