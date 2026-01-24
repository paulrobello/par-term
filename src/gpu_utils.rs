//! Common GPU utilities for texture and sampler creation.
//!
//! This module provides reusable helper functions for common wgpu operations
//! to reduce code duplication across renderer modules.

#![allow(dead_code)]

use wgpu::{
    AddressMode, Device, Extent3d, FilterMode, Queue, Sampler, SamplerDescriptor, Texture,
    TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};

/// Create a linear sampler with ClampToEdge address mode.
///
/// This is the most common sampler configuration, suitable for:
/// - Terminal cell rendering
/// - Sixel graphics
/// - UI textures
pub fn create_linear_sampler(device: &Device, label: Option<&str>) -> Sampler {
    device.create_sampler(&SamplerDescriptor {
        label,
        address_mode_u: AddressMode::ClampToEdge,
        address_mode_v: AddressMode::ClampToEdge,
        address_mode_w: AddressMode::ClampToEdge,
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        mipmap_filter: FilterMode::Nearest,
        ..Default::default()
    })
}

/// Create a linear sampler with Repeat address mode.
///
/// Suitable for:
/// - Tiled background textures
/// - Shader channel textures that should tile
pub fn create_repeat_sampler(device: &Device, label: Option<&str>) -> Sampler {
    device.create_sampler(&SamplerDescriptor {
        label,
        address_mode_u: AddressMode::Repeat,
        address_mode_v: AddressMode::Repeat,
        address_mode_w: AddressMode::Repeat,
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        mipmap_filter: FilterMode::Linear,
        ..Default::default()
    })
}

/// Create an RGBA texture with the specified dimensions.
///
/// The texture is created with:
/// - COPY_DST usage (for writing data)
/// - TEXTURE_BINDING usage (for sampling in shaders)
///
/// # Arguments
/// * `device` - The wgpu device
/// * `width` - Texture width in pixels
/// * `height` - Texture height in pixels
/// * `label` - Optional debug label
pub fn create_rgba_texture(
    device: &Device,
    width: u32,
    height: u32,
    label: Option<&str>,
) -> Texture {
    device.create_texture(&TextureDescriptor {
        label,
        size: Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8UnormSrgb,
        usage: TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    })
}

/// Create an RGBA texture with render target usage.
///
/// Similar to `create_rgba_texture` but also includes RENDER_ATTACHMENT usage
/// for use as a render target (e.g., for intermediate textures in shader pipelines).
///
/// # Arguments
/// * `device` - The wgpu device
/// * `width` - Texture width in pixels
/// * `height` - Texture height in pixels
/// * `format` - Texture format (usually matches surface format)
/// * `label` - Optional debug label
pub fn create_render_texture(
    device: &Device,
    width: u32,
    height: u32,
    format: TextureFormat,
    label: Option<&str>,
) -> Texture {
    device.create_texture(&TextureDescriptor {
        label,
        size: Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format,
        usage: TextureUsages::COPY_DST
            | TextureUsages::TEXTURE_BINDING
            | TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    })
}

/// Write RGBA pixel data to a texture.
///
/// # Arguments
/// * `queue` - The wgpu queue
/// * `texture` - The target texture
/// * `data` - RGBA pixel data (4 bytes per pixel)
/// * `width` - Image width in pixels
/// * `height` - Image height in pixels
pub fn write_rgba_texture(queue: &Queue, texture: &Texture, data: &[u8], width: u32, height: u32) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        data,
        wgpu::TexelCopyBufferLayout {
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
}

/// Write RGBA pixel data to a specific region of a texture.
///
/// # Arguments
/// * `queue` - The wgpu queue
/// * `texture` - The target texture
/// * `data` - RGBA pixel data (4 bytes per pixel)
/// * `x` - X offset in pixels
/// * `y` - Y offset in pixels
/// * `width` - Region width in pixels
/// * `height` - Region height in pixels
pub fn write_rgba_texture_region(
    queue: &Queue,
    texture: &Texture,
    data: &[u8],
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d { x, y, z: 0 },
            aspect: wgpu::TextureAspect::All,
        },
        data,
        wgpu::TexelCopyBufferLayout {
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
}
