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

/// Unicode ranges for symbols that should render monochromatically.
/// These characters have emoji default presentation but are commonly used
/// as symbols in terminal contexts (spinners, decorations, etc.) and should
/// use the terminal foreground color rather than colorful emoji rendering.
pub mod symbol_ranges {
    /// Dingbats block (U+2700–U+27BF)
    /// Contains asterisks, stars, sparkles, arrows, etc.
    pub const DINGBATS_START: u32 = 0x2700;
    pub const DINGBATS_END: u32 = 0x27BF;

    /// Miscellaneous Symbols (U+2600–U+26FF)
    /// Contains weather, zodiac, chess, etc.
    pub const MISC_SYMBOLS_START: u32 = 0x2600;
    pub const MISC_SYMBOLS_END: u32 = 0x26FF;

    /// Miscellaneous Symbols and Arrows (U+2B00–U+2BFF)
    /// Contains various arrows and stars like ⭐
    pub const MISC_SYMBOLS_ARROWS_START: u32 = 0x2B00;
    pub const MISC_SYMBOLS_ARROWS_END: u32 = 0x2BFF;
}

/// Check if a character should be rendered as a monochrome symbol.
///
/// Returns true for characters that:
/// 1. Are in symbol/dingbat Unicode ranges
/// 2. Have emoji default presentation but are commonly used as symbols
///
/// These characters should use the terminal foreground color rather than
/// colorful emoji bitmaps, even if the emoji font provides a colored glyph.
pub fn should_render_as_symbol(ch: char) -> bool {
    let code = ch as u32;

    // Dingbats (U+2700–U+27BF)
    if (symbol_ranges::DINGBATS_START..=symbol_ranges::DINGBATS_END).contains(&code) {
        return true;
    }

    // Miscellaneous Symbols (U+2600–U+26FF)
    if (symbol_ranges::MISC_SYMBOLS_START..=symbol_ranges::MISC_SYMBOLS_END).contains(&code) {
        return true;
    }

    // Miscellaneous Symbols and Arrows (U+2B00–U+2BFF)
    if (symbol_ranges::MISC_SYMBOLS_ARROWS_START..=symbol_ranges::MISC_SYMBOLS_ARROWS_END)
        .contains(&code)
    {
        return true;
    }

    false
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
        force_monochrome: bool,
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

        // For symbol characters (dingbats, etc.), prefer outline rendering to get
        // monochrome glyphs that use the terminal foreground color. For emoji,
        // prefer color bitmaps for proper colorful rendering.
        let sources = if force_monochrome {
            // Symbol character: try outline first, fall back to color if unavailable
            // This ensures dingbats like ✳ ✴ ❇ render as monochrome symbols
            // with the terminal foreground color, not as colorful emoji.
            [
                swash::scale::Source::Outline,
                swash::scale::Source::ColorOutline(0),
                swash::scale::Source::ColorBitmap(swash::scale::StrikeWith::BestFit),
            ]
        } else {
            // Regular emoji: prefer color sources for colorful rendering
            [
                swash::scale::Source::ColorBitmap(swash::scale::StrikeWith::BestFit),
                swash::scale::Source::ColorOutline(0),
                swash::scale::Source::Outline,
            ]
        };

        let image = Render::new(&sources)
            .format(render_format)
            .render(&mut scaler, glyph_id)?;

        let (pixels, is_colored) = match image.content {
            Content::Color => {
                // If this is a symbol character that should be monochrome,
                // convert the color image to a monochrome alpha mask using
                // luminance as the alpha channel.
                if force_monochrome {
                    let pixels = convert_color_to_alpha_mask(&image);
                    (pixels, false)
                } else {
                    (image.data.clone(), true)
                }
            }
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

/// Convert a color RGBA image to a monochrome alpha mask.
///
/// This is used when a symbol character (like dingbats ✳ ✴ ❇) is rendered
/// from a color emoji font but should be displayed as a monochrome glyph
/// using the terminal foreground color.
///
/// The alpha channel is derived from the luminance of the original color,
/// preserving the shape while discarding the color information.
fn convert_color_to_alpha_mask(image: &swash::scale::image::Image) -> Vec<u8> {
    let width = image.placement.width as usize;
    let height = image.placement.height as usize;
    let mut pixels = Vec::with_capacity(width * height * 4);

    // Color emoji images are typically RGBA (4 bytes per pixel)
    for chunk in image.data.chunks_exact(4) {
        let r = chunk[0];
        let g = chunk[1];
        let b = chunk[2];
        let a = chunk[3];

        // Use luminance as alpha, multiplied by original alpha
        // This preserves the shape of colored emoji while making them monochrome
        let luminance = ((r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000) as u8;
        // Blend luminance with original alpha
        let alpha = ((luminance as u32 * a as u32) / 255) as u8;

        pixels.extend_from_slice(&[255, 255, 255, alpha]);
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

    use super::should_render_as_symbol;

    #[test]
    fn test_dingbats_are_symbols() {
        // Dingbats block (U+2700-U+27BF)
        assert!(
            should_render_as_symbol('\u{2733}'),
            "✳ EIGHT SPOKED ASTERISK"
        );
        assert!(
            should_render_as_symbol('\u{2734}'),
            "✴ EIGHT POINTED BLACK STAR"
        );
        assert!(should_render_as_symbol('\u{2747}'), "❇ SPARKLE");
        assert!(should_render_as_symbol('\u{2744}'), "❄ SNOWFLAKE");
        assert!(should_render_as_symbol('\u{2702}'), "✂ SCISSORS");
        assert!(should_render_as_symbol('\u{2714}'), "✔ HEAVY CHECK MARK");
        assert!(
            should_render_as_symbol('\u{2716}'),
            "✖ HEAVY MULTIPLICATION X"
        );
        assert!(should_render_as_symbol('\u{2728}'), "✨ SPARKLES");
    }

    #[test]
    fn test_misc_symbols_are_symbols() {
        // Miscellaneous Symbols block (U+2600-U+26FF)
        assert!(should_render_as_symbol('\u{2600}'), "☀ SUN");
        assert!(should_render_as_symbol('\u{2601}'), "☁ CLOUD");
        assert!(should_render_as_symbol('\u{263A}'), "☺ SMILING FACE");
        assert!(should_render_as_symbol('\u{2665}'), "♥ BLACK HEART SUIT");
        assert!(should_render_as_symbol('\u{2660}'), "♠ BLACK SPADE SUIT");
    }

    #[test]
    fn test_misc_symbols_arrows_are_symbols() {
        // Miscellaneous Symbols and Arrows block (U+2B00-U+2BFF)
        assert!(should_render_as_symbol('\u{2B50}'), "⭐ WHITE MEDIUM STAR");
        assert!(should_render_as_symbol('\u{2B55}'), "⭕ HEAVY LARGE CIRCLE");
    }

    #[test]
    fn test_regular_emoji_not_symbols() {
        // Full emoji characters (outside symbol ranges) should NOT be treated as symbols
        // They should render as colorful emoji
        assert!(
            !should_render_as_symbol('\u{1F600}'),
            "😀 GRINNING FACE should not be a symbol"
        );
        assert!(
            !should_render_as_symbol('\u{1F389}'),
            "🎉 PARTY POPPER should not be a symbol"
        );
        assert!(
            !should_render_as_symbol('\u{1F44D}'),
            "👍 THUMBS UP should not be a symbol"
        );
    }

    #[test]
    fn test_regular_chars_not_symbols() {
        // Regular text characters should NOT be treated as symbols
        assert!(
            !should_render_as_symbol('A'),
            "Letter A should not be a symbol"
        );
        assert!(
            !should_render_as_symbol('*'),
            "Asterisk should not be a symbol (it's ASCII)"
        );
        assert!(
            !should_render_as_symbol('1'),
            "Digit 1 should not be a symbol"
        );
    }
}
