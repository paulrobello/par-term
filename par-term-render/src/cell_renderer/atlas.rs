use super::{CellRenderer, GlyphInfo};

pub(crate) struct RasterizedGlyph {
    pub width: u32,
    pub height: u32,
    pub bearing_x: f32,
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

    /// Miscellaneous Technical (U+2300–U+23FF)
    /// Contains media controls (⏩⏪⏸⏹⏺), hourglass (⌛), power symbols, etc.
    pub const MISC_TECHNICAL_START: u32 = 0x2300;
    pub const MISC_TECHNICAL_END: u32 = 0x23FF;

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

    // Miscellaneous Technical (U+2300–U+23FF)
    if (symbol_ranges::MISC_TECHNICAL_START..=symbol_ranges::MISC_TECHNICAL_END).contains(&code) {
        return true;
    }

    // Miscellaneous Symbols (U+2600–U+26FF)
    if (symbol_ranges::MISC_SYMBOLS_START..=symbol_ranges::MISC_SYMBOLS_END).contains(&code) {
        return true;
    }

    // Dingbats (U+2700–U+27BF)
    if (symbol_ranges::DINGBATS_START..=symbol_ranges::DINGBATS_END).contains(&code) {
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
        self.atlas.glyph_cache.clear();
        self.atlas.lru_head = None;
        self.atlas.lru_tail = None;
        self.atlas.atlas_next_x = 0;
        self.atlas.atlas_next_y = 0;
        self.atlas.atlas_row_height = 0;
        self.dirty_rows.fill(true);
        // Re-upload the solid white pixel for geometric block rendering
        self.upload_solid_pixel();
    }

    pub(crate) fn lru_remove(&mut self, key: u64) {
        let info = self
            .atlas
            .glyph_cache
            .get(&key)
            .expect("Glyph cache entry must exist before calling lru_remove");
        let prev = info.prev;
        let next = info.next;

        if let Some(p) = prev {
            self.atlas
                .glyph_cache
                .get_mut(&p)
                .expect("Glyph cache LRU prev entry must exist")
                .next = next;
        } else {
            self.atlas.lru_head = next;
        }

        if let Some(n) = next {
            self.atlas
                .glyph_cache
                .get_mut(&n)
                .expect("Glyph cache LRU next entry must exist")
                .prev = prev;
        } else {
            self.atlas.lru_tail = prev;
        }
    }

    pub(crate) fn lru_push_front(&mut self, key: u64) {
        let next = self.atlas.lru_head;
        if let Some(n) = next {
            self.atlas
                .glyph_cache
                .get_mut(&n)
                .expect("Glyph cache LRU head entry must exist")
                .prev = Some(key);
        } else {
            self.atlas.lru_tail = Some(key);
        }

        let info = self
            .atlas
            .glyph_cache
            .get_mut(&key)
            .expect("Glyph cache entry must exist before calling lru_push_front");
        info.prev = None;
        info.next = next;
        self.atlas.lru_head = Some(key);
    }

    pub(crate) fn rasterize_glyph(
        &mut self,
        font_idx: usize,
        glyph_id: u16,
        force_monochrome: bool,
    ) -> Option<RasterizedGlyph> {
        let font = self.font_manager.get_font(font_idx)?;
        // Use swash to rasterize
        use swash::scale::Render;
        use swash::scale::image::Content;
        use swash::zeno::Format;

        // Determine render format before creating the scaler so there is no live
        // mutable borrow of `self.scale_context` when we call `should_use_thin_strokes`.
        let use_thin_strokes = self.should_use_thin_strokes();
        let render_format = if !self.font.font_antialias {
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

        // Build the scaler after computing `render_format` to avoid a
        // mutable+immutable borrow overlap on `self`.
        let mut scaler = self
            .scale_context
            .builder(*font)
            .size(self.font.font_size_pixels)
            .hint(self.font.font_hinting)
            .build();

        let mut image = Render::new(&sources)
            .format(render_format)
            .render(&mut scaler, glyph_id)?;

        // Detect degenerate outlines: some fonts (e.g., Apple Color Emoji) have charmap
        // entries but produce empty outlines (all-zero alpha) when the font only has
        // bitmap data (sbix).
        if matches!(image.content, Content::Mask) && image.data.iter().all(|&b| b == 0) {
            if force_monochrome {
                // For monochrome symbol rendering, don't fall back to color bitmaps.
                // Return None so the caller can try the next font in the fallback
                // chain. If no text font has the character, the caller's last resort
                // will retry with force_monochrome=false to get colored emoji.
                return None;
            }
            // For normal (non-monochrome) rendering, try color bitmap sources.
            // Drop `scaler` so the exclusive borrow on `self.scale_context` is
            // released, allowing us to rebuild a new scaler for the retry pass.
            #[allow(clippy::drop_non_drop)]
            // Intentional: ends borrow lifetime on self.scale_context
            drop(scaler);
            let mut retry_scaler = self
                .scale_context
                .builder(*font)
                .size(self.font.font_size_pixels)
                .hint(self.font.font_hinting)
                .build();
            let color_sources = [
                swash::scale::Source::ColorBitmap(swash::scale::StrikeWith::BestFit),
                swash::scale::Source::ColorOutline(0),
            ];
            if let Some(color_image) = Render::new(&color_sources)
                .format(render_format)
                .render(&mut retry_scaler, glyph_id)
            {
                image = color_image;
            } else {
                return None;
            }
        }

        let (pixels, is_colored) = match image.content {
            Content::Color => {
                if force_monochrome {
                    // Convert color emoji to monochrome using the original alpha channel.
                    // This is more accurate than luminance-based conversion: colored
                    // symbols (e.g., yellow ⭐, colored ✨) keep full opacity, while
                    // luminance would make non-white colors appear faint.
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
                    let alpha = if !self.font.font_antialias {
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

        // Final check: reject glyphs that are still all-transparent after processing.
        // This catches cases where even color bitmap conversion produced no visible pixels.
        if !is_colored && pixels.iter().skip(3).step_by(4).all(|&a| a == 0) {
            return None;
        }

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
        let padding = super::ATLAS_GLYPH_PADDING;
        let atlas_size = self.atlas.atlas_size;
        if self.atlas.atlas_next_x + raster.width + padding > atlas_size {
            self.atlas.atlas_next_x = 0;
            self.atlas.atlas_next_y += self.atlas.atlas_row_height + padding;
            self.atlas.atlas_row_height = 0;
        }

        if self.atlas.atlas_next_y + raster.height + padding > atlas_size {
            self.clear_glyph_cache();
        }

        let info = GlyphInfo {
            key: _key,
            x: self.atlas.atlas_next_x,
            y: self.atlas.atlas_next_y,
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
                texture: &self.atlas.atlas_texture,
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

        // Clear the padding strips with transparent black so bilinear sampling at glyph
        // edges never bleeds into stale data from previously evicted glyphs.
        let pad_right_x = info.x + raster.width;
        let pad_bottom_y = info.y + raster.height;

        // Right border: `padding` columns × glyph height
        if pad_right_x + padding <= atlas_size && raster.height > 0 {
            let zero = vec![0u8; (padding * raster.height * 4) as usize];
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.atlas.atlas_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: pad_right_x,
                        y: info.y,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &zero,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padding * 4),
                    rows_per_image: Some(raster.height),
                },
                wgpu::Extent3d {
                    width: padding,
                    height: raster.height,
                    depth_or_array_layers: 1,
                },
            );
        }

        // Bottom border: glyph width × `padding` rows
        if pad_bottom_y + padding <= atlas_size && raster.width > 0 {
            let zero = vec![0u8; (raster.width * padding * 4) as usize];
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.atlas.atlas_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: info.x,
                        y: pad_bottom_y,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &zero,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(raster.width * 4),
                    rows_per_image: Some(padding),
                },
                wgpu::Extent3d {
                    width: raster.width,
                    height: padding,
                    depth_or_array_layers: 1,
                },
            );
        }

        self.atlas.atlas_next_x += raster.width + padding;
        self.atlas.atlas_row_height = self.atlas.atlas_row_height.max(raster.height);

        info
    }

    /// Look up a glyph by `cache_key` in the atlas, rasterizing and uploading it on
    /// a cache miss.  Returns `None` when rasterization produces an empty bitmap.
    ///
    /// `cache_key` must be computed by the caller as:
    ///   `((font_idx as u64) << 32) | (glyph_id as u64)`
    /// with bit 63 set when querying the colored-emoji variant of a symbol character.
    ///
    /// On a cache hit the LRU order is updated before returning.
    pub(crate) fn get_or_rasterize_glyph(
        &mut self,
        font_idx: usize,
        glyph_id: u16,
        force_monochrome: bool,
        cache_key: u64,
    ) -> Option<GlyphInfo> {
        if self.atlas.glyph_cache.contains_key(&cache_key) {
            self.lru_remove(cache_key);
            self.lru_push_front(cache_key);
            return Some(
                self.atlas
                    .glyph_cache
                    .get(&cache_key)
                    .expect("Glyph cache entry must exist after contains_key check")
                    .clone(),
            );
        }
        let raster = self.rasterize_glyph(font_idx, glyph_id, force_monochrome)?;
        let info = self.upload_glyph(cache_key, &raster);
        self.atlas.glyph_cache.insert(cache_key, info.clone());
        self.lru_push_front(cache_key);
        Some(info)
    }

    /// Resolve a renderable glyph for a character, walking font fallbacks until one succeeds.
    ///
    /// This is the single canonical implementation of the font-fallback loop previously
    /// duplicated in `text_instance_builder.rs` and `pane_render/mod.rs` (see ARC-004 / QA-003).
    ///
    /// # Arguments
    /// * `base_char`       — the base Unicode scalar to look up (after stripping VS16 etc.)
    /// * `grapheme`        — the full grapheme cluster string (may be multi-char for ZWJ/flags)
    /// * `bold`            — bold style flag
    /// * `italic`          — italic style flag
    /// * `force_monochrome` — when true use single-char lookup and suppress colored-emoji
    ///   rasterization; falls back to colored-emoji as last resort
    ///
    /// # Returns
    /// The first [`GlyphInfo`] that rasterizes successfully, or `None` if every font
    /// (including the colored-emoji last-resort) fails.
    ///
    /// # Caching
    /// Results are cached in the glyph atlas.  The cache key encodes `(font_idx, glyph_id)`
    /// as `((font_idx as u64) << 32) | (glyph_id as u64)`, with bit 63 set for the
    /// colored-emoji fallback variant.
    pub(crate) fn resolve_glyph_with_fallback(
        &mut self,
        base_char: char,
        grapheme: &str,
        bold: bool,
        italic: bool,
        force_monochrome: bool,
    ) -> Option<GlyphInfo> {
        // Initial lookup: use grapheme-aware path for multi-char sequences (flags, ZWJ emoji,
        // skin-tone modifiers), unless force_monochrome has already stripped VS16 to a single char.
        let chars: Vec<char> = grapheme.chars().collect();
        let mut glyph_result = if force_monochrome || chars.len() == 1 {
            self.font_manager.find_glyph(base_char, bold, italic)
        } else {
            self.font_manager
                .find_grapheme_glyph(grapheme, bold, italic)
        };

        // Walk font fallbacks until a glyph rasterizes successfully.
        // Rasterization can fail even when a font has a charmap entry (e.g. Apple Color Emoji
        // charmap entries exist for some symbols but produce empty outlines).
        let mut excluded_fonts: Vec<usize> = Vec::new();
        let resolved = loop {
            match glyph_result {
                Some((font_idx, glyph_id)) => {
                    let cache_key = ((font_idx as u64) << 32) | (glyph_id as u64);
                    if let Some(info) =
                        self.get_or_rasterize_glyph(font_idx, glyph_id, force_monochrome, cache_key)
                    {
                        break Some(info);
                    }
                    // This font's outline was empty — exclude it and retry.
                    excluded_fonts.push(font_idx);
                    glyph_result = self.font_manager.find_glyph_excluding(
                        base_char,
                        bold,
                        italic,
                        &excluded_fonts,
                    );
                }
                None => break None,
            }
        };

        // Last resort: if monochrome rendering failed across all fonts (no font has vector
        // outlines for this char), retry with colored-emoji rasterization.  Characters like ✨
        // only exist in Apple Color Emoji — rendering them colored is better than nothing.
        // Bit 63 of the cache key distinguishes the colored-fallback entry from the monochrome
        // entry for the same (font_idx, glyph_id) pair.
        if resolved.is_none() && force_monochrome {
            let mut glyph_result2 = self.font_manager.find_glyph(base_char, bold, italic);
            loop {
                match glyph_result2 {
                    Some((font_idx, glyph_id)) => {
                        let cache_key =
                            ((font_idx as u64) << 32) | (glyph_id as u64) | (1u64 << 63);
                        if let Some(info) =
                            self.get_or_rasterize_glyph(font_idx, glyph_id, false, cache_key)
                        {
                            break Some(info);
                        }
                        glyph_result2 = self.font_manager.find_glyph_excluding(
                            base_char,
                            bold,
                            italic,
                            &[font_idx],
                        );
                    }
                    None => break None,
                }
            }
        } else {
            resolved
        }
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
/// This is used when a symbol character (like dingbats ✨ ⭐ ✔) is rendered
/// from a color emoji font but should be displayed as a monochrome glyph
/// using the terminal foreground color.
///
/// Uses the original alpha channel directly rather than luminance-derived alpha.
/// This produces more accurate results: colored symbols (e.g., yellow ⭐,
/// multi-colored ✨) retain full opacity, whereas luminance-based conversion
/// makes non-white colors appear faint (a red symbol would only be ~30% visible).
fn convert_color_to_alpha_mask(image: &swash::scale::image::Image) -> Vec<u8> {
    let width = image.placement.width as usize;
    let height = image.placement.height as usize;
    let mut pixels = Vec::with_capacity(width * height * 4);

    // Color emoji images are RGBA (4 bytes per pixel).
    // Use the original alpha channel directly to preserve the symbol shape.
    for chunk in image.data.chunks_exact(4) {
        let a = chunk[3];
        pixels.extend_from_slice(&[255, 255, 255, a]);
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
