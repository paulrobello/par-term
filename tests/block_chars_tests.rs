//! Integration tests for block character rendering utilities.

use par_term::cell_renderer::block_chars::{
    BlockCharType, GeometricBlock, classify_char, get_geometric_block, should_render_geometrically,
    should_snap_to_boundaries, snap_glyph_to_cell,
};

/// Test that box drawing characters are correctly classified
#[test]
fn test_box_drawing_classification() {
    // Light box drawing
    let light_chars = ['─', '│', '┌', '┐', '└', '┘', '├', '┤', '┬', '┴', '┼'];
    for ch in light_chars {
        assert_eq!(
            classify_char(ch),
            BlockCharType::BoxDrawing,
            "Character '{}' should be BoxDrawing",
            ch
        );
    }

    // Heavy box drawing
    let heavy_chars = ['━', '┃', '┏', '┓', '┗', '┛', '┣', '┫', '┳', '┻', '╋'];
    for ch in heavy_chars {
        assert_eq!(
            classify_char(ch),
            BlockCharType::BoxDrawing,
            "Character '{}' should be BoxDrawing",
            ch
        );
    }

    // Double box drawing
    let double_chars = ['═', '║', '╔', '╗', '╚', '╝', '╠', '╣', '╦', '╩', '╬'];
    for ch in double_chars {
        assert_eq!(
            classify_char(ch),
            BlockCharType::BoxDrawing,
            "Character '{}' should be BoxDrawing",
            ch
        );
    }
}

/// Test that block elements are correctly classified
#[test]
fn test_block_element_classification() {
    // Full block should be solid
    assert_eq!(classify_char('█'), BlockCharType::SolidBlock);

    // Half blocks should be partial
    let half_blocks = ['▀', '▄', '▌', '▐'];
    for ch in half_blocks {
        assert_eq!(
            classify_char(ch),
            BlockCharType::PartialBlock,
            "Character '{}' should be PartialBlock",
            ch
        );
    }

    // Shade characters should be shade type
    let shades = ['░', '▒', '▓'];
    for ch in shades {
        assert_eq!(
            classify_char(ch),
            BlockCharType::Shade,
            "Character '{}' should be Shade",
            ch
        );
    }

    // Quadrants should be partial
    let quadrants = ['▖', '▗', '▘', '▝'];
    for ch in quadrants {
        assert_eq!(
            classify_char(ch),
            BlockCharType::PartialBlock,
            "Character '{}' should be PartialBlock",
            ch
        );
    }
}

/// Test that regular ASCII characters are not classified as block chars
#[test]
fn test_regular_chars_not_block() {
    let regular_chars = [
        'a', 'z', 'A', 'Z', '0', '9', ' ', '!', '@', '#', '$', '%', '^', '&', '*', '(', ')', '-',
        '+', '=',
    ];
    for ch in regular_chars {
        assert_eq!(
            classify_char(ch),
            BlockCharType::None,
            "Character '{}' should be None",
            ch
        );
    }
}

/// Test geometric block generation for solid blocks
#[test]
fn test_geometric_block_solid() {
    let block = get_geometric_block('█').expect("Full block should have geometric representation");
    assert_eq!(block.x, 0.0, "Full block x should be 0");
    assert_eq!(block.y, 0.0, "Full block y should be 0");
    assert_eq!(block.width, 1.0, "Full block width should be 1");
    assert_eq!(block.height, 1.0, "Full block height should be 1");
}

/// Test geometric block generation for half blocks
#[test]
fn test_geometric_block_halves() {
    // Upper half
    let upper = get_geometric_block('▀').expect("Upper half should have geometric representation");
    assert_eq!(upper.x, 0.0);
    assert_eq!(upper.y, 0.0);
    assert_eq!(upper.width, 1.0);
    assert_eq!(upper.height, 0.5);

    // Lower half
    let lower = get_geometric_block('▄').expect("Lower half should have geometric representation");
    assert_eq!(lower.x, 0.0);
    assert_eq!(lower.y, 0.5);
    assert_eq!(lower.width, 1.0);
    assert_eq!(lower.height, 0.5);

    // Left half
    let left = get_geometric_block('▌').expect("Left half should have geometric representation");
    assert_eq!(left.x, 0.0);
    assert_eq!(left.y, 0.0);
    assert_eq!(left.width, 0.5);
    assert_eq!(left.height, 1.0);

    // Right half
    let right = get_geometric_block('▐').expect("Right half should have geometric representation");
    assert_eq!(right.x, 0.5);
    assert_eq!(right.y, 0.0);
    assert_eq!(right.width, 0.5);
    assert_eq!(right.height, 1.0);
}

/// Test geometric block generation for eighth blocks
#[test]
fn test_geometric_block_eighths() {
    // Lower one-eighth
    let eighth = get_geometric_block('▁').expect("Lower 1/8 should have geometric representation");
    assert!((eighth.y - 0.875).abs() < 0.001);
    assert!((eighth.height - 0.125).abs() < 0.001);

    // Left one-eighth
    let left_eighth =
        get_geometric_block('▏').expect("Left 1/8 should have geometric representation");
    assert_eq!(left_eighth.x, 0.0);
    assert!((left_eighth.width - 0.125).abs() < 0.001);

    // Upper one-eighth
    let upper_eighth =
        get_geometric_block('▔').expect("Upper 1/8 should have geometric representation");
    assert_eq!(upper_eighth.y, 0.0);
    assert!((upper_eighth.height - 0.125).abs() < 0.001);

    // Right one-eighth
    let right_eighth =
        get_geometric_block('▕').expect("Right 1/8 should have geometric representation");
    assert!((right_eighth.x - 0.875).abs() < 0.001);
    assert!((right_eighth.width - 0.125).abs() < 0.001);
}

/// Test geometric block to pixel rect conversion
#[test]
fn test_geometric_to_pixel_rect() {
    let block = GeometricBlock::new(0.0, 0.5, 1.0, 0.5); // Lower half
    let rect = block.to_pixel_rect(100.0, 200.0, 10.0, 20.0);

    assert_eq!(rect.x, 100.0, "Pixel rect x should match cell x");
    assert_eq!(
        rect.y, 210.0,
        "Pixel rect y should be cell_y + 0.5 * cell_h"
    );
    assert_eq!(rect.width, 10.0, "Pixel rect width should match cell width");
    assert_eq!(
        rect.height, 10.0,
        "Pixel rect height should be 0.5 * cell_h"
    );
}

/// Test snapping behavior for glyphs near cell boundaries
#[test]
fn test_snap_glyph_near_boundaries() {
    // Glyph that's close to all four cell boundaries
    let (left, top, w, h) = snap_glyph_to_cell(
        10.5, 20.5, // glyph slightly offset from cell
        9.0, 19.0, // glyph slightly smaller than cell
        10.0, 20.0, // cell top-left
        20.0, 40.0, // cell bottom-right
        2.0,  // snap threshold
        0.5,  // extension
    );

    // Should snap to boundaries with extension
    assert!(
        (left - 9.5).abs() < 0.01,
        "Left should snap to cell_x0 - extension"
    );
    assert!(
        (top - 19.5).abs() < 0.01,
        "Top should snap to cell_y0 - extension"
    );
    assert!(
        (left + w - 20.5).abs() < 0.01,
        "Right should snap to cell_x1 + extension"
    );
    assert!(
        (top + h - 40.5).abs() < 0.01,
        "Bottom should snap to cell_y1 + extension"
    );
}

/// Test that glyphs far from boundaries are not snapped
#[test]
fn test_snap_glyph_far_from_boundaries() {
    let (left, top, w, h) = snap_glyph_to_cell(
        15.0, 28.0, // glyph centered in cell
        3.0, 5.0, // small glyph
        10.0, 20.0, // cell top-left
        20.0, 40.0, // cell bottom-right
        2.0,  // snap threshold
        0.5,  // extension
    );

    // Should not change - glyph is far from boundaries
    assert_eq!(left, 15.0, "Left should not change");
    assert_eq!(top, 28.0, "Top should not change");
    assert_eq!(w, 3.0, "Width should not change");
    assert_eq!(h, 5.0, "Height should not change");
}

/// Test snapping to middle boundaries (for half-block characters)
#[test]
fn test_snap_glyph_middle_boundaries() {
    // Glyph that ends near vertical middle
    let (left, top, w, h) = snap_glyph_to_cell(
        10.0, 20.0, // glyph at top-left
        10.0, 9.5, // height ends near middle (20 + 9.5 = 29.5, middle is 30)
        10.0, 20.0, // cell top-left
        20.0, 40.0, // cell bottom-right (middle at y=30)
        1.0,  // snap threshold
        0.0,  // no extension
    );

    // Height should snap to reach middle
    assert!(
        (top + h - 30.0).abs() < 0.01,
        "Bottom should snap to vertical middle"
    );
}

/// Test that should_snap_to_boundaries returns correct values
#[test]
fn test_should_snap_to_boundaries_values() {
    assert!(should_snap_to_boundaries(BlockCharType::BoxDrawing));
    assert!(should_snap_to_boundaries(BlockCharType::SolidBlock));
    assert!(should_snap_to_boundaries(BlockCharType::PartialBlock));
    assert!(should_snap_to_boundaries(BlockCharType::Geometric));
    assert!(should_snap_to_boundaries(BlockCharType::Powerline));

    assert!(!should_snap_to_boundaries(BlockCharType::None));
    assert!(!should_snap_to_boundaries(BlockCharType::Shade));
    assert!(!should_snap_to_boundaries(BlockCharType::Braille));
}

/// Test that should_render_geometrically returns correct values
#[test]
fn test_should_render_geometrically_values() {
    assert!(should_render_geometrically(BlockCharType::SolidBlock));
    assert!(should_render_geometrically(BlockCharType::PartialBlock));
    assert!(should_render_geometrically(BlockCharType::BoxDrawing));

    assert!(!should_render_geometrically(BlockCharType::None));
    assert!(!should_render_geometrically(BlockCharType::Shade));
    assert!(!should_render_geometrically(BlockCharType::Geometric));
    assert!(!should_render_geometrically(BlockCharType::Powerline));
    assert!(!should_render_geometrically(BlockCharType::Braille));
}

/// Test Braille character classification
#[test]
fn test_braille_classification() {
    // Empty braille pattern
    assert_eq!(classify_char('\u{2800}'), BlockCharType::Braille);

    // Full braille pattern
    assert_eq!(classify_char('\u{28FF}'), BlockCharType::Braille);

    // Some middle braille patterns
    assert_eq!(classify_char('\u{2801}'), BlockCharType::Braille);
    assert_eq!(classify_char('\u{2880}'), BlockCharType::Braille);
}

/// Test combined quadrant blocks return None for geometric rendering
/// (they require multiple rectangles and should use font rendering)
#[test]
fn test_combined_quadrants_no_geometric() {
    // These characters have multiple non-contiguous regions
    let combined_quadrants = [
        '\u{2599}', // QUADRANT UPPER LEFT AND LOWER LEFT AND LOWER RIGHT
        '\u{259A}', // QUADRANT UPPER LEFT AND LOWER RIGHT
        '\u{259B}', // QUADRANT UPPER LEFT AND UPPER RIGHT AND LOWER LEFT
        '\u{259C}', // QUADRANT UPPER LEFT AND UPPER RIGHT AND LOWER RIGHT
        '\u{259E}', // QUADRANT UPPER RIGHT AND LOWER LEFT
        '\u{259F}', // QUADRANT UPPER LEFT AND UPPER RIGHT AND LOWER LEFT AND LOWER RIGHT
    ];

    for ch in combined_quadrants {
        assert!(
            get_geometric_block(ch).is_none(),
            "Combined quadrant '{}' (U+{:04X}) should not have geometric representation",
            ch,
            ch as u32
        );
    }
}
