use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::window::Window;

use crate::font_manager::FontManager;
use crate::scrollbar::Scrollbar;
use crate::text_shaper::ShapingOptions;

/// Vertex for cell rendering
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 2],
    tex_coords: [f32; 2],
}

/// Instance data for background rendering
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct BackgroundInstance {
    position: [f32; 2],
    size: [f32; 2],
    color: [f32; 4],
}

/// Instance data for text rendering
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct TextInstance {
    position: [f32; 2],
    size: [f32; 2],
    tex_offset: [f32; 2],
    tex_size: [f32; 2],
    color: [f32; 4],
    is_colored: u32, // 1 for emoji/colored glyphs, 0 for regular text
}

/// A single terminal cell
#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    pub grapheme: String,
    pub fg_color: [u8; 4],
    pub bg_color: [u8; 4],
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub hyperlink_id: Option<u32>,
    pub wide_char: bool,
    pub wide_char_spacer: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            grapheme: " ".to_string(),
            fg_color: [255, 255, 255, 255],
            bg_color: [0, 0, 0, 0],
            bold: false,
            italic: false,
            underline: false,
            strikethrough: false,
            hyperlink_id: None,
            wide_char: false,
            wide_char_spacer: false,
        }
    }
}

/// Glyph info for atlas
#[derive(Clone, Debug)]
struct GlyphInfo {
    #[allow(dead_code)]
    key: u64,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    #[allow(dead_code)]
    bearing_x: f32,
    #[allow(dead_code)]
    bearing_y: f32,
    is_colored: bool,
    prev: Option<u64>,
    next: Option<u64>,
}

/// Row cache entry
struct RowCacheEntry {}

pub struct CellRenderer {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,

    // Pipelines
    bg_pipeline: wgpu::RenderPipeline,
    text_pipeline: wgpu::RenderPipeline,
    bg_image_pipeline: wgpu::RenderPipeline,
    #[allow(dead_code)]
    visual_bell_pipeline: wgpu::RenderPipeline,

    // Buffers
    vertex_buffer: wgpu::Buffer,
    bg_instance_buffer: wgpu::Buffer,
    text_instance_buffer: wgpu::Buffer,
    bg_image_uniform_buffer: wgpu::Buffer,
    #[allow(dead_code)]
    visual_bell_uniform_buffer: wgpu::Buffer,

    // Bind groups
    text_bind_group: wgpu::BindGroup,
    #[allow(dead_code)]
    text_bind_group_layout: wgpu::BindGroupLayout,
    bg_image_bind_group: Option<wgpu::BindGroup>,
    bg_image_bind_group_layout: wgpu::BindGroupLayout,
    #[allow(dead_code)]
    visual_bell_bind_group: wgpu::BindGroup,

    // Glyph atlas
    atlas_texture: wgpu::Texture,
    #[allow(dead_code)]
    atlas_view: wgpu::TextureView,
    glyph_cache: HashMap<u64, GlyphInfo>,
    lru_head: Option<u64>,
    lru_tail: Option<u64>,
    atlas_next_x: u32,
    atlas_next_y: u32,
    atlas_row_height: u32,

    // Grid state
    cols: usize,
    rows: usize,
    cell_width: f32,
    cell_height: f32,
    window_padding: f32,
    #[allow(dead_code)]
    scale_factor: f32,

    // Components
    font_manager: FontManager,
    scrollbar: Scrollbar,

    // Dynamic state
    cells: Vec<Cell>,
    dirty_rows: Vec<bool>,
    row_cache: Vec<Option<RowCacheEntry>>,
    cursor_pos: (usize, usize),
    cursor_opacity: f32,
    cursor_style: par_term_emu_core_rust::cursor::CursorStyle,
    /// Separate cursor instance for beam/underline styles (rendered as overlay)
    cursor_overlay: Option<BackgroundInstance>,
    /// Cursor color [R, G, B] as floats (0.0-1.0)
    cursor_color: [f32; 3],
    visual_bell_intensity: f32,
    window_opacity: f32,
    background_color: [f32; 4],

    // Metrics
    font_ascent: f32,
    font_descent: f32,
    font_leading: f32,
    font_size_pixels: f32,

    // Background image
    bg_image_texture: Option<wgpu::Texture>,
    bg_image_mode: crate::config::BackgroundImageMode,
    bg_image_opacity: f32,

    // Metrics
    max_bg_instances: usize,
    max_text_instances: usize,

    // CPU-side instance buffers for incremental updates
    bg_instances: Vec<BackgroundInstance>,
    text_instances: Vec<TextInstance>,

    // Shaping options
    #[allow(dead_code)]
    enable_text_shaping: bool,
    enable_ligatures: bool,
    enable_kerning: bool,
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

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: vsync_mode.to_present_mode(),
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let scale_factor = window.scale_factor() as f32;

        // Standard DPI for the platform
        // macOS typically uses 72 DPI for points, Windows and most Linux use 96 DPI
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

        // Extract font metrics for better baseline alignment
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

        // Shaders
        let bg_shader = device.create_shader_module(wgpu::include_wgsl!("shaders/cell_bg.wgsl"));
        let text_shader =
            device.create_shader_module(wgpu::include_wgsl!("shaders/cell_text.wgsl"));
        let bg_image_shader =
            device.create_shader_module(wgpu::include_wgsl!("shaders/background_image.wgsl"));

        // Vertex buffer
        let vertices = [
            Vertex {
                position: [0.0, 0.0],
                tex_coords: [0.0, 0.0],
            },
            Vertex {
                position: [1.0, 0.0],
                tex_coords: [1.0, 0.0],
            },
            Vertex {
                position: [0.0, 1.0],
                tex_coords: [0.0, 1.0],
            },
            Vertex {
                position: [1.0, 1.0],
                tex_coords: [1.0, 1.0],
            },
        ];
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vertex buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // Background pipeline
        let bg_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("bg pipeline layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let bg_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("bg pipeline"),
            layout: Some(&bg_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &bg_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2],
                    },
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<BackgroundInstance>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &wgpu::vertex_attr_array![2 => Float32x2, 3 => Float32x2, 4 => Float32x4],
                    },
                ],
            },
            fragment: Some(wgpu::FragmentState {
                module: &bg_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Atlas
        let atlas_size = 2048;
        let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("atlas texture"),
            size: wgpu::Extent3d {
                width: atlas_size,
                height: atlas_size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let text_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("text bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let text_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("text bind group"),
            layout: &text_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&atlas_sampler),
                },
            ],
        });

        let text_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("text pipeline layout"),
            bind_group_layouts: &[&text_bind_group_layout],
            push_constant_ranges: &[],
        });

        let text_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("text pipeline"),
            layout: Some(&text_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &text_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2],
                    },
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<TextInstance>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &wgpu::vertex_attr_array![
                            2 => Float32x2,
                            3 => Float32x2,
                            4 => Float32x2,
                            5 => Float32x2,
                            6 => Float32x4,
                            7 => Uint32
                        ],
                    },
                ],
            },
            fragment: Some(wgpu::FragmentState {
                module: &text_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Background image
        let bg_image_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("bg image bind group layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let bg_image_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("bg image pipeline layout"),
                bind_group_layouts: &[&bg_image_bind_group_layout],
                push_constant_ranges: &[],
            });

        let bg_image_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("bg image pipeline"),
            layout: Some(&bg_image_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &bg_image_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &bg_image_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let bg_image_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bg image uniform buffer"),
            size: 64, // Sufficient for basic uniforms
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Visual bell
        let visual_bell_shader =
            device.create_shader_module(wgpu::include_wgsl!("shaders/cell_bg.wgsl"));
        let visual_bell_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("visual bell bind group layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let visual_bell_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("visual bell pipeline layout"),
                bind_group_layouts: &[&visual_bell_bind_group_layout],
                push_constant_ranges: &[],
            });

        let visual_bell_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("visual bell pipeline"),
            layout: Some(&visual_bell_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &visual_bell_shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2],
                    },
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<BackgroundInstance>() as wgpu::BufferAddress,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &wgpu::vertex_attr_array![2 => Float32x2, 3 => Float32x2, 4 => Float32x4],
                    },
                ],
            },
            fragment: Some(wgpu::FragmentState {
                module: &visual_bell_shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let visual_bell_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("visual bell uniform buffer"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let visual_bell_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("visual bell bind group"),
            layout: &visual_bell_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: visual_bell_uniform_buffer.as_entire_binding(),
            }],
        });

        // Initialize instance buffers (+1 for cursor overlay)
        let max_bg_instances = cols * rows + 1;
        let max_text_instances = cols * rows * 2; // Extra for shaped text
        let bg_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bg instance buffer"),
            size: (max_bg_instances * std::mem::size_of::<BackgroundInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let text_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("text instance buffer"),
            size: (max_text_instances * std::mem::size_of::<TextInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut renderer = Self {
            device,
            queue,
            surface,
            config,
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
            cursor_color: [1.0, 1.0, 1.0], // Default white
            visual_bell_intensity: 0.0,
            window_opacity,
            background_color: [
                background_color[0] as f32 / 255.0,
                background_color[1] as f32 / 255.0,
                background_color[2] as f32 / 255.0,
                1.0,
            ],
            font_ascent,
            font_descent,
            font_leading,
            font_size_pixels,
            bg_image_texture: None,
            bg_image_mode: background_image_mode,
            bg_image_opacity: background_image_opacity,
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
        };

        if let Some(path) = background_image_path {
            renderer.load_background_image(path)?;
        }

        Ok(renderer)
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
    pub fn grid_size(&self) -> (usize, usize) {
        (self.cols, self.rows)
    }

    pub fn resize(&mut self, width: u32, height: u32) -> (usize, usize) {
        if width == 0 || height == 0 {
            return (self.cols, self.rows);
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);

        let available_width = (width as f32 - self.window_padding * 2.0).max(0.0);
        let available_height = (height as f32 - self.window_padding * 2.0).max(0.0);
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
        self.max_bg_instances = self.cols * self.rows + 1; // +1 for cursor overlay
        self.max_text_instances = self.cols * self.rows * 2;
        self.bg_instance_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bg instance buffer"),
            size: (self.max_bg_instances * std::mem::size_of::<BackgroundInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.text_instance_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("text instance buffer"),
            size: (self.max_text_instances * std::mem::size_of::<TextInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

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
                let y0 = (self.window_padding + row as f32 * self.cell_height).round();
                let y1 = (self.window_padding + (row + 1) as f32 * self.cell_height).round();

                match style {
                    // Block cursor: handled in cell background, no overlay needed
                    CursorStyle::SteadyBlock | CursorStyle::BlinkingBlock => None,
                    // Beam/Bar cursor: thin vertical line on the left (2 pixels wide)
                    CursorStyle::SteadyBar | CursorStyle::BlinkingBar => {
                        Some(BackgroundInstance {
                            position: [
                                x0 / self.config.width as f32 * 2.0 - 1.0,
                                1.0 - (y0 / self.config.height as f32 * 2.0),
                            ],
                            size: [
                                2.0 / self.config.width as f32 * 2.0,
                                (y1 - y0) / self.config.height as f32 * 2.0,
                            ],
                            color: [self.cursor_color[0], self.cursor_color[1], self.cursor_color[2], opacity],
                        })
                    }
                    // Underline cursor: thin horizontal line at the bottom (2 pixels tall)
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
                            color: [self.cursor_color[0], self.cursor_color[1], self.cursor_color[2], opacity],
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
        // Mark cursor row as dirty to redraw with new color
        self.dirty_rows[self.cursor_pos.1.min(self.rows - 1)] = true;
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
        self.update_bg_image_uniforms();
    }

    pub fn update_scale_factor(&mut self, scale_factor: f64) {
        self.scale_factor = scale_factor as f32;
    }

    pub fn update_window_padding(&mut self, padding: f32) -> Option<(usize, usize)> {
        if (self.window_padding - padding).abs() > f32::EPSILON {
            self.window_padding = padding;
            let size = (self.config.width, self.config.height);
            return Some(self.resize(size.0, size.1));
        }
        None
    }

    pub fn set_background_image(
        &mut self,
        path: Option<&str>,
        mode: crate::config::BackgroundImageMode,
        opacity: f32,
    ) {
        self.bg_image_mode = mode;
        self.bg_image_opacity = opacity;
        if let Some(p) = path {
            let _ = self.load_background_image(p);
        } else {
            self.bg_image_texture = None;
            self.bg_image_bind_group = None;
        }
        self.update_bg_image_uniforms();
    }

    pub fn update_background_image_opacity(&mut self, opacity: f32) {
        self.bg_image_opacity = opacity;
        self.update_bg_image_uniforms();
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

    pub fn clear_glyph_cache(&mut self) {
        self.glyph_cache.clear();
        self.lru_head = None;
        self.lru_tail = None;
        self.atlas_next_x = 0;
        self.atlas_next_y = 0;
        self.atlas_row_height = 0;
        self.dirty_rows.fill(true);
    }

    fn lru_remove(&mut self, key: u64) {
        let info = self.glyph_cache.get(&key).unwrap();
        let prev = info.prev;
        let next = info.next;

        if let Some(p) = prev {
            self.glyph_cache.get_mut(&p).unwrap().next = next;
        } else {
            self.lru_head = next;
        }

        if let Some(n) = next {
            self.glyph_cache.get_mut(&n).unwrap().prev = prev;
        } else {
            self.lru_tail = prev;
        }
    }

    fn lru_push_front(&mut self, key: u64) {
        let next = self.lru_head;
        if let Some(n) = next {
            self.glyph_cache.get_mut(&n).unwrap().prev = Some(key);
        } else {
            self.lru_tail = Some(key);
        }

        let info = self.glyph_cache.get_mut(&key).unwrap();
        info.prev = None;
        info.next = next;
        self.lru_head = Some(key);
    }

    fn load_background_image(&mut self, path: &str) -> Result<()> {
        let img = image::open(path)?.to_rgba8();
        let (width, height) = img.dimensions();
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("bg image"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &img,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        self.bg_image_bind_group =
            Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("bg image bind group"),
                layout: &self.bg_image_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: self.bg_image_uniform_buffer.as_entire_binding(),
                    },
                ],
            }));
        self.bg_image_texture = Some(texture);
        self.update_bg_image_uniforms();
        Ok(())
    }

    fn update_bg_image_uniforms(&mut self) {
        let mut data = [0.0f32; 16];
        data[0] = self.bg_image_opacity;
        data[1] = self.window_opacity;
        data[2] = self.bg_image_mode as u32 as f32;
        data[3] = self.config.width as f32;
        data[4] = self.config.height as f32;
        self.queue.write_buffer(
            &self.bg_image_uniform_buffer,
            0,
            bytemuck::cast_slice(&data),
        );
    }

    pub fn render(&mut self, show_scrollbar: bool) -> Result<wgpu::SurfaceTexture> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.build_instance_buffers()?;

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: self.background_color[0] as f64,
                            g: self.background_color[1] as f64,
                            b: self.background_color[2] as f64,
                            a: self.window_opacity as f64,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            if let Some(ref bg_bind_group) = self.bg_image_bind_group {
                render_pass.set_pipeline(&self.bg_image_pipeline);
                render_pass.set_bind_group(0, bg_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                render_pass.draw(0..4, 0..1);
            }

            render_pass.set_pipeline(&self.bg_pipeline);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.bg_instance_buffer.slice(..));
            render_pass.draw(0..4, 0..self.max_bg_instances as u32);

            render_pass.set_pipeline(&self.text_pipeline);
            render_pass.set_bind_group(0, &self.text_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.text_instance_buffer.slice(..));
            render_pass.draw(0..4, 0..self.max_text_instances as u32);

            if show_scrollbar {
                self.scrollbar.render(&mut render_pass);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        Ok(output)
    }

    pub fn render_to_texture(
        &mut self,
        target_view: &wgpu::TextureView,
    ) -> Result<wgpu::SurfaceTexture> {
        let output = self.surface.get_current_texture()?;
        self.build_instance_buffers()?;

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render to texture encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            if let Some(ref bg_bind_group) = self.bg_image_bind_group {
                render_pass.set_pipeline(&self.bg_image_pipeline);
                render_pass.set_bind_group(0, bg_bind_group, &[]);
                render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                render_pass.draw(0..4, 0..1);
            }

            render_pass.set_pipeline(&self.bg_pipeline);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.bg_instance_buffer.slice(..));
            render_pass.draw(0..4, 0..self.max_bg_instances as u32);

            render_pass.set_pipeline(&self.text_pipeline);
            render_pass.set_bind_group(0, &self.text_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.text_instance_buffer.slice(..));
            render_pass.draw(0..4, 0..self.max_text_instances as u32);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        Ok(output)
    }

    pub fn render_overlays(
        &mut self,
        surface_texture: &wgpu::SurfaceTexture,
        show_scrollbar: bool,
    ) -> Result<()> {
        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("overlay encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("overlay pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
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

            if show_scrollbar {
                self.scrollbar.render(&mut render_pass);
            }

            if self.visual_bell_intensity > 0.0 {
                // Visual bell logic
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        Ok(())
    }

    fn build_instance_buffers(&mut self) -> Result<()> {
        let _shaping_options = ShapingOptions {
            enable_ligatures: self.enable_ligatures,
            enable_kerning: self.enable_kerning,
            ..Default::default()
        };

        for row in 0..self.rows {
            if self.dirty_rows[row] || self.row_cache[row].is_none() {
                let start = row * self.cols;
                let end = (row + 1) * self.cols;
                let row_cells = &self.cells[start..end];

                let mut row_bg = Vec::with_capacity(self.cols);
                let mut row_text = Vec::with_capacity(self.cols);

                // Background
                for (col, cell) in row_cells.iter().enumerate() {
                    let is_default_bg =
                        (cell.bg_color[0] as f32 / 255.0 - self.background_color[0]).abs() < 0.001
                            && (cell.bg_color[1] as f32 / 255.0 - self.background_color[1]).abs()
                                < 0.001
                            && (cell.bg_color[2] as f32 / 255.0 - self.background_color[2]).abs()
                                < 0.001;

                    let has_cursor = self.cursor_opacity > 0.0
                        && self.cursor_pos.1 == row
                        && self.cursor_pos.0 == col;

                    if is_default_bg && !has_cursor {
                        row_bg.push(BackgroundInstance {
                            position: [0.0, 0.0],
                            size: [0.0, 0.0],
                            color: [0.0, 0.0, 0.0, 0.0],
                        });
                        continue;
                    }

                    let bg_color = [
                        cell.bg_color[0] as f32 / 255.0,
                        cell.bg_color[1] as f32 / 255.0,
                        cell.bg_color[2] as f32 / 255.0,
                        cell.bg_color[3] as f32 / 255.0,
                    ];

                    let x0 = (self.window_padding + col as f32 * self.cell_width).round();
                    let x1 = (self.window_padding + (col + 1) as f32 * self.cell_width).round();
                    let y0 = (self.window_padding + row as f32 * self.cell_height).round();
                    let y1 = (self.window_padding + (row + 1) as f32 * self.cell_height).round();

                    // Geometric cursor rendering based on cursor style
                    // For block cursor, blend into cell background; for others, add overlay later
                    let mut final_bg_color = bg_color;
                    if has_cursor && self.cursor_opacity > 0.0 {
                        use par_term_emu_core_rust::cursor::CursorStyle;
                        match self.cursor_style {
                            // Block cursor: blend cursor color into background
                            CursorStyle::SteadyBlock | CursorStyle::BlinkingBlock => {
                                for (bg, &cursor) in
                                    final_bg_color.iter_mut().take(3).zip(&self.cursor_color)
                                {
                                    *bg = *bg * (1.0 - self.cursor_opacity)
                                        + cursor * self.cursor_opacity;
                                }
                                final_bg_color[3] = final_bg_color[3].max(self.cursor_opacity);
                            }
                            // Beam/Bar and Underline: handled separately in cursor_instance
                            _ => {}
                        }
                    }

                    // Add cell background
                    row_bg.push(BackgroundInstance {
                        position: [
                            x0 / self.config.width as f32 * 2.0 - 1.0,
                            1.0 - (y0 / self.config.height as f32 * 2.0),
                        ],
                        size: [
                            (x1 - x0) / self.config.width as f32 * 2.0,
                            (y1 - y0) / self.config.height as f32 * 2.0,
                        ],
                        color: final_bg_color,
                    });
                }

                // Text
                let mut x_offset = 0.0;
                let cell_data: Vec<(String, bool, bool, [u8; 4], bool, bool)> = row_cells
                    .iter()
                    .map(|c| {
                        (
                            c.grapheme.clone(),
                            c.bold,
                            c.italic,
                            c.fg_color,
                            c.wide_char_spacer,
                            c.wide_char,
                        )
                    })
                    .collect();

                // Dynamic baseline calculation based on font metrics
                let natural_line_height = self.font_ascent + self.font_descent + self.font_leading;
                let vertical_padding = (self.cell_height - natural_line_height).max(0.0) / 2.0;
                let baseline_y_unrounded = self.window_padding
                    + (row as f32 * self.cell_height)
                    + vertical_padding
                    + self.font_ascent;

                for (grapheme, bold, italic, fg_color, is_spacer, is_wide) in cell_data {
                    if is_spacer || grapheme == " " {
                        x_offset += self.cell_width;
                        continue;
                    }

                    let chars: Vec<char> = grapheme.chars().collect();
                    #[allow(clippy::collapsible_if)]
                    if let Some(ch) = chars.first() {
                        if let Some((font_idx, glyph_id)) =
                            self.font_manager.find_glyph(*ch, bold, italic)
                        {
                            let cache_key = ((font_idx as u64) << 32) | (glyph_id as u64);
                            let info = if self.glyph_cache.contains_key(&cache_key) {
                                // Move to front of LRU
                                self.lru_remove(cache_key);
                                self.lru_push_front(cache_key);
                                self.glyph_cache.get(&cache_key).unwrap().clone()
                            } else if let Some(raster) = self.rasterize_glyph(font_idx, glyph_id) {
                                let info = self.upload_glyph(cache_key, &raster);
                                self.glyph_cache.insert(cache_key, info.clone());
                                self.lru_push_front(cache_key);
                                info
                            } else {
                                x_offset += self.cell_width;
                                continue;
                            };

                            let char_w = if is_wide {
                                self.cell_width * 2.0
                            } else {
                                self.cell_width
                            };
                            let x0 = (self.window_padding + x_offset).round();
                            let x1 = (self.window_padding + x_offset + char_w).round();
                            let y0 = (self.window_padding + row as f32 * self.cell_height).round();
                            let y1 =
                                (self.window_padding + (row + 1) as f32 * self.cell_height).round();

                            let cell_w = x1 - x0;
                            let cell_h = y1 - y0;

                            let scale_x = cell_w / char_w;
                            let scale_y = cell_h / self.cell_height;

                            // Position glyph relative to snapped cell top-left
                            let baseline_offset = baseline_y_unrounded
                                - (self.window_padding + row as f32 * self.cell_height);
                            let mut glyph_left = x0 + (info.bearing_x * scale_x).round();
                            let mut glyph_top =
                                y0 + ((baseline_offset - info.bearing_y) * scale_y).round();

                            let mut render_w = info.width as f32 * scale_x;
                            let mut render_h = info.height as f32 * scale_y;

                            // Special case: for box drawing and block elements, ensure they fill the cell
                            // if they are close to the edges to avoid 1px gaps.
                            let char_code = *ch as u32;
                            let is_block_char = (0x2500..=0x259F).contains(&char_code)
                                || (0xE0A0..=0xE0D4).contains(&char_code)
                                || (0x25A0..=0x25FF).contains(&char_code); // Geometric shapes

                            if is_block_char {
                                // Snap to left/right cell boundaries
                                if (glyph_left - x0).abs() < 3.0 {
                                    let right = glyph_left + render_w;
                                    glyph_left = x0;
                                    render_w = (right - x0).max(render_w);
                                }
                                if (x1 - (glyph_left + render_w)).abs() < 3.0 {
                                    render_w = x1 - glyph_left;
                                }

                                // Snap to top/bottom cell boundaries
                                if (glyph_top - y0).abs() < 3.0 {
                                    let bottom = glyph_top + render_h;
                                    glyph_top = y0;
                                    render_h = (bottom - y0).max(render_h);
                                }
                                if (y1 - (glyph_top + render_h)).abs() < 3.0 {
                                    render_h = y1 - glyph_top;
                                }

                                // For half-blocks and quadrants, also snap to middle boundaries
                                let cx = (x0 + x1) / 2.0;
                                let cy = (y0 + y1) / 2.0;

                                // Vertical middle snap
                                if (glyph_top + render_h - cy).abs() < 2.0 {
                                    render_h = cy - glyph_top;
                                } else if (glyph_top - cy).abs() < 2.0 {
                                    let bottom = glyph_top + render_h;
                                    glyph_top = cy;
                                    render_h = bottom - cy;
                                }

                                // Horizontal middle snap
                                if (glyph_left + render_w - cx).abs() < 2.0 {
                                    render_w = cx - glyph_left;
                                } else if (glyph_left - cx).abs() < 2.0 {
                                    let right = glyph_left + render_w;
                                    glyph_left = cx;
                                    render_w = right - cx;
                                }
                            }

                            row_text.push(TextInstance {
                                position: [
                                    glyph_left / self.config.width as f32 * 2.0 - 1.0,
                                    1.0 - (glyph_top / self.config.height as f32 * 2.0),
                                ],
                                size: [
                                    render_w / self.config.width as f32 * 2.0,
                                    render_h / self.config.height as f32 * 2.0,
                                ],
                                tex_offset: [info.x as f32 / 2048.0, info.y as f32 / 2048.0],
                                tex_size: [info.width as f32 / 2048.0, info.height as f32 / 2048.0],
                                color: [
                                    fg_color[0] as f32 / 255.0,
                                    fg_color[1] as f32 / 255.0,
                                    fg_color[2] as f32 / 255.0,
                                    fg_color[3] as f32 / 255.0,
                                ],
                                is_colored: if info.is_colored { 1 } else { 0 },
                            });
                        }
                    }
                    x_offset += self.cell_width;
                }

                // Update CPU-side buffers
                let bg_start = row * self.cols;
                self.bg_instances[bg_start..bg_start + self.cols].copy_from_slice(&row_bg);

                let text_start = row * self.cols * 2;
                // Clear row text segment first
                for i in 0..(self.cols * 2) {
                    self.text_instances[text_start + i].size = [0.0, 0.0];
                }
                // Copy new text instances
                let text_count = row_text.len().min(self.cols * 2);
                self.text_instances[text_start..text_start + text_count]
                    .copy_from_slice(&row_text[..text_count]);

                // Update GPU-side buffers incrementally
                self.queue.write_buffer(
                    &self.bg_instance_buffer,
                    (bg_start * std::mem::size_of::<BackgroundInstance>()) as u64,
                    bytemuck::cast_slice(&row_bg),
                );
                self.queue.write_buffer(
                    &self.text_instance_buffer,
                    (text_start * std::mem::size_of::<TextInstance>()) as u64,
                    bytemuck::cast_slice(
                        &self.text_instances[text_start..text_start + self.cols * 2],
                    ),
                );

                self.row_cache[row] = Some(RowCacheEntry {});
                self.dirty_rows[row] = false;
            }
        }

        // Write cursor overlay to the last slot of bg_instances (for beam/underline cursors)
        let cursor_overlay_index = self.cols * self.rows;
        let cursor_overlay_instance = self.cursor_overlay.unwrap_or(BackgroundInstance {
            position: [0.0, 0.0],
            size: [0.0, 0.0],
            color: [0.0, 0.0, 0.0, 0.0],
        });
        self.bg_instances[cursor_overlay_index] = cursor_overlay_instance;
        self.queue.write_buffer(
            &self.bg_instance_buffer,
            (cursor_overlay_index * std::mem::size_of::<BackgroundInstance>()) as u64,
            bytemuck::cast_slice(&[cursor_overlay_instance]),
        );

        Ok(())
    }

    fn rasterize_glyph(&self, font_idx: usize, glyph_id: u16) -> Option<RasterizedGlyph> {
        let font = self.font_manager.get_font(font_idx)?;
        // Use swash to rasterize
        use swash::scale::image::Content;
        use swash::scale::{Render, ScaleContext};
        let mut context = ScaleContext::new();
        let mut scaler = context
            .builder(*font)
            .size(self.font_size_pixels)
            .hint(true)
            .build();
        let image = Render::new(&[
            swash::scale::Source::ColorOutline(0),
            swash::scale::Source::ColorBitmap(swash::scale::StrikeWith::BestFit),
            swash::scale::Source::Outline,
        ])
        .render(&mut scaler, glyph_id)?;

        let mut pixels = Vec::with_capacity(image.data.len() * 4);
        let is_colored = match image.content {
            Content::Color => {
                pixels.extend_from_slice(&image.data);
                true
            }
            Content::Mask => {
                for &mask in &image.data {
                    pixels.push(255);
                    pixels.push(255);
                    pixels.push(255);
                    pixels.push(mask);
                }
                false
            }
            _ => return None,
        };

        Some(RasterizedGlyph {
            width: image.placement.width,
            height: image.placement.height,
            bearing_x: image.placement.left as f32,
            bearing_y: image.placement.top as f32,
            pixels,
            is_colored,
        })
    }

    fn upload_glyph(&mut self, _key: u64, raster: &RasterizedGlyph) -> GlyphInfo {
        let padding = 2;
        if self.atlas_next_x + raster.width + padding > 2048 {
            self.atlas_next_x = 0;
            self.atlas_next_y += self.atlas_row_height + padding;
            self.atlas_row_height = 0;
        }

        if self.atlas_next_y + raster.height + padding > 2048 {
            self.clear_glyph_cache();
        }

        let info = GlyphInfo {
            key: _key,
            x: self.atlas_next_x,
            y: self.atlas_next_y,
            width: raster.width,
            height: raster.height,
            bearing_x: raster.bearing_x,
            bearing_y: raster.bearing_y,
            is_colored: raster.is_colored,
            prev: None,
            next: None,
        };

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.atlas_texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: info.x,
                    y: info.y,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &raster.pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * raster.width),
                rows_per_image: Some(raster.height),
            },
            wgpu::Extent3d {
                width: raster.width,
                height: raster.height,
                depth_or_array_layers: 1,
            },
        );

        self.atlas_next_x += raster.width + padding;
        self.atlas_row_height = self.atlas_row_height.max(raster.height);

        info
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

    #[allow(dead_code)]
    pub fn update_background_image_opacity_only(&mut self, opacity: f32) {
        self.bg_image_opacity = opacity;
        self.update_bg_image_uniforms();
    }
}

struct RasterizedGlyph {
    width: u32,
    height: u32,
    #[allow(dead_code)]
    bearing_x: f32,
    #[allow(dead_code)]
    bearing_y: f32,
    pixels: Vec<u8>,
    is_colored: bool,
}
