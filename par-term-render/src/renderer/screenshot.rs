//! Offscreen frame capture.
//!
//! `take_screenshot` renders a composited frame into a `COPY_SRC` texture and reads
//! it back as an `RgbaImage`. `render_cells_to_target` encapsulates the four
//! shader-combination paths shared with live rendering.
//!
//! QA-011: this path still renders from the `CellRenderer`'s single-grid state
//! (`self.cells`) rather than the live split-pane layout, so with more than one
//! pane the capture shows the single-grid content, not what is on screen. Routing
//! it through the pane path needs `SplitPanesRenderParams` at both call sites in
//! the root crate; see the QA-011 note in `rendering.rs`.

use super::Renderer;

impl Renderer {
    /// Render the cell content through the shader chain to a target texture view.
    ///
    /// This encapsulates the 4 shader-combination paths:
    /// 1. No shaders: render cells directly to target
    /// 2. Custom shader only: cells -> custom shader -> target
    /// 3. Cursor shader only: cells -> cursor shader -> target
    /// 4. Custom + cursor: cells -> custom shader -> cursor shader -> target
    ///
    /// QA-003: Extracted from `take_screenshot` to deduplicate the shader-chaining logic.
    /// Both `render_split_panes` (live rendering) and `take_screenshot` (offscreen) use
    /// the same 4-branch pattern; this method handles the screenshot path where cells are
    /// rendered via `render_to_texture`/`render_to_view` (no split-pane layout).
    fn render_cells_to_target(
        &mut self,
        target_view: &wgpu::TextureView,
    ) -> Result<(), crate::error::RenderError> {
        let has_custom_shader = self.custom_shader_renderer.is_some();
        let use_cursor_shader =
            self.cursor_shader_renderer.is_some() && !self.cursor_shader_disabled_for_alt_screen;

        let map_err = |e: anyhow::Error| {
            crate::error::RenderError::ScreenshotMap(format!("Render failed: {:#}", e))
        };

        if has_custom_shader {
            // Render cells to the custom shader's intermediate texture
            let intermediate_view = self
                .custom_shader_renderer
                .as_ref()
                .ok_or_else(|| {
                    crate::error::RenderError::ShaderUnavailable(
                        "custom_shader_renderer unavailable (GPU device loss?)".into(),
                    )
                })?
                .intermediate_texture_view()
                .clone();
            self.cell_renderer
                .render_to_texture(&intermediate_view, true)
                .map_err(map_err)?;

            if use_cursor_shader {
                // Chain: cells -> custom shader -> cursor shader -> target
                let cursor_intermediate = self
                    .cursor_shader_renderer
                    .as_ref()
                    .ok_or_else(|| crate::error::RenderError::ShaderUnavailable(
                        "cursor_shader_renderer unavailable during shader chain (GPU device loss?)".into(),
                    ))?
                    .intermediate_texture_view()
                    .clone();
                self.custom_shader_renderer
                    .as_mut()
                    .ok_or_else(|| crate::error::RenderError::ShaderUnavailable(
                        "custom_shader_renderer unavailable during shader chain (GPU device loss?)".into(),
                    ))?
                    .render(
                        self.cell_renderer.device(),
                        self.cell_renderer.queue(),
                        &cursor_intermediate,
                        false,
                    )
                    .map_err(map_err)?;
                self.cursor_shader_renderer
                    .as_mut()
                    .ok_or_else(|| crate::error::RenderError::ShaderUnavailable(
                        "cursor_shader_renderer unavailable during shader chain (GPU device loss?)".into(),
                    ))?
                    .render(
                        self.cell_renderer.device(),
                        self.cell_renderer.queue(),
                        target_view,
                        true,
                    )
                    .map_err(map_err)?;
            } else {
                // Chain: cells -> custom shader -> target
                self.custom_shader_renderer
                    .as_mut()
                    .ok_or_else(|| {
                        crate::error::RenderError::ShaderUnavailable(
                            "custom_shader_renderer unavailable during render (GPU device loss?)"
                                .into(),
                        )
                    })?
                    .render(
                        self.cell_renderer.device(),
                        self.cell_renderer.queue(),
                        target_view,
                        true,
                    )
                    .map_err(map_err)?;
            }
        } else if use_cursor_shader {
            // Chain: cells -> cursor shader -> target
            let cursor_intermediate = self
                .cursor_shader_renderer
                .as_ref()
                .ok_or_else(|| {
                    crate::error::RenderError::ShaderUnavailable(
                        "cursor_shader_renderer unavailable (GPU device loss?)".into(),
                    )
                })?
                .intermediate_texture_view()
                .clone();
            self.cell_renderer
                .render_to_texture(&cursor_intermediate, true)
                .map_err(map_err)?;
            self.cursor_shader_renderer
                .as_mut()
                .ok_or_else(|| {
                    crate::error::RenderError::ShaderUnavailable(
                        "cursor_shader_renderer unavailable during render (GPU device loss?)"
                            .into(),
                    )
                })?
                .render(
                    self.cell_renderer.device(),
                    self.cell_renderer.queue(),
                    target_view,
                    true,
                )
                .map_err(map_err)?;
        } else {
            // No shaders: render cells directly to target
            self.cell_renderer
                .render_to_view(target_view)
                .map_err(map_err)?;
        }

        Ok(())
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

        // Render the full composited frame through the shader chain (QA-003: deduplicated).
        log::info!("take_screenshot: Rendering composited frame...");
        self.render_cells_to_target(&screenshot_view)?;

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

        // Wait for the GPU to finish — bounded so a stalled GPU can't freeze the
        // event loop indefinitely. This readback runs on the main thread when an
        // MCP screenshot is requested; a healthy GPU completes in milliseconds,
        // but `wait_indefinitely` could hang the loop if the device is lost.
        let gpu_timeout = std::time::Duration::from_secs(5);
        log::info!("take_screenshot: Waiting for GPU...");
        if let Err(e) = device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(gpu_timeout),
        }) {
            log::warn!("take_screenshot: GPU poll returned error: {:?}", e);
        }
        log::info!("take_screenshot: GPU poll complete, waiting for buffer map...");
        rx.recv_timeout(gpu_timeout)
            .map_err(|e| {
                crate::error::RenderError::ScreenshotMap(format!(
                    "Timed out or failed to receive map result: {}",
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
