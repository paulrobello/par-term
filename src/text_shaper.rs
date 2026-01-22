/// Text shaping module using HarfBuzz via rustybuzz
///
/// This module provides text shaping capabilities for:
/// - Ligatures (fi, fl, ffi, ffl, etc.)
/// - Complex emoji sequences (flags, skin tones, ZWJ sequences)
/// - Complex scripts (Arabic, Devanagari, etc.)
/// - Bidirectional text (RTL languages)
/// - Kerning and contextual alternates
///
/// # Architecture
///
/// The text shaping pipeline:
/// 1. Grapheme cluster detection (unicode-segmentation)
/// 2. Script and direction detection (unicode-bidi)
/// 3. Font feature selection (based on script/language)
/// 4. Text shaping (rustybuzz)
/// 5. Glyph positioning and advances
/// 6. Result caching for performance
///
/// # Usage
///
/// ```ignore
/// let shaper = TextShaper::new();
/// let shaped = shaper.shape_text(
///     "Hello 🇺🇸 world",
///     &font,
///     ShapingOptions::default()
/// );
/// ```
use rustybuzz::{Face, Feature, GlyphBuffer, Language, Script, UnicodeBuffer};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;

/// A single shaped glyph with positioning information
#[derive(Debug, Clone, Copy)]
pub struct ShapedGlyph {
    /// Glyph ID from the font
    #[allow(dead_code)]
    pub glyph_id: u32,

    /// Cluster index (which input character(s) this glyph represents)
    #[allow(dead_code)]
    pub cluster: u32,

    /// Horizontal advance width in pixels
    pub x_advance: f32,

    /// Vertical advance (usually 0 for horizontal text)
    #[allow(dead_code)]
    pub y_advance: f32,

    /// Horizontal offset from the current position
    #[allow(dead_code)]
    pub x_offset: f32,

    /// Vertical offset from the baseline
    #[allow(dead_code)]
    pub y_offset: f32,
}

/// Options for text shaping
#[derive(Debug, Clone)]
pub struct ShapingOptions {
    /// Enable standard ligatures (fi, fl, etc.)
    pub enable_ligatures: bool,

    /// Enable kerning adjustments
    pub enable_kerning: bool,

    /// Enable contextual alternates
    pub enable_contextual_alternates: bool,

    /// Script hint (e.g., "arab" for Arabic, "deva" for Devanagari)
    pub script: Option<String>,

    /// Language hint (e.g., "en" for English, "ar" for Arabic)
    pub language: Option<String>,

    /// Text direction (true = RTL, false = LTR)
    pub rtl: bool,
}

impl Default for ShapingOptions {
    fn default() -> Self {
        Self {
            enable_ligatures: true,
            enable_kerning: true,
            enable_contextual_alternates: true,
            script: None,
            language: None,
            rtl: false,
        }
    }
}

/// Result of shaping a text run
#[derive(Debug, Clone)]
pub struct ShapedRun {
    /// The input text that was shaped
    #[allow(dead_code)]
    pub text: String,

    /// The shaped glyphs
    #[allow(dead_code)]
    pub glyphs: Vec<ShapedGlyph>,

    /// Total advance width in pixels
    #[allow(dead_code)]
    pub total_advance: f32,

    /// Grapheme cluster boundaries (indices into the text)
    #[allow(dead_code)]
    pub cluster_boundaries: Vec<usize>,
}

/// Cache key for shaped text runs
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct ShapeCacheKey {
    text: String,
    font_index: usize,
    enable_ligatures: bool,
    enable_kerning: bool,
    script: Option<String>,
    language: Option<String>,
    rtl: bool,
}

/// Text shaper using HarfBuzz via rustybuzz
pub struct TextShaper {
    /// Cache of shaped text runs
    shape_cache: HashMap<ShapeCacheKey, Arc<ShapedRun>>,

    /// Maximum cache size (number of entries)
    max_cache_size: usize,
}

impl TextShaper {
    /// Create a new text shaper with default settings
    pub fn new() -> Self {
        Self::with_cache_size(1000)
    }

    /// Create a new text shaper with a specific cache size
    pub fn with_cache_size(max_cache_size: usize) -> Self {
        Self {
            shape_cache: HashMap::new(),
            max_cache_size,
        }
    }

    /// Detect grapheme clusters in the input text
    ///
    /// This is crucial for:
    /// - Regional indicator pairs (flag emoji like 🇺🇸)
    /// - ZWJ sequences (emoji like 👨‍👩‍👧‍👦)
    /// - Combining characters (diacritics like é)
    /// - Emoji with skin tone modifiers (👋🏽)
    pub fn detect_grapheme_clusters<'a>(&self, text: &'a str) -> Vec<(usize, &'a str)> {
        text.grapheme_indices(true).collect()
    }

    /// Detect regional indicator pairs (flag emoji)
    ///
    /// Regional indicators are pairs of characters U+1F1E6-U+1F1FF
    /// that combine to form flag emoji (e.g., 🇺🇸 = U+1F1FA + U+1F1F8)
    #[allow(dead_code)]
    pub fn is_regional_indicator_pair(&self, grapheme: &str) -> bool {
        let chars: Vec<char> = grapheme.chars().collect();
        if chars.len() == 2 {
            let is_ri = |c: char| {
                let code = c as u32;
                (0x1F1E6..=0x1F1FF).contains(&code)
            };
            is_ri(chars[0]) && is_ri(chars[1])
        } else {
            false
        }
    }

    /// Check if a grapheme contains a Zero Width Joiner (ZWJ)
    ///
    /// ZWJ sequences are used for complex emoji like family emoji (👨‍👩‍👧‍👦)
    #[allow(dead_code)]
    pub fn contains_zwj(&self, grapheme: &str) -> bool {
        grapheme.contains('\u{200D}')
    }

    /// Shape a text run using rustybuzz
    ///
    /// This performs the actual text shaping, applying OpenType features
    /// like ligatures, kerning, and contextual alternates.
    ///
    /// # Arguments
    /// * `text` - The text to shape
    /// * `font_data` - The font data (TrueType/OpenType)
    /// * `font_index` - Font index for cache key
    /// * `options` - Shaping options
    ///
    /// # Returns
    /// A `ShapedRun` containing the shaped glyphs and metadata
    pub fn shape_text(
        &mut self,
        text: &str,
        font_data: &[u8],
        font_index: usize,
        options: ShapingOptions,
    ) -> Arc<ShapedRun> {
        // Check cache first
        let cache_key = ShapeCacheKey {
            text: text.to_string(),
            font_index,
            enable_ligatures: options.enable_ligatures,
            enable_kerning: options.enable_kerning,
            script: options.script.clone(),
            language: options.language.clone(),
            rtl: options.rtl,
        };

        if let Some(cached) = self.shape_cache.get(&cache_key) {
            return Arc::clone(cached);
        }

        // Detect grapheme clusters
        let clusters = self.detect_grapheme_clusters(text);
        let cluster_boundaries: Vec<usize> = clusters.iter().map(|(idx, _)| *idx).collect();

        // Create rustybuzz Face from font data
        let face = match Face::from_slice(font_data, 0) {
            Some(face) => face,
            None => {
                // If font parsing fails, return empty shaped run
                let run = Arc::new(ShapedRun {
                    text: text.to_string(),
                    glyphs: vec![],
                    total_advance: 0.0,
                    cluster_boundaries,
                });
                return run;
            }
        };

        // Create Unicode buffer and add text
        let mut unicode_buffer = UnicodeBuffer::new();
        unicode_buffer.push_str(text);

        // Set direction
        unicode_buffer.set_direction(if options.rtl {
            rustybuzz::Direction::RightToLeft
        } else {
            rustybuzz::Direction::LeftToRight
        });

        // Set script hint if provided
        if let Some(ref script_str) = options.script {
            // Convert 4-letter script code to Script (e.g., "arab", "latn", "deva")
            if let Ok(script) = Script::from_str(script_str) {
                unicode_buffer.set_script(script);
            }
        }

        // Set language hint if provided
        if let Some(ref lang_str) = options.language {
            // Convert language code to Language (e.g., "en", "ar", "zh")
            if let Ok(lang) = Language::from_str(lang_str) {
                unicode_buffer.set_language(lang);
            }
        }

        // Build OpenType feature list based on options
        // Use Feature::from_str() which parses standard feature notation
        let mut features = Vec::new();

        // Standard ligatures (liga): fi, fl, ffi, ffl
        if options.enable_ligatures {
            if let Ok(feat) = Feature::from_str("liga") {
                features.push(feat);
            }
            // Contextual ligatures (clig) - often includes programming ligatures like ->, =>
            if let Ok(feat) = Feature::from_str("clig") {
                features.push(feat);
            }
            // Discretionary ligatures (dlig) - programming ligatures in many fonts
            if let Ok(feat) = Feature::from_str("dlig") {
                features.push(feat);
            }
        }

        // Kerning adjustments (kern)
        if options.enable_kerning
            && let Ok(feat) = Feature::from_str("kern")
        {
            features.push(feat);
        }

        // Contextual alternates (calt) - enables context-sensitive glyph substitution
        if options.enable_contextual_alternates
            && let Ok(feat) = Feature::from_str("calt")
        {
            features.push(feat);
        }

        // Glyph composition/decomposition (ccmp) - required for proper emoji and complex scripts
        if let Ok(feat) = Feature::from_str("ccmp") {
            features.push(feat);
        }

        // Localized forms (locl) - language-specific glyph variants
        if let Ok(feat) = Feature::from_str("locl") {
            features.push(feat);
        }

        // Shape the text with OpenType features
        let glyph_buffer = rustybuzz::shape(&face, &features, unicode_buffer);

        // Extract shaped glyphs
        let glyphs = self.extract_shaped_glyphs(&glyph_buffer);

        // Calculate total advance
        let total_advance = glyphs.iter().map(|g| g.x_advance).sum();

        // Create shaped run
        let shaped_run = Arc::new(ShapedRun {
            text: text.to_string(),
            glyphs,
            total_advance,
            cluster_boundaries,
        });

        // Cache the result (with LRU eviction if needed)
        if self.shape_cache.len() >= self.max_cache_size {
            // Simple eviction: remove first entry
            // TODO: Implement proper LRU eviction
            if let Some(key) = self.shape_cache.keys().next().cloned() {
                self.shape_cache.remove(&key);
            }
        }

        self.shape_cache.insert(cache_key, Arc::clone(&shaped_run));

        shaped_run
    }

    /// Extract shaped glyphs from HarfBuzz glyph buffer
    fn extract_shaped_glyphs(&self, buffer: &GlyphBuffer) -> Vec<ShapedGlyph> {
        let glyph_infos = buffer.glyph_infos();
        let glyph_positions = buffer.glyph_positions();

        glyph_infos
            .iter()
            .zip(glyph_positions.iter())
            .map(|(info, pos)| ShapedGlyph {
                glyph_id: info.glyph_id,
                cluster: info.cluster,
                x_advance: pos.x_advance as f32,
                y_advance: pos.y_advance as f32,
                x_offset: pos.x_offset as f32,
                y_offset: pos.y_offset as f32,
            })
            .collect()
    }

    /// Clear the shape cache
    #[allow(dead_code)]
    pub fn clear_cache(&mut self) {
        self.shape_cache.clear();
    }

    /// Get the current cache size
    #[allow(dead_code)]
    pub fn cache_size(&self) -> usize {
        self.shape_cache.len()
    }
}

impl Default for TextShaper {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grapheme_cluster_detection() {
        let shaper = TextShaper::new();

        // Test simple ASCII
        let clusters = shaper.detect_grapheme_clusters("hello");
        assert_eq!(clusters.len(), 5);

        // Test emoji with skin tone
        let clusters = shaper.detect_grapheme_clusters("👋🏽");
        assert_eq!(clusters.len(), 1); // Should be one grapheme

        // Test flag emoji
        let clusters = shaper.detect_grapheme_clusters("🇺🇸");
        assert_eq!(clusters.len(), 1); // Should be one grapheme
    }

    #[test]
    fn test_regional_indicator_detection() {
        let shaper = TextShaper::new();

        // US flag
        assert!(shaper.is_regional_indicator_pair("🇺🇸"));

        // Regular text
        assert!(!shaper.is_regional_indicator_pair("US"));

        // Single character
        assert!(!shaper.is_regional_indicator_pair("A"));
    }

    #[test]
    fn test_zwj_detection() {
        let shaper = TextShaper::new();

        // Family emoji (contains ZWJ)
        assert!(shaper.contains_zwj("👨‍👩‍👧‍👦"));

        // Regular emoji (no ZWJ)
        assert!(!shaper.contains_zwj("👋"));

        // Regular text (no ZWJ)
        assert!(!shaper.contains_zwj("hello"));
    }
}
