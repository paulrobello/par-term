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
        use swash::zeno::Format;

        let mut context = ScaleContext::new();

        // Apply hinting based on config setting
        let mut scaler = context
            .builder(*font)
            .size(self.font_size_pixels)
            .hint(self.font_hinting)
            .build();

        // Determine render format based on anti-aliasing and thin strokes settings
        let use_thin_strokes = self.should_use_thin_strokes();
        let render_format = if !self.font_antialias {
            // No anti-aliasing: render as alpha mask (will be thresholded)
            Format::Alpha
        } else if use_thin_strokes {
            // Thin strokes: use subpixel rendering for lighter appearance
            Format::Subpixel
        } else {
            // Standard anti-aliased rendering
            Format::Alpha
        };

        // Try color sources first so emoji fonts (Apple Color Emoji,
        // Noto Color Emoji) render as colored bitmaps. Regular text fonts
        // have no color data so will fall through to Outline automatically.
        let image = Render::new(&[
            swash::scale::Source::ColorBitmap(swash::scale::StrikeWith::BestFit),
            swash::scale::Source::ColorOutline(0),
            swash::scale::Source::Outline,
        ])
        .format(render_format)
        .render(&mut scaler, glyph_id)?;

        let (pixels, is_colored) = match image.content {
            Content::Color => (image.data.clone(), true),
            Content::Mask => {
                let mut pixels = Vec::with_capacity(image.data.len() * 4);
                for &mask in &image.data {
                    // If anti-aliasing is disabled, threshold the alpha to create crisp edges
                    let alpha = if !self.font_antialias {
                        if mask > 127 { 255 } else { 0 }
                    } else {
                        mask
                    };
                    pixels.extend_from_slice(&[255, 255, 255, alpha]);
                }
                (pixels, false)
            }
            Content::SubpixelMask => {
                let pixels = convert_subpixel_mask_to_rgba(&image);
                (pixels, false)
            }
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

/// Convert a swash subpixel mask into an RGBA alpha mask.
/// Some swash builds emit 3 bytes/pixel (RGB), others 4 bytes/pixel (RGBA).
/// We derive alpha from luminance of RGB and ignore the packed alpha to avoid
/// dropping coverage when alpha is zeroed by the rasterizer.
fn convert_subpixel_mask_to_rgba(image: &swash::scale::image::Image) -> Vec<u8> {
    let width = image.placement.width as usize;
    let height = image.placement.height as usize;
    let mut pixels = Vec::with_capacity(width * height * 4);

    let stride = if width > 0 && height > 0 {
        image.data.len() / (width * height)
    } else {
        0
    };

    match stride {
        3 => {
            for chunk in image.data.chunks_exact(3) {
                let r = chunk[0];
                let g = chunk[1];
                let b = chunk[2];
                let alpha = ((r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000) as u8;
                pixels.extend_from_slice(&[255, 255, 255, alpha]);
            }
        }
        4 => {
            for chunk in image.data.chunks_exact(4) {
                let r = chunk[0];
                let g = chunk[1];
                let b = chunk[2];
                // Ignore chunk[3] because it can be zeroed in some builds.
                let alpha = ((r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000) as u8;
                pixels.extend_from_slice(&[255, 255, 255, alpha]);
            }
        }
        _ => {
            // Fallback: treat as opaque white to avoid invisibility if layout changes.
            pixels.resize(width * height * 4, 255);
        }
    }

    pixels
}

#[cfg(test)]
mod tests {
    use super::convert_subpixel_mask_to_rgba;
    use swash::scale::{Render, ScaleContext, Source};
    use swash::zeno::Format;

    #[test]
    fn subpixel_mask_uses_rgba_stride() {
        let data = std::fs::read("../par-term-fonts/fonts/DejaVuSansMono.ttf").expect("font file");
        let font = swash::FontRef::from_index(&data, 0).expect("font ref");
        let mut context = ScaleContext::new();
        let glyph_id = font.charmap().map('a');
        let mut scaler = context.builder(font).size(18.0).hint(true).build();

        let image = Render::new(&[
            Source::ColorOutline(0),
            Source::ColorBitmap(swash::scale::StrikeWith::BestFit),
            Source::Outline,
            Source::Bitmap(swash::scale::StrikeWith::BestFit),
        ])
        .format(Format::Subpixel)
        .render(&mut scaler, glyph_id)
        .expect("render");

        let converted = convert_subpixel_mask_to_rgba(&image);

        let width = image.placement.width as usize;
        let height = image.placement.height as usize;
        let mut expected = Vec::with_capacity(width * height * 4);
        let stride = if width > 0 && height > 0 {
            image.data.len() / (width * height)
        } else {
            0
        };

        match stride {
            3 => {
                for chunk in image.data.chunks_exact(3) {
                    let r = chunk[0];
                    let g = chunk[1];
                    let b = chunk[2];
                    let alpha = ((r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000) as u8;
                    expected.extend_from_slice(&[255, 255, 255, alpha]);
                }
            }
            4 => {
                for chunk in image.data.chunks_exact(4) {
                    let r = chunk[0];
                    let g = chunk[1];
                    let b = chunk[2];
                    let alpha = ((r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000) as u8;
                    expected.extend_from_slice(&[255, 255, 255, alpha]);
                }
            }
            _ => expected.resize(width * height * 4, 255),
        }

        assert_eq!(converted, expected);
    }
}
