use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use winit::window::Window;

use crate::scrollbar::Scrollbar;
use par_term_config::{SeparatorMark, color_u8_to_f32_a};
use par_term_fonts::font_manager::FontManager;

pub mod atlas;
pub mod background;
pub mod block_chars;
mod cursor;
mod font;
mod instance_buffers;
mod instance_builders;
mod layout;
pub(crate) mod pane_render;
pub mod pipeline;
pub mod render;
mod settings;
mod surface;
pub mod types;
// Re-export public types for external use
pub use types::{Cell, PaneViewport};
pub(crate) use pane_render::PaneRenderViewParams;
// Re-export internal types for use within the cell_renderer module
pub(crate) use types::{BackgroundInstance, GlyphInfo, RowCacheEntry, TextInstance};
// Re-export instance buffer constants so mod.rs can reference them
pub(crate) use instance_buffers::{CURSOR_OVERLAY_SLOTS, TEXT_INSTANCES_PER_CELL};
// Re-export extracted sub-module types for use within this module
pub(crate) use cursor::CursorState;
pub(crate) use font::FontState;
pub(crate) use layout::GridLayout;

/// Physical DPI on macOS (points-based at 72 ppi).
pub(crate) const MACOS_PLATFORM_DPI: f32 = 72.0;

/// Physical DPI on non-macOS platforms (screen pixels at 96 ppi).
pub(crate) const DEFAULT_PLATFORM_DPI: f32 = 96.0;

/// Reference DPI used in the font-size conversion formula.
/// Font sizes are specified in typographic points at 72 ppi.
pub(crate) const FONT_REFERENCE_DPI: f32 = 72.0;

/// Size (width and height) of the solid white pixel block uploaded to the glyph atlas.
/// A 2×2 block provides better sampling behaviour than a single texel at borders.
const SOLID_PIXEL_SIZE: u32 = 2;

/// Pixel padding added around each glyph in the atlas to prevent bilinear bleed.
pub(crate) const ATLAS_GLYPH_PADDING: u32 = 2;

/// Maximum frame latency hint passed to the wgpu surface configuration.
/// Controls how many frames may be queued ahead of the display; 2 balances
/// throughput against input latency.
const SURFACE_FRAME_LATENCY: u32 = 2;

/// Default cursor guide line opacity.
/// A very low value keeps the guide visible without overpowering text.
const DEFAULT_GUIDE_OPACITY: f32 = 0.08;

/// Default cursor shadow alpha component.
const DEFAULT_SHADOW_ALPHA: f32 = 0.5;

/// Default cursor shadow offset in pixels (x and y).
const DEFAULT_SHADOW_OFFSET_PX: f32 = 2.0;

/// Default cursor shadow blur radius in pixels.
const DEFAULT_SHADOW_BLUR_PX: f32 = 3.0;

/// GPU render pipelines and their associated bind group layouts.
pub(crate) struct GpuPipelines {
    pub(crate) bg_pipeline: wgpu::RenderPipeline,
    pub(crate) text_pipeline: wgpu::RenderPipeline,
    pub(crate) bg_image_pipeline: wgpu::RenderPipeline,
    #[allow(dead_code)] // GPU resource: visual bell rendering (infrastructure in progress)
    pub(crate) visual_bell_pipeline: wgpu::RenderPipeline,
    pub(crate) text_bind_group: wgpu::BindGroup,
    #[allow(dead_code)] // GPU lifetime: must outlive bind groups created from this layout
    pub(crate) text_bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) bg_image_bind_group: Option<wgpu::BindGroup>,
    pub(crate) bg_image_bind_group_layout: wgpu::BindGroupLayout,
    #[allow(dead_code)] // GPU resource: visual bell rendering (infrastructure in progress)
    pub(crate) visual_bell_bind_group: wgpu::BindGroup,
}

/// GPU vertex, instance, and uniform buffers with capacity tracking.
pub(crate) struct GpuBuffers {
    pub(crate) vertex_buffer: wgpu::Buffer,
    pub(crate) bg_instance_buffer: wgpu::Buffer,
    pub(crate) text_instance_buffer: wgpu::Buffer,
    pub(crate) bg_image_uniform_buffer: wgpu::Buffer,
    #[allow(dead_code)] // GPU resource: visual bell rendering (infrastructure in progress)
    pub(crate) visual_bell_uniform_buffer: wgpu::Buffer,
    /// Maximum capacity of the bg_instance_buffer (GPU buffer size)
    pub(crate) max_bg_instances: usize,
    /// Maximum capacity of the text_instance_buffer (GPU buffer size)
    pub(crate) max_text_instances: usize,
    /// Actual number of background instances written (used for draw calls)
    pub(crate) actual_bg_instances: usize,
    /// Actual number of text instances written (used for draw calls)
    pub(crate) actual_text_instances: usize,
}

/// Glyph atlas texture, cache, and LRU eviction state.
pub(crate) struct GlyphAtlas {
    pub(crate) atlas_texture: wgpu::Texture,
    #[allow(dead_code)] // GPU lifetime: must outlive text_bind_group which references this view
    pub(crate) atlas_view: wgpu::TextureView,
    pub(crate) glyph_cache: HashMap<u64, GlyphInfo>,
    pub(crate) lru_head: Option<u64>,
    pub(crate) lru_tail: Option<u64>,
    pub(crate) atlas_next_x: u32,
    pub(crate) atlas_next_y: u32,
    pub(crate) atlas_row_height: u32,
    /// Actual atlas size (may be smaller than preferred on devices with low texture limits)
    pub(crate) atlas_size: u32,
    /// Solid white pixel offset in atlas for geometric block rendering
    pub(crate) solid_pixel_offset: (u32, u32),
}

/// Background image/solid-color texture state and per-pane cache.
pub(crate) struct BackgroundImageState {
    pub(crate) bg_image_texture: Option<wgpu::Texture>,
    pub(crate) bg_image_mode: par_term_config::BackgroundImageMode,
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
    /// Cache of per-pane uniform buffers and bind groups keyed by image path.
    /// Reused across frames via `queue.write_buffer()` to avoid per-frame GPU allocations.
    pub(crate) pane_bg_uniform_cache: HashMap<String, background::PaneBgUniformEntry>,
}

/// Command separator line settings and visible marks.
pub(crate) struct SeparatorConfig {
    /// Whether to render separator lines between commands
    pub(crate) enabled: bool,
    /// Thickness of separator lines in pixels
    pub(crate) thickness: f32,
    /// Opacity of separator lines (0.0-1.0)
    pub(crate) opacity: f32,
    /// Whether to color separator lines by exit code
    pub(crate) exit_color: bool,
    /// Custom separator color [R, G, B] as floats (0.0-1.0)
    pub(crate) color: [f32; 3],
    /// Visible separator marks for current frame: (screen_row, exit_code, custom_color)
    pub(crate) visible_marks: Vec<SeparatorMark>,
}

pub struct CellRenderer {
    // Core wgpu state
    pub(crate) device: Arc<wgpu::Device>,
    pub(crate) queue: Arc<wgpu::Queue>,
    pub(crate) surface: wgpu::Surface<'static>,
    pub(crate) config: wgpu::SurfaceConfiguration,
    /// Supported present modes for this surface (for vsync mode validation)
    pub(crate) supported_present_modes: Vec<wgpu::PresentMode>,

    // Sub-structs grouping related GPU and rendering state
    pub(crate) pipelines: GpuPipelines,
    pub(crate) buffers: GpuBuffers,
    pub(crate) atlas: GlyphAtlas,
    pub(crate) grid: GridLayout,
    pub(crate) cursor: CursorState,
    pub(crate) font: FontState,
    pub(crate) bg_state: BackgroundImageState,
    pub(crate) separator: SeparatorConfig,

    /// Display scale factor (accessed directly from renderer module)
    pub(crate) scale_factor: f32,

    // Components
    pub(crate) font_manager: FontManager,
    pub(crate) scrollbar: Scrollbar,

    // Dynamic state
    pub(crate) cells: Vec<Cell>,
    pub(crate) dirty_rows: Vec<bool>,
    pub(crate) row_cache: Vec<Option<RowCacheEntry>>,

    // Rendering state
    pub(crate) visual_bell_intensity: f32,
    pub(crate) window_opacity: f32,
    pub(crate) background_color: [f32; 4],
    /// Whether the window is currently focused (for unfocused cursor style)
    pub(crate) is_focused: bool,

    // CPU-side instance buffers for incremental updates
    pub(crate) bg_instances: Vec<BackgroundInstance>,
    pub(crate) text_instances: Vec<TextInstance>,

    // Scratch buffers reused across dirty-row iterations (avoids per-row Vec allocation)
    pub(crate) scratch_row_bg: Vec<BackgroundInstance>,
    pub(crate) scratch_row_text: Vec<TextInstance>,

    // Transparency mode
    /// When true, only default background cells are transparent.
    /// Non-default (colored) backgrounds remain opaque for readability.
    pub(crate) transparency_affects_only_default_background: bool,
    /// When true, text is always rendered at full opacity regardless of window transparency.
    pub(crate) keep_text_opaque: bool,
    /// Style for link underlines (solid or stipple)
    pub(crate) link_underline_style: par_term_config::LinkUnderlineStyle,

    /// Gutter indicator marks for current frame: (screen_row, rgba_color)
    pub(crate) gutter_indicators: Vec<(usize, [f32; 4])>,
}

/// Configuration for [`CellRenderer::new`].
///
/// Bundles all font, grid, scrollbar, and background parameters so the
/// constructor does not exceed the `clippy::too_many_arguments` threshold.
pub struct CellRendererConfig<'a> {
    pub font_family: Option<&'a str>,
    pub font_family_bold: Option<&'a str>,
    pub font_family_italic: Option<&'a str>,
    pub font_family_bold_italic: Option<&'a str>,
    pub font_ranges: &'a [par_term_config::FontRange],
    pub font_size: f32,
    pub cols: usize,
    pub rows: usize,
    pub window_padding: f32,
    pub line_spacing: f32,
    pub char_spacing: f32,
    pub scrollbar_position: &'a str,
    pub scrollbar_width: f32,
    pub scrollbar_thumb_color: [f32; 4],
    pub scrollbar_track_color: [f32; 4],
    pub enable_text_shaping: bool,
    pub enable_ligatures: bool,
    pub enable_kerning: bool,
    pub font_antialias: bool,
    pub font_hinting: bool,
    pub font_thin_strokes: par_term_config::ThinStrokesMode,
    pub minimum_contrast: f32,
    pub vsync_mode: par_term_config::VsyncMode,
    pub power_preference: par_term_config::PowerPreference,
    pub window_opacity: f32,
    pub background_color: [u8; 3],
    pub background_image_path: Option<&'a str>,
    pub background_image_mode: par_term_config::BackgroundImageMode,
    pub background_image_opacity: f32,
}

impl CellRenderer {
    pub async fn new(window: Arc<Window>, config: CellRendererConfig<'_>) -> Result<Self> {
        let CellRendererConfig {
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
            font_antialias,
            font_hinting,
            font_thin_strokes,
            minimum_contrast,
            vsync_mode,
            power_preference,
            window_opacity,
            background_color,
            background_image_path,
            background_image_mode,
            background_image_opacity,
        } = config;
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
            desired_maximum_frame_latency: SURFACE_FRAME_LATENCY,
        };
        surface.configure(&device, &config);

        let scale_factor = window.scale_factor() as f32;

        let platform_dpi = if cfg!(target_os = "macos") {
            MACOS_PLATFORM_DPI
        } else {
            DEFAULT_PLATFORM_DPI
        };

        let base_font_pixels = font_size * platform_dpi / FONT_REFERENCE_DPI;
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
            let primary_font = font_manager
                .get_font(0)
                .expect("Primary font at index 0 must exist after FontManager initialization");
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

        let (atlas_texture, atlas_view, atlas_sampler, atlas_size) =
            pipeline::create_atlas(&device);
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
        // Extra slots: CURSOR_OVERLAY_SLOTS for cursor overlays + rows for separator lines + rows for gutter indicators
        let max_bg_instances = cols * rows + CURSOR_OVERLAY_SLOTS + rows + rows;
        let max_text_instances = cols * rows * TEXT_INSTANCES_PER_CELL;
        let (bg_instance_buffer, text_instance_buffer) =
            pipeline::create_instance_buffers(&device, max_bg_instances, max_text_instances);

        let mut renderer = Self {
            device,
            queue,
            surface,
            config,
            supported_present_modes,
            pipelines: GpuPipelines {
                bg_pipeline,
                text_pipeline,
                bg_image_pipeline,
                visual_bell_pipeline,
                text_bind_group,
                text_bind_group_layout,
                bg_image_bind_group: None,
                bg_image_bind_group_layout,
                visual_bell_bind_group,
            },
            buffers: GpuBuffers {
                vertex_buffer,
                bg_instance_buffer,
                text_instance_buffer,
                bg_image_uniform_buffer,
                visual_bell_uniform_buffer,
                max_bg_instances,
                max_text_instances,
                actual_bg_instances: 0,
                actual_text_instances: 0,
            },
            atlas: GlyphAtlas {
                atlas_texture,
                atlas_view,
                glyph_cache: HashMap::new(),
                lru_head: None,
                lru_tail: None,
                atlas_next_x: 0,
                atlas_next_y: 0,
                atlas_row_height: 0,
                atlas_size,
                solid_pixel_offset: (0, 0),
            },
            grid: GridLayout {
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
            },
            cursor: CursorState {
                pos: (0, 0),
                opacity: 0.0,
                style: par_term_emu_core_rust::cursor::CursorStyle::SteadyBlock,
                overlay: None,
                color: [1.0, 1.0, 1.0],
                text_color: None,
                hidden_for_shader: false,
                guide_enabled: false,
                guide_color: [1.0, 1.0, 1.0, DEFAULT_GUIDE_OPACITY],
                shadow_enabled: false,
                shadow_color: [0.0, 0.0, 0.0, DEFAULT_SHADOW_ALPHA],
                shadow_offset: [DEFAULT_SHADOW_OFFSET_PX, DEFAULT_SHADOW_OFFSET_PX],
                shadow_blur: DEFAULT_SHADOW_BLUR_PX,
                boost: 0.0,
                boost_color: [1.0, 1.0, 1.0],
                unfocused_style: par_term_config::UnfocusedCursorStyle::default(),
            },
            font: FontState {
                base_font_size: font_size,
                line_spacing,
                char_spacing,
                font_ascent,
                font_descent,
                font_leading,
                font_size_pixels,
                char_advance,
                enable_text_shaping,
                enable_ligatures,
                enable_kerning,
                font_antialias,
                font_hinting,
                font_thin_strokes,
                minimum_contrast: minimum_contrast.clamp(1.0, 21.0),
            },
            bg_state: BackgroundImageState {
                bg_image_texture: None,
                bg_image_mode: background_image_mode,
                bg_image_opacity: background_image_opacity,
                bg_image_width: 0,
                bg_image_height: 0,
                bg_is_solid_color: false,
                solid_bg_color: [0.0, 0.0, 0.0],
                pane_bg_cache: HashMap::new(),
                pane_bg_uniform_cache: HashMap::new(),
            },
            separator: SeparatorConfig {
                enabled: false,
                thickness: 1.0,
                opacity: 0.4,
                exit_color: true,
                color: [0.5, 0.5, 0.5],
                visible_marks: Vec::new(),
            },
            scale_factor,
            font_manager,
            scrollbar,
            cells: vec![Cell::default(); cols * rows],
            dirty_rows: vec![true; rows],
            row_cache: (0..rows).map(|_| None).collect(),
            is_focused: true,
            visual_bell_intensity: 0.0,
            window_opacity,
            background_color: color_u8_to_f32_a(background_color, 1.0),
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
            transparency_affects_only_default_background: false,
            keep_text_opaque: true,
            link_underline_style: par_term_config::LinkUnderlineStyle::default(),
            gutter_indicators: Vec::new(),
            scratch_row_bg: Vec::with_capacity(cols),
            scratch_row_text: Vec::with_capacity(cols * 2),
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
        let size = SOLID_PIXEL_SIZE;
        let white_pixels: Vec<u8> = vec![255; (size * size * 4) as usize];

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.atlas.atlas_texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: self.atlas.atlas_next_x,
                    y: self.atlas.atlas_next_y,
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

        self.atlas.solid_pixel_offset = (self.atlas.atlas_next_x, self.atlas.atlas_next_y);
        self.atlas.atlas_next_x += size + ATLAS_GLYPH_PADDING;
        self.atlas.atlas_row_height = self.atlas.atlas_row_height.max(size);
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
    pub fn keep_text_opaque(&self) -> bool {
        self.keep_text_opaque
    }

    /// Update cells. Returns `true` if any row actually changed.
    pub fn update_cells(&mut self, new_cells: &[Cell]) -> bool {
        let mut changed = false;
        for row in 0..self.grid.rows {
            let start = row * self.grid.cols;
            let end = (row + 1) * self.grid.cols;
            if start < new_cells.len() && end <= new_cells.len() {
                let row_slice = &new_cells[start..end];
                if row_slice != &self.cells[start..end] {
                    self.cells[start..end].clone_from_slice(row_slice);
                    self.dirty_rows[row] = true;
                    changed = true;
                }
            }
        }
        changed
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
