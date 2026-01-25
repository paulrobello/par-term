use super::{CellRenderer, GlyphInfo};

pub(crate) struct RasterizedGlyph {
    pub width: u32,
    pub height: u32,
    #[allow(dead_code)]
    pub bearing_x: f32,
    #[allow(dead_code)]
    pub bearing_y: f32,
    pub pixels: Vec<u8>,
    pub is_colored: bool,
}

impl CellRenderer {
    pub fn clear_glyph_cache(&mut self) {
        self.glyph_cache.clear();
        self.lru_head = None;
        self.lru_tail = None;
        self.atlas_next_x = 0;
        self.atlas_next_y = 0;
        self.atlas_row_height = 0;
        self.dirty_rows.fill(true);
        // Re-upload the solid white pixel for geometric block rendering
        self.upload_solid_pixel();
    }

    pub(crate) fn lru_remove(&mut self, key: u64) {
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

    pub(crate) fn lru_push_front(&mut self, key: u64) {
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

    pub(crate) fn rasterize_glyph(
        &self,
        font_idx: usize,
        glyph_id: u16,
    ) -> Option<RasterizedGlyph> {
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

    pub(crate) fn upload_glyph(&mut self, _key: u64, raster: &RasterizedGlyph) -> GlyphInfo {
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
}
