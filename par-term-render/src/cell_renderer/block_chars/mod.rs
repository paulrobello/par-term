//! Block character detection and geometric rendering utilities.
//!
//! This module provides utilities for detecting Unicode block drawing characters
//! and rendering them geometrically to avoid gaps between adjacent cells.
//!
//! The box drawing implementation uses a 7-position grid system inspired by iTerm2,
//! where lines share exact endpoint coordinates at corners and junctions.

mod block_elements;
mod box_drawing;
mod geometric_shapes;
mod snapping;
pub(super) mod types;

// Re-export public API
pub use block_elements::get_geometric_block;
pub use box_drawing::get_box_drawing_geometry;
pub use geometric_shapes::get_geometric_shape_rect;
pub use snapping::snap_glyph_to_cell;
pub use types::{BlockCharType, BoxDrawingGeometry, GeometricBlock, PixelRect, ranges};

/// Classify a character for rendering optimization
pub fn classify_char(ch: char) -> BlockCharType {
    let code = ch as u32;

    // Box Drawing (U+2500–U+257F)
    if (ranges::BOX_DRAWING_START..=ranges::BOX_DRAWING_END).contains(&code) {
        return BlockCharType::BoxDrawing;
    }

    // Block Elements (U+2580–U+259F)
    if (ranges::BLOCK_ELEMENTS_START..=ranges::BLOCK_ELEMENTS_END).contains(&code) {
        return classify_block_element(ch);
    }

    // Geometric Shapes (U+25A0–U+25FF)
    if (ranges::GEOMETRIC_SHAPES_START..=ranges::GEOMETRIC_SHAPES_END).contains(&code) {
        return BlockCharType::Geometric;
    }

    // Powerline symbols
    if (ranges::POWERLINE_START..=ranges::POWERLINE_END).contains(&code) {
        return BlockCharType::Powerline;
    }

    // Braille patterns
    if (ranges::BRAILLE_START..=ranges::BRAILLE_END).contains(&code) {
        return BlockCharType::Braille;
    }

    // Miscellaneous Symbols (ballot boxes, etc.)
    if (ranges::MISC_SYMBOLS_START..=ranges::MISC_SYMBOLS_END).contains(&code) {
        return BlockCharType::Symbol;
    }

    // Dingbats (check marks, etc.)
    if (ranges::DINGBATS_START..=ranges::DINGBATS_END).contains(&code) {
        return BlockCharType::Symbol;
    }

    BlockCharType::None
}

/// Classify block elements into solid, partial, or shade
fn classify_block_element(ch: char) -> BlockCharType {
    match ch {
        // Shade characters
        '\u{2591}' | '\u{2592}' | '\u{2593}' => BlockCharType::Shade,

        // Full block
        '\u{2588}' => BlockCharType::SolidBlock,

        // Partial blocks (half blocks, quadrants, eighth blocks)
        '\u{2580}'..='\u{2590}' | '\u{2594}'..='\u{259F}' => BlockCharType::PartialBlock,

        _ => BlockCharType::PartialBlock,
    }
}

/// Check if a character should have its glyph snapped to cell boundaries
pub fn should_snap_to_boundaries(char_type: BlockCharType) -> bool {
    matches!(
        char_type,
        BlockCharType::BoxDrawing
            | BlockCharType::SolidBlock
            | BlockCharType::PartialBlock
            | BlockCharType::Geometric
            | BlockCharType::Powerline
            | BlockCharType::Symbol
    )
}

/// Check if a character should be rendered geometrically instead of using font glyphs
pub fn should_render_geometrically(char_type: BlockCharType) -> bool {
    matches!(
        char_type,
        BlockCharType::SolidBlock
            | BlockCharType::PartialBlock
            | BlockCharType::BoxDrawing
            | BlockCharType::Geometric
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_box_drawing() {
        // Horizontal lines
        assert_eq!(classify_char('─'), BlockCharType::BoxDrawing);
        assert_eq!(classify_char('━'), BlockCharType::BoxDrawing);
        assert_eq!(classify_char('═'), BlockCharType::BoxDrawing);

        // Vertical lines
        assert_eq!(classify_char('│'), BlockCharType::BoxDrawing);
        assert_eq!(classify_char('┃'), BlockCharType::BoxDrawing);
        assert_eq!(classify_char('║'), BlockCharType::BoxDrawing);

        // Corners
        assert_eq!(classify_char('┌'), BlockCharType::BoxDrawing);
        assert_eq!(classify_char('┐'), BlockCharType::BoxDrawing);
        assert_eq!(classify_char('└'), BlockCharType::BoxDrawing);
        assert_eq!(classify_char('┘'), BlockCharType::BoxDrawing);

        // Double line corners
        assert_eq!(classify_char('╔'), BlockCharType::BoxDrawing);
        assert_eq!(classify_char('╗'), BlockCharType::BoxDrawing);
        assert_eq!(classify_char('╚'), BlockCharType::BoxDrawing);
        assert_eq!(classify_char('╝'), BlockCharType::BoxDrawing);
    }

    #[test]
    fn test_classify_block_elements() {
        // Full block
        assert_eq!(classify_char('█'), BlockCharType::SolidBlock);

        // Half blocks
        assert_eq!(classify_char('▀'), BlockCharType::PartialBlock);
        assert_eq!(classify_char('▄'), BlockCharType::PartialBlock);
        assert_eq!(classify_char('▌'), BlockCharType::PartialBlock);
        assert_eq!(classify_char('▐'), BlockCharType::PartialBlock);

        // Shade characters
        assert_eq!(classify_char('░'), BlockCharType::Shade);
        assert_eq!(classify_char('▒'), BlockCharType::Shade);
        assert_eq!(classify_char('▓'), BlockCharType::Shade);

        // Quadrants
        assert_eq!(classify_char('▖'), BlockCharType::PartialBlock);
        assert_eq!(classify_char('▗'), BlockCharType::PartialBlock);
        assert_eq!(classify_char('▘'), BlockCharType::PartialBlock);
        assert_eq!(classify_char('▝'), BlockCharType::PartialBlock);
    }

    #[test]
    fn test_classify_geometric_shapes() {
        assert_eq!(classify_char('■'), BlockCharType::Geometric);
        assert_eq!(classify_char('□'), BlockCharType::Geometric);
        assert_eq!(classify_char('▪'), BlockCharType::Geometric);
        assert_eq!(classify_char('▫'), BlockCharType::Geometric);
    }

    #[test]
    fn test_classify_regular_chars() {
        assert_eq!(classify_char('a'), BlockCharType::None);
        assert_eq!(classify_char('Z'), BlockCharType::None);
        assert_eq!(classify_char('0'), BlockCharType::None);
        assert_eq!(classify_char(' '), BlockCharType::None);
        assert_eq!(classify_char('!'), BlockCharType::None);
    }

    #[test]
    fn test_should_snap_to_boundaries() {
        assert!(should_snap_to_boundaries(BlockCharType::BoxDrawing));
        assert!(should_snap_to_boundaries(BlockCharType::SolidBlock));
        assert!(should_snap_to_boundaries(BlockCharType::PartialBlock));
        assert!(should_snap_to_boundaries(BlockCharType::Geometric));
        assert!(should_snap_to_boundaries(BlockCharType::Powerline));

        assert!(!should_snap_to_boundaries(BlockCharType::None));
        assert!(!should_snap_to_boundaries(BlockCharType::Shade));
        assert!(!should_snap_to_boundaries(BlockCharType::Braille));
    }

    #[test]
    fn test_should_render_geometrically() {
        assert!(should_render_geometrically(BlockCharType::SolidBlock));
        assert!(should_render_geometrically(BlockCharType::PartialBlock));
        assert!(should_render_geometrically(BlockCharType::BoxDrawing));
        assert!(should_render_geometrically(BlockCharType::Geometric));

        assert!(!should_render_geometrically(BlockCharType::None));
        assert!(!should_render_geometrically(BlockCharType::Shade));
        assert!(!should_render_geometrically(BlockCharType::Powerline));
        assert!(!should_render_geometrically(BlockCharType::Braille));
    }

    #[test]
    fn test_geometric_block_full() {
        let block = get_geometric_block('█').unwrap();
        assert_eq!(block.x, 0.0);
        assert_eq!(block.y, 0.0);
        assert_eq!(block.width, 1.0);
        assert_eq!(block.height, 1.0);
    }

    #[test]
    fn test_geometric_block_halves() {
        // Upper half
        let block = get_geometric_block('▀').unwrap();
        assert_eq!(block.x, 0.0);
        assert_eq!(block.y, 0.0);
        assert_eq!(block.width, 1.0);
        assert_eq!(block.height, 0.5);

        // Lower half
        let block = get_geometric_block('▄').unwrap();
        assert_eq!(block.x, 0.0);
        assert_eq!(block.y, 0.5);
        assert_eq!(block.width, 1.0);
        assert_eq!(block.height, 0.5);

        // Left half
        let block = get_geometric_block('▌').unwrap();
        assert_eq!(block.x, 0.0);
        assert_eq!(block.y, 0.0);
        assert_eq!(block.width, 0.5);
        assert_eq!(block.height, 1.0);

        // Right half
        let block = get_geometric_block('▐').unwrap();
        assert_eq!(block.x, 0.5);
        assert_eq!(block.y, 0.0);
        assert_eq!(block.width, 0.5);
        assert_eq!(block.height, 1.0);
    }

    #[test]
    fn test_geometric_block_quadrants() {
        // Lower left quadrant
        let block = get_geometric_block('▖').unwrap();
        assert_eq!(block.x, 0.0);
        assert_eq!(block.y, 0.5);
        assert_eq!(block.width, 0.5);
        assert_eq!(block.height, 0.5);

        // Lower right quadrant
        let block = get_geometric_block('▗').unwrap();
        assert_eq!(block.x, 0.5);
        assert_eq!(block.y, 0.5);
        assert_eq!(block.width, 0.5);
        assert_eq!(block.height, 0.5);

        // Upper left quadrant
        let block = get_geometric_block('▘').unwrap();
        assert_eq!(block.x, 0.0);
        assert_eq!(block.y, 0.0);
        assert_eq!(block.width, 0.5);
        assert_eq!(block.height, 0.5);

        // Upper right quadrant
        let block = get_geometric_block('▝').unwrap();
        assert_eq!(block.x, 0.5);
        assert_eq!(block.y, 0.0);
        assert_eq!(block.width, 0.5);
        assert_eq!(block.height, 0.5);
    }

    #[test]
    fn test_geometric_block_to_pixel_rect() {
        let block = GeometricBlock::new(0.0, 0.5, 1.0, 0.5); // Lower half
        let rect = block.to_pixel_rect(10.0, 20.0, 8.0, 16.0);

        assert_eq!(rect.x, 10.0);
        assert_eq!(rect.y, 28.0); // 20.0 + 0.5 * 16.0
        assert_eq!(rect.width, 8.0);
        assert_eq!(rect.height, 8.0);
    }

    #[test]
    fn test_box_drawing_light_horizontal() {
        let geo = get_box_drawing_geometry('─', 2.0).unwrap();
        assert_eq!(geo.segments.len(), 1);
        let seg = &geo.segments[0];
        assert_eq!(seg.x, 0.0);
        assert!(seg.width > 0.99); // Full width
    }

    #[test]
    fn test_box_drawing_light_corner() {
        let geo = get_box_drawing_geometry('┌', 2.0).unwrap();
        assert_eq!(geo.segments.len(), 2);
        // Should have horizontal and vertical segments meeting at center
    }

    #[test]
    fn test_box_drawing_double_lines() {
        let geo = get_box_drawing_geometry('═', 2.0).unwrap();
        assert_eq!(geo.segments.len(), 2);
        // Two parallel horizontal lines
    }

    #[test]
    fn test_snap_glyph_to_cell_basic() {
        // Glyph that's close to cell boundaries should snap
        let (left, top, w, h) = snap_glyph_to_cell(
            10.5, 20.5, // glyph position (slightly off from cell)
            7.8, 15.8, // glyph size (slightly smaller than cell)
            10.0, 20.0, // cell top-left
            18.0, 36.0, // cell bottom-right
            3.0,  // snap threshold
            0.5,  // extension
        );

        // Should snap left to cell boundary minus extension
        assert!((left - 9.5).abs() < 0.01);
        // Should snap top to cell boundary minus extension
        assert!((top - 19.5).abs() < 0.01);
        // Width should extend to right cell boundary plus extension
        assert!((left + w - 18.5).abs() < 0.01);
        // Height should extend to bottom cell boundary plus extension
        assert!((top + h - 36.5).abs() < 0.01);
    }

    #[test]
    fn test_snap_glyph_no_snap_when_far() {
        // Glyph that's far from cell boundaries and midpoints should not snap
        // Cell: x=[10, 20], y=[20, 40], middle_x=15, middle_y=30
        // Glyph: x=[12, 17], y=[24, 34] - all edges >2 pixels from boundaries/midpoints
        let (left, top, w, h) = snap_glyph_to_cell(
            12.0, 24.0, // glyph position (away from edges and midpoints)
            5.0, 10.0, // glyph size (ends at x=17, y=34)
            10.0, 20.0, // cell top-left
            20.0, 40.0, // cell bottom-right (midpoints at 15, 30)
            1.5,  // snap threshold (narrow)
            0.5,  // extension
        );

        // Should not change anything since glyph is far from boundaries
        assert_eq!(left, 12.0);
        assert_eq!(top, 24.0);
        assert_eq!(w, 5.0);
        assert_eq!(h, 10.0);
    }

    #[test]
    fn test_snap_glyph_middle_snap() {
        // Test snapping to middle boundaries (for half-block characters)
        let (_left, top, _w, h) = snap_glyph_to_cell(
            10.0, 20.0, // glyph at cell corner
            8.0, 9.8, // glyph ends near vertical middle (30 - 0.2 = 29.8)
            10.0, 20.0, // cell top-left
            18.0, 40.0, // cell bottom-right (middle at y=30)
            1.0,  // snap threshold
            0.0,  // no extension for this test
        );

        // Height should snap to middle
        assert!((top + h - 30.0).abs() < 0.01);
    }

    #[test]
    fn test_geometric_shape_rect_black_square() {
        // ■ U+25A0 — full cell_w square, centered vertically
        let rect = get_geometric_shape_rect('\u{25A0}', 10.0, 20.0, 8.0, 16.0).unwrap();
        assert_eq!(rect.x, 10.0);
        assert_eq!(rect.y, 24.0); // 20 + (16 - 8) / 2
        assert_eq!(rect.width, 8.0);
        assert_eq!(rect.height, 8.0);
    }

    #[test]
    fn test_geometric_shape_rect_medium_square() {
        // ◼ U+25FC — 0.75× cell_w square, centered
        let rect = get_geometric_shape_rect('\u{25FC}', 10.0, 20.0, 8.0, 16.0).unwrap();
        let size = 8.0 * 0.75; // 6.0
        assert_eq!(rect.x, 10.0 + (8.0 - size) / 2.0);
        assert_eq!(rect.y, 20.0 + (16.0 - size) / 2.0);
        assert_eq!(rect.width, size);
        assert_eq!(rect.height, size);
    }

    #[test]
    fn test_geometric_shape_rect_small_square() {
        // ▪ U+25AA — 0.5× cell_w square, centered
        let rect = get_geometric_shape_rect('\u{25AA}', 10.0, 20.0, 8.0, 16.0).unwrap();
        let size = 8.0 * 0.5; // 4.0
        assert_eq!(rect.x, 10.0 + (8.0 - size) / 2.0);
        assert_eq!(rect.y, 20.0 + (16.0 - size) / 2.0);
        assert_eq!(rect.width, size);
        assert_eq!(rect.height, size);
    }

    #[test]
    fn test_geometric_shape_rect_rectangle() {
        // ▬ U+25AC — horizontal rectangle, full width, 0.33 height
        let rect = get_geometric_shape_rect('\u{25AC}', 10.0, 20.0, 8.0, 16.0).unwrap();
        let h = 16.0 * 0.33;
        assert_eq!(rect.x, 10.0);
        assert!((rect.y - (20.0 + (16.0 - h) / 2.0)).abs() < 0.01);
        assert_eq!(rect.width, 8.0);
        assert!((rect.height - h).abs() < 0.01);
    }

    #[test]
    fn test_geometric_shape_rect_vertical_rectangle() {
        // ▮ U+25AE — vertical rectangle, 0.5 width, full height
        let rect = get_geometric_shape_rect('\u{25AE}', 10.0, 20.0, 8.0, 16.0).unwrap();
        let w = 8.0 * 0.5;
        assert_eq!(rect.x, 10.0 + (8.0 - w) / 2.0);
        assert_eq!(rect.y, 20.0);
        assert_eq!(rect.width, w);
        assert_eq!(rect.height, 16.0);
    }

    #[test]
    fn test_geometric_shape_rect_outline_returns_none() {
        // Outline/hollow shapes should return None (use font rendering)
        assert!(get_geometric_shape_rect('\u{25A1}', 0.0, 0.0, 8.0, 16.0).is_none()); // □
        assert!(get_geometric_shape_rect('\u{25AB}', 0.0, 0.0, 8.0, 16.0).is_none()); // ▫
        assert!(get_geometric_shape_rect('\u{25FB}', 0.0, 0.0, 8.0, 16.0).is_none()); // ◻
        assert!(get_geometric_shape_rect('\u{25FD}', 0.0, 0.0, 8.0, 16.0).is_none()); // ◽
    }
}
