/// Font management with fallback chain for comprehensive Unicode coverage
/// Supports CJK characters, emoji, flags, and symbols
use anyhow::Result;
use fontdb::{Database, Family, Query};
use std::sync::Arc;
use swash::FontRef;

use crate::text_shaper::{ShapedRun, ShapingOptions, TextShaper};

/// Stores font data with lifetime management
#[derive(Clone)]
pub struct FontData {
    #[allow(dead_code)]
    pub data: Arc<Vec<u8>>,
    pub font_ref: FontRef<'static>,
}

impl std::fmt::Debug for FontData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FontData")
            .field("data_len", &self.data.len())
            .finish()
    }
}

impl FontData {
    /// Create a new FontData from bytes
    pub fn new(data: Vec<u8>) -> Option<Self> {
        let data_arc = Arc::new(data);

        // SAFETY: We ensure the data outlives the FontRef by storing it in an Arc
        // The FontRef will never outlive the FontData struct
        let font_ref = unsafe {
            let bytes = data_arc.as_slice();
            let static_bytes: &'static [u8] = std::mem::transmute(bytes);
            FontRef::from_index(static_bytes, 0)?
        };

        Some(FontData {
            data: data_arc,
            font_ref,
        })
    }
}

/// Font mapping for a Unicode range
#[derive(Debug, Clone)]
pub struct UnicodeRangeFont {
    /// Start of Unicode range (inclusive)
    pub start: u32,
    /// End of Unicode range (inclusive)
    pub end: u32,
    /// Font data for this range
    pub font: FontData,
    /// Font index in the overall font list
    pub font_index: usize,
}

/// Manages multiple fonts with fallback chain
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
    /// Create a new FontManager with primary font and system fallbacks
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
        font_ranges: &[crate::config::FontRange],
    ) -> Result<Self> {
        let mut font_db = Database::new();

        // Load system fonts
        font_db.load_system_fonts();

        log::info!("Loaded {} system fonts", font_db.len());

        // Try to load primary font from system or use embedded
        let primary = if let Some(family) = primary_family {
            log::info!("Attempting to load primary font: {}", family);
            match Self::load_font_from_db(&mut font_db, family) {
                Some(font_data) => {
                    log::info!("Successfully loaded primary font: {}", family);
                    font_data
                }
                None => {
                    log::warn!(
                        "Primary font '{}' not found, using embedded DejaVu Sans Mono",
                        family
                    );
                    Self::load_embedded_font()?
                }
            }
        } else {
            log::info!("No primary font specified, using embedded DejaVu Sans Mono");
            Self::load_embedded_font()?
        };

        // Build fallback chain with fonts known to have good coverage
        let mut fallbacks = Vec::new();

        // Define fallback chain with priority order
        let fallback_families = [
            // Nerd Fonts (first priority for icon/symbol support)
            "JetBrainsMono Nerd Font",
            "JetBrainsMono NF",
            "FiraCode Nerd Font",
            "FiraCode NF",
            "Hack Nerd Font",
            "Hack NF",
            "MesloLGS NF",
            // Standard monospace fonts
            "JetBrains Mono",
            "Fira Code",
            "Consolas",
            "Monaco",
            "Menlo",
            "Courier New",
            // CJK fonts (critical for Asian language support)
            "Noto Sans CJK JP",
            "Noto Sans CJK SC",
            "Noto Sans CJK TC",
            "Noto Sans CJK KR",
            "Microsoft YaHei",
            "MS Gothic",
            "SimHei",
            "Malgun Gothic",
            // Symbol and emoji fonts (includes flag support)
            "Symbols Nerd Font",
            "Noto Color Emoji",
            "Apple Color Emoji",
            "Segoe UI Emoji",
            "Segoe UI Symbol",
            "Symbola",
            "Arial Unicode MS",
            // General fallbacks
            "DejaVu Sans",
            "Arial",
            "Liberation Sans",
        ];

        for family_name in fallback_families {
            if let Some(font_data) = Self::load_font_from_db(&mut font_db, family_name) {
                log::debug!("Added fallback font: {}", family_name);
                fallbacks.push(font_data);
            }
        }

        log::info!("Loaded {} fallback fonts", fallbacks.len());

        // Load bold font if specified
        let bold = bold_family.and_then(|family| {
            log::info!("Attempting to load bold font: {}", family);
            // Try loading with bold weight
            let font_data = Self::load_font_from_db_with_style(
                &mut font_db,
                family,
                Some(fontdb::Weight::BOLD),
                None,
            );
            if font_data.is_some() {
                log::info!("Successfully loaded bold font: {}", family);
            } else {
                log::warn!("Bold font '{}' not found, will use primary font", family);
            }
            font_data
        });

        // Load italic font if specified
        let italic = italic_family.and_then(|family| {
            log::info!("Attempting to load italic font: {}", family);
            // Try loading with italic style
            let font_data = Self::load_font_from_db_with_style(
                &mut font_db,
                family,
                None,
                Some(fontdb::Style::Italic),
            );
            if font_data.is_some() {
                log::info!("Successfully loaded italic font: {}", family);
            } else {
                log::warn!("Italic font '{}' not found, will use primary font", family);
            }
            font_data
        });

        // Load bold italic font if specified
        let bold_italic = bold_italic_family.and_then(|family| {
            log::info!("Attempting to load bold italic font: {}", family);
            // Try loading with bold weight and italic style
            let font_data = Self::load_font_from_db_with_style(
                &mut font_db,
                family,
                Some(fontdb::Weight::BOLD),
                Some(fontdb::Style::Italic),
            );
            if font_data.is_some() {
                log::info!("Successfully loaded bold italic font: {}", family);
            } else {
                log::warn!(
                    "Bold italic font '{}' not found, will use primary font",
                    family
                );
            }
            font_data
        });

        // Load Unicode range-specific fonts
        let mut range_fonts = Vec::new();
        let mut next_font_index = 4; // After styled fonts (0-3), BEFORE fallbacks

        for range in font_ranges {
            log::info!(
                "Loading range font for U+{:04X}-U+{:04X}: {}",
                range.start,
                range.end,
                range.font_family
            );

            if let Some(font_data) = Self::load_font_from_db(&mut font_db, &range.font_family) {
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

    /// Load the embedded DejaVu Sans Mono font
    fn load_embedded_font() -> Result<FontData> {
        let font_data: &'static [u8] = include_bytes!("../fonts/DejaVuSansMono.ttf");
        let data = font_data.to_vec();

        FontData::new(data).ok_or_else(|| anyhow::anyhow!("Failed to load embedded font"))
    }

    /// Load a font from the system font database
    fn load_font_from_db(db: &mut Database, family_name: &str) -> Option<FontData> {
        Self::load_font_from_db_with_style(db, family_name, None, None)
    }

    fn load_font_from_db_with_style(
        db: &mut Database,
        family_name: &str,
        weight: Option<fontdb::Weight>,
        style: Option<fontdb::Style>,
    ) -> Option<FontData> {
        // Query for the font family with optional weight and style
        let query = Query {
            families: &[Family::Name(family_name)],
            weight: weight.unwrap_or(fontdb::Weight::NORMAL),
            style: style.unwrap_or(fontdb::Style::Normal),
            ..Query::default()
        };

        let id = db.query(&query)?;

        // Load font data from the database
        // SAFETY: make_shared_face_data is safe when called with a valid ID from query()
        let (data, _) = unsafe { db.make_shared_face_data(id)? };

        // Convert the shared data to Vec<u8>
        let bytes = data.as_ref().as_ref();
        FontData::new(bytes.to_vec())
    }

    /// Get the appropriate font based on bold and italic attributes
    ///
    /// # Arguments
    /// * `bold` - Whether text should be bold
    /// * `italic` - Whether text should be italic
    ///
    /// # Returns
    /// Reference to the font to use (primary, bold, italic, bold-italic, or fallback to primary)
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

    /// Find a glyph for a character across the font fallback chain
    ///
    /// # Arguments
    /// * `character` - Character to find glyph for
    /// * `bold` - Whether text should be bold
    /// * `italic` - Whether text should be italic
    ///
    /// # Returns
    /// (font_index, glyph_id) where font_index 0-3 are styled fonts, >3 are fallbacks
    /// Font indices: 0 = primary/regular, 1 = bold, 2 = italic, 3 = bold-italic, 4+ = fallbacks
    pub fn find_glyph(&self, character: char, bold: bool, italic: bool) -> Option<(usize, u16)> {
        // Try styled font first (bold, italic, or bold-italic)
        let styled_font = self.get_styled_font(bold, italic);
        let glyph_id = styled_font.charmap().map(character);
        if glyph_id != 0 {
            // Determine which font index to return based on style
            let font_idx = match (bold, italic) {
                (true, true) if self.bold_italic.is_some() => 3, // Bold-italic font
                (true, false) if self.bold.is_some() => 1,       // Bold font
                (false, true) if self.italic.is_some() => 2,     // Italic font
                _ => 0,                                          // Primary/regular font
            };
            return Some((font_idx, glyph_id));
        }

        // Check Unicode range-specific fonts if character falls in a range
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

        // Try fallback fonts (starting after styled fonts and range fonts)
        let fallback_start_index = 4 + self.range_fonts.len();
        for (idx, fallback) in self.fallbacks.iter().enumerate() {
            let glyph_id = fallback.font_ref.charmap().map(character);
            if glyph_id != 0 {
                // Log when we use a fallback font (useful for debugging)
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

        // Character not found in any font
        log::debug!(
            "Character '{}' (U+{:04X}) not found in any font ({} total fonts)",
            character,
            character as u32,
            self.font_count()
        );
        None
    }

    /// Find the font index for a character in range fonts
    /// Returns None if the character is not in any range font
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

    /// Get font reference by index
    ///
    /// # Arguments
    /// * `font_index` - Font index: 0 = primary, 1 = bold, 2 = italic, 3 = bold-italic,
    ///   4..4+range_fonts.len() = range fonts, remaining = fallbacks
    ///
    /// # Returns
    /// Reference to the font at the given index, or None if invalid index
    pub fn get_font(&self, font_index: usize) -> Option<&FontRef<'static>> {
        match font_index {
            0 => Some(&self.primary.font_ref),
            1 => self.bold.as_ref().map(|f| &f.font_ref),
            2 => self.italic.as_ref().map(|f| &f.font_ref),
            3 => self.bold_italic.as_ref().map(|f| &f.font_ref),
            idx if idx >= 4 => {
                let range_offset = idx - 4;
                if range_offset < self.range_fonts.len() {
                    // Range font
                    Some(&self.range_fonts[range_offset].font.font_ref)
                } else {
                    // Fallback font
                    let fallback_offset = range_offset - self.range_fonts.len();
                    self.fallbacks.get(fallback_offset).map(|fd| &fd.font_ref)
                }
            }
            _ => None,
        }
    }

    /// Get the primary font reference
    #[allow(dead_code)]
    pub fn primary_font(&self) -> &FontRef<'static> {
        &self.primary.font_ref
    }

    /// Get number of fonts loaded (including primary, styled fonts, range fonts, and fallbacks)
    pub fn font_count(&self) -> usize {
        let styled_count = 1 // Primary
            + self.bold.is_some() as usize
            + self.italic.is_some() as usize
            + self.bold_italic.is_some() as usize;
        styled_count + self.range_fonts.len() + self.fallbacks.len()
    }

    /// Get raw font data bytes for a font index
    ///
    /// This is used by the text shaper to access the font data for shaping.
    ///
    /// # Arguments
    /// * `font_index` - Font index: 0 = primary, 1 = bold, 2 = italic, 3 = bold-italic,
    ///   4..4+range_fonts.len() = range fonts, remaining = fallbacks
    ///
    /// # Returns
    /// Reference to the raw font data bytes, or None if invalid index
    #[allow(dead_code)]
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
                    // Range font
                    Some(self.range_fonts[range_offset].font.data.as_slice())
                } else {
                    // Fallback font
                    let fallback_offset = range_offset - self.range_fonts.len();
                    self.fallbacks
                        .get(fallback_offset)
                        .map(|fd| fd.data.as_slice())
                }
            }
            _ => None,
        }
    }

    /// Shape text using the appropriate font
    ///
    /// This method uses HarfBuzz (via rustybuzz) to shape text with ligatures,
    /// kerning, and complex script support.
    ///
    /// # Arguments
    /// * `text` - The text to shape
    /// * `bold` - Whether the text is bold
    /// * `italic` - Whether the text is italic
    /// * `options` - Shaping options (ligatures, kerning, etc.)
    ///
    /// # Returns
    /// A `ShapedRun` containing the shaped glyphs and metadata
    #[allow(dead_code)]
    #[allow(dead_code)]
    pub fn shape_text(
        &mut self,
        text: &str,
        bold: bool,
        italic: bool,
        options: ShapingOptions,
    ) -> Arc<ShapedRun> {
        // Determine which font to use based on style
        let font_index = self.get_styled_font_index(bold, italic);

        // Get the font data for shaping (cloning to avoid borrow checker issues)
        // The Arc::clone is cheap as it's just incrementing a reference count
        let font_data_arc = match font_index {
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
        };

        // Shape the text using the text shaper
        self.text_shaper
            .shape_text(text, font_data_arc.as_slice(), font_index, options)
    }

    /// Shape text using a specific font index
    ///
    /// This method allows you to shape text with a specific font, rather than
    /// using the styled font selection. This is useful for emoji and symbols
    /// that need to be shaped with their dedicated fonts.
    ///
    /// # Arguments
    /// * `text` - The text to shape
    /// * `font_index` - The specific font index to use
    /// * `options` - Shaping options (ligatures, kerning, etc.)
    ///
    /// # Returns
    /// A `ShapedRun` containing the shaped glyphs and metadata
    #[allow(dead_code)]
    pub fn shape_text_with_font_index(
        &mut self,
        text: &str,
        font_index: usize,
        options: ShapingOptions,
    ) -> Arc<ShapedRun> {
        // Get the font data for the specified index
        let font_data_arc = match font_index {
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
        };

        // Shape the text using the text shaper
        self.text_shaper
            .shape_text(text, font_data_arc.as_slice(), font_index, options)
    }

    /// Get the font index for a given style combination
    ///
    /// This is used internally to determine which font to use for shaping.
    #[allow(dead_code)]
    #[allow(dead_code)]
    fn get_styled_font_index(&self, bold: bool, italic: bool) -> usize {
        match (bold, italic) {
            (true, true) if self.bold_italic.is_some() => 3, // Bold-italic font
            (true, false) if self.bold.is_some() => 1,       // Bold font
            (false, true) if self.italic.is_some() => 2,     // Italic font
            _ => 0,                                          // Primary/regular font
        }
    }

    /// Clear the text shaping cache
    ///
    /// This should be called when fonts are reloaded or changed.
    #[allow(dead_code)]
    #[allow(dead_code)]
    pub fn clear_shape_cache(&mut self) {
        self.text_shaper.clear_cache();
    }

    /// Get the current size of the shape cache
    #[allow(dead_code)]
    #[allow(dead_code)]
    pub fn shape_cache_size(&self) -> usize {
        self.text_shaper.cache_size()
    }
}
