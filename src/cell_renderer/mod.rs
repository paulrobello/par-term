use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::window::Window;

use crate::font_manager::FontManager;
use crate::scrollbar::Scrollbar;

pub mod atlas;
pub mod background;
pub mod render;
pub mod types;
pub use types::*;

pub struct CellRenderer {
    pub(crate) device: Arc<wgpu::Device>,
    pub(crate) queue: Arc<wgpu::Queue>,
    pub(crate) surface: wgpu::Surface<'static>,
    pub(crate) config: wgpu::SurfaceConfiguration,

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
    pub(crate) visual_bell_intensity: f32,
    pub(crate) window_opacity: f32,
    pub(crate) background_color: [f32; 4],

    // Metrics
    pub(crate) font_ascent: f32,
    pub(crate) font_descent: f32,
    pub(crate) font_leading: f32,
    pub(crate) font_size_pixels: f32,

    // Background image
    pub(crate) bg_image_texture: Option<wgpu::Texture>,
    pub(crate) bg_image_mode: crate::config::BackgroundImageMode,
    pub(crate) bg_image_opacity: f32,

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
        let bg_shader = device.create_shader_module(wgpu::include_wgsl!("../shaders/cell_bg.wgsl"));
        let text_shader =
            device.create_shader_module(wgpu::include_wgsl!("../shaders/cell_text.wgsl"));
        let bg_image_shader =
            device.create_shader_module(wgpu::include_wgsl!("../shaders/background_image.wgsl"));

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
            device.create_shader_module(wgpu::include_wgsl!("../shaders/cell_bg.wgsl"));
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


