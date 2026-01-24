//! GPU pipeline creation for cell renderer.
//!
//! This module contains functions for creating wgpu render pipelines
//! for backgrounds, text, background images, and visual bell.

use wgpu::*;

use super::types::{BackgroundInstance, TextInstance, Vertex};

/// Create the background pipeline for cell backgrounds
pub fn create_bg_pipeline(device: &Device, surface_format: TextureFormat) -> RenderPipeline {
    let bg_shader = device.create_shader_module(include_wgsl!("../shaders/cell_bg.wgsl"));

    let bg_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("bg pipeline layout"),
        bind_group_layouts: &[],
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("bg pipeline"),
        layout: Some(&bg_pipeline_layout),
        vertex: VertexState {
            module: &bg_shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[
                VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as BufferAddress,
                    step_mode: VertexStepMode::Vertex,
                    attributes: &vertex_attr_array![0 => Float32x2, 1 => Float32x2],
                },
                VertexBufferLayout {
                    array_stride: std::mem::size_of::<BackgroundInstance>() as BufferAddress,
                    step_mode: VertexStepMode::Instance,
                    attributes: &vertex_attr_array![2 => Float32x2, 3 => Float32x2, 4 => Float32x4],
                },
            ],
        },
        fragment: Some(FragmentState {
            module: &bg_shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(ColorTargetState {
                format: surface_format,
                blend: Some(BlendState::ALPHA_BLENDING),
                write_mask: ColorWrites::ALL,
            })],
        }),
        primitive: PrimitiveState {
            topology: PrimitiveTopology::TriangleStrip,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

/// Create the text bind group layout
pub fn create_text_bind_group_layout(device: &Device) -> BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("text bind group layout"),
        entries: &[
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: true },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

/// Create the text bind group
pub fn create_text_bind_group(
    device: &Device,
    layout: &BindGroupLayout,
    atlas_view: &TextureView,
    atlas_sampler: &Sampler,
) -> BindGroup {
    device.create_bind_group(&BindGroupDescriptor {
        label: Some("text bind group"),
        layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(atlas_view),
            },
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::Sampler(atlas_sampler),
            },
        ],
    })
}

/// Create the text pipeline for glyph rendering
pub fn create_text_pipeline(
    device: &Device,
    surface_format: TextureFormat,
    text_bind_group_layout: &BindGroupLayout,
) -> RenderPipeline {
    let text_shader = device.create_shader_module(include_wgsl!("../shaders/cell_text.wgsl"));

    let text_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("text pipeline layout"),
        bind_group_layouts: &[text_bind_group_layout],
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("text pipeline"),
        layout: Some(&text_pipeline_layout),
        vertex: VertexState {
            module: &text_shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[
                VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as BufferAddress,
                    step_mode: VertexStepMode::Vertex,
                    attributes: &vertex_attr_array![0 => Float32x2, 1 => Float32x2],
                },
                VertexBufferLayout {
                    array_stride: std::mem::size_of::<TextInstance>() as BufferAddress,
                    step_mode: VertexStepMode::Instance,
                    attributes: &vertex_attr_array![
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
        fragment: Some(FragmentState {
            module: &text_shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(ColorTargetState {
                format: surface_format,
                blend: Some(BlendState::ALPHA_BLENDING),
                write_mask: ColorWrites::ALL,
            })],
        }),
        primitive: PrimitiveState {
            topology: PrimitiveTopology::TriangleStrip,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

/// Create the background image bind group layout
pub fn create_bg_image_bind_group_layout(device: &Device) -> BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("bg image bind group layout"),
        entries: &[
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: true },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    })
}

/// Create the background image pipeline
pub fn create_bg_image_pipeline(
    device: &Device,
    surface_format: TextureFormat,
    bg_image_bind_group_layout: &BindGroupLayout,
) -> RenderPipeline {
    let bg_image_shader =
        device.create_shader_module(include_wgsl!("../shaders/background_image.wgsl"));

    let bg_image_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("bg image pipeline layout"),
        bind_group_layouts: &[bg_image_bind_group_layout],
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("bg image pipeline"),
        layout: Some(&bg_image_pipeline_layout),
        vertex: VertexState {
            module: &bg_image_shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[VertexBufferLayout {
                array_stride: std::mem::size_of::<Vertex>() as BufferAddress,
                step_mode: VertexStepMode::Vertex,
                attributes: &vertex_attr_array![0 => Float32x2, 1 => Float32x2],
            }],
        },
        fragment: Some(FragmentState {
            module: &bg_image_shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(ColorTargetState {
                format: surface_format,
                blend: Some(BlendState::ALPHA_BLENDING),
                write_mask: ColorWrites::ALL,
            })],
        }),
        primitive: PrimitiveState {
            topology: PrimitiveTopology::TriangleStrip,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}

/// Create the visual bell pipeline (reuses background shader)
pub fn create_visual_bell_pipeline(
    device: &Device,
    surface_format: TextureFormat,
) -> (RenderPipeline, BindGroup, BindGroupLayout, Buffer) {
    let visual_bell_shader = device.create_shader_module(include_wgsl!("../shaders/cell_bg.wgsl"));

    let visual_bell_bind_group_layout =
        device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("visual bell bind group layout"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

    let visual_bell_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some("visual bell pipeline layout"),
        bind_group_layouts: &[&visual_bell_bind_group_layout],
        push_constant_ranges: &[],
    });

    let visual_bell_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some("visual bell pipeline"),
        layout: Some(&visual_bell_pipeline_layout),
        vertex: VertexState {
            module: &visual_bell_shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[
                VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as BufferAddress,
                    step_mode: VertexStepMode::Vertex,
                    attributes: &vertex_attr_array![0 => Float32x2, 1 => Float32x2],
                },
                VertexBufferLayout {
                    array_stride: std::mem::size_of::<BackgroundInstance>() as BufferAddress,
                    step_mode: VertexStepMode::Instance,
                    attributes: &vertex_attr_array![2 => Float32x2, 3 => Float32x2, 4 => Float32x4],
                },
            ],
        },
        fragment: Some(FragmentState {
            module: &visual_bell_shader,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(ColorTargetState {
                format: surface_format,
                blend: Some(BlendState::ALPHA_BLENDING),
                write_mask: ColorWrites::ALL,
            })],
        }),
        primitive: PrimitiveState {
            topology: PrimitiveTopology::TriangleStrip,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    let visual_bell_uniform_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("visual bell uniform buffer"),
        size: 64,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let visual_bell_bind_group = device.create_bind_group(&BindGroupDescriptor {
        label: Some("visual bell bind group"),
        layout: &visual_bell_bind_group_layout,
        entries: &[BindGroupEntry {
            binding: 0,
            resource: visual_bell_uniform_buffer.as_entire_binding(),
        }],
    });

    (
        visual_bell_pipeline,
        visual_bell_bind_group,
        visual_bell_bind_group_layout,
        visual_bell_uniform_buffer,
    )
}

/// Create the glyph atlas texture and sampler
pub fn create_atlas(device: &Device) -> (Texture, TextureView, Sampler) {
    let atlas_size = 2048;
    let atlas_texture = device.create_texture(&TextureDescriptor {
        label: Some("atlas texture"),
        size: Extent3d {
            width: atlas_size,
            height: atlas_size,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let atlas_view = atlas_texture.create_view(&TextureViewDescriptor::default());
    let atlas_sampler = device.create_sampler(&SamplerDescriptor {
        address_mode_u: AddressMode::ClampToEdge,
        address_mode_v: AddressMode::ClampToEdge,
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        ..Default::default()
    });

    (atlas_texture, atlas_view, atlas_sampler)
}

/// Create the vertex buffer with unit quad vertices
pub fn create_vertex_buffer(device: &Device) -> Buffer {
    use wgpu::util::DeviceExt;

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

    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("vertex buffer"),
        contents: bytemuck::cast_slice(&vertices),
        usage: BufferUsages::VERTEX,
    })
}

/// Create instance buffers for backgrounds and text
pub fn create_instance_buffers(
    device: &Device,
    max_bg_instances: usize,
    max_text_instances: usize,
) -> (Buffer, Buffer) {
    let bg_instance_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("bg instance buffer"),
        size: (max_bg_instances * std::mem::size_of::<BackgroundInstance>()) as u64,
        usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let text_instance_buffer = device.create_buffer(&BufferDescriptor {
        label: Some("text instance buffer"),
        size: (max_text_instances * std::mem::size_of::<TextInstance>()) as u64,
        usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    (bg_instance_buffer, text_instance_buffer)
}

/// Create the background image uniform buffer
pub fn create_bg_image_uniform_buffer(device: &Device) -> Buffer {
    device.create_buffer(&BufferDescriptor {
        label: Some("bg image uniform buffer"),
        size: 64,
        usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}
