//! Integration tests for par-term-fonts crate.

use par_term_fonts::font_manager::{FALLBACK_FAMILIES, FontData, FontManager};
use par_term_fonts::text_shaper::{ShapingOptions, TextShaper};

/// Embedded DejaVu Sans Mono for testing.
const TEST_FONT: &[u8] = include_bytes!("../fonts/DejaVuSansMono.ttf");

#[test]
fn test_font_data_from_embedded() {
    let data = TEST_FONT.to_vec();
    let font_data = FontData::new(data);
    assert!(
        font_data.is_some(),
        "Should load embedded font successfully"
    );
}

#[test]
fn test_font_data_invalid_bytes() {
    let data = vec![0u8; 100];
    let font_data = FontData::new(data);
    assert!(
        font_data.is_none(),
        "Should return None for invalid font data"
    );
}

#[test]
fn test_font_data_empty_bytes() {
    let data = vec![];
    let font_data = FontData::new(data);
    assert!(font_data.is_none(), "Should return None for empty data");
}

#[test]
fn test_font_data_clone() {
    let data = TEST_FONT.to_vec();
    let font_data = FontData::new(data).unwrap();
    let cloned = font_data.clone();
    // Both should have the same underlying data
    assert_eq!(font_data.data.len(), cloned.data.len());
}

#[test]
fn test_font_data_debug() {
    let data = TEST_FONT.to_vec();
    let font_data = FontData::new(data).unwrap();
    let debug_str = format!("{:?}", font_data);
    assert!(debug_str.contains("FontData"));
    assert!(debug_str.contains("data_len"));
}

#[test]
fn test_font_manager_with_embedded() {
    // Create FontManager with no specified fonts (uses embedded fallback)
    let manager = FontManager::new(None, None, None, None, &[]);
    assert!(
        manager.is_ok(),
        "FontManager should create with embedded font"
    );
    let manager = manager.unwrap();
    assert!(
        manager.font_count() >= 1,
        "Should have at least primary font"
    );
}

#[test]
fn test_font_manager_find_ascii_glyph() {
    let manager = FontManager::new(None, None, None, None, &[]).unwrap();
    // ASCII 'A' should always be found in the primary font
    let result = manager.find_glyph('A', false, false);
    assert!(result.is_some(), "Should find glyph for 'A'");
    let (font_idx, glyph_id) = result.unwrap();
    assert_eq!(font_idx, 0, "ASCII should be in primary font (index 0)");
    assert_ne!(glyph_id, 0, "Glyph ID should not be 0");
}

#[test]
fn test_font_manager_find_space_glyph() {
    let manager = FontManager::new(None, None, None, None, &[]).unwrap();
    let result = manager.find_glyph(' ', false, false);
    assert!(result.is_some(), "Should find glyph for space");
}

#[test]
fn test_font_manager_get_font() {
    let manager = FontManager::new(None, None, None, None, &[]).unwrap();
    // Primary font should always be accessible
    assert!(manager.get_font(0).is_some(), "Primary font should exist");
}

#[test]
fn test_fallback_families_not_empty() {
    assert!(
        !FALLBACK_FAMILIES.is_empty(),
        "Fallback families list should not be empty"
    );
}

#[test]
fn test_text_shaper_creation() {
    let shaper = TextShaper::new();
    assert_eq!(shaper.cache_size(), 0, "New shaper should have empty cache");
}

#[test]
fn test_text_shaper_with_cache_size() {
    let shaper = TextShaper::with_cache_size(500);
    assert_eq!(shaper.cache_size(), 0);
}

#[test]
fn test_text_shaper_default() {
    let shaper = TextShaper::default();
    assert_eq!(shaper.cache_size(), 0);
}

#[test]
fn test_text_shaper_shape_with_embedded_font() {
    let mut shaper = TextShaper::new();
    let options = ShapingOptions::default();
    let result = shaper.shape_text("Hello", TEST_FONT, 0, options);
    assert!(!result.glyphs.is_empty(), "Should produce shaped glyphs");
    assert_eq!(result.text, "Hello");
    assert!(result.total_advance > 0.0, "Should have positive advance");
}

#[test]
fn test_text_shaper_caching() {
    let mut shaper = TextShaper::new();
    let options = ShapingOptions::default();

    // Shape the same text twice
    let _result1 = shaper.shape_text("test", TEST_FONT, 0, options.clone());
    assert_eq!(shaper.cache_size(), 1, "Should have 1 cached entry");

    let _result2 = shaper.shape_text("test", TEST_FONT, 0, options);
    assert_eq!(
        shaper.cache_size(),
        1,
        "Cache should still have 1 entry (hit)"
    );
}

#[test]
fn test_text_shaper_clear_cache() {
    let mut shaper = TextShaper::new();
    let options = ShapingOptions::default();
    let _ = shaper.shape_text("test", TEST_FONT, 0, options);
    assert_eq!(shaper.cache_size(), 1);
    shaper.clear_cache();
    assert_eq!(shaper.cache_size(), 0, "Cache should be empty after clear");
}

#[test]
fn test_text_shaper_grapheme_clusters() {
    let shaper = TextShaper::new();
    let clusters = shaper.detect_grapheme_clusters("hello");
    assert_eq!(clusters.len(), 5, "5 ASCII chars = 5 graphemes");
}

#[test]
fn test_text_shaper_emoji_graphemes() {
    let shaper = TextShaper::new();
    // Flag emoji should be a single grapheme
    let clusters = shaper.detect_grapheme_clusters("🇺🇸");
    assert_eq!(clusters.len(), 1, "Flag emoji = 1 grapheme");
}

#[test]
fn test_shaping_options_default() {
    let opts = ShapingOptions::default();
    assert!(opts.enable_ligatures);
    assert!(opts.enable_kerning);
    assert!(opts.enable_contextual_alternates);
    assert!(!opts.rtl);
    assert!(opts.script.is_none());
    assert!(opts.language.is_none());
}
