//! Font management and text shaping for par-term terminal emulator.
//!
//! This crate provides:
//! - Font loading with system font discovery and fallback chains
//! - Unicode range-specific font mappings (e.g., CJK, emoji)
//! - HarfBuzz-based text shaping via rustybuzz for ligatures and complex scripts
//! - Grapheme cluster detection for proper Unicode rendering
//!
//! # Architecture
//!
//! The `FontManager` orchestrates font loading and glyph lookup across a
//! priority-ordered chain of fonts:
//! 1. Primary font (with bold/italic/bold-italic variants)
//! 2. Unicode range-specific fonts
//! 3. System fallback fonts
//!
//! The `TextShaper` provides HarfBuzz-based text shaping with LRU caching
//! for performance.

pub mod font_manager;
pub mod text_shaper;

// Re-export main types for convenience
pub use font_manager::{FALLBACK_FAMILIES, FontData, FontManager, UnicodeRangeFont};
pub use text_shaper::{ShapedGlyph, ShapedRun, ShapingOptions, TextShaper};
