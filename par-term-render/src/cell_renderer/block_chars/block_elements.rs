//! Block element character rendering (U+2580–U+259F).
//!
//! Provides geometric representations of Unicode block elements such as
//! half blocks (▀▄▌▐), eighth blocks, and quadrant blocks (▖▗▘▝).

use super::types::GeometricBlock;

/// Get the geometric representation of a block element character.
/// Returns `None` if the character should use font rendering.
pub fn get_geometric_block(ch: char) -> Option<GeometricBlock> {
    match ch {
        // Full block
        '\u{2588}' => Some(GeometricBlock::full()),

        // Upper half block
        '\u{2580}' => Some(GeometricBlock::new(0.0, 0.0, 1.0, 0.5)),

        // Lower one eighth block to lower seven eighths block
        '\u{2581}' => Some(GeometricBlock::new(0.0, 0.875, 1.0, 0.125)),
        '\u{2582}' => Some(GeometricBlock::new(0.0, 0.75, 1.0, 0.25)),
        '\u{2583}' => Some(GeometricBlock::new(0.0, 0.625, 1.0, 0.375)),
        '\u{2584}' => Some(GeometricBlock::new(0.0, 0.5, 1.0, 0.5)), // Lower half
        '\u{2585}' => Some(GeometricBlock::new(0.0, 0.375, 1.0, 0.625)),
        '\u{2586}' => Some(GeometricBlock::new(0.0, 0.25, 1.0, 0.75)),
        '\u{2587}' => Some(GeometricBlock::new(0.0, 0.125, 1.0, 0.875)),

        // Left blocks (one eighth to seven eighths)
        '\u{2589}' => Some(GeometricBlock::new(0.0, 0.0, 0.875, 1.0)),
        '\u{258A}' => Some(GeometricBlock::new(0.0, 0.0, 0.75, 1.0)),
        '\u{258B}' => Some(GeometricBlock::new(0.0, 0.0, 0.625, 1.0)),
        '\u{258C}' => Some(GeometricBlock::new(0.0, 0.0, 0.5, 1.0)), // Left half
        '\u{258D}' => Some(GeometricBlock::new(0.0, 0.0, 0.375, 1.0)),
        '\u{258E}' => Some(GeometricBlock::new(0.0, 0.0, 0.25, 1.0)),
        '\u{258F}' => Some(GeometricBlock::new(0.0, 0.0, 0.125, 1.0)),

        // Right half block
        '\u{2590}' => Some(GeometricBlock::new(0.5, 0.0, 0.5, 1.0)),

        // Upper one eighth block
        '\u{2594}' => Some(GeometricBlock::new(0.0, 0.0, 1.0, 0.125)),

        // Right one eighth block
        '\u{2595}' => Some(GeometricBlock::new(0.875, 0.0, 0.125, 1.0)),

        // Quadrant blocks
        '\u{2596}' => Some(GeometricBlock::new(0.0, 0.5, 0.5, 0.5)), // Lower left
        '\u{2597}' => Some(GeometricBlock::new(0.5, 0.5, 0.5, 0.5)), // Lower right
        '\u{2598}' => Some(GeometricBlock::new(0.0, 0.0, 0.5, 0.5)), // Upper left
        '\u{259D}' => Some(GeometricBlock::new(0.5, 0.0, 0.5, 0.5)), // Upper right

        // Combined quadrants - these need multiple rectangles, handled separately
        // For now, return None to use font rendering with snapping
        '\u{2599}'..='\u{259C}' | '\u{259E}' | '\u{259F}' => None,

        _ => None,
    }
}
