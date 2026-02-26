use anyhow::Result;
use winit::dpi::PhysicalSize;

use super::Renderer;

impl Renderer {
    /// Get the current size
    pub fn size(&self) -> PhysicalSize<u32> {
        self.size
    }

    /// Get the current grid dimensions (columns, rows)
    pub fn grid_size(&self) -> (usize, usize) {
        self.cell_renderer.grid_size()
    }

    /// Get cell width in pixels
    pub fn cell_width(&self) -> f32 {
        self.cell_renderer.cell_width()
    }

    /// Get cell height in pixels
    pub fn cell_height(&self) -> f32 {
        self.cell_renderer.cell_height()
    }

    /// Get window padding in physical pixels (scaled by DPI)
    pub fn window_padding(&self) -> f32 {
        self.cell_renderer.window_padding()
    }

    /// Get the vertical content offset in physical pixels (e.g., tab bar height scaled by DPI)
    pub fn content_offset_y(&self) -> f32 {
        self.cell_renderer.content_offset_y()
    }

    /// Get the display scale factor (e.g., 2.0 on Retina displays)
    pub fn scale_factor(&self) -> f32 {
        self.cell_renderer.scale_factor
    }

    /// Set the vertical content offset (e.g., tab bar height) in logical pixels.
    /// The offset is scaled by the display scale factor to physical pixels internally,
    /// since the cell renderer works in physical pixel coordinates while egui (tab bar)
    /// uses logical pixels.
    /// Returns Some((cols, rows)) if grid size changed, None otherwise.
    pub fn set_content_offset_y(&mut self, logical_offset: f32) -> Option<(usize, usize)> {
        // Scale from logical pixels (egui/config) to physical pixels (wgpu surface)
        let physical_offset = logical_offset * self.cell_renderer.scale_factor;
        let result = self.cell_renderer.set_content_offset_y(physical_offset);
        // Always update graphics renderer offset, even if grid size didn't change
        self.graphics_renderer.set_content_offset_y(physical_offset);
        // Update custom shader renderer content offset
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.set_content_offset_y(physical_offset);
        }
        // Update cursor shader renderer content offset
        if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            cursor_shader.set_content_offset_y(physical_offset);
        }
        if result.is_some() {
            self.dirty = true;
        }
        result
    }

    /// Get the horizontal content offset in physical pixels
    pub fn content_offset_x(&self) -> f32 {
        self.cell_renderer.content_offset_x()
    }

    /// Set the horizontal content offset (e.g., tab bar on left) in logical pixels.
    /// Returns Some((cols, rows)) if grid size changed, None otherwise.
    pub fn set_content_offset_x(&mut self, logical_offset: f32) -> Option<(usize, usize)> {
        let physical_offset = logical_offset * self.cell_renderer.scale_factor;
        let result = self.cell_renderer.set_content_offset_x(physical_offset);
        self.graphics_renderer.set_content_offset_x(physical_offset);
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.set_content_offset_x(physical_offset);
        }
        if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            cursor_shader.set_content_offset_x(physical_offset);
        }
        if result.is_some() {
            self.dirty = true;
        }
        result
    }

    /// Get the bottom content inset in physical pixels
    pub fn content_inset_bottom(&self) -> f32 {
        self.cell_renderer.content_inset_bottom()
    }

    /// Get the right content inset in physical pixels
    pub fn content_inset_right(&self) -> f32 {
        self.cell_renderer.content_inset_right()
    }

    /// Set the bottom content inset (e.g., tab bar at bottom) in logical pixels.
    /// Returns Some((cols, rows)) if grid size changed, None otherwise.
    pub fn set_content_inset_bottom(&mut self, logical_inset: f32) -> Option<(usize, usize)> {
        let physical_inset = logical_inset * self.cell_renderer.scale_factor;
        let result = self.cell_renderer.set_content_inset_bottom(physical_inset);
        if result.is_some() {
            self.dirty = true;
            // Invalidate the scrollbar cache — the track height depends on
            // the bottom inset, so the scrollbar must be repositioned.
            self.last_scrollbar_state = (usize::MAX, 0, 0);
        }
        result
    }

    /// Set the right content inset (e.g., AI Inspector panel) in logical pixels.
    /// Returns Some((cols, rows)) if grid size changed, None otherwise.
    pub fn set_content_inset_right(&mut self, logical_inset: f32) -> Option<(usize, usize)> {
        let physical_inset = logical_inset * self.cell_renderer.scale_factor;
        let result = self.cell_renderer.set_content_inset_right(physical_inset);

        // Also update custom shader renderer to exclude panel area from effects
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.set_content_inset_right(physical_inset);
        }
        // Also update cursor shader renderer
        if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            cursor_shader.set_content_inset_right(physical_inset);
        }

        if result.is_some() {
            self.dirty = true;
            // Invalidate the scrollbar cache so the next update_scrollbar()
            // repositions the scrollbar at the new right inset. Without this,
            // the cache guard sees the same (scroll_offset, visible_lines,
            // total_lines) tuple and skips the GPU upload, leaving the
            // scrollbar stuck at the old position.
            self.last_scrollbar_state = (usize::MAX, 0, 0);
        }
        result
    }

    /// Set the additional bottom inset from egui panels (status bar, tmux bar).
    ///
    /// This inset reduces the terminal grid height so content does not render
    /// behind the status bar. Also affects scrollbar bounds.
    /// Returns `Some((cols, rows))` if the grid was resized.
    pub fn set_egui_bottom_inset(&mut self, logical_inset: f32) -> Option<(usize, usize)> {
        let physical_inset = logical_inset * self.cell_renderer.scale_factor;
        if (self.cell_renderer.grid.egui_bottom_inset - physical_inset).abs() > f32::EPSILON {
            self.cell_renderer.grid.egui_bottom_inset = physical_inset;
            let (w, h) = (
                self.cell_renderer.config.width,
                self.cell_renderer.config.height,
            );
            return Some(self.cell_renderer.resize(w, h));
        }
        None
    }

    /// Set the additional right inset from egui panels (AI Inspector).
    ///
    /// This inset is added to `content_inset_right` for scrollbar bounds only.
    /// egui panels already claim space before wgpu rendering, so this doesn't
    /// affect the terminal grid sizing.
    pub fn set_egui_right_inset(&mut self, logical_inset: f32) {
        let physical_inset = logical_inset * self.cell_renderer.scale_factor;
        self.cell_renderer.grid.egui_right_inset = physical_inset;
    }

    /// Check if a point (in pixel coordinates) is within the scrollbar bounds
    ///
    /// # Arguments
    /// * `x` - X coordinate in pixels (from left edge)
    /// * `y` - Y coordinate in pixels (from top edge)
    pub fn scrollbar_contains_point(&self, x: f32, y: f32) -> bool {
        self.cell_renderer.scrollbar_contains_point(x, y)
    }

    /// Get the scrollbar thumb bounds (top Y, height) in pixels
    pub fn scrollbar_thumb_bounds(&self) -> Option<(f32, f32)> {
        self.cell_renderer.scrollbar_thumb_bounds()
    }

    /// Check if an X coordinate is within the scrollbar track
    pub fn scrollbar_track_contains_x(&self, x: f32) -> bool {
        self.cell_renderer.scrollbar_track_contains_x(x)
    }

    /// Convert a mouse Y position to a scroll offset
    ///
    /// # Arguments
    /// * `mouse_y` - Mouse Y coordinate in pixels (from top edge)
    ///
    /// # Returns
    /// The scroll offset corresponding to the mouse position, or None if scrollbar is not visible
    pub fn scrollbar_mouse_y_to_scroll_offset(&self, mouse_y: f32) -> Option<usize> {
        self.cell_renderer
            .scrollbar_mouse_y_to_scroll_offset(mouse_y)
    }

    /// Find a scrollbar mark at the given mouse position for tooltip display.
    ///
    /// # Arguments
    /// * `mouse_x` - Mouse X coordinate in pixels
    /// * `mouse_y` - Mouse Y coordinate in pixels
    /// * `tolerance` - Maximum distance in pixels to match a mark
    ///
    /// # Returns
    /// The mark at that position, or None if no mark is within tolerance
    pub fn scrollbar_mark_at_position(
        &self,
        mouse_x: f32,
        mouse_y: f32,
        tolerance: f32,
    ) -> Option<&par_term_config::ScrollbackMark> {
        self.cell_renderer
            .scrollbar_mark_at_position(mouse_x, mouse_y, tolerance)
    }

    /// Check if the renderer needs to be redrawn
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Mark the renderer as dirty, forcing a redraw on next render call
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Set debug overlay text to be rendered
    pub fn render_debug_overlay(&mut self, text: &str) {
        self.debug_text = Some(text.to_string());
        self.dirty = true; // Mark dirty to ensure debug overlay renders
    }

    /// Reconfigure the surface (call when surface becomes outdated or lost)
    /// This typically happens when dragging the window between displays
    pub fn reconfigure_surface(&mut self) {
        self.cell_renderer.reconfigure_surface();
        self.dirty = true;
    }

    /// Check if a vsync mode is supported
    pub fn is_vsync_mode_supported(&self, mode: par_term_config::VsyncMode) -> bool {
        self.cell_renderer.is_vsync_mode_supported(mode)
    }

    /// Update the vsync mode. Returns the actual mode applied (may differ if requested mode unsupported).
    /// Also returns whether the mode was changed.
    pub fn update_vsync_mode(
        &mut self,
        mode: par_term_config::VsyncMode,
    ) -> (par_term_config::VsyncMode, bool) {
        let result = self.cell_renderer.update_vsync_mode(mode);
        if result.1 {
            self.dirty = true;
        }
        result
    }

    /// Get the current vsync mode
    pub fn current_vsync_mode(&self) -> par_term_config::VsyncMode {
        self.cell_renderer.current_vsync_mode()
    }

    /// Clear the glyph cache to force re-rasterization
    /// Useful after display changes where font rendering may differ
    pub fn clear_glyph_cache(&mut self) {
        self.cell_renderer.clear_glyph_cache();
        self.dirty = true;
    }

    /// Update font anti-aliasing setting
    /// Returns true if the setting changed (requiring glyph cache clear)
    pub fn update_font_antialias(&mut self, enabled: bool) -> bool {
        let changed = self.cell_renderer.update_font_antialias(enabled);
        if changed {
            self.dirty = true;
        }
        changed
    }

    /// Update font hinting setting
    /// Returns true if the setting changed (requiring glyph cache clear)
    pub fn update_font_hinting(&mut self, enabled: bool) -> bool {
        let changed = self.cell_renderer.update_font_hinting(enabled);
        if changed {
            self.dirty = true;
        }
        changed
    }

    /// Update thin strokes mode
    /// Returns true if the setting changed (requiring glyph cache clear)
    pub fn update_font_thin_strokes(&mut self, mode: par_term_config::ThinStrokesMode) -> bool {
        let changed = self.cell_renderer.update_font_thin_strokes(mode);
        if changed {
            self.dirty = true;
        }
        changed
    }

    /// Update minimum contrast ratio
    /// Returns true if the setting changed (requiring redraw)
    pub fn update_minimum_contrast(&mut self, ratio: f32) -> bool {
        let changed = self.cell_renderer.update_minimum_contrast(ratio);
        if changed {
            self.dirty = true;
        }
        changed
    }

    /// Pause shader animations (e.g., when window loses focus)
    /// This reduces GPU usage when the terminal is not actively being viewed
    pub fn pause_shader_animations(&mut self) {
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.set_animation_enabled(false);
        }
        if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            cursor_shader.set_animation_enabled(false);
        }
        log::info!("[SHADER] Shader animations paused");
    }

    /// Resume shader animations (e.g., when window regains focus)
    /// Only resumes if the user's config has animation enabled
    pub fn resume_shader_animations(
        &mut self,
        custom_shader_animation: bool,
        cursor_shader_animation: bool,
    ) {
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.set_animation_enabled(custom_shader_animation);
        }
        if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            cursor_shader.set_animation_enabled(cursor_shader_animation);
        }
        self.dirty = true;
        log::info!(
            "[SHADER] Shader animations resumed (custom: {}, cursor: {})",
            custom_shader_animation,
            cursor_shader_animation
        );
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
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
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
