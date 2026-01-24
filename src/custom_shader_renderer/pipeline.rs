//! Pipeline and bind group creation for custom shader renderer.
//!
//! This module contains helpers for creating wgpu bind groups, bind group layouts,
//! and render pipelines for custom shaders.

use wgpu::*;

use super::textures::ChannelTexture;

/// Create the bind group layout for custom shaders with all 11 entries.
///
/// Layout:
/// - 0: Uniform buffer
/// - 1: iChannel0 texture (terminal content)
/// - 2: iChannel0 sampler
/// - 3: iChannel1 texture
/// - 4: iChannel1 sampler
/// - 5: iChannel2 texture
/// - 6: iChannel2 sampler
/// - 7: iChannel3 texture
/// - 8: iChannel3 sampler
/// - 9: iChannel4 texture
/// - 10: iChannel4 sampler
pub fn create_bind_group_layout(device: &Device) -> BindGroupLayout {
    device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        label: Some("Custom Shader Bind Group Layout"),
        entries: &[
            // Uniform buffer (binding 0)
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // iChannel0 texture (binding 1) - terminal content
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: true },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // iChannel0 sampler (binding 2)
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                count: None,
            },
            // iChannel1 texture (binding 3)
            BindGroupLayoutEntry {
                binding: 3,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: true },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // iChannel1 sampler (binding 4)
            BindGroupLayoutEntry {
                binding: 4,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                count: None,
            },
            // iChannel2 texture (binding 5)
            BindGroupLayoutEntry {
                binding: 5,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: true },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // iChannel2 sampler (binding 6)
            BindGroupLayoutEntry {
                binding: 6,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                count: None,
            },
            // iChannel3 texture (binding 7)
            BindGroupLayoutEntry {
                binding: 7,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: true },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // iChannel3 sampler (binding 8)
            BindGroupLayoutEntry {
                binding: 8,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                count: None,
            },
            // iChannel4 texture (binding 9)
            BindGroupLayoutEntry {
                binding: 9,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: true },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // iChannel4 sampler (binding 10)
            BindGroupLayoutEntry {
                binding: 10,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Sampler(SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

/// Create a bind group for the custom shader with all textures and uniforms.
///
/// # Arguments
/// * `device` - The wgpu device
/// * `layout` - The bind group layout
/// * `uniform_buffer` - Uniform buffer for shader parameters
/// * `intermediate_texture_view` - Terminal content texture view (iChannel0)
/// * `sampler` - Sampler for the intermediate texture
/// * `channel_textures` - Array of 4 channel textures (iChannel1-4)
pub fn create_bind_group(
    device: &Device,
    layout: &BindGroupLayout,
    uniform_buffer: &Buffer,
    intermediate_texture_view: &TextureView,
    sampler: &Sampler,
    channel_textures: &[ChannelTexture; 4],
) -> BindGroup {
    device.create_bind_group(&BindGroupDescriptor {
        label: Some("Custom Shader Bind Group"),
        layout,
        entries: &[
            BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            },
            // iChannel0 (terminal content)
            BindGroupEntry {
                binding: 1,
                resource: BindingResource::TextureView(intermediate_texture_view),
            },
            BindGroupEntry {
                binding: 2,
                resource: BindingResource::Sampler(sampler),
            },
            // iChannel1
            BindGroupEntry {
                binding: 3,
                resource: BindingResource::TextureView(&channel_textures[0].view),
            },
            BindGroupEntry {
                binding: 4,
                resource: BindingResource::Sampler(&channel_textures[0].sampler),
            },
            // iChannel2
            BindGroupEntry {
                binding: 5,
                resource: BindingResource::TextureView(&channel_textures[1].view),
            },
            BindGroupEntry {
                binding: 6,
                resource: BindingResource::Sampler(&channel_textures[1].sampler),
            },
            // iChannel3
            BindGroupEntry {
                binding: 7,
                resource: BindingResource::TextureView(&channel_textures[2].view),
            },
            BindGroupEntry {
                binding: 8,
                resource: BindingResource::Sampler(&channel_textures[2].sampler),
            },
            // iChannel4
            BindGroupEntry {
                binding: 9,
                resource: BindingResource::TextureView(&channel_textures[3].view),
            },
            BindGroupEntry {
                binding: 10,
                resource: BindingResource::Sampler(&channel_textures[3].sampler),
            },
        ],
    })
}

/// Create the render pipeline for custom shaders.
///
/// # Arguments
/// * `device` - The wgpu device
/// * `shader_module` - Compiled shader module
/// * `bind_group_layout` - Bind group layout for the pipeline
/// * `surface_format` - Target surface texture format
/// * `label` - Optional label for the pipeline
pub fn create_render_pipeline(
    device: &Device,
    shader_module: &ShaderModule,
    bind_group_layout: &BindGroupLayout,
    surface_format: TextureFormat,
    label: Option<&str>,
) -> RenderPipeline {
    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
        label: Some(label.unwrap_or("Custom Shader Pipeline Layout")),
        bind_group_layouts: &[bind_group_layout],
        push_constant_ranges: &[],
    });

    device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some(label.unwrap_or("Custom Shader Pipeline")),
        layout: Some(&pipeline_layout),
        vertex: VertexState {
            module: shader_module,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(FragmentState {
            module: shader_module,
            entry_point: Some("fs_main"),
            targets: &[Some(ColorTargetState {
                format: surface_format,
                blend: Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
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
