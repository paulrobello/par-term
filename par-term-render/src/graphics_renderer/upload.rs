//! Texture upload / cache invalidation logic.
//!
//! Extracted from the graphics_renderer.rs root per ARC-009: the LRU texture
//! cache and the GPU upload path that populates it.

use crate::error::RenderError;
use std::time::Instant;
use wgpu::*;

/// Maximum number of textures to cache before evicting least-recently-used entries.
/// Secondary bound only — the byte budget below is the primary eviction trigger.
const MAX_TEXTURE_CACHE_SIZE: usize = 100;

/// Byte budget for cached texture pixel data (Rgba8Unorm, width*height*4 per entry).
/// A 5K screenshot is ~56MB RGBA; without a byte budget a handful of them fits the
/// 100-texture count cap yet costs hundreds of MB of GPU memory.
const MAX_TEXTURE_CACHE_BYTES: u64 = 256 * 1024 * 1024;

/// Pixel bytes held by one cached texture (Rgba8Unorm = 4 bytes per pixel).
pub(super) fn texture_bytes(info: &SixelTextureInfo) -> u64 {
    u64::from(info.width) * u64::from(info.height) * 4
}

/// Compute which cache entries to evict (oldest `last_used` first) so that a new
/// texture of `incoming_bytes` fits both the byte budget and the count cap.
/// Returns ids to evict, empty when nothing needs eviction. Pure so the policy is
/// unit-testable without a wgpu device.
fn plan_lru_evictions(
    entries: &[(u64, Instant, u64)],
    incoming_bytes: u64,
    total_bytes: u64,
) -> Vec<u64> {
    let mut order: Vec<(Instant, u64, u64)> = entries
        .iter()
        .map(|(id, last_used, bytes)| (*last_used, *id, *bytes))
        .collect();
    order.sort_unstable_by_key(|(last_used, _, _)| *last_used);
    let mut evict = Vec::new();
    let mut bytes = total_bytes;
    let mut count = entries.len();
    let mut cursor = 0;
    while cursor < order.len()
        && (bytes + incoming_bytes > MAX_TEXTURE_CACHE_BYTES || count >= MAX_TEXTURE_CACHE_SIZE)
    {
        let (_, id, entry_bytes) = order[cursor];
        bytes -= entry_bytes;
        count -= 1;
        evict.push(id);
        cursor += 1;
    }
    evict
}

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

        // Evict least-recently-used textures until the incoming texture fits
        // both the byte budget and the count cap
        let incoming_bytes = expected_size as u64;
        let entries: Vec<(u64, Instant, u64)> = self
            .texture_cache
            .iter()
            .map(|(id, cached)| (*id, cached.last_used, texture_bytes(&cached.texture)))
            .collect();
        for lru_id in plan_lru_evictions(&entries, incoming_bytes, self.texture_cache_bytes) {
            if let Some(evicted) = self.texture_cache.remove(&lru_id) {
                self.texture_cache_bytes -= texture_bytes(&evicted.texture);
                log::debug!(
                    "[GRAPHICS] Evicting LRU texture: id={}, freed={} bytes, cache_bytes={}",
                    lru_id,
                    texture_bytes(&evicted.texture),
                    self.texture_cache_bytes
                );
            }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn mb(n: u64) -> u64 {
        n * 1024 * 1024
    }

    fn entry(id: u64, age_secs: u64, w: u32, h: u32) -> (u64, Instant, u64) {
        (
            id,
            Instant::now() - Duration::from_secs(age_secs),
            u64::from(w) * u64::from(h) * 4,
        )
    }

    #[test]
    fn evicts_oldest_until_byte_budget_fits() {
        // 3 x 100MB entries (300MB total), 256MB budget, 100MB incoming:
        // evict the two oldest so 100 + 100 fits under 256MB
        let entries = vec![
            entry(1, 30, 5120, 5120),
            entry(2, 20, 5120, 5120),
            entry(3, 10, 5120, 5120),
        ];
        assert_eq!(plan_lru_evictions(&entries, mb(100), mb(300)), vec![1, 2]);
    }

    #[test]
    fn evicts_exactly_one_for_count_cap() {
        // 100 tiny entries at the count cap: one eviction frees a slot,
        // the byte budget is not binding
        let entries: Vec<_> = (1..=100u64).map(|i| entry(i, 100 - i, 16, 16)).collect();
        assert_eq!(
            plan_lru_evictions(&entries, 16 * 16 * 4, 100 * 16 * 16 * 4),
            vec![1]
        );
    }

    #[test]
    fn oversized_incoming_on_empty_cache_proceeds_unbudgeted() {
        // Nothing to evict: an oversized texture proceeds (same behavior class
        // as the old count-only cap on an empty cache)
        assert!(plan_lru_evictions(&[], mb(300), 0).is_empty());
    }

    #[test]
    fn no_eviction_when_within_budget() {
        let entries = vec![entry(1, 5, 100, 100), entry(2, 3, 100, 100)];
        assert!(plan_lru_evictions(&entries, 100 * 100 * 4, 2 * 100 * 100 * 4).is_empty());
    }
}
