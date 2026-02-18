use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use winit::window::Window;

use crate::font_manager::FontManager;
use crate::scrollbar::Scrollbar;
use par_term_config::SeparatorMark;

pub mod atlas;
pub mod background;
pub mod block_chars;
pub mod pipeline;
pub mod render;
pub mod types;
// Re-export public types for external use
pub use types::{Cell, PaneViewport};
// Re-export internal types for use within the cell_renderer module
pub(crate) use types::{BackgroundInstance, GlyphInfo, RowCacheEntry, TextInstance};

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
    /// Vertical offset for terminal content (e.g., tab bar at top).
    /// Content is rendered starting at y = window_padding + content_offset_y.
    pub(crate) content_offset_y: f32,
    /// Horizontal offset for terminal content (e.g., tab bar on left).
    /// Content is rendered starting at x = window_padding + content_offset_x.
    pub(crate) content_offset_x: f32,
    /// Bottom inset for terminal content (e.g., tab bar at bottom).
    /// Reduces available height without shifting content vertically.
    pub(crate) content_inset_bottom: f32,
    /// Right inset for terminal content (e.g., AI Inspector panel).
    /// Reduces available width without shifting content horizontally.
    pub(crate) content_inset_right: f32,
    /// Additional bottom inset from egui panels (status bar, tmux bar).
    /// This is added to content_inset_bottom for scrollbar bounds only,
    /// since egui panels already claim space before wgpu rendering.
    pub(crate) egui_bottom_inset: f32,
    /// Additional right inset from egui panels (AI Inspector).
    /// This is added to content_inset_right for scrollbar bounds only,
    /// since egui panels already claim space before wgpu rendering.
    pub(crate) egui_right_inset: f32,
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
    /// Text color under block cursor [R, G, B] as floats (0.0-1.0), or None for auto-contrast
    pub(crate) cursor_text_color: Option<[f32; 3]>,
    /// Hide cursor when cursor shader is active (let shader handle cursor rendering)
    pub(crate) cursor_hidden_for_shader: bool,
    /// Whether the window is currently focused (for unfocused cursor style)
    pub(crate) is_focused: bool,

    // Cursor enhancement settings
    /// Enable cursor guide (horizontal line at cursor row)
    pub(crate) cursor_guide_enabled: bool,
    /// Cursor guide color [R, G, B, A] as floats (0.0-1.0)
    pub(crate) cursor_guide_color: [f32; 4],
    /// Enable cursor shadow
    pub(crate) cursor_shadow_enabled: bool,
    /// Cursor shadow color [R, G, B, A] as floats (0.0-1.0)
    pub(crate) cursor_shadow_color: [f32; 4],
    /// Cursor shadow offset in pixels [x, y]
    pub(crate) cursor_shadow_offset: [f32; 2],
    /// Cursor shadow blur radius (not fully supported yet, but stores config)
    #[allow(dead_code)]
    pub(crate) cursor_shadow_blur: f32,
    /// Cursor boost (glow) intensity (0.0-1.0)
    pub(crate) cursor_boost: f32,
    /// Cursor boost glow color [R, G, B] as floats (0.0-1.0)
    pub(crate) cursor_boost_color: [f32; 3],
    /// Unfocused cursor style (hollow, same, hidden)
    pub(crate) unfocused_cursor_style: crate::config::UnfocusedCursorStyle,
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

    /// Cache of per-pane background textures keyed by image path
    pub(crate) pane_bg_cache: HashMap<String, background::PaneBackgroundEntry>,

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

    // Font rendering options
    /// Enable anti-aliasing for font rendering
    pub(crate) font_antialias: bool,
    /// Enable hinting for font rendering
    pub(crate) font_hinting: bool,
    /// Thin strokes mode for font rendering
    pub(crate) font_thin_strokes: crate::config::ThinStrokesMode,
    /// Minimum contrast ratio for text against background (WCAG standard)
    /// 1.0 = disabled, 4.5 = WCAG AA, 7.0 = WCAG AAA
    pub(crate) minimum_contrast: f32,

    // Solid white pixel in atlas for geometric block rendering
    pub(crate) solid_pixel_offset: (u32, u32),

    // Transparency mode
    /// When true, only default background cells are transparent.
    /// Non-default (colored) backgrounds remain opaque for readability.
    pub(crate) transparency_affects_only_default_background: bool,

    /// When true, text is always rendered at full opacity regardless of window transparency.
    pub(crate) keep_text_opaque: bool,

    // Command separator line settings
    /// Whether to render separator lines between commands
    pub(crate) command_separator_enabled: bool,
    /// Thickness of separator lines in pixels
    pub(crate) command_separator_thickness: f32,
    /// Opacity of separator lines (0.0-1.0)
    pub(crate) command_separator_opacity: f32,
    /// Whether to color separator lines by exit code
    pub(crate) command_separator_exit_color: bool,
    /// Custom separator color [R, G, B] as floats (0.0-1.0)
    pub(crate) command_separator_color: [f32; 3],
    /// Visible separator marks for current frame: (screen_row, exit_code, custom_color)
    pub(crate) visible_separator_marks: Vec<SeparatorMark>,
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
        font_antialias: bool,
        font_hinting: bool,
        font_thin_strokes: crate::config::ThinStrokesMode,
        minimum_contrast: f32,
        vsync_mode: crate::config::VsyncMode,
        power_preference: crate::config::PowerPreference,
        window_opacity: f32,
        background_color: [u8; 3],
        background_image_path: Option<&str>,
        background_image_mode: crate::config::BackgroundImageMode,
        background_image_opacity: f32,
    ) -> Result<Self> {
        // Platform-specific backend selection for better VM compatibility
        // Windows: Use DX12 (Vulkan may not work in VMs like Parallels)
        // macOS: Use Metal (native)
        // Linux: Try Vulkan first, fall back to GL for VM compatibility
        // Platform-specific backend selection for better VM compatibility
        // Windows: Use DX12 (Vulkan may not work in VMs like Parallels)
        // macOS: Use Metal (native)
        // Linux: Try Vulkan first, fall back to GL for VM compatibility
        #[cfg(target_os = "windows")]
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::DX12,
            ..Default::default()
        });
        #[cfg(target_os = "macos")]
        let instance = wgpu::Instance::default();
        #[cfg(target_os = "linux")]
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN | wgpu::Backends::GL,
            ..Default::default()
        });
        let surface = instance.create_surface(window.clone())?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: power_preference.to_wgpu(),
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
            Arc::clone(&device),
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
        let max_bg_instances = cols * rows + 10 + rows; // Extra slots for cursor overlays + separator lines
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
            content_offset_x: 0.0,
            content_inset_bottom: 0.0,
            content_inset_right: 0.0,
            egui_bottom_inset: 0.0,
            egui_right_inset: 0.0,
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
            cursor_text_color: None,
            cursor_hidden_for_shader: false,
            is_focused: true,
            cursor_guide_enabled: false,
            cursor_guide_color: [1.0, 1.0, 1.0, 0.08],
            cursor_shadow_enabled: false,
            cursor_shadow_color: [0.0, 0.0, 0.0, 0.5],
            cursor_shadow_offset: [2.0, 2.0],
            cursor_shadow_blur: 3.0,
            cursor_boost: 0.0,
            cursor_boost_color: [1.0, 1.0, 1.0],
            unfocused_cursor_style: crate::config::UnfocusedCursorStyle::default(),
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
            pane_bg_cache: HashMap::new(),
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
            font_antialias,
            font_hinting,
            font_thin_strokes,
            minimum_contrast: minimum_contrast.clamp(1.0, 21.0),
            solid_pixel_offset: (0, 0),
            transparency_affects_only_default_background: false,
            keep_text_opaque: true,
            command_separator_enabled: false,
            command_separator_thickness: 1.0,
            command_separator_opacity: 0.4,
            command_separator_exit_color: true,
            command_separator_color: [0.5, 0.5, 0.5],
            visible_separator_marks: Vec::new(),
        };

        // Upload a solid white 2x2 pixel block to the atlas for geometric block rendering
        renderer.upload_solid_pixel();

        log::info!(
            "CellRenderer::new: background_image_path={:?}",
            background_image_path
        );
        if let Some(path) = background_image_path {
            // Handle missing background image gracefully - don't crash, just log and continue
            if let Err(e) = renderer.load_background_image(path) {
                log::warn!(
                    "Could not load background image '{}': {} - continuing without background image",
                    path,
                    e
                );
            }
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
    /// Set the vertical content offset (e.g., tab bar height at top).
    /// Returns Some((cols, rows)) if grid size changed, None otherwise.
    pub fn set_content_offset_y(&mut self, offset: f32) -> Option<(usize, usize)> {
        if (self.content_offset_y - offset).abs() > f32::EPSILON {
            self.content_offset_y = offset;
            let size = (self.config.width, self.config.height);
            return Some(self.resize(size.0, size.1));
        }
        None
    }
    pub fn content_offset_x(&self) -> f32 {
        self.content_offset_x
    }
    /// Set the horizontal content offset (e.g., tab bar on left).
    /// Returns Some((cols, rows)) if grid size changed, None otherwise.
    pub fn set_content_offset_x(&mut self, offset: f32) -> Option<(usize, usize)> {
        if (self.content_offset_x - offset).abs() > f32::EPSILON {
            self.content_offset_x = offset;
            let size = (self.config.width, self.config.height);
            return Some(self.resize(size.0, size.1));
        }
        None
    }
    pub fn content_inset_bottom(&self) -> f32 {
        self.content_inset_bottom
    }
    /// Set the bottom content inset (e.g., tab bar at bottom).
    /// Returns Some((cols, rows)) if grid size changed, None otherwise.
    pub fn set_content_inset_bottom(&mut self, inset: f32) -> Option<(usize, usize)> {
        if (self.content_inset_bottom - inset).abs() > f32::EPSILON {
            self.content_inset_bottom = inset;
            let size = (self.config.width, self.config.height);
            return Some(self.resize(size.0, size.1));
        }
        None
    }
    pub fn content_inset_right(&self) -> f32 {
        self.content_inset_right
    }
    /// Set the right content inset (e.g., AI Inspector panel).
    /// Returns Some((cols, rows)) if grid size changed, None otherwise.
    pub fn set_content_inset_right(&mut self, inset: f32) -> Option<(usize, usize)> {
        if (self.content_inset_right - inset).abs() > f32::EPSILON {
            crate::debug_info!(
                "SCROLLBAR",
                "set_content_inset_right: {:.1} -> {:.1} (physical px)",
                self.content_inset_right,
                inset
            );
            self.content_inset_right = inset;
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

        let available_width = (width as f32
            - self.window_padding * 2.0
            - self.content_offset_x
            - self.content_inset_right)
            .max(0.0);
        let available_height = (height as f32
            - self.window_padding * 2.0
            - self.content_offset_y
            - self.content_inset_bottom
            - self.egui_bottom_inset)
            .max(0.0);
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
        self.max_bg_instances = self.cols * self.rows + 10 + self.rows; // Extra slots for cursor overlays + separator lines
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
                let x0 =
                    (self.window_padding + self.content_offset_x + col as f32 * self.cell_width)
                        .round();
                let x1 = (self.window_padding
                    + self.content_offset_x
                    + (col + 1) as f32 * self.cell_width)
                    .round();
                let y0 =
                    (self.window_padding + self.content_offset_y + row as f32 * self.cell_height)
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

    /// Update cursor text color (color of text under block cursor)
    pub fn update_cursor_text_color(&mut self, color: Option<[u8; 3]>) {
        self.cursor_text_color = color.map(|c| {
            [
                c[0] as f32 / 255.0,
                c[1] as f32 / 255.0,
                c[2] as f32 / 255.0,
            ]
        });
        self.dirty_rows[self.cursor_pos.1.min(self.rows - 1)] = true;
    }

    /// Set whether cursor should be hidden when cursor shader is active
    pub fn set_cursor_hidden_for_shader(&mut self, hidden: bool) {
        if self.cursor_hidden_for_shader != hidden {
            self.cursor_hidden_for_shader = hidden;
            self.dirty_rows[self.cursor_pos.1.min(self.rows - 1)] = true;
        }
    }

    /// Set window focus state (affects unfocused cursor rendering)
    pub fn set_focused(&mut self, focused: bool) {
        if self.is_focused != focused {
            self.is_focused = focused;
            self.dirty_rows[self.cursor_pos.1.min(self.rows - 1)] = true;
        }
    }

    /// Update cursor guide settings
    pub fn update_cursor_guide(&mut self, enabled: bool, color: [u8; 4]) {
        self.cursor_guide_enabled = enabled;
        self.cursor_guide_color = [
            color[0] as f32 / 255.0,
            color[1] as f32 / 255.0,
            color[2] as f32 / 255.0,
            color[3] as f32 / 255.0,
        ];
        if enabled {
            self.dirty_rows[self.cursor_pos.1.min(self.rows - 1)] = true;
        }
    }

    /// Update cursor shadow settings
    pub fn update_cursor_shadow(
        &mut self,
        enabled: bool,
        color: [u8; 4],
        offset: [f32; 2],
        blur: f32,
    ) {
        self.cursor_shadow_enabled = enabled;
        self.cursor_shadow_color = [
            color[0] as f32 / 255.0,
            color[1] as f32 / 255.0,
            color[2] as f32 / 255.0,
            color[3] as f32 / 255.0,
        ];
        self.cursor_shadow_offset = offset;
        self.cursor_shadow_blur = blur;
        if enabled {
            self.dirty_rows[self.cursor_pos.1.min(self.rows - 1)] = true;
        }
    }

    /// Update cursor boost settings
    pub fn update_cursor_boost(&mut self, intensity: f32, color: [u8; 3]) {
        self.cursor_boost = intensity.clamp(0.0, 1.0);
        self.cursor_boost_color = [
            color[0] as f32 / 255.0,
            color[1] as f32 / 255.0,
            color[2] as f32 / 255.0,
        ];
        if intensity > 0.0 {
            self.dirty_rows[self.cursor_pos.1.min(self.rows - 1)] = true;
        }
    }

    /// Update unfocused cursor style
    pub fn update_unfocused_cursor_style(&mut self, style: crate::config::UnfocusedCursorStyle) {
        self.unfocused_cursor_style = style;
        if !self.is_focused {
            self.dirty_rows[self.cursor_pos.1.min(self.rows - 1)] = true;
        }
    }

    pub fn update_scrollbar(
        &mut self,
        scroll_offset: usize,
        visible_lines: usize,
        total_lines: usize,
        marks: &[crate::scrollback_metadata::ScrollbackMark],
    ) {
        let right_inset = self.content_inset_right + self.egui_right_inset;
        self.scrollbar.update(
            &self.queue,
            scroll_offset,
            visible_lines,
            total_lines,
            self.config.width,
            self.config.height,
            self.content_offset_y,
            self.content_inset_bottom + self.egui_bottom_inset,
            right_inset,
            marks,
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

    /// Update command separator settings from config
    pub fn update_command_separator(
        &mut self,
        enabled: bool,
        thickness: f32,
        opacity: f32,
        exit_color: bool,
        color: [u8; 3],
    ) {
        self.command_separator_enabled = enabled;
        self.command_separator_thickness = thickness;
        self.command_separator_opacity = opacity;
        self.command_separator_exit_color = exit_color;
        self.command_separator_color = [
            color[0] as f32 / 255.0,
            color[1] as f32 / 255.0,
            color[2] as f32 / 255.0,
        ];
    }

    /// Set the visible separator marks for the current frame
    pub fn set_separator_marks(&mut self, marks: Vec<SeparatorMark>) {
        self.visible_separator_marks = marks;
    }

    /// Compute separator color based on exit code and settings
    fn separator_color(
        &self,
        exit_code: Option<i32>,
        custom_color: Option<(u8, u8, u8)>,
        opacity_mult: f32,
    ) -> [f32; 4] {
        let alpha = self.command_separator_opacity * opacity_mult;
        // Custom color from trigger marks takes priority
        if let Some((r, g, b)) = custom_color {
            return [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, alpha];
        }
        if self.command_separator_exit_color {
            match exit_code {
                Some(0) => [0.3, 0.75, 0.3, alpha],   // Green for success
                Some(_) => [0.85, 0.25, 0.25, alpha], // Red for failure
                None => [0.5, 0.5, 0.5, alpha],       // Gray for unknown
            }
        } else {
            [
                self.command_separator_color[0],
                self.command_separator_color[1],
                self.command_separator_color[2],
                alpha,
            ]
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

    /// Find a scrollbar mark at the given mouse position for tooltip display.
    /// Returns the mark if mouse is within `tolerance` pixels of a mark.
    pub fn scrollbar_mark_at_position(
        &self,
        mouse_x: f32,
        mouse_y: f32,
        tolerance: f32,
    ) -> Option<&crate::scrollback_metadata::ScrollbackMark> {
        self.scrollbar.mark_at_position(mouse_x, mouse_y, tolerance)
    }

    pub fn reconfigure_surface(&mut self) {
        self.surface.configure(&self.device, &self.config);
    }

    /// Update font anti-aliasing setting
    /// Returns true if the setting changed (requiring glyph cache clear)
    pub fn update_font_antialias(&mut self, enabled: bool) -> bool {
        if self.font_antialias != enabled {
            self.font_antialias = enabled;
            self.clear_glyph_cache();
            self.dirty_rows.fill(true);
            true
        } else {
            false
        }
    }

    /// Update font hinting setting
    /// Returns true if the setting changed (requiring glyph cache clear)
    pub fn update_font_hinting(&mut self, enabled: bool) -> bool {
        if self.font_hinting != enabled {
            self.font_hinting = enabled;
            self.clear_glyph_cache();
            self.dirty_rows.fill(true);
            true
        } else {
            false
        }
    }

    /// Update thin strokes mode
    /// Returns true if the setting changed (requiring glyph cache clear)
    pub fn update_font_thin_strokes(&mut self, mode: crate::config::ThinStrokesMode) -> bool {
        if self.font_thin_strokes != mode {
            self.font_thin_strokes = mode;
            self.clear_glyph_cache();
            self.dirty_rows.fill(true);
            true
        } else {
            false
        }
    }

    /// Update minimum contrast ratio
    /// Returns true if the setting changed (requiring redraw)
    pub fn update_minimum_contrast(&mut self, ratio: f32) -> bool {
        // Clamp to valid range: 1.0 (disabled) to 21.0 (max possible contrast)
        let ratio = ratio.clamp(1.0, 21.0);
        if (self.minimum_contrast - ratio).abs() > 0.001 {
            self.minimum_contrast = ratio;
            self.dirty_rows.fill(true);
            true
        } else {
            false
        }
    }

    /// Adjust foreground color to meet minimum contrast ratio against background
    /// Uses WCAG luminance formula for accurate contrast calculation.
    /// Returns the adjusted color [R, G, B, A] with preserved alpha.
    pub(crate) fn ensure_minimum_contrast(&self, fg: [f32; 4], bg: [f32; 4]) -> [f32; 4] {
        // If minimum_contrast is 1.0 (disabled) or less, no adjustment needed
        if self.minimum_contrast <= 1.0 {
            return fg;
        }

        // Calculate luminance using WCAG formula
        fn luminance(color: [f32; 4]) -> f32 {
            let r = color[0].powf(2.2);
            let g = color[1].powf(2.2);
            let b = color[2].powf(2.2);
            0.2126 * r + 0.7152 * g + 0.0722 * b
        }

        fn contrast_ratio(l1: f32, l2: f32) -> f32 {
            let (lighter, darker) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
            (lighter + 0.05) / (darker + 0.05)
        }

        let fg_lum = luminance(fg);
        let bg_lum = luminance(bg);
        let current_ratio = contrast_ratio(fg_lum, bg_lum);

        // If already meets minimum contrast, return unchanged
        if current_ratio >= self.minimum_contrast {
            return fg;
        }

        // Determine if we need to lighten or darken the foreground
        // If background is dark, lighten fg; if light, darken fg
        let bg_is_dark = bg_lum < 0.5;

        // Binary search for the minimum adjustment needed
        let mut low = 0.0f32;
        let mut high = 1.0f32;

        for _ in 0..20 {
            // 20 iterations gives ~1/1_000_000 precision
            let mid = (low + high) / 2.0;

            let adjusted = if bg_is_dark {
                // Lighten: mix with white
                [
                    fg[0] + (1.0 - fg[0]) * mid,
                    fg[1] + (1.0 - fg[1]) * mid,
                    fg[2] + (1.0 - fg[2]) * mid,
                    fg[3],
                ]
            } else {
                // Darken: mix with black
                [
                    fg[0] * (1.0 - mid),
                    fg[1] * (1.0 - mid),
                    fg[2] * (1.0 - mid),
                    fg[3],
                ]
            };

            let adjusted_lum = luminance(adjusted);
            let new_ratio = contrast_ratio(adjusted_lum, bg_lum);

            if new_ratio >= self.minimum_contrast {
                high = mid;
            } else {
                low = mid;
            }
        }

        // Apply the final adjustment
        if bg_is_dark {
            [
                fg[0] + (1.0 - fg[0]) * high,
                fg[1] + (1.0 - fg[1]) * high,
                fg[2] + (1.0 - fg[2]) * high,
                fg[3],
            ]
        } else {
            [
                fg[0] * (1.0 - high),
                fg[1] * (1.0 - high),
                fg[2] * (1.0 - high),
                fg[3],
            ]
        }
    }

    /// Check if thin strokes should be applied based on current mode and context
    pub(crate) fn should_use_thin_strokes(&self) -> bool {
        use crate::config::ThinStrokesMode;

        // Check if we're on a Retina/HiDPI display (scale factor > 1.5)
        let is_retina = self.scale_factor > 1.5;

        // Check if background is dark (average < 128)
        let bg_brightness =
            (self.background_color[0] + self.background_color[1] + self.background_color[2]) / 3.0;
        let is_dark_background = bg_brightness < 0.5;

        match self.font_thin_strokes {
            ThinStrokesMode::Never => false,
            ThinStrokesMode::Always => true,
            ThinStrokesMode::RetinaOnly => is_retina,
            ThinStrokesMode::DarkBackgroundsOnly => is_dark_background,
            ThinStrokesMode::RetinaDarkBackgroundsOnly => is_retina && is_dark_background,
        }
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
