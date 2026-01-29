use super::cell_renderer::{Cell, CellRenderer};
use super::graphics_renderer::GraphicsRenderer;
use crate::custom_shader_renderer::CustomShaderRenderer;
use anyhow::Result;
use std::sync::Arc;
use winit::dpi::PhysicalSize;
use winit::window::Window;

pub mod graphics;
pub mod shaders;

/// Renderer for the terminal using custom wgpu cell renderer
pub struct Renderer {
    // Cell renderer (owns the scrollbar)
    pub(crate) cell_renderer: CellRenderer,

    // Graphics renderer for sixel images
    pub(crate) graphics_renderer: GraphicsRenderer,

    // Current sixel graphics to render: (id, row, col, width_cells, height_cells, alpha, scroll_offset_rows)
    // Note: row is isize to allow negative values for graphics scrolled off top
    pub(crate) sixel_graphics: Vec<(u64, isize, usize, usize, usize, f32, usize)>,

    // egui renderer for settings UI
    pub(crate) egui_renderer: egui_wgpu::Renderer,

    // Custom shader renderer for post-processing effects (background shader)
    pub(crate) custom_shader_renderer: Option<CustomShaderRenderer>,
    // Track current shader path to detect changes
    pub(crate) custom_shader_path: Option<String>,

    // Cursor shader renderer for cursor-specific effects (separate from background shader)
    pub(crate) cursor_shader_renderer: Option<CustomShaderRenderer>,
    // Track current cursor shader path to detect changes
    pub(crate) cursor_shader_path: Option<String>,

    // Cached for convenience
    pub(crate) size: PhysicalSize<u32>,

    // Dirty flag for optimization - only render when content has changed
    pub(crate) dirty: bool,

    // Skip cursor shader when alt screen is active (TUI apps like vim, htop)
    pub(crate) cursor_shader_disabled_for_alt_screen: bool,

    // Debug overlay text
    #[allow(dead_code)]
    #[allow(dead_code)]
    pub(crate) debug_text: Option<String>,
}

impl Renderer {
    /// Create a new renderer
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        window: Arc<Window>,
        font_family: Option<&str>,
        font_family_bold: Option<&str>,
        font_family_italic: Option<&str>,
        font_family_bold_italic: Option<&str>,
        font_ranges: &[crate::config::FontRange],
        font_size: f32,
        window_padding: f32,
        line_spacing: f32,
        char_spacing: f32,
        scrollbar_position: &str,
        scrollbar_width: f32,
        scrollbar_thumb_color: [f32; 4],
        scrollbar_track_color: [f32; 4],
        enable_text_shaping: bool,
        enable_ligatures: bool,
        enable_kerning: bool,
        vsync_mode: crate::config::VsyncMode,
        window_opacity: f32,
        background_color: [u8; 3],
        background_image_path: Option<&str>,
        background_image_enabled: bool,
        background_image_mode: crate::config::BackgroundImageMode,
        background_image_opacity: f32,
        custom_shader_path: Option<&str>,
        custom_shader_enabled: bool,
        custom_shader_animation: bool,
        custom_shader_animation_speed: f32,
        custom_shader_text_opacity: f32,
        custom_shader_full_content: bool,
        custom_shader_brightness: f32,
        // Custom shader channel textures (iChannel0-3)
        custom_shader_channel_paths: &[Option<std::path::PathBuf>; 4],
        // Cubemap texture path prefix for environment mapping (iCubemap)
        custom_shader_cubemap_path: Option<&std::path::Path>,
        // Use background image as iChannel0 for custom shaders
        use_background_as_channel0: bool,
        // Cursor shader settings (separate from background shader)
        cursor_shader_path: Option<&str>,
        cursor_shader_enabled: bool,
        cursor_shader_animation: bool,
        cursor_shader_animation_speed: f32,
    ) -> Result<Self> {
        let size = window.inner_size();
        let scale_factor = window.scale_factor();

        // Standard DPI for the platform
        // macOS typically uses 72 DPI for points, Windows and most Linux use 96 DPI
        let platform_dpi = if cfg!(target_os = "macos") {
            72.0
        } else {
            96.0
        };

        // Convert font size from points to pixels for cell size calculation, honoring DPI and scale
        let base_font_pixels = font_size * platform_dpi / 72.0;
        let font_size_pixels = (base_font_pixels * scale_factor as f32).max(1.0);

        // Preliminary font lookup to get metrics for accurate cell height
        let font_manager = crate::font_manager::FontManager::new(
            font_family,
            font_family_bold,
            font_family_italic,
            font_family_bold_italic,
            font_ranges,
        )?;

        let (font_ascent, font_descent, font_leading, char_advance) = {
            let primary_font = font_manager.get_font(0).unwrap();
            let metrics = primary_font.metrics(&[]);
            let scale = font_size_pixels / metrics.units_per_em as f32;

            // Get advance width of a standard character ('m' is common for monospace width)
            let glyph_id = primary_font.charmap().map('m');
            let advance = primary_font.glyph_metrics(&[]).advance_width(glyph_id) * scale;

            (
                metrics.ascent * scale,
                metrics.descent * scale,
                metrics.leading * scale,
                advance,
            )
        };

        // Use font metrics for cell height if line_spacing is 1.0
        // Natural line height = ascent + descent + leading
        let natural_line_height = font_ascent + font_descent + font_leading;
        let char_height = (natural_line_height * line_spacing).max(1.0);

        // Calculate available space after padding (padding on all sides)
        let available_width = (size.width as f32 - window_padding * 2.0).max(0.0);
        let available_height = (size.height as f32 - window_padding * 2.0).max(0.0);

        // Calculate terminal dimensions based on font size in pixels and spacing
        let char_width = (char_advance * char_spacing).max(1.0); // Configurable character width
        let cols = (available_width / char_width).max(1.0) as usize;
        let rows = (available_height / char_height).max(1.0) as usize;

        // Create cell renderer with font fallback support (owns scrollbar)
        let cell_renderer = CellRenderer::new(
            window.clone(),
            font_family,
            font_family_bold,
            font_family_italic,
            font_family_bold_italic,
            font_ranges,
            font_size,
            cols,
            rows,
            window_padding,
            line_spacing,
            char_spacing,
            scrollbar_position,
            scrollbar_width,
            scrollbar_thumb_color,
            scrollbar_track_color,
            enable_text_shaping,
            enable_ligatures,
            enable_kerning,
            vsync_mode,
            window_opacity,
            background_color,
            {
                let bg_path = if background_image_enabled {
                    background_image_path
                } else {
                    None
                };
                log::info!(
                    "Renderer::new: background_image_enabled={}, path={:?}",
                    background_image_enabled,
                    bg_path
                );
                bg_path
            },
            background_image_mode,
            background_image_opacity,
        )
        .await?;

        // Create egui renderer for settings UI
        let egui_renderer = egui_wgpu::Renderer::new(
            cell_renderer.device(),
            cell_renderer.surface_format(),
            egui_wgpu::RendererOptions {
                msaa_samples: 1,
                depth_stencil_format: None,
                dithering: false,
                predictable_texture_filtering: false,
            },
        );

        // Create graphics renderer for sixel images
        let graphics_renderer = GraphicsRenderer::new(
            cell_renderer.device(),
            cell_renderer.surface_format(),
            cell_renderer.cell_width(),
            cell_renderer.cell_height(),
            cell_renderer.window_padding(),
        )?;

        // Create custom shader renderer if configured
        let (custom_shader_renderer, initial_shader_path) = shaders::init_custom_shader(
            &cell_renderer,
            size.width,
            size.height,
            window_padding,
            custom_shader_path,
            custom_shader_enabled,
            custom_shader_animation,
            custom_shader_animation_speed,
            window_opacity,
            custom_shader_text_opacity,
            custom_shader_full_content,
            custom_shader_brightness,
            custom_shader_channel_paths,
            custom_shader_cubemap_path,
            use_background_as_channel0,
        );

        // Create cursor shader renderer if configured (separate from background shader)
        let (cursor_shader_renderer, initial_cursor_shader_path) = shaders::init_cursor_shader(
            &cell_renderer,
            size.width,
            size.height,
            window_padding,
            cursor_shader_path,
            cursor_shader_enabled,
            cursor_shader_animation,
            cursor_shader_animation_speed,
            window_opacity,
        );

        debug_info!(
            "renderer",
            "Renderer created: custom_shader_loaded={}, cursor_shader_loaded={}",
            initial_shader_path.is_some(),
            initial_cursor_shader_path.is_some()
        );

        Ok(Self {
            cell_renderer,
            graphics_renderer,
            sixel_graphics: Vec::new(),
            egui_renderer,
            custom_shader_renderer,
            custom_shader_path: initial_shader_path,
            cursor_shader_renderer,
            cursor_shader_path: initial_cursor_shader_path,
            size,
            dirty: true, // Start dirty to ensure initial render
            cursor_shader_disabled_for_alt_screen: false,
            debug_text: None,
        })
    }

    /// Resize the renderer and recalculate grid dimensions based on padding/font metrics
    pub fn resize(&mut self, new_size: PhysicalSize<u32>) -> (usize, usize) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.dirty = true; // Mark dirty on resize
            let result = self.cell_renderer.resize(new_size.width, new_size.height);

            // Update graphics renderer cell dimensions
            self.graphics_renderer.update_cell_dimensions(
                self.cell_renderer.cell_width(),
                self.cell_renderer.cell_height(),
                self.cell_renderer.window_padding(),
            );

            // Update custom shader renderer dimensions
            if let Some(ref mut custom_shader) = self.custom_shader_renderer {
                custom_shader.resize(self.cell_renderer.device(), new_size.width, new_size.height);
                // Sync cell dimensions for cursor position calculation
                custom_shader.update_cell_dimensions(
                    self.cell_renderer.cell_width(),
                    self.cell_renderer.cell_height(),
                    self.cell_renderer.window_padding(),
                );
            }

            // Update cursor shader renderer dimensions
            if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
                cursor_shader.resize(self.cell_renderer.device(), new_size.width, new_size.height);
                // Sync cell dimensions for cursor position calculation
                cursor_shader.update_cell_dimensions(
                    self.cell_renderer.cell_width(),
                    self.cell_renderer.cell_height(),
                    self.cell_renderer.window_padding(),
                );
            }

            return result;
        }

        self.cell_renderer.grid_size()
    }

    /// Update scale factor and resize so the PTY grid matches the new DPI.
    pub fn handle_scale_factor_change(
        &mut self,
        scale_factor: f64,
        new_size: PhysicalSize<u32>,
    ) -> (usize, usize) {
        self.cell_renderer.update_scale_factor(scale_factor);
        self.resize(new_size)
    }

    /// Update the terminal cells
    pub fn update_cells(&mut self, cells: &[Cell]) {
        self.cell_renderer.update_cells(cells);
        self.dirty = true; // Mark dirty when cells change
    }

    /// Clear all cells in the renderer.
    /// Call this when switching tabs to ensure a clean slate.
    pub fn clear_all_cells(&mut self) {
        self.cell_renderer.clear_all_cells();
        self.dirty = true;
    }

    /// Update cursor position and style for geometric rendering
    pub fn update_cursor(
        &mut self,
        position: (usize, usize),
        opacity: f32,
        style: par_term_emu_core_rust::cursor::CursorStyle,
    ) {
        self.cell_renderer.update_cursor(position, opacity, style);
        self.dirty = true;
    }

    /// Clear cursor (hide it)
    pub fn clear_cursor(&mut self) {
        self.cell_renderer.clear_cursor();
        self.dirty = true;
    }

    /// Update scrollbar state
    ///
    /// # Arguments
    /// * `scroll_offset` - Current scroll offset (0 = at bottom)
    /// * `visible_lines` - Number of lines visible on screen
    /// * `total_lines` - Total number of lines including scrollback
    pub fn update_scrollbar(
        &mut self,
        scroll_offset: usize,
        visible_lines: usize,
        total_lines: usize,
    ) {
        self.cell_renderer
            .update_scrollbar(scroll_offset, visible_lines, total_lines);
        self.dirty = true; // Mark dirty when scrollbar changes
    }

    /// Set the visual bell flash intensity
    ///
    /// # Arguments
    /// * `intensity` - Flash intensity from 0.0 (no flash) to 1.0 (full white flash)
    pub fn set_visual_bell_intensity(&mut self, intensity: f32) {
        self.cell_renderer.set_visual_bell_intensity(intensity);
        if intensity > 0.0 {
            self.dirty = true; // Mark dirty when flash is active
        }
    }

    /// Update window opacity in real-time
    pub fn update_opacity(&mut self, opacity: f32) {
        self.cell_renderer.update_opacity(opacity);

        // Propagate to custom shader renderer if present
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.set_opacity(opacity);
        }

        // Propagate to cursor shader renderer if present
        if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            cursor_shader.set_opacity(opacity);
        }

        self.dirty = true;
    }

    /// Update cursor color for cell rendering
    pub fn update_cursor_color(&mut self, color: [u8; 3]) {
        self.cell_renderer.update_cursor_color(color);
        self.dirty = true;
    }

    /// Set whether cursor should be hidden when cursor shader is active
    pub fn set_cursor_hidden_for_shader(&mut self, hidden: bool) {
        self.cell_renderer.set_cursor_hidden_for_shader(hidden);
        self.dirty = true;
    }

    /// Set whether transparency affects only default background cells.
    /// When true, non-default (colored) backgrounds remain opaque for readability.
    pub fn set_transparency_affects_only_default_background(&mut self, value: bool) {
        self.cell_renderer
            .set_transparency_affects_only_default_background(value);
        self.dirty = true;
    }

    /// Set whether text should always be rendered at full opacity.
    /// When true, text remains opaque regardless of window transparency settings.
    pub fn set_keep_text_opaque(&mut self, value: bool) {
        self.cell_renderer.set_keep_text_opaque(value);

        // Also propagate to custom shader renderer if present
        if let Some(ref mut custom_shader) = self.custom_shader_renderer {
            custom_shader.set_keep_text_opaque(value);
        }

        // And to cursor shader renderer if present
        if let Some(ref mut cursor_shader) = self.cursor_shader_renderer {
            cursor_shader.set_keep_text_opaque(value);
        }

        self.dirty = true;
    }

    /// Set whether cursor shader should be disabled due to alt screen being active
    ///
    /// When alt screen is active (e.g., vim, htop, less), cursor shader effects
    /// are disabled since TUI applications typically have their own cursor handling.
    pub fn set_cursor_shader_disabled_for_alt_screen(&mut self, disabled: bool) {
        if self.cursor_shader_disabled_for_alt_screen != disabled {
            debug_log!("cursor-shader", "Alt-screen disable set to {}", disabled);
            self.cursor_shader_disabled_for_alt_screen = disabled;
        } else {
            self.cursor_shader_disabled_for_alt_screen = disabled;
        }
    }

    /// Update window padding in real-time without full renderer rebuild
    /// Returns Some((cols, rows)) if grid size changed and terminal needs resize
    #[allow(dead_code)]
    pub fn update_window_padding(&mut self, padding: f32) -> Option<(usize, usize)> {
        let result = self.cell_renderer.update_window_padding(padding);
        self.dirty = true;
        result
    }

    /// Enable/disable background image and reload if needed
    pub fn set_background_image_enabled(
        &mut self,
        enabled: bool,
        path: Option<&str>,
        mode: crate::config::BackgroundImageMode,
        opacity: f32,
    ) {
        let path = if enabled { path } else { None };
        self.cell_renderer.set_background_image(path, mode, opacity);

        // Sync background texture to custom shader if it's using background as channel0
        self.sync_background_texture_to_shader();

        self.dirty = true;
    }

    /// Update scrollbar appearance in real-time
    pub fn update_scrollbar_appearance(
        &mut self,
        width: f32,
        thumb_color: [f32; 4],
        track_color: [f32; 4],
    ) {
        self.cell_renderer
            .update_scrollbar_appearance(width, thumb_color, track_color);
        self.dirty = true;
    }

    /// Update scrollbar position (left/right) in real-time
    #[allow(dead_code)]
    pub fn update_scrollbar_position(&mut self, position: &str) {
        self.cell_renderer.update_scrollbar_position(position);
        self.dirty = true;
    }

    /// Update background image opacity in real-time
    #[allow(dead_code)]
    pub fn update_background_image_opacity(&mut self, opacity: f32) {
        self.cell_renderer.update_background_image_opacity(opacity);
        self.dirty = true;
    }

    /// Check if animation requires continuous rendering
    ///
    /// Returns true if shader animation is enabled or a cursor trail animation
    /// might still be in progress.
    pub fn needs_continuous_render(&self) -> bool {
        let custom_needs = self
            .custom_shader_renderer
            .as_ref()
            .is_some_and(|r| r.animation_enabled() || r.cursor_needs_animation());
        let cursor_needs = self
            .cursor_shader_renderer
            .as_ref()
            .is_some_and(|r| r.animation_enabled() || r.cursor_needs_animation());
        custom_needs || cursor_needs
    }

    /// Render a frame with optional egui overlay
    /// Returns true if rendering was performed, false if skipped
    pub fn render(
        &mut self,
        egui_data: Option<(egui::FullOutput, &egui::Context)>,
        force_egui_opaque: bool,
        show_scrollbar: bool,
    ) -> Result<bool> {
        // Custom shader animation forces continuous rendering
        let force_render = self.needs_continuous_render();

        if !self.dirty && egui_data.is_none() && !force_render {
            // Skip rendering if nothing has changed
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
                    .unwrap()
                    .intermediate_texture_view(),
                true, // Always skip background image - shader handles background
            )?
        } else if use_cursor_shader {
            // Render terminal to intermediate texture for cursor shader
            // Cursor shader doesn't use background as channel0, so always render it
            self.cell_renderer.render_to_texture(
                self.cursor_shader_renderer
                    .as_ref()
                    .unwrap()
                    .intermediate_texture_view(),
                false, // Don't skip background image
            )?
        } else {
            // Render directly to surface (no shaders, or cursor shader disabled for alt screen)
            self.cell_renderer.render(show_scrollbar)?
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
                        .unwrap()
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

                // Render overlays (scrollbar, visual bell) on top after shader
                self.cell_renderer
                    .render_overlays(&surface_texture, show_scrollbar)?;
            }
            t_custom.elapsed()
        } else {
            std::time::Duration::ZERO
        };

        // Apply cursor shader if enabled (skip when alt screen is active for TUI apps)
        let t_cursor = std::time::Instant::now();
        let cursor_shader_time = if use_cursor_shader {
            log::trace!("Rendering cursor shader");
            let cursor_shader = self.cursor_shader_renderer.as_mut().unwrap();
            let surface_view = surface_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            cursor_shader.render(
                self.cell_renderer.device(),
                self.cell_renderer.queue(),
                &surface_view,
                true, // Apply opacity - this is the final render to surface
            )?;

            // Render overlays (scrollbar, visual bell) on top after cursor shader
            self.cell_renderer
                .render_overlays(&surface_texture, show_scrollbar)?;
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
            crate::debug_info!(
                "RENDER",
                "RENDER_BREAKDOWN: CellRender={:.2}ms BgShader={:.2}ms CursorShader={:.2}ms Sixel={:.2}ms Egui={:.2}ms PRESENT={:.2}ms Total={:.2}ms",
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

    /// Get window padding in pixels
    pub fn window_padding(&self) -> f32 {
        self.cell_renderer.window_padding()
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

    /// Check if the renderer needs to be redrawn
    #[allow(dead_code)]
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Mark the renderer as dirty, forcing a redraw on next render call
    #[allow(dead_code)]
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Set debug overlay text to be rendered
    #[allow(dead_code)]
    #[allow(dead_code)]
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
    pub fn is_vsync_mode_supported(&self, mode: crate::config::VsyncMode) -> bool {
        self.cell_renderer.is_vsync_mode_supported(mode)
    }

    /// Update the vsync mode. Returns the actual mode applied (may differ if requested mode unsupported).
    /// Also returns whether the mode was changed.
    pub fn update_vsync_mode(
        &mut self,
        mode: crate::config::VsyncMode,
    ) -> (crate::config::VsyncMode, bool) {
        let result = self.cell_renderer.update_vsync_mode(mode);
        if result.1 {
            self.dirty = true;
        }
        result
    }

    /// Get the current vsync mode
    #[allow(dead_code)]
    pub fn current_vsync_mode(&self) -> crate::config::VsyncMode {
        self.cell_renderer.current_vsync_mode()
    }

    /// Clear the glyph cache to force re-rasterization
    /// Useful after display changes where font rendering may differ
    pub fn clear_glyph_cache(&mut self) {
        self.cell_renderer.clear_glyph_cache();
        self.dirty = true;
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
        crate::debug_info!("SHADER", "Shader animations paused");
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
        crate::debug_info!(
            "SHADER",
            "Shader animations resumed (custom: {}, cursor: {})",
            custom_shader_animation,
            cursor_shader_animation
        );
    }
}
