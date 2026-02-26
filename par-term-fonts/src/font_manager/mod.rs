//! Font management with fallback chain for comprehensive Unicode coverage.
//!
//! This module provides font loading, fallback chain management, and text shaping
//! capabilities. It supports:
//! - Primary font with bold/italic/bold-italic variants
//! - Unicode range-specific fonts (e.g., CJK, emoji)
//! - Automatic fallback chain for missing glyphs
//! - HarfBuzz-based text shaping via rustybuzz

mod fallbacks;
mod loader;
mod types;

use std::sync::Arc;

use anyhow::Result;
use fontdb::Database;
use swash::FontRef;

use crate::text_shaper::{ShapedRun, ShapingOptions, TextShaper};

pub use fallbacks::FALLBACK_FAMILIES;
pub use types::{FontData, UnicodeRangeFont};

/// Manages multiple fonts with fallback chain.
///
/// Font indices are assigned as follows:
/// - 0: Primary/regular font
/// - 1: Bold font (if available)
/// - 2: Italic font (if available)
/// - 3: Bold-italic font (if available)
/// - 4..4+N: Unicode range fonts (N = number of range fonts)
/// - 4+N..: Fallback fonts
pub struct FontManager {
    /// Primary font (regular weight, from config or embedded)
    primary: FontData,

    /// Bold font (optional, falls back to primary if not specified)
    bold: Option<FontData>,

    /// Italic font (optional, falls back to primary if not specified)
    italic: Option<FontData>,

    /// Bold italic font (optional, falls back to primary if not specified)
    bold_italic: Option<FontData>,

    /// Unicode range-specific fonts (checked before fallbacks)
    range_fonts: Vec<UnicodeRangeFont>,

    /// Fallback fonts in priority order
    fallbacks: Vec<FontData>,

    /// Font database for system font queries
    #[allow(dead_code)]
    font_db: Database,

    /// Text shaper for ligatures and complex scripts
    text_shaper: TextShaper,
}

impl FontManager {
    /// Create a new FontManager with primary font and system fallbacks.
    ///
    /// # Arguments
    /// * `primary_family` - Regular/normal weight font family name
    /// * `bold_family` - Bold weight font family name (optional)
    /// * `italic_family` - Italic font family name (optional)
    /// * `bold_italic_family` - Bold italic font family name (optional)
    /// * `font_ranges` - Unicode range-specific font mappings
    pub fn new(
        primary_family: Option<&str>,
        bold_family: Option<&str>,
        italic_family: Option<&str>,
        bold_italic_family: Option<&str>,
        font_ranges: &[par_term_config::FontRange],
    ) -> Result<Self> {
        let mut font_db = Database::new();

        // Load system fonts
        font_db.load_system_fonts();
        log::info!("Loaded {} system fonts", font_db.len());

        // Load primary font
        let primary = Self::load_primary_font(&mut font_db, primary_family)?;

        // Build fallback chain
        let fallbacks = Self::build_fallback_chain(&mut font_db);
        log::info!("Loaded {} fallback fonts", fallbacks.len());

        // Load styled font variants
        let bold = Self::load_styled_font(
            &mut font_db,
            bold_family,
            "bold",
            fontdb::Weight::BOLD,
            None,
        );
        let italic = Self::load_styled_font(
            &mut font_db,
            italic_family,
            "italic",
            fontdb::Weight::NORMAL,
            Some(fontdb::Style::Italic),
        );
        let bold_italic = Self::load_styled_font(
            &mut font_db,
            bold_italic_family,
            "bold italic",
            fontdb::Weight::BOLD,
            Some(fontdb::Style::Italic),
        );

        // Load Unicode range-specific fonts
        let range_fonts = Self::load_range_fonts(&mut font_db, font_ranges);

        Ok(FontManager {
            primary,
            bold,
            italic,
            bold_italic,
            range_fonts,
            fallbacks,
            font_db,
            text_shaper: TextShaper::new(),
        })
    }

    /// Load the primary font from system or embedded.
    fn load_primary_font(font_db: &mut Database, family: Option<&str>) -> Result<FontData> {
        if let Some(family_name) = family {
            log::info!("Attempting to load primary font: {}", family_name);
            if let Some(font_data) = loader::load_font_from_db(font_db, family_name) {
                log::info!("Successfully loaded primary font: {}", family_name);
                return Ok(font_data);
            }
            log::warn!(
                "Primary font '{}' not found, using embedded DejaVu Sans Mono",
                family_name
            );
        } else {
            log::info!("No primary font specified, using embedded DejaVu Sans Mono");
        }
        loader::load_embedded_font()
    }

    /// Build the fallback font chain from available system fonts.
    fn build_fallback_chain(font_db: &mut Database) -> Vec<FontData> {
        let mut fallbacks = Vec::new();
        for family_name in FALLBACK_FAMILIES {
            if let Some(font_data) = loader::load_font_from_db(font_db, family_name) {
                log::debug!("Added fallback font: {}", family_name);
                fallbacks.push(font_data);
            }
        }
        fallbacks
    }

    /// Load a styled font variant (bold, italic, or bold-italic).
    fn load_styled_font(
        font_db: &mut Database,
        family: Option<&str>,
        style_name: &str,
        weight: fontdb::Weight,
        style: Option<fontdb::Style>,
    ) -> Option<FontData> {
        family.and_then(|family_name| {
            log::info!("Attempting to load {} font: {}", style_name, family_name);
            let font_data =
                loader::load_font_from_db_with_style(font_db, family_name, Some(weight), style);
            if font_data.is_some() {
                log::info!("Successfully loaded {} font: {}", style_name, family_name);
            } else {
                log::warn!(
                    "{} font '{}' not found, will use primary font",
                    style_name
                        .chars()
                        .next()
                        .unwrap()
                        .to_uppercase()
                        .chain(style_name.chars().skip(1))
                        .collect::<String>(),
                    family_name
                );
            }
            font_data
        })
    }

    /// Load Unicode range-specific fonts.
    fn load_range_fonts(
        font_db: &mut Database,
        font_ranges: &[par_term_config::FontRange],
    ) -> Vec<UnicodeRangeFont> {
        let mut range_fonts = Vec::new();
        let mut next_font_index = 4; // After styled fonts (0-3)

        for range in font_ranges {
            log::info!(
                "Loading range font for U+{:04X}-U+{:04X}: {}",
                range.start,
                range.end,
                range.font_family
            );

            if let Some(font_data) = loader::load_font_from_db(font_db, &range.font_family) {
                range_fonts.push(UnicodeRangeFont {
                    start: range.start,
                    end: range.end,
                    font: font_data,
                    font_index: next_font_index,
                });
                log::info!(
                    "Successfully loaded range font: {} (index {})",
                    range.font_family,
                    next_font_index
                );
                next_font_index += 1;
            } else {
                log::warn!(
                    "Range font '{}' not found for U+{:04X}-U+{:04X}, skipping",
                    range.font_family,
                    range.start,
                    range.end
                );
            }
        }
        range_fonts
    }

    /// Get the appropriate font based on bold and italic attributes.
    fn get_styled_font(&self, bold: bool, italic: bool) -> &FontRef<'static> {
        match (bold, italic) {
            (true, true) => self
                .bold_italic
                .as_ref()
                .map(|f| &f.font_ref)
                .unwrap_or(&self.primary.font_ref),
            (true, false) => self
                .bold
                .as_ref()
                .map(|f| &f.font_ref)
                .unwrap_or(&self.primary.font_ref),
            (false, true) => self
                .italic
                .as_ref()
                .map(|f| &f.font_ref)
                .unwrap_or(&self.primary.font_ref),
            (false, false) => &self.primary.font_ref,
        }
    }

    /// Find a glyph for a character across the font fallback chain.
    ///
    /// # Arguments
    /// * `character` - Character to find glyph for
    /// * `bold` - Whether text should be bold
    /// * `italic` - Whether text should be italic
    ///
    /// # Returns
    /// `(font_index, glyph_id)` where font_index identifies which font contains the glyph.
    pub fn find_glyph(&self, character: char, bold: bool, italic: bool) -> Option<(usize, u16)> {
        // Try styled font first
        let styled_font = self.get_styled_font(bold, italic);
        let glyph_id = styled_font.charmap().map(character);
        if glyph_id != 0 {
            let font_idx = match (bold, italic) {
                (true, true) if self.bold_italic.is_some() => 3,
                (true, false) if self.bold.is_some() => 1,
                (false, true) if self.italic.is_some() => 2,
                _ => 0,
            };
            return Some((font_idx, glyph_id));
        }

        // Check Unicode range-specific fonts
        let char_code = character as u32;
        for range_font in &self.range_fonts {
            if char_code >= range_font.start && char_code <= range_font.end {
                let glyph_id = range_font.font.font_ref.charmap().map(character);
                if glyph_id != 0 {
                    log::info!(
                        "✓ Character '{}' (U+{:04X}) found in range font U+{:04X}-U+{:04X} (index {})",
                        character,
                        char_code,
                        range_font.start,
                        range_font.end,
                        range_font.font_index
                    );
                    return Some((range_font.font_index, glyph_id));
                } else {
                    log::warn!(
                        "✗ Character '{}' (U+{:04X}) in range U+{:04X}-U+{:04X} but glyph_id=0 (not in font)",
                        character,
                        char_code,
                        range_font.start,
                        range_font.end
                    );
                }
            }
        }

        // Try fallback fonts
        let fallback_start_index = 4 + self.range_fonts.len();
        for (idx, fallback) in self.fallbacks.iter().enumerate() {
            let glyph_id = fallback.font_ref.charmap().map(character);
            if glyph_id != 0 {
                if !character.is_ascii()
                    || character.is_ascii_punctuation()
                    || character.is_ascii_graphic()
                {
                    log::debug!(
                        "Character '{}' (U+{:04X}) found in fallback font index {}",
                        character,
                        character as u32,
                        fallback_start_index + idx
                    );
                }
                return Some((fallback_start_index + idx, glyph_id));
            }
        }

        log::debug!(
            "Character '{}' (U+{:04X}) not found in any font ({} total fonts)",
            character,
            character as u32,
            self.font_count()
        );
        None
    }

    /// Find a glyph for a character, excluding specific font indices.
    ///
    /// This is used when a font claims to have a glyph but can't render it
    /// (e.g., Apple Color Emoji has charmap entries but empty outlines for some symbols).
    /// The caller can retry with the font that failed excluded from the search.
    pub fn find_glyph_excluding(
        &self,
        character: char,
        bold: bool,
        italic: bool,
        excluded: &[usize],
    ) -> Option<(usize, u16)> {
        // Try styled font first (unless excluded)
        let styled_font = self.get_styled_font(bold, italic);
        let font_idx = match (bold, italic) {
            (true, true) if self.bold_italic.is_some() => 3,
            (true, false) if self.bold.is_some() => 1,
            (false, true) if self.italic.is_some() => 2,
            _ => 0,
        };
        if !excluded.contains(&font_idx) {
            let glyph_id = styled_font.charmap().map(character);
            if glyph_id != 0 {
                return Some((font_idx, glyph_id));
            }
        }

        // Check Unicode range-specific fonts
        let char_code = character as u32;
        for range_font in &self.range_fonts {
            if !excluded.contains(&range_font.font_index)
                && char_code >= range_font.start
                && char_code <= range_font.end
            {
                let glyph_id = range_font.font.font_ref.charmap().map(character);
                if glyph_id != 0 {
                    return Some((range_font.font_index, glyph_id));
                }
            }
        }

        // Try fallback fonts
        let fallback_start_index = 4 + self.range_fonts.len();
        for (idx, fallback) in self.fallbacks.iter().enumerate() {
            let font_index = fallback_start_index + idx;
            if excluded.contains(&font_index) {
                continue;
            }
            let glyph_id = fallback.font_ref.charmap().map(character);
            if glyph_id != 0 {
                return Some((font_index, glyph_id));
            }
        }

        None
    }

    /// Find the font index for a character in range fonts.
    #[allow(dead_code)]
    pub fn find_range_font_index(&self, char_code: u32) -> Option<(usize, u16)> {
        for range_font in &self.range_fonts {
            if char_code >= range_font.start && char_code <= range_font.end {
                let character = char::from_u32(char_code)?;
                let glyph_id = range_font.font.font_ref.charmap().map(character);
                if glyph_id != 0 {
                    return Some((range_font.font_index, glyph_id));
                }
            }
        }
        None
    }

    /// Get font reference by index.
    ///
    /// # Arguments
    /// * `font_index` - Font index (see struct documentation for layout)
    pub fn get_font(&self, font_index: usize) -> Option<&FontRef<'static>> {
        match font_index {
            0 => Some(&self.primary.font_ref),
            1 => self.bold.as_ref().map(|f| &f.font_ref),
            2 => self.italic.as_ref().map(|f| &f.font_ref),
            3 => self.bold_italic.as_ref().map(|f| &f.font_ref),
            idx if idx >= 4 => {
                let range_offset = idx - 4;
                if range_offset < self.range_fonts.len() {
                    Some(&self.range_fonts[range_offset].font.font_ref)
                } else {
                    let fallback_offset = range_offset - self.range_fonts.len();
                    self.fallbacks.get(fallback_offset).map(|fd| &fd.font_ref)
                }
            }
            _ => None,
        }
    }

    /// Get the primary font reference.
    #[allow(dead_code)]
    pub fn primary_font(&self) -> &FontRef<'static> {
        &self.primary.font_ref
    }

    /// Get number of fonts loaded (primary + styled + range + fallbacks).
    pub fn font_count(&self) -> usize {
        let styled_count = 1
            + self.bold.is_some() as usize
            + self.italic.is_some() as usize
            + self.bold_italic.is_some() as usize;
        styled_count + self.range_fonts.len() + self.fallbacks.len()
    }

    /// Get raw font data bytes for a font index.
    #[allow(dead_code)]
    pub fn get_font_data(&self, font_index: usize) -> Option<&[u8]> {
        match font_index {
            0 => Some(self.primary.data.as_slice()),
            1 => self.bold.as_ref().map(|f| f.data.as_slice()),
            2 => self.italic.as_ref().map(|f| f.data.as_slice()),
            3 => self.bold_italic.as_ref().map(|f| f.data.as_slice()),
            idx if idx >= 4 => {
                let range_offset = idx - 4;
                if range_offset < self.range_fonts.len() {
                    Some(self.range_fonts[range_offset].font.data.as_slice())
                } else {
                    let fallback_offset = range_offset - self.range_fonts.len();
                    self.fallbacks
                        .get(fallback_offset)
                        .map(|fd| fd.data.as_slice())
                }
            }
            _ => None,
        }
    }

    /// Get Arc reference to font data for a font index.
    fn get_font_data_arc(&self, font_index: usize) -> Arc<Vec<u8>> {
        match font_index {
            0 => Arc::clone(&self.primary.data),
            1 => self
                .bold
                .as_ref()
                .map(|f| Arc::clone(&f.data))
                .unwrap_or_else(|| Arc::clone(&self.primary.data)),
            2 => self
                .italic
                .as_ref()
                .map(|f| Arc::clone(&f.data))
                .unwrap_or_else(|| Arc::clone(&self.primary.data)),
            3 => self
                .bold_italic
                .as_ref()
                .map(|f| Arc::clone(&f.data))
                .unwrap_or_else(|| Arc::clone(&self.primary.data)),
            idx if idx >= 4 => {
                let range_offset = idx - 4;
                if range_offset < self.range_fonts.len() {
                    Arc::clone(&self.range_fonts[range_offset].font.data)
                } else {
                    let fallback_offset = range_offset - self.range_fonts.len();
                    self.fallbacks
                        .get(fallback_offset)
                        .map(|fd| Arc::clone(&fd.data))
                        .unwrap_or_else(|| Arc::clone(&self.primary.data))
                }
            }
            _ => Arc::clone(&self.primary.data),
        }
    }

    /// Shape text using the appropriate font.
    ///
    /// Uses HarfBuzz (via rustybuzz) for ligatures, kerning, and complex script support.
    #[allow(dead_code)]
    pub fn shape_text(
        &mut self,
        text: &str,
        bold: bool,
        italic: bool,
        options: ShapingOptions,
    ) -> Arc<ShapedRun> {
        let font_index = self.get_styled_font_index(bold, italic);
        let font_data_arc = self.get_font_data_arc(font_index);
        self.text_shaper
            .shape_text(text, font_data_arc.as_slice(), font_index, options)
    }

    /// Shape text using a specific font index.
    #[allow(dead_code)]
    pub fn shape_text_with_font_index(
        &mut self,
        text: &str,
        font_index: usize,
        options: ShapingOptions,
    ) -> Arc<ShapedRun> {
        let font_data_arc = self.get_font_data_arc(font_index);
        self.text_shaper
            .shape_text(text, font_data_arc.as_slice(), font_index, options)
    }

    /// Clear the text shaping cache.
    #[allow(dead_code)]
    pub fn clear_shape_cache(&mut self) {
        self.text_shaper.clear_cache();
    }

    /// Get the current size of the shape cache.
    #[allow(dead_code)]
    pub fn shape_cache_size(&self) -> usize {
        self.text_shaper.cache_size()
    }

    /// Find glyph(s) for an entire grapheme cluster.
    ///
    /// This is essential for rendering multi-character sequences like:
    /// - Flag emoji (🇺🇸) - regional indicator pairs
    /// - ZWJ sequences (👨‍👩‍👧‍👦) - family emoji
    /// - Skin tone modifiers (👋🏽)
    /// - Combining characters (é = e + acute accent)
    ///
    /// # Arguments
    /// * `grapheme` - The grapheme cluster string (may be multiple Unicode codepoints)
    /// * `bold` - Whether text should be bold
    /// * `italic` - Whether text should be italic
    ///
    /// # Returns
    /// `Some((font_index, glyph_id))` for the primary glyph representing the grapheme,
    /// or `None` if no suitable glyph was found.
    pub fn find_grapheme_glyph(
        &mut self,
        grapheme: &str,
        bold: bool,
        italic: bool,
    ) -> Option<(usize, u16)> {
        let chars: Vec<char> = grapheme.chars().collect();

        // Fast path: single character graphemes use existing lookup
        if chars.len() == 1 {
            return self.find_glyph(chars[0], bold, italic);
        }

        // Multi-character grapheme: use text shaping to find the composed glyph
        // First, determine which font to use based on the first character
        let first_char = chars[0];
        let char_code = first_char as u32;

        // Check Unicode range-specific fonts first (emoji fonts)
        for range_font in &self.range_fonts {
            if char_code >= range_font.start && char_code <= range_font.end {
                // Shape the grapheme with this font
                let font_data = range_font.font.data.as_slice();
                let options = ShapingOptions::default();
                let shaped = self.text_shaper.shape_text(
                    grapheme,
                    font_data,
                    range_font.font_index,
                    options,
                );

                // Check if shaping produced a valid glyph
                if !shaped.glyphs.is_empty() && shaped.glyphs[0].glyph_id != 0 {
                    log::debug!(
                        "Grapheme '{}' ({} chars) shaped to glyph {} in range font index {}",
                        grapheme,
                        chars.len(),
                        shaped.glyphs[0].glyph_id,
                        range_font.font_index
                    );
                    return Some((range_font.font_index, shaped.glyphs[0].glyph_id as u16));
                }
            }
        }

        // Try styled font
        let font_index = self.get_styled_font_index(bold, italic);
        let font_data_arc = self.get_font_data_arc(font_index);
        let options = ShapingOptions::default();
        let shaped =
            self.text_shaper
                .shape_text(grapheme, font_data_arc.as_slice(), font_index, options);

        if !shaped.glyphs.is_empty() && shaped.glyphs[0].glyph_id != 0 {
            log::debug!(
                "Grapheme '{}' ({} chars) shaped to glyph {} in styled font index {}",
                grapheme,
                chars.len(),
                shaped.glyphs[0].glyph_id,
                font_index
            );
            return Some((font_index, shaped.glyphs[0].glyph_id as u16));
        }

        // Try fallback fonts
        let fallback_start_index = 4 + self.range_fonts.len();
        for (idx, fallback) in self.fallbacks.iter().enumerate() {
            let font_idx = fallback_start_index + idx;
            let options = ShapingOptions::default();
            let shaped =
                self.text_shaper
                    .shape_text(grapheme, fallback.data.as_slice(), font_idx, options);

            if !shaped.glyphs.is_empty() && shaped.glyphs[0].glyph_id != 0 {
                log::debug!(
                    "Grapheme '{}' ({} chars) shaped to glyph {} in fallback font index {}",
                    grapheme,
                    chars.len(),
                    shaped.glyphs[0].glyph_id,
                    font_idx
                );
                return Some((font_idx, shaped.glyphs[0].glyph_id as u16));
            }
        }

        // Fallback: try to render just the first character
        log::debug!(
            "Grapheme '{}' ({} chars) not found as composed glyph, falling back to first char",
            grapheme,
            chars.len()
        );
        self.find_glyph(first_char, bold, italic)
    }

    /// Get the font index for a given style combination.
    fn get_styled_font_index(&self, bold: bool, italic: bool) -> usize {
        match (bold, italic) {
            (true, true) if self.bold_italic.is_some() => 3,
            (true, false) if self.bold.is_some() => 1,
            (false, true) if self.italic.is_some() => 2,
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedded_font_loads() {
        let fm = FontManager::new(None, None, None, None, &[]);
        assert!(fm.is_ok(), "FontManager should load with embedded font");
        let fm = fm.unwrap();
        assert!(fm.font_count() >= 1, "Should have at least one font");
    }

    #[test]
    fn test_primary_font_glyph_lookup() {
        let fm = FontManager::new(None, None, None, None, &[]).unwrap();
        // ASCII characters should be found in the embedded font
        let result = fm.find_glyph('A', false, false);
        assert!(result.is_some(), "Should find glyph for 'A'");
        let (font_idx, glyph_id) = result.unwrap();
        assert_eq!(font_idx, 0, "Should be in primary font");
        assert!(glyph_id > 0, "Glyph ID should be nonzero");
    }

    #[test]
    fn test_get_font_by_index() {
        let fm = FontManager::new(None, None, None, None, &[]).unwrap();
        assert!(
            fm.get_font(0).is_some(),
            "Primary font should exist at index 0"
        );
    }
}
