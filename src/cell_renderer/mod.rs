use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use winit::window::Window;

use crate::font_manager::FontManager;
use crate::scrollbar::Scrollbar;

pub mod atlas;
pub mod background;
pub mod block_chars;
pub mod pipeline;
pub mod render;
pub mod types;
pub use types::*;

pub struct CellRenderer {
    pub(crate) device: Arc<wgpu::Device>,
    pub(crate) queue: Arc<wgpu::Queue>,
    pub(crate) surface: wgpu::Surface<'static>,
    pub(crate) config: wgpu::SurfaceConfiguration,
    /// Supported present modes for this surface (for vsync mode validation)
    pub(crate) supported_present_modes: Vec<wgpu::PresentMode>,

    // Pipelines
    pub(crate) bg_pipeline: wgpu::RenderPipeline,
    pub(crate) text_pipeline: wgpu::RenderPipeline,
    pub(crate) bg_image_pipeline: wgpu::RenderPipeline,
    #[allow(dead_code)]
    pub(crate) visual_bell_pipeline: wgpu::RenderPipeline,

    // Buffers
    pub(crate) vertex_buffer: wgpu::Buffer,
    pub(crate) bg_instance_buffer: wgpu::Buffer,
    pub(crate) text_instance_buffer: wgpu::Buffer,
    pub(crate) bg_image_uniform_buffer: wgpu::Buffer,
    #[allow(dead_code)]
    pub(crate) visual_bell_uniform_buffer: wgpu::Buffer,

    // Bind groups
    pub(crate) text_bind_group: wgpu::BindGroup,
    #[allow(dead_code)]
    pub(crate) text_bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) bg_image_bind_group: Option<wgpu::BindGroup>,
    pub(crate) bg_image_bind_group_layout: wgpu::BindGroupLayout,
    #[allow(dead_code)]
    pub(crate) visual_bell_bind_group: wgpu::BindGroup,

    // Glyph atlas
    pub(crate) atlas_texture: wgpu::Texture,
    #[allow(dead_code)]
    pub(crate) atlas_view: wgpu::TextureView,
    pub(crate) glyph_cache: HashMap<u64, GlyphInfo>,
    pub(crate) lru_head: Option<u64>,
    pub(crate) lru_tail: Option<u64>,
    pub(crate) atlas_next_x: u32,
    pub(crate) atlas_next_y: u32,
    pub(crate) atlas_row_height: u32,

    // Grid state
    pub(crate) cols: usize,
    pub(crate) rows: usize,
    pub(crate) cell_width: f32,
    pub(crate) cell_height: f32,
    pub(crate) window_padding: f32,
    /// Vertical offset for terminal content (e.g., tab bar height).
    /// Content is rendered starting at y = window_padding + content_offset_y.
    pub(crate) content_offset_y: f32,
    #[allow(dead_code)]
    pub(crate) scale_factor: f32,

    // Components
    pub(crate) font_manager: FontManager,
    pub(crate) scrollbar: Scrollbar,

    // Dynamic state
    pub(crate) cells: Vec<Cell>,
    pub(crate) dirty_rows: Vec<bool>,
    pub(crate) row_cache: Vec<Option<RowCacheEntry>>,
    pub(crate) cursor_pos: (usize, usize),
    pub(crate) cursor_opacity: f32,
    pub(crate) cursor_style: par_term_emu_core_rust::cursor::CursorStyle,
    /// Separate cursor instance for beam/underline styles (rendered as overlay)
    pub(crate) cursor_overlay: Option<BackgroundInstance>,
    /// Cursor color [R, G, B] as floats (0.0-1.0)
    pub(crate) cursor_color: [f32; 3],
    /// Hide cursor when cursor shader is active (let shader handle cursor rendering)
    pub(crate) cursor_hidden_for_shader: bool,
    pub(crate) visual_bell_intensity: f32,
    pub(crate) window_opacity: f32,
    pub(crate) background_color: [f32; 4],

    // Font configuration (base values, before scale factor)
    pub(crate) base_font_size: f32,
    pub(crate) line_spacing: f32,
    pub(crate) char_spacing: f32,

    // Font metrics (scaled by current scale_factor)
    pub(crate) font_ascent: f32,
    pub(crate) font_descent: f32,
    pub(crate) font_leading: f32,
    pub(crate) font_size_pixels: f32,
    pub(crate) char_advance: f32,

    // Background image
    pub(crate) bg_image_texture: Option<wgpu::Texture>,
    pub(crate) bg_image_mode: crate::config::BackgroundImageMode,
    pub(crate) bg_image_opacity: f32,
    pub(crate) bg_image_width: u32,
    pub(crate) bg_image_height: u32,
    /// When true, current background is a solid color (not an image).
    /// Solid colors should be rendered via clear color to respect window_opacity,
    /// not via bg_image_pipeline which would cover the transparent background.
    pub(crate) bg_is_solid_color: bool,
    /// The solid background color [R, G, B] as floats (0.0-1.0).
    /// Only used when bg_is_solid_color is true.
    pub(crate) solid_bg_color: [f32; 3],

    // Metrics
    pub(crate) max_bg_instances: usize,
    pub(crate) max_text_instances: usize,

    // CPU-side instance buffers for incremental updates
    pub(crate) bg_instances: Vec<BackgroundInstance>,
    pub(crate) text_instances: Vec<TextInstance>,

    // Shaping options
    #[allow(dead_code)]
    pub(crate) enable_text_shaping: bool,
    pub(crate) enable_ligatures: bool,
    pub(crate) enable_kerning: bool,

    // Solid white pixel in atlas for geometric block rendering
    pub(crate) solid_pixel_offset: (u32, u32),

    // Transparency mode
    /// When true, only default background cells are transparent.
    /// Non-default (colored) backgrounds remain opaque for readability.
    pub(crate) transparency_affects_only_default_background: bool,

    /// When true, text is always rendered at full opacity regardless of window transparency.
    pub(crate) keep_text_opaque: bool,
}

impl CellRenderer {
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        window: Arc<Window>,
        font_family: Option<&str>,
        font_family_bold: Option<&str>,
        font_family_italic: Option<&str>,
        font_family_bold_italic: Option<&str>,
        font_ranges: &[crate::config::FontRange],
        font_size: f32,
        cols: usize,
        rows: usize,
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
        background_image_mode: crate::config::BackgroundImageMode,
        background_image_opacity: f32,
    ) -> Result<Self> {
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window.clone())?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .context("Failed to find wgpu adapter")?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
                ..Default::default()
            })
            .await?;

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        let size = window.inner_size();
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| !f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        // Store supported present modes for runtime validation
        let supported_present_modes = surface_caps.present_modes.clone();

        // Select present mode with fallback if requested mode isn't supported
        let requested_mode = vsync_mode.to_present_mode();
        let present_mode = if supported_present_modes.contains(&requested_mode) {
            requested_mode
        } else {
            // Fall back to Fifo (always supported) or first available
            log::warn!(
                "Requested present mode {:?} not supported (available: {:?}), falling back",
                requested_mode,
                supported_present_modes
            );
            if supported_present_modes.contains(&wgpu::PresentMode::Fifo) {
                wgpu::PresentMode::Fifo
            } else {
                supported_present_modes[0]
            }
        };

        // Select alpha mode for window transparency
        // Prefer PreMultiplied (best for compositing) > PostMultiplied > Auto > first available
        let alpha_mode = if surface_caps
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::PreMultiplied)
        {
            wgpu::CompositeAlphaMode::PreMultiplied
        } else if surface_caps
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::PostMultiplied)
        {
            wgpu::CompositeAlphaMode::PostMultiplied
        } else if surface_caps
            .alpha_modes
            .contains(&wgpu::CompositeAlphaMode::Auto)
        {
            wgpu::CompositeAlphaMode::Auto
        } else {
            surface_caps.alpha_modes[0]
        };
        log::info!(
            "Selected alpha mode: {:?} (available: {:?})",
            alpha_mode,
            surface_caps.alpha_modes
        );

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let scale_factor = window.scale_factor() as f32;

        let platform_dpi = if cfg!(target_os = "macos") {
            72.0
        } else {
            96.0
        };

        let base_font_pixels = font_size * platform_dpi / 72.0;
        let font_size_pixels = (base_font_pixels * scale_factor).max(1.0);

        let font_manager = FontManager::new(
            font_family,
            font_family_bold,
            font_family_italic,
            font_family_bold_italic,
            font_ranges,
        )?;

        // Extract font metrics
        let (font_ascent, font_descent, font_leading, char_advance) = {
            let primary_font = font_manager.get_font(0).unwrap();
            let metrics = primary_font.metrics(&[]);
            let scale = font_size_pixels / metrics.units_per_em as f32;
            let glyph_id = primary_font.charmap().map('m');
            let advance = primary_font.glyph_metrics(&[]).advance_width(glyph_id) * scale;
            (
                metrics.ascent * scale,
                metrics.descent * scale,
                metrics.leading * scale,
                advance,
            )
        };

        let natural_line_height = font_ascent + font_descent + font_leading;
        let cell_height = (natural_line_height * line_spacing).max(1.0);
        let cell_width = (char_advance * char_spacing).max(1.0);

        let scrollbar = Scrollbar::new(
            &device,
            surface_format,
            scrollbar_width,
            scrollbar_position,
            scrollbar_thumb_color,
            scrollbar_track_color,
        );

        // Create pipelines using the pipeline module
        let bg_pipeline = pipeline::create_bg_pipeline(&device, surface_format);

        let (atlas_texture, atlas_view, atlas_sampler) = pipeline::create_atlas(&device);
        let text_bind_group_layout = pipeline::create_text_bind_group_layout(&device);
        let text_bind_group = pipeline::create_text_bind_group(
            &device,
            &text_bind_group_layout,
            &atlas_view,
            &atlas_sampler,
        );
        let text_pipeline =
            pipeline::create_text_pipeline(&device, surface_format, &text_bind_group_layout);

        let bg_image_bind_group_layout = pipeline::create_bg_image_bind_group_layout(&device);
        let bg_image_pipeline = pipeline::create_bg_image_pipeline(
            &device,
            surface_format,
            &bg_image_bind_group_layout,
        );
        let bg_image_uniform_buffer = pipeline::create_bg_image_uniform_buffer(&device);

        let (visual_bell_pipeline, visual_bell_bind_group, _, visual_bell_uniform_buffer) =
            pipeline::create_visual_bell_pipeline(&device, surface_format);

        let vertex_buffer = pipeline::create_vertex_buffer(&device);

        // Instance buffers
        let max_bg_instances = cols * rows + 1;
        let max_text_instances = cols * rows * 2;
        let (bg_instance_buffer, text_instance_buffer) =
            pipeline::create_instance_buffers(&device, max_bg_instances, max_text_instances);

        let mut renderer = Self {
            device,
            queue,
            surface,
            config,
            supported_present_modes,
            bg_pipeline,
            text_pipeline,
            bg_image_pipeline,
            visual_bell_pipeline,
            vertex_buffer,
            bg_instance_buffer,
            text_instance_buffer,
            bg_image_uniform_buffer,
            visual_bell_uniform_buffer,
            text_bind_group,
            text_bind_group_layout,
            bg_image_bind_group: None,
            bg_image_bind_group_layout,
            visual_bell_bind_group,
            atlas_texture,
            atlas_view,
            glyph_cache: HashMap::new(),
            lru_head: None,
            lru_tail: None,
            atlas_next_x: 0,
            atlas_next_y: 0,
            atlas_row_height: 0,
            cols,
            rows,
            cell_width,
            cell_height,
            window_padding,
            content_offset_y: 0.0,
            scale_factor,
            font_manager,
            scrollbar,
            cells: vec![Cell::default(); cols * rows],
            dirty_rows: vec![true; rows],
            row_cache: (0..rows).map(|_| None).collect(),
            cursor_pos: (0, 0),
            cursor_opacity: 0.0,
            cursor_style: par_term_emu_core_rust::cursor::CursorStyle::SteadyBlock,
            cursor_overlay: None,
            cursor_color: [1.0, 1.0, 1.0],
            cursor_hidden_for_shader: false,
            visual_bell_intensity: 0.0,
            window_opacity,
            background_color: [
                background_color[0] as f32 / 255.0,
                background_color[1] as f32 / 255.0,
                background_color[2] as f32 / 255.0,
                1.0,
            ],
            base_font_size: font_size,
            line_spacing,
            char_spacing,
            font_ascent,
            font_descent,
            font_leading,
            font_size_pixels,
            char_advance,
            bg_image_texture: None,
            bg_image_mode: background_image_mode,
            bg_image_opacity: background_image_opacity,
            bg_image_width: 0,
            bg_image_height: 0,
            bg_is_solid_color: false,
            solid_bg_color: [0.0, 0.0, 0.0],
            max_bg_instances,
            max_text_instances,
            bg_instances: vec![
                BackgroundInstance {
                    position: [0.0, 0.0],
                    size: [0.0, 0.0],
                    color: [0.0, 0.0, 0.0, 0.0],
                };
                max_bg_instances
            ],
            text_instances: vec![
                TextInstance {
                    position: [0.0, 0.0],
                    size: [0.0, 0.0],
                    tex_offset: [0.0, 0.0],
                    tex_size: [0.0, 0.0],
                    color: [0.0, 0.0, 0.0, 0.0],
                    is_colored: 0,
                };
                max_text_instances
            ],
            enable_text_shaping,
            enable_ligatures,
            enable_kerning,
            solid_pixel_offset: (0, 0),
            transparency_affects_only_default_background: false,
            keep_text_opaque: true,
        };

        // Upload a solid white 2x2 pixel block to the atlas for geometric block rendering
        renderer.upload_solid_pixel();

        log::info!(
            "CellRenderer::new: background_image_path={:?}",
            background_image_path
        );
        if let Some(path) = background_image_path {
            renderer.load_background_image(path)?;
        }

        Ok(renderer)
    }

    /// Upload a solid white pixel to the atlas for use in geometric block rendering
    pub(crate) fn upload_solid_pixel(&mut self) {
        let size = 2u32; // 2x2 for better sampling
        let white_pixels: Vec<u8> = vec![255; (size * size * 4) as usize];

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.atlas_texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: self.atlas_next_x,
                    y: self.atlas_next_y,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &white_pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * size),
                rows_per_image: Some(size),
            },
            wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
        );

        self.solid_pixel_offset = (self.atlas_next_x, self.atlas_next_y);
        self.atlas_next_x += size + 2; // padding
        self.atlas_row_height = self.atlas_row_height.max(size);
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }
    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.config.format
    }
    pub fn cell_width(&self) -> f32 {
        self.cell_width
    }
    pub fn cell_height(&self) -> f32 {
        self.cell_height
    }
    pub fn window_padding(&self) -> f32 {
        self.window_padding
    }
    pub fn content_offset_y(&self) -> f32 {
        self.content_offset_y
    }
    /// Set the vertical content offset (e.g., tab bar height).
    /// Returns Some((cols, rows)) if grid size changed, None otherwise.
    pub fn set_content_offset_y(&mut self, offset: f32) -> Option<(usize, usize)> {
        if (self.content_offset_y - offset).abs() > f32::EPSILON {
            self.content_offset_y = offset;
            // Recalculate grid size with new offset
            let size = (self.config.width, self.config.height);
            return Some(self.resize(size.0, size.1));
        }
        None
    }
    pub fn grid_size(&self) -> (usize, usize) {
        (self.cols, self.rows)
    }
    pub fn keep_text_opaque(&self) -> bool {
        self.keep_text_opaque
    }

    pub fn resize(&mut self, width: u32, height: u32) -> (usize, usize) {
        if width == 0 || height == 0 {
            return (self.cols, self.rows);
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);

        let available_width = (width as f32 - self.window_padding * 2.0).max(0.0);
        // Subtract content_offset_y (tab bar height) from available height
        let available_height =
            (height as f32 - self.window_padding * 2.0 - self.content_offset_y).max(0.0);
        let new_cols = (available_width / self.cell_width).max(1.0) as usize;
        let new_rows = (available_height / self.cell_height).max(1.0) as usize;

        if new_cols != self.cols || new_rows != self.rows {
            self.cols = new_cols;
            self.rows = new_rows;
            self.cells = vec![Cell::default(); self.cols * self.rows];
            self.dirty_rows = vec![true; self.rows];
            self.row_cache = (0..self.rows).map(|_| None).collect();
            self.recreate_instance_buffers();
        }

        self.update_bg_image_uniforms();
        (self.cols, self.rows)
    }

    fn recreate_instance_buffers(&mut self) {
        self.max_bg_instances = self.cols * self.rows + 1;
        self.max_text_instances = self.cols * self.rows * 2;
        let (bg_buf, text_buf) = pipeline::create_instance_buffers(
            &self.device,
            self.max_bg_instances,
            self.max_text_instances,
        );
        self.bg_instance_buffer = bg_buf;
        self.text_instance_buffer = text_buf;

        self.bg_instances = vec![
            BackgroundInstance {
                position: [0.0, 0.0],
                size: [0.0, 0.0],
                color: [0.0, 0.0, 0.0, 0.0],
            };
            self.max_bg_instances
        ];
        self.text_instances = vec![
            TextInstance {
                position: [0.0, 0.0],
                size: [0.0, 0.0],
                tex_offset: [0.0, 0.0],
                tex_size: [0.0, 0.0],
                color: [0.0, 0.0, 0.0, 0.0],
                is_colored: 0,
            };
            self.max_text_instances
        ];
    }

    pub fn update_cells(&mut self, new_cells: &[Cell]) {
        for row in 0..self.rows {
            let start = row * self.cols;
            let end = (row + 1) * self.cols;
            if start < new_cells.len() && end <= new_cells.len() {
                let row_slice = &new_cells[start..end];
                if row_slice != &self.cells[start..end] {
                    self.cells[start..end].clone_from_slice(row_slice);
                    self.dirty_rows[row] = true;
                }
            }
        }
    }

    /// Clear all cells and mark all rows as dirty.
    pub fn clear_all_cells(&mut self) {
        for cell in &mut self.cells {
            *cell = Cell::default();
        }
        for dirty in &mut self.dirty_rows {
            *dirty = true;
        }
    }

    pub fn update_cursor(
        &mut self,
        pos: (usize, usize),
        opacity: f32,
        style: par_term_emu_core_rust::cursor::CursorStyle,
    ) {
        if self.cursor_pos != pos || self.cursor_opacity != opacity || self.cursor_style != style {
            self.dirty_rows[self.cursor_pos.1.min(self.rows - 1)] = true;
            self.cursor_pos = pos;
            self.cursor_opacity = opacity;
            self.cursor_style = style;
            self.dirty_rows[self.cursor_pos.1.min(self.rows - 1)] = true;

            // Compute cursor overlay for beam/underline styles
            use par_term_emu_core_rust::cursor::CursorStyle;
            self.cursor_overlay = if opacity > 0.0 {
                let col = pos.0;
                let row = pos.1;
                let x0 = (self.window_padding + col as f32 * self.cell_width).round();
                let x1 = (self.window_padding + (col + 1) as f32 * self.cell_width).round();
                let y0 = (self.window_padding
                    + self.content_offset_y
                    + row as f32 * self.cell_height)
                    .round();
                let y1 = (self.window_padding
                    + self.content_offset_y
                    + (row + 1) as f32 * self.cell_height)
                    .round();

                match style {
                    CursorStyle::SteadyBlock | CursorStyle::BlinkingBlock => None,
                    CursorStyle::SteadyBar | CursorStyle::BlinkingBar => Some(BackgroundInstance {
                        position: [
                            x0 / self.config.width as f32 * 2.0 - 1.0,
                            1.0 - (y0 / self.config.height as f32 * 2.0),
                        ],
                        size: [
                            2.0 / self.config.width as f32 * 2.0,
                            (y1 - y0) / self.config.height as f32 * 2.0,
                        ],
                        color: [
                            self.cursor_color[0],
                            self.cursor_color[1],
                            self.cursor_color[2],
                            opacity,
                        ],
                    }),
                    CursorStyle::SteadyUnderline | CursorStyle::BlinkingUnderline => {
                        Some(BackgroundInstance {
                            position: [
                                x0 / self.config.width as f32 * 2.0 - 1.0,
                                1.0 - ((y1 - 2.0) / self.config.height as f32 * 2.0),
                            ],
                            size: [
                                (x1 - x0) / self.config.width as f32 * 2.0,
                                2.0 / self.config.height as f32 * 2.0,
                            ],
                            color: [
                                self.cursor_color[0],
                                self.cursor_color[1],
                                self.cursor_color[2],
                                opacity,
                            ],
                        })
                    }
                }
            } else {
                None
            };
        }
    }

    pub fn clear_cursor(&mut self) {
        self.update_cursor(self.cursor_pos, 0.0, self.cursor_style);
    }

    /// Update cursor color
    pub fn update_cursor_color(&mut self, color: [u8; 3]) {
        self.cursor_color = [
            color[0] as f32 / 255.0,
            color[1] as f32 / 255.0,
            color[2] as f32 / 255.0,
        ];
        self.dirty_rows[self.cursor_pos.1.min(self.rows - 1)] = true;
    }

    /// Set whether cursor should be hidden when cursor shader is active
    pub fn set_cursor_hidden_for_shader(&mut self, hidden: bool) {
        if self.cursor_hidden_for_shader != hidden {
            self.cursor_hidden_for_shader = hidden;
            self.dirty_rows[self.cursor_pos.1.min(self.rows - 1)] = true;
        }
    }

    pub fn update_scrollbar(
        &mut self,
        scroll_offset: usize,
        visible_lines: usize,
        total_lines: usize,
    ) {
        self.scrollbar.update(
            &self.queue,
            scroll_offset,
            visible_lines,
            total_lines,
            self.config.width,
            self.config.height,
        );
    }

    pub fn set_visual_bell_intensity(&mut self, intensity: f32) {
        self.visual_bell_intensity = intensity;
    }

    pub fn update_opacity(&mut self, opacity: f32) {
        self.window_opacity = opacity;
        // update_bg_image_uniforms() multiplies bg_image_opacity by window_opacity,
        // so both images and solid colors respect window transparency
        self.update_bg_image_uniforms();
    }

    /// Set whether transparency affects only default background cells.
    /// When true, non-default (colored) backgrounds remain opaque for readability.
    pub fn set_transparency_affects_only_default_background(&mut self, value: bool) {
        if self.transparency_affects_only_default_background != value {
            log::info!(
                "transparency_affects_only_default_background: {} -> {} (window_opacity={})",
                self.transparency_affects_only_default_background,
                value,
                self.window_opacity
            );
            self.transparency_affects_only_default_background = value;
            // Mark all rows dirty to re-render with new transparency behavior
            self.dirty_rows.fill(true);
        }
    }

    /// Set whether text should always be rendered at full opacity.
    /// When true, text remains opaque regardless of window transparency settings.
    pub fn set_keep_text_opaque(&mut self, value: bool) {
        if self.keep_text_opaque != value {
            log::info!(
                "keep_text_opaque: {} -> {} (window_opacity={}, transparency_affects_only_default_bg={})",
                self.keep_text_opaque,
                value,
                self.window_opacity,
                self.transparency_affects_only_default_background
            );
            self.keep_text_opaque = value;
            // Mark all rows dirty to re-render with new text opacity behavior
            self.dirty_rows.fill(true);
        }
    }

    /// Update scale factor and recalculate all font metrics and cell dimensions.
    /// This is called when the window is dragged between displays with different DPIs.
    pub fn update_scale_factor(&mut self, scale_factor: f64) {
        let new_scale = scale_factor as f32;

        // Skip if scale factor hasn't changed
        if (self.scale_factor - new_scale).abs() < f32::EPSILON {
            return;
        }

        log::info!(
            "Recalculating font metrics for scale factor change: {} -> {}",
            self.scale_factor,
            new_scale
        );

        self.scale_factor = new_scale;

        // Recalculate font_size_pixels based on new scale factor
        let platform_dpi = if cfg!(target_os = "macos") {
            72.0
        } else {
            96.0
        };
        let base_font_pixels = self.base_font_size * platform_dpi / 72.0;
        self.font_size_pixels = (base_font_pixels * new_scale).max(1.0);

        // Re-extract font metrics at new scale
        let (font_ascent, font_descent, font_leading, char_advance) = {
            let primary_font = self.font_manager.get_font(0).unwrap();
            let metrics = primary_font.metrics(&[]);
            let scale = self.font_size_pixels / metrics.units_per_em as f32;
            let glyph_id = primary_font.charmap().map('m');
            let advance = primary_font.glyph_metrics(&[]).advance_width(glyph_id) * scale;
            (
                metrics.ascent * scale,
                metrics.descent * scale,
                metrics.leading * scale,
                advance,
            )
        };

        self.font_ascent = font_ascent;
        self.font_descent = font_descent;
        self.font_leading = font_leading;
        self.char_advance = char_advance;

        // Recalculate cell dimensions
        let natural_line_height = font_ascent + font_descent + font_leading;
        self.cell_height = (natural_line_height * self.line_spacing).max(1.0);
        self.cell_width = (char_advance * self.char_spacing).max(1.0);

        log::info!(
            "New cell dimensions: {}x{} (font_size_pixels: {})",
            self.cell_width,
            self.cell_height,
            self.font_size_pixels
        );

        // Clear glyph cache - glyphs need to be re-rasterized at new DPI
        self.clear_glyph_cache();

        // Mark all rows as dirty to force re-rendering
        self.dirty_rows.fill(true);
    }

    #[allow(dead_code)]
    pub fn update_window_padding(&mut self, padding: f32) -> Option<(usize, usize)> {
        if (self.window_padding - padding).abs() > f32::EPSILON {
            self.window_padding = padding;
            let size = (self.config.width, self.config.height);
            return Some(self.resize(size.0, size.1));
        }
        None
    }

    pub fn update_scrollbar_appearance(
        &mut self,
        width: f32,
        thumb_color: [f32; 4],
        track_color: [f32; 4],
    ) {
        self.scrollbar
            .update_appearance(width, thumb_color, track_color);
    }

    pub fn update_scrollbar_position(&mut self, position: &str) {
        self.scrollbar.update_position(position);
    }

    pub fn scrollbar_contains_point(&self, x: f32, y: f32) -> bool {
        self.scrollbar.contains_point(x, y)
    }

    pub fn scrollbar_thumb_bounds(&self) -> Option<(f32, f32)> {
        self.scrollbar.thumb_bounds()
    }

    pub fn scrollbar_track_contains_x(&self, x: f32) -> bool {
        self.scrollbar.track_contains_x(x)
    }

    pub fn scrollbar_mouse_y_to_scroll_offset(&self, mouse_y: f32) -> Option<usize> {
        self.scrollbar.mouse_y_to_scroll_offset(mouse_y)
    }

    pub fn reconfigure_surface(&mut self) {
        self.surface.configure(&self.device, &self.config);
    }

    /// Get the list of supported present modes for this surface
    #[allow(dead_code)]
    pub fn supported_present_modes(&self) -> &[wgpu::PresentMode] {
        &self.supported_present_modes
    }

    /// Check if a vsync mode is supported
    pub fn is_vsync_mode_supported(&self, mode: crate::config::VsyncMode) -> bool {
        self.supported_present_modes
            .contains(&mode.to_present_mode())
    }

    /// Update the vsync mode. Returns the actual mode applied (may differ if requested mode unsupported).
    /// Also returns whether the mode was changed.
    pub fn update_vsync_mode(
        &mut self,
        mode: crate::config::VsyncMode,
    ) -> (crate::config::VsyncMode, bool) {
        let requested = mode.to_present_mode();
        let current = self.config.present_mode;

        // Determine the actual mode to use
        let actual = if self.supported_present_modes.contains(&requested) {
            requested
        } else {
            log::warn!(
                "Requested present mode {:?} not supported, falling back to Fifo",
                requested
            );
            wgpu::PresentMode::Fifo
        };

        // Only reconfigure if the mode actually changed
        if actual != current {
            self.config.present_mode = actual;
            self.surface.configure(&self.device, &self.config);
            log::info!("VSync mode changed to {:?}", actual);
        }

        // Convert back to VsyncMode for return
        let actual_vsync = match actual {
            wgpu::PresentMode::Immediate => crate::config::VsyncMode::Immediate,
            wgpu::PresentMode::Mailbox => crate::config::VsyncMode::Mailbox,
            wgpu::PresentMode::Fifo | wgpu::PresentMode::FifoRelaxed => {
                crate::config::VsyncMode::Fifo
            }
            _ => crate::config::VsyncMode::Fifo,
        };

        (actual_vsync, actual != current)
    }

    /// Get the current vsync mode
    #[allow(dead_code)]
    pub fn current_vsync_mode(&self) -> crate::config::VsyncMode {
        match self.config.present_mode {
            wgpu::PresentMode::Immediate => crate::config::VsyncMode::Immediate,
            wgpu::PresentMode::Mailbox => crate::config::VsyncMode::Mailbox,
            wgpu::PresentMode::Fifo | wgpu::PresentMode::FifoRelaxed => {
                crate::config::VsyncMode::Fifo
            }
            _ => crate::config::VsyncMode::Fifo,
        }
    }

    #[allow(dead_code)]
    pub fn update_graphics(
        &mut self,
        _graphics: &[par_term_emu_core_rust::graphics::TerminalGraphic],
        _scroll_offset: usize,
        _scrollback_len: usize,
        _visible_lines: usize,
    ) -> Result<()> {
        Ok(())
    }
}
