//! Offscreen frame capture.
//!
//! `take_screenshot` renders a composited frame into a `COPY_SRC` texture and reads
//! it back as an `RgbaImage`.
//!
//! QA-011 (resolved): this used to render from the `CellRenderer`'s single-grid
//! state (`self.cells`) rather than the live split-pane layout, so with more than
//! one pane the capture showed the single-grid content, not what was on screen. It
//! now runs the same `composite_panes` the live frame does, against an offscreen
//! target instead of the surface, so the capture is the screen.
//!
//! Still absent from the image: the egui overlay (tab bar, settings window, menus).
//! `render_egui` consumes an `egui::FullOutput` produced once per frame by the live
//! egui pass; a capture taken between frames has none, so `PaneCaptureParams`
//! carries no egui data by construction. This is unchanged from the old path.

use super::{PaneCaptureParams, Renderer};

impl Renderer {
    /// Take a screenshot of the current terminal content.
    /// Returns an RGBA image that can be saved to disk.
    ///
    /// This captures the fully composited output including shader effects, every
    /// pane of a split, dividers, pane titles and the focus indicator — the same
    /// draw sequence `render_split_panes` puts on the surface, minus the egui
    /// overlay (see the module docs).
    ///
    /// `params` is the same pane data the live frame renders; the caller gathers
    /// it exactly as it would for `render_split_panes`. Rendering is unconditional:
    /// unlike the live path this does not consult or clear `dirty`.
    pub fn take_screenshot(
        &mut self,
        params: PaneCaptureParams<'_>,
    ) -> Result<image::RgbaImage, crate::error::RenderError> {
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

        // Render the full composited frame through the live pane path (QA-011).
        log::info!("take_screenshot: Rendering composited frame...");
        self.composite_panes_offscreen(params, &screenshot_view)
            .map_err(|e| {
                crate::error::RenderError::ScreenshotMap(format!("Render failed: {:#}", e))
            })?;
        // Match the surface path's alpha stamp so an opaque window reads back opaque.
        self.cell_renderer
            .render_opaque_alpha_to_view(&screenshot_view)
            .map_err(|e| {
                crate::error::RenderError::ScreenshotMap(format!("Alpha stamp failed: {:#}", e))
            })?;

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
