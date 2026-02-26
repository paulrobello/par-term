//! Channel texture management for custom shaders
//!
//! Provides loading and management of texture channels (iChannel0-3)
//! that can be used by custom shaders alongside the terminal content (iChannel4).

use crate::error::RenderError;
use std::path::Path;
use wgpu::*;

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
