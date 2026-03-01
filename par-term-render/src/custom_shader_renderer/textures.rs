//! Channel texture management and intermediate texture management for custom shaders
//!
//! Provides loading and management of texture channels (iChannel0-3) that can be
//! used by custom shaders alongside the terminal content (iChannel4).
//!
//! Also provides the intermediate (ping-pong) texture used to render terminal
//! content into before the custom shader reads it, as well as the bind group
//! recreation logic that wires all textures together.

use crate::error::RenderError;
use std::path::Path;
use wgpu::*;

use super::CustomShaderRenderer;

/// A texture channel that can be bound to a custom shader
pub struct ChannelTexture {
    /// The GPU texture (kept alive to ensure view/sampler remain valid)
    /// When using an external texture (e.g., background image), this is None
    pub texture: Option<Texture>,
    /// View for binding to shaders
    pub view: TextureView,
    /// Sampler for texture filtering
    pub sampler: Sampler,
    /// Texture width in pixels
    pub width: u32,
    /// Texture height in pixels
    pub height: u32,
}

impl ChannelTexture {
    /// Create a 1x1 transparent black placeholder texture
    ///
    /// This is used when no texture is configured for a channel,
    /// ensuring the shader can still sample from it without errors.
    pub fn placeholder(device: &Device, queue: &Queue) -> Self {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("Channel Placeholder Texture"),
            size: Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Write transparent black pixel
        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &[0u8, 0, 0, 0], // RGBA: transparent black
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&TextureViewDescriptor::default());
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Channel Placeholder Sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Linear,
            ..Default::default()
        });

        Self {
            texture: Some(texture),
            view,
            sampler,
            width: 1,
            height: 1,
        }
    }

    /// Create a ChannelTexture from an existing texture view and sampler.
    ///
    /// This is used when sharing a texture from another source (e.g., background image)
    /// without creating a new copy. The caller is responsible for keeping the source
    /// texture alive while this ChannelTexture is in use.
    ///
    /// # Arguments
    /// * `view` - The texture view to use
    /// * `sampler` - The sampler for texture filtering
    /// * `width` - Texture width in pixels
    /// * `height` - Texture height in pixels
    pub fn from_view(view: TextureView, sampler: Sampler, width: u32, height: u32) -> Self {
        Self {
            texture: None,
            view,
            sampler,
            width,
            height,
        }
    }

    /// Create a ChannelTexture from a view, sampler, and owned texture.
    ///
    /// This is used when creating a new texture that should be kept alive
    /// by this ChannelTexture instance (e.g., solid color textures).
    ///
    /// # Arguments
    /// * `view` - The texture view to use
    /// * `sampler` - The sampler for texture filtering
    /// * `width` - Texture width in pixels
    /// * `height` - Texture height in pixels
    /// * `texture` - The owned texture to keep alive
    pub fn from_view_and_texture(
        view: TextureView,
        sampler: Sampler,
        width: u32,
        height: u32,
        texture: Texture,
    ) -> Self {
        Self {
            texture: Some(texture),
            view,
            sampler,
            width,
            height,
        }
    }

    /// Load a texture from an image file
    ///
    /// Supports common image formats (PNG, JPEG, etc.) via the `image` crate.
    ///
    /// # Arguments
    /// * `device` - The wgpu device
    /// * `queue` - The wgpu queue
    /// * `path` - Path to the image file
    ///
    /// # Returns
    /// The loaded texture, or an error if loading fails
    pub fn from_file(device: &Device, queue: &Queue, path: &Path) -> Result<Self, RenderError> {
        // Load image and convert to RGBA8
        let img = image::open(path)
            .map_err(|e| RenderError::ImageLoad {
                path: path.display().to_string(),
                source: e,
            })?
            .to_rgba8();

        let (width, height) = img.dimensions();

        // Create GPU texture
        let texture = device.create_texture(&TextureDescriptor {
            label: Some(&format!("Channel Texture: {}", path.display())),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload image data to GPU
        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &img,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&TextureViewDescriptor::default());

        // Create sampler with wrapping for tiled textures
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some(&format!("Channel Sampler: {}", path.display())),
            address_mode_u: AddressMode::Repeat,
            address_mode_v: AddressMode::Repeat,
            address_mode_w: AddressMode::Repeat,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Linear,
            ..Default::default()
        });

        log::info!(
            "Loaded channel texture: {} ({}x{})",
            path.display(),
            width,
            height
        );

        Ok(Self {
            texture: Some(texture),
            view,
            sampler,
            width,
            height,
        })
    }

    /// Get the resolution as a vec4 [width, height, 1.0, 0.0]
    ///
    /// This format matches Shadertoy's iChannelResolution uniform.
    pub fn resolution(&self) -> [f32; 4] {
        [self.width as f32, self.height as f32, 1.0, 0.0]
    }
}

/// Load channel textures from optional paths
///
/// # Arguments
/// * `device` - The wgpu device
/// * `queue` - The wgpu queue
/// * `paths` - Array of 4 optional paths for iChannel0-3
///
/// # Returns
/// Array of 4 ChannelTexture instances (placeholders for None paths)
pub fn load_channel_textures(
    device: &Device,
    queue: &Queue,
    paths: &[Option<std::path::PathBuf>; 4],
) -> [ChannelTexture; 4] {
    let load_or_placeholder = |path: &Option<std::path::PathBuf>, index: usize| -> ChannelTexture {
        match path {
            Some(p) => match ChannelTexture::from_file(device, queue, p) {
                Ok(tex) => tex,
                Err(e) => {
                    log::error!(
                        "Failed to load iChannel{} texture '{}': {}",
                        index,
                        p.display(),
                        e
                    );
                    ChannelTexture::placeholder(device, queue)
                }
            },
            None => ChannelTexture::placeholder(device, queue),
        }
    };

    [
        load_or_placeholder(&paths[0], 0),
        load_or_placeholder(&paths[1], 1),
        load_or_placeholder(&paths[2], 2),
        load_or_placeholder(&paths[3], 3),
    ]
}

// ============ Intermediate texture and bind-group management ============

impl CustomShaderRenderer {
    /// Create the intermediate texture for rendering terminal content.
    ///
    /// The terminal scene is rendered into this texture first; the custom
    /// shader then reads it via `iChannel4`.
    pub(super) fn create_intermediate_texture(
        device: &Device,
        format: TextureFormat,
        width: u32,
        height: u32,
    ) -> (Texture, TextureView) {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("Custom Shader Intermediate Texture"),
            size: Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let view = texture.create_view(&TextureViewDescriptor::default());
        (texture, view)
    }

    /// Clear the intermediate texture (e.g., when switching to split pane mode).
    ///
    /// This prevents old single-pane content from showing through the shader.
    pub fn clear_intermediate_texture(&self, device: &Device, queue: &Queue) {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Clear Intermediate Texture Encoder"),
        });

        {
            let _clear_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Clear Intermediate Texture Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.intermediate_texture_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        queue.submit(std::iter::once(encoder.finish()));
    }

    /// Resize the intermediate texture when the window size changes.
    pub fn resize(&mut self, device: &Device, width: u32, height: u32) {
        if width == self.texture_width && height == self.texture_height {
            return;
        }

        self.texture_width = width;
        self.texture_height = height;

        // Recreate intermediate texture
        let (texture, view) =
            Self::create_intermediate_texture(device, self.surface_format, width, height);
        self.intermediate_texture = texture;
        self.intermediate_texture_view = view;

        // Recreate bind group with new texture view (handles background as channel0 if enabled)
        self.recreate_bind_group(device);
    }

    /// Recreate the bind group, using the background texture for channel0 if enabled.
    ///
    /// Priority for iChannel0:
    /// 1. If `use_background_as_channel0` is enabled and a background texture is set,
    ///    use the background texture.
    /// 2. If channel0 has a configured texture (not a 1x1 placeholder), use it.
    /// 3. Otherwise use the placeholder.
    ///
    /// This is called when:
    /// - The background texture changes (and `use_background_as_channel0` is true)
    /// - `use_background_as_channel0` flag changes
    /// - The window resizes (intermediate texture changes)
    pub(super) fn recreate_bind_group(&mut self, device: &Device) {
        // Priority: use_background_as_channel0 (explicit override) > configured channel0 > placeholder
        let channel0_texture = if self.use_background_as_channel0 {
            // User explicitly wants background image as channel0
            self.background_channel_texture
                .as_ref()
                .unwrap_or(&self.channel_textures[0])
        } else if self.channel0_has_real_texture() {
            // Channel0 has a real texture configured
            &self.channel_textures[0]
        } else {
            // Use the placeholder
            &self.channel_textures[0]
        };

        // Create a temporary array with the potentially swapped channel0
        let effective_channels = [
            channel0_texture,
            &self.channel_textures[1],
            &self.channel_textures[2],
            &self.channel_textures[3],
        ];

        self.bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Custom Shader Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
                // iChannel0 (background or configured texture)
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&effective_channels[0].view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&effective_channels[0].sampler),
                },
                // iChannel1
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&effective_channels[1].view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&effective_channels[1].sampler),
                },
                // iChannel2
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&effective_channels[2].view),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: wgpu::BindingResource::Sampler(&effective_channels[2].sampler),
                },
                // iChannel3
                wgpu::BindGroupEntry {
                    binding: 7,
                    resource: wgpu::BindingResource::TextureView(&effective_channels[3].view),
                },
                wgpu::BindGroupEntry {
                    binding: 8,
                    resource: wgpu::BindingResource::Sampler(&effective_channels[3].sampler),
                },
                // iChannel4 (terminal content)
                wgpu::BindGroupEntry {
                    binding: 9,
                    resource: wgpu::BindingResource::TextureView(&self.intermediate_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                // iCubemap
                wgpu::BindGroupEntry {
                    binding: 11,
                    resource: wgpu::BindingResource::TextureView(&self.cubemap.view),
                },
                wgpu::BindGroupEntry {
                    binding: 12,
                    resource: wgpu::BindingResource::Sampler(&self.cubemap.sampler),
                },
            ],
        });
    }

    /// Check if channel0 has a real configured texture (not just a 1x1 placeholder).
    pub(super) fn channel0_has_real_texture(&self) -> bool {
        let ch0 = &self.channel_textures[0];
        // Placeholder textures are 1x1
        ch0.width > 1 || ch0.height > 1
    }

    /// Get the effective channel0 resolution for the `iChannelResolution` uniform.
    ///
    /// Priority:
    /// 1. If `use_background_as_channel0` is enabled and a background texture is set,
    ///    return its resolution.
    /// 2. Otherwise return channel0 texture resolution (configured or placeholder).
    pub(super) fn effective_channel0_resolution(&self) -> [f32; 4] {
        if self.use_background_as_channel0 {
            self.background_channel_texture
                .as_ref()
                .map(|t| t.resolution())
                .unwrap_or_else(|| self.channel_textures[0].resolution())
        } else {
            self.channel_textures[0].resolution()
        }
    }
}
