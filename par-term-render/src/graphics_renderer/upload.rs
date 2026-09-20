//! Texture upload / cache invalidation logic.
//!
//! Extracted from the graphics_renderer.rs root per ARC-009: the LRU texture
//! cache and the GPU upload path that populates it.

use crate::error::RenderError;
use std::time::Instant;
use wgpu::*;

/// Maximum number of textures to cache before evicting least-recently-used entries.
/// This prevents unbounded GPU memory growth when displaying many inline images.
const MAX_TEXTURE_CACHE_SIZE: usize = 100;

/// Metadata for a cached sixel texture
pub(super) struct SixelTextureInfo {
    pub(super) texture: Texture,
    #[allow(dead_code)] // GPU lifetime: must outlive the bind_group which references this view
    view: TextureView,
    pub(super) bind_group: BindGroup,
    pub(super) width: u32,
    pub(super) height: u32,
}

/// Cached texture wrapper with LRU tracking
pub(super) struct CachedTexture {
    pub(super) texture: SixelTextureInfo,
    /// Timestamp of last access for LRU eviction
    pub(super) last_used: Instant,
}

impl super::GraphicsRenderer {
    /// Create or get a cached texture for a sixel graphic
    ///
    /// # Arguments
    /// * `device` - WGPU device for creating textures
    /// * `queue` - WGPU queue for writing texture data
    /// * `id` - Unique identifier for this sixel graphic
    /// * `rgba_data` - RGBA pixel data (width * height * 4 bytes)
    /// * `width` - Image width in pixels
    /// * `height` - Image height in pixels
    pub fn get_or_create_texture(
        &mut self,
        device: &Device,
        queue: &Queue,
        id: u64,
        rgba_data: &[u8],
        width: u32,
        height: u32,
    ) -> Result<(), RenderError> {
        // Check if texture already exists in cache
        // For animations, we need to update the texture data even if it exists
        if let Some(cached) = self.texture_cache.get_mut(&id) {
            // Update LRU timestamp on cache hit
            cached.last_used = Instant::now();

            // Kitty TGP virtual placements (high-bit flag set on the cache id;
            // see par-term-render/src/renderer/graphics.rs) reuse the same
            // image data every frame — they're static placements anchored by
            // grid placeholder cells, not animations. Re-uploading the
            // pixels per frame here costs ~640 KB × 60 fps for a 400×400
            // image, saturating the GPU command queue and freezing the pane.
            // For these IDs, treat the cache hit as final.
            const VIRTUAL_PLACEMENT_ID_FLAG: u64 = 1u64 << 63;
            if id & VIRTUAL_PLACEMENT_ID_FLAG != 0 {
                return Ok(());
            }

            // Texture exists - update it if the data might have changed
            // Validate data size
            let expected_size = (width * height * 4) as usize;
            if rgba_data.len() != expected_size {
                return Err(RenderError::InvalidTextureData {
                    expected: expected_size,
                    actual: rgba_data.len(),
                });
            }

            // Update existing texture with new pixel data (for animations)
            queue.write_texture(
                TexelCopyTextureInfo {
                    texture: &cached.texture.texture,
                    mip_level: 0,
                    origin: Origin3d::ZERO,
                    aspect: TextureAspect::All,
                },
                rgba_data,
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

            return Ok(());
        }

        // Validate data size
        let expected_size = (width * height * 4) as usize;
        if rgba_data.len() != expected_size {
            return Err(RenderError::InvalidTextureData {
                expected: expected_size,
                actual: rgba_data.len(),
            });
        }

        // Evict least-recently-used texture if cache is full
        if self.texture_cache.len() >= MAX_TEXTURE_CACHE_SIZE
            && let Some((&lru_id, _)) = self
                .texture_cache
                .iter()
                .min_by_key(|(_, cached)| cached.last_used)
        {
            log::debug!(
                "[GRAPHICS] Evicting LRU texture: id={}, cache_size={}",
                lru_id,
                self.texture_cache.len()
            );
            self.texture_cache.remove(&lru_id);
        }

        // Create texture
        let texture = device.create_texture(&TextureDescriptor {
            label: Some(&format!("Sixel Texture {}", id)),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Write RGBA data to texture
        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            rgba_data,
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

        // Create bind group for this texture
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some(&format!("Sixel Bind Group {}", id)),
            layout: &self.bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        // Cache texture info with current timestamp
        self.texture_cache.insert(
            id,
            CachedTexture {
                texture: SixelTextureInfo {
                    texture,
                    view,
                    bind_group,
                    width,
                    height,
                },
                last_used: Instant::now(),
            },
        );

        log::debug!(
            "[GRAPHICS] Created sixel texture: id={}, size={}x{}, cache_size={}/{}",
            id,
            width,
            height,
            self.texture_cache.len(),
            MAX_TEXTURE_CACHE_SIZE
        );

        Ok(())
    }
}
