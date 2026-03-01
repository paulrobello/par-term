//! Per-frame render orchestration entry point.
//!
//! `Renderer::render` is the primary single-pane frame render method.  It owns
//! the "what happens each frame" narrative:
//!
//! 1. Fast-path check (skip expensive passes when nothing changed)
//! 2. Cell content → intermediate texture (or directly to surface)
//! 3. Background custom shader pass
//! 4. Cursor shader pass
//! 5. Sixel/inline graphics pass
//! 6. Scrollbar + visual bell overlay pass
//! 7. egui overlay pass
//! 8. Surface present (vsync wait happens here)
//!
//! The method lives here rather than in `rendering.rs` to keep the
//! "orchestration narrative" separate from the larger per-pane / split-pane
//! variants in `rendering.rs`.

use anyhow::Result;

use super::Renderer;

impl Renderer {
    /// Render a frame with optional egui overlay.
    ///
    /// Returns `true` if rendering was performed, `false` if skipped
    /// (nothing changed and no egui data was provided).
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
                        .expect(
                            "Cursor shader renderer must be Some when use_cursor_shader is true",
                        )
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
            let cursor_shader = self
                .cursor_shader_renderer
                .as_mut()
                .expect("Cursor shader renderer must be Some when use_cursor_shader is true");
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
}
