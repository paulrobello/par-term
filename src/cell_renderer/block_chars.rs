//! Block character detection and geometric rendering utilities.
//!
//! This module provides utilities for detecting Unicode block drawing characters
//! and rendering them geometrically to avoid gaps between adjacent cells.

/// Unicode ranges for block drawing and related characters
pub mod ranges {
    /// Box Drawing characters (U+2500–U+257F)
    pub const BOX_DRAWING_START: u32 = 0x2500;
    pub const BOX_DRAWING_END: u32 = 0x257F;

    /// Block Elements (U+2580–U+259F)
    pub const BLOCK_ELEMENTS_START: u32 = 0x2580;
    pub const BLOCK_ELEMENTS_END: u32 = 0x259F;

    /// Geometric Shapes (U+25A0–U+25FF)
    pub const GEOMETRIC_SHAPES_START: u32 = 0x25A0;
    pub const GEOMETRIC_SHAPES_END: u32 = 0x25FF;

    /// Powerline symbols (Private Use Area)
    pub const POWERLINE_START: u32 = 0xE0A0;
    pub const POWERLINE_END: u32 = 0xE0D4;

    /// Braille Patterns (U+2800–U+28FF)
    pub const BRAILLE_START: u32 = 0x2800;
    pub const BRAILLE_END: u32 = 0x28FF;
}

/// Classification of block characters for rendering optimization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockCharType {
    /// Not a block character - render normally
    None,
    /// Box drawing lines (─, │, ┌, ┐, etc.) - snap to cell boundaries
    BoxDrawing,
    /// Solid block elements (█, ▄, ▀, etc.) - render geometrically
    SolidBlock,
    /// Partial block elements (▌, ▐, ▖, etc.) - render geometrically
    PartialBlock,
    /// Shade characters (░, ▒, ▓) - use font glyph with snapping
    Shade,
    /// Geometric shapes (■, □, etc.) - snap to boundaries
    Geometric,
    /// Powerline symbols - snap to boundaries
    Powerline,
    /// Braille patterns - use font glyph
    Braille,
}

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
    )
}

/// Check if a character should be rendered geometrically instead of using font glyphs
pub fn should_render_geometrically(char_type: BlockCharType) -> bool {
    matches!(
        char_type,
        BlockCharType::SolidBlock | BlockCharType::PartialBlock | BlockCharType::BoxDrawing
    )
}

/// Base line thickness for box drawing (as fraction of cell height)
const LINE_THICKNESS: f32 = 0.12;
const HEAVY_LINE_THICKNESS: f32 = 0.20;

/// Represents line segments for box drawing characters
/// Each segment is a rectangle: (x, y, width, height) in normalized cell coordinates
#[derive(Debug, Clone)]
pub struct BoxDrawingGeometry {
    pub segments: Vec<GeometricBlock>,
}

impl BoxDrawingGeometry {
    fn with_capacity(cap: usize) -> Self {
        Self {
            segments: Vec::with_capacity(cap),
        }
    }

    /// Create horizontal line segment
    /// thickness is in vertical (y) normalized units
    fn horizontal(y: f32, x_start: f32, x_end: f32, thickness: f32) -> GeometricBlock {
        GeometricBlock::new(x_start, y - thickness / 2.0, x_end - x_start, thickness)
    }

    /// Create vertical line segment
    /// thickness_x is already adjusted for cell aspect ratio
    fn vertical(x: f32, y_start: f32, y_end: f32, thickness_x: f32) -> GeometricBlock {
        GeometricBlock::new(x - thickness_x / 2.0, y_start, thickness_x, y_end - y_start)
    }

    fn push(&mut self, block: GeometricBlock) {
        self.segments.push(block);
    }
}

/// Get geometric representation of a box drawing character
/// aspect_ratio = cell_height / cell_width (used to make lines visually equal thickness)
/// Returns None if the character should use font rendering
pub fn get_box_drawing_geometry(ch: char, aspect_ratio: f32) -> Option<BoxDrawingGeometry> {
    let mut geo = BoxDrawingGeometry::with_capacity(2);
    // Thickness for horizontal lines (in y-normalized units)
    let t = LINE_THICKNESS;
    let ht = HEAVY_LINE_THICKNESS;
    // Thickness for vertical lines (adjusted for aspect ratio to appear same width)
    let tv = LINE_THICKNESS * aspect_ratio;
    let htv = HEAVY_LINE_THICKNESS * aspect_ratio;

    match ch {
        // Light horizontal line ─
        '\u{2500}' => {
            geo.push(BoxDrawingGeometry::horizontal(0.5, 0.0, 1.0, t));
        }
        // Heavy horizontal line ━
        '\u{2501}' => {
            geo.push(BoxDrawingGeometry::horizontal(0.5, 0.0, 1.0, ht));
        }
        // Light vertical line │
        '\u{2502}' => {
            geo.push(BoxDrawingGeometry::vertical(0.5, 0.0, 1.0, tv));
        }
        // Heavy vertical line ┃
        '\u{2503}' => {
            geo.push(BoxDrawingGeometry::vertical(0.5, 0.0, 1.0, htv));
        }

        // Light corners
        // ┌ Box Drawings Light Down and Right
        '\u{250C}' => {
            geo.push(BoxDrawingGeometry::horizontal(0.5, 0.5, 1.0, t));
            geo.push(BoxDrawingGeometry::vertical(0.5, 0.5, 1.0, tv));
        }
        // ┐ Box Drawings Light Down and Left
        '\u{2510}' => {
            geo.push(BoxDrawingGeometry::horizontal(0.5, 0.0, 0.5 + t / 2.0, t));
            geo.push(BoxDrawingGeometry::vertical(0.5, 0.5, 1.0, tv));
        }
        // └ Box Drawings Light Up and Right
        '\u{2514}' => {
            geo.push(BoxDrawingGeometry::horizontal(0.5, 0.5, 1.0, t));
            geo.push(BoxDrawingGeometry::vertical(0.5, 0.0, 0.5 + t / 2.0, tv));
        }
        // ┘ Box Drawings Light Up and Left
        '\u{2518}' => {
            geo.push(BoxDrawingGeometry::horizontal(0.5, 0.0, 0.5 + t / 2.0, t));
            geo.push(BoxDrawingGeometry::vertical(0.5, 0.0, 0.5 + t / 2.0, tv));
        }

        // Light T-junctions
        // ├ Box Drawings Light Vertical and Right
        '\u{251C}' => {
            geo.push(BoxDrawingGeometry::vertical(0.5, 0.0, 1.0, tv));
            geo.push(BoxDrawingGeometry::horizontal(0.5, 0.5, 1.0, t));
        }
        // ┤ Box Drawings Light Vertical and Left
        '\u{2524}' => {
            geo.push(BoxDrawingGeometry::vertical(0.5, 0.0, 1.0, tv));
            geo.push(BoxDrawingGeometry::horizontal(0.5, 0.0, 0.5 + t / 2.0, t));
        }
        // ┬ Box Drawings Light Down and Horizontal
        '\u{252C}' => {
            geo.push(BoxDrawingGeometry::horizontal(0.5, 0.0, 1.0, t));
            geo.push(BoxDrawingGeometry::vertical(0.5, 0.5, 1.0, tv));
        }
        // ┴ Box Drawings Light Up and Horizontal
        '\u{2534}' => {
            geo.push(BoxDrawingGeometry::horizontal(0.5, 0.0, 1.0, t));
            geo.push(BoxDrawingGeometry::vertical(0.5, 0.0, 0.5 + t / 2.0, tv));
        }

        // Light cross ┼
        '\u{253C}' => {
            geo.push(BoxDrawingGeometry::horizontal(0.5, 0.0, 1.0, t));
            geo.push(BoxDrawingGeometry::vertical(0.5, 0.0, 1.0, tv));
        }

        // Heavy corners
        // ┏ Box Drawings Heavy Down and Right
        '\u{250F}' => {
            geo.push(BoxDrawingGeometry::horizontal(0.5, 0.5, 1.0, ht));
            geo.push(BoxDrawingGeometry::vertical(0.5, 0.5, 1.0, htv));
        }
        // ┓ Box Drawings Heavy Down and Left
        '\u{2513}' => {
            geo.push(BoxDrawingGeometry::horizontal(0.5, 0.0, 0.5 + ht / 2.0, ht));
            geo.push(BoxDrawingGeometry::vertical(0.5, 0.5, 1.0, htv));
        }
        // ┗ Box Drawings Heavy Up and Right
        '\u{2517}' => {
            geo.push(BoxDrawingGeometry::horizontal(0.5, 0.5, 1.0, ht));
            geo.push(BoxDrawingGeometry::vertical(0.5, 0.0, 0.5 + ht / 2.0, htv));
        }
        // ┛ Box Drawings Heavy Up and Left
        '\u{251B}' => {
            geo.push(BoxDrawingGeometry::horizontal(0.5, 0.0, 0.5 + ht / 2.0, ht));
            geo.push(BoxDrawingGeometry::vertical(0.5, 0.0, 0.5 + ht / 2.0, htv));
        }

        // Heavy T-junctions
        // ┣ Box Drawings Heavy Vertical and Right
        '\u{2523}' => {
            geo.push(BoxDrawingGeometry::vertical(0.5, 0.0, 1.0, htv));
            geo.push(BoxDrawingGeometry::horizontal(0.5, 0.5, 1.0, ht));
        }
        // ┫ Box Drawings Heavy Vertical and Left
        '\u{252B}' => {
            geo.push(BoxDrawingGeometry::vertical(0.5, 0.0, 1.0, htv));
            geo.push(BoxDrawingGeometry::horizontal(0.5, 0.0, 0.5 + ht / 2.0, ht));
        }
        // ┳ Box Drawings Heavy Down and Horizontal
        '\u{2533}' => {
            geo.push(BoxDrawingGeometry::horizontal(0.5, 0.0, 1.0, ht));
            geo.push(BoxDrawingGeometry::vertical(0.5, 0.5, 1.0, htv));
        }
        // ┻ Box Drawings Heavy Up and Horizontal
        '\u{253B}' => {
            geo.push(BoxDrawingGeometry::horizontal(0.5, 0.0, 1.0, ht));
            geo.push(BoxDrawingGeometry::vertical(0.5, 0.0, 0.5 + ht / 2.0, htv));
        }

        // Heavy cross ╋
        '\u{254B}' => {
            geo.push(BoxDrawingGeometry::horizontal(0.5, 0.0, 1.0, ht));
            geo.push(BoxDrawingGeometry::vertical(0.5, 0.0, 1.0, htv));
        }

        // Double lines
        // ═ Box Drawings Double Horizontal
        '\u{2550}' => {
            geo.push(BoxDrawingGeometry::horizontal(0.35, 0.0, 1.0, tv * 0.7));
            geo.push(BoxDrawingGeometry::horizontal(0.65, 0.0, 1.0, tv * 0.7));
        }
        // ║ Box Drawings Double Vertical
        '\u{2551}' => {
            geo.push(BoxDrawingGeometry::vertical(0.35, 0.0, 1.0, tv * 0.7));
            geo.push(BoxDrawingGeometry::vertical(0.65, 0.0, 1.0, tv * 0.7));
        }
        // ╔ Box Drawings Double Down and Right
        '\u{2554}' => {
            geo.push(BoxDrawingGeometry::horizontal(0.35, 0.65, 1.0, tv * 0.7));
            geo.push(BoxDrawingGeometry::horizontal(0.65, 0.35, 1.0, tv * 0.7));
            geo.push(BoxDrawingGeometry::vertical(0.35, 0.35, 1.0, tv * 0.7));
            geo.push(BoxDrawingGeometry::vertical(0.65, 0.65, 1.0, tv * 0.7));
        }
        // ╗ Box Drawings Double Down and Left
        '\u{2557}' => {
            geo.push(BoxDrawingGeometry::horizontal(0.35, 0.0, 0.35, tv * 0.7));
            geo.push(BoxDrawingGeometry::horizontal(0.65, 0.0, 0.65, tv * 0.7));
            geo.push(BoxDrawingGeometry::vertical(0.35, 0.65, 1.0, tv * 0.7));
            geo.push(BoxDrawingGeometry::vertical(0.65, 0.35, 1.0, tv * 0.7));
        }
        // ╚ Box Drawings Double Up and Right
        '\u{255A}' => {
            geo.push(BoxDrawingGeometry::horizontal(0.35, 0.35, 1.0, tv * 0.7));
            geo.push(BoxDrawingGeometry::horizontal(0.65, 0.65, 1.0, tv * 0.7));
            geo.push(BoxDrawingGeometry::vertical(0.35, 0.0, 0.35, tv * 0.7));
            geo.push(BoxDrawingGeometry::vertical(0.65, 0.0, 0.65, tv * 0.7));
        }
        // ╝ Box Drawings Double Up and Left
        '\u{255D}' => {
            geo.push(BoxDrawingGeometry::horizontal(0.35, 0.0, 0.65, tv * 0.7));
            geo.push(BoxDrawingGeometry::horizontal(0.65, 0.0, 0.35, tv * 0.7));
            geo.push(BoxDrawingGeometry::vertical(0.35, 0.0, 0.65, tv * 0.7));
            geo.push(BoxDrawingGeometry::vertical(0.65, 0.0, 0.35, tv * 0.7));
        }
        // ╠ Box Drawings Double Vertical and Right
        '\u{2560}' => {
            geo.push(BoxDrawingGeometry::vertical(0.35, 0.0, 1.0, tv * 0.7));
            geo.push(BoxDrawingGeometry::vertical(0.65, 0.0, 1.0, tv * 0.7));
            geo.push(BoxDrawingGeometry::horizontal(0.35, 0.65, 1.0, tv * 0.7));
            geo.push(BoxDrawingGeometry::horizontal(0.65, 0.65, 1.0, tv * 0.7));
        }
        // ╣ Box Drawings Double Vertical and Left
        '\u{2563}' => {
            geo.push(BoxDrawingGeometry::vertical(0.35, 0.0, 1.0, tv * 0.7));
            geo.push(BoxDrawingGeometry::vertical(0.65, 0.0, 1.0, tv * 0.7));
            geo.push(BoxDrawingGeometry::horizontal(0.35, 0.0, 0.35, tv * 0.7));
            geo.push(BoxDrawingGeometry::horizontal(0.65, 0.0, 0.35, tv * 0.7));
        }
        // ╦ Box Drawings Double Down and Horizontal
        '\u{2566}' => {
            geo.push(BoxDrawingGeometry::horizontal(0.35, 0.0, 1.0, tv * 0.7));
            geo.push(BoxDrawingGeometry::horizontal(0.65, 0.0, 1.0, tv * 0.7));
            geo.push(BoxDrawingGeometry::vertical(0.35, 0.65, 1.0, tv * 0.7));
            geo.push(BoxDrawingGeometry::vertical(0.65, 0.65, 1.0, tv * 0.7));
        }
        // ╩ Box Drawings Double Up and Horizontal
        '\u{2569}' => {
            geo.push(BoxDrawingGeometry::horizontal(0.35, 0.0, 1.0, tv * 0.7));
            geo.push(BoxDrawingGeometry::horizontal(0.65, 0.0, 1.0, tv * 0.7));
            geo.push(BoxDrawingGeometry::vertical(0.35, 0.0, 0.35, tv * 0.7));
            geo.push(BoxDrawingGeometry::vertical(0.65, 0.0, 0.35, tv * 0.7));
        }
        // ╬ Box Drawings Double Vertical and Horizontal
        '\u{256C}' => {
            geo.push(BoxDrawingGeometry::horizontal(0.35, 0.0, 1.0, tv * 0.7));
            geo.push(BoxDrawingGeometry::horizontal(0.65, 0.0, 1.0, tv * 0.7));
            geo.push(BoxDrawingGeometry::vertical(0.35, 0.0, 1.0, tv * 0.7));
            geo.push(BoxDrawingGeometry::vertical(0.65, 0.0, 1.0, tv * 0.7));
        }

        // Dashed lines - render as solid for now
        // ┄ Box Drawings Light Triple Dash Horizontal
        '\u{2504}' | '\u{2505}' | '\u{2508}' | '\u{2509}' => {
            geo.push(BoxDrawingGeometry::horizontal(0.5, 0.0, 1.0, t));
        }
        // ┆ Box Drawings Light Triple Dash Vertical
        '\u{2506}' | '\u{2507}' | '\u{250A}' | '\u{250B}' => {
            geo.push(BoxDrawingGeometry::vertical(0.5, 0.0, 1.0, tv));
        }

        // Rounded corners - render as regular corners for now
        // ╭ Box Drawings Light Arc Down and Right
        '\u{256D}' => {
            geo.push(BoxDrawingGeometry::horizontal(0.5, 0.5, 1.0, t));
            geo.push(BoxDrawingGeometry::vertical(0.5, 0.5, 1.0, tv));
        }
        // ╮ Box Drawings Light Arc Down and Left
        '\u{256E}' => {
            geo.push(BoxDrawingGeometry::horizontal(0.5, 0.0, 0.5 + t / 2.0, t));
            geo.push(BoxDrawingGeometry::vertical(0.5, 0.5, 1.0, tv));
        }
        // ╯ Box Drawings Light Arc Up and Left
        '\u{256F}' => {
            geo.push(BoxDrawingGeometry::horizontal(0.5, 0.0, 0.5 + t / 2.0, t));
            geo.push(BoxDrawingGeometry::vertical(0.5, 0.0, 0.5 + t / 2.0, tv));
        }
        // ╰ Box Drawings Light Arc Up and Right
        '\u{2570}' => {
            geo.push(BoxDrawingGeometry::horizontal(0.5, 0.5, 1.0, t));
            geo.push(BoxDrawingGeometry::vertical(0.5, 0.0, 0.5 + t / 2.0, tv));
        }

        _ => return None,
    }

    if geo.segments.is_empty() {
        None
    } else {
        Some(geo)
    }
}

/// Represents a geometric block that can be rendered as a colored rectangle
#[derive(Debug, Clone, Copy)]
pub struct GeometricBlock {
    /// Normalized X position within cell (0.0 = left edge, 1.0 = right edge)
    pub x: f32,
    /// Normalized Y position within cell (0.0 = top edge, 1.0 = bottom edge)
    pub y: f32,
    /// Normalized width within cell (0.0 to 1.0)
    pub width: f32,
    /// Normalized height within cell (0.0 to 1.0)
    pub height: f32,
}

impl GeometricBlock {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Full cell block
    pub const fn full() -> Self {
        Self::new(0.0, 0.0, 1.0, 1.0)
    }

    /// Convert to pixel coordinates given cell bounds
    pub fn to_pixel_rect(self, cell_x: f32, cell_y: f32, cell_w: f32, cell_h: f32) -> PixelRect {
        PixelRect {
            x: cell_x + self.x * cell_w,
            y: cell_y + self.y * cell_h,
            width: self.width * cell_w,
            height: self.height * cell_h,
        }
    }
}

/// Pixel rectangle for rendering
#[derive(Debug, Clone, Copy)]
pub struct PixelRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Get the geometric representation of a block element character
/// Returns None if the character should use font rendering
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

/// Calculate snapped glyph bounds for block characters
///
/// This function adjusts glyph position and size to align with cell boundaries,
/// preventing gaps between adjacent block characters.
///
/// # Arguments
/// * `glyph_left` - Original glyph left position in pixels
/// * `glyph_top` - Original glyph top position in pixels
/// * `render_w` - Original glyph render width in pixels
/// * `render_h` - Original glyph render height in pixels
/// * `cell_x0` - Cell left boundary in pixels
/// * `cell_y0` - Cell top boundary in pixels
/// * `cell_x1` - Cell right boundary in pixels
/// * `cell_y1` - Cell bottom boundary in pixels
/// * `snap_threshold` - Distance in pixels to consider "close enough" to snap
/// * `extension` - Amount to extend beyond boundaries to prevent gaps
///
/// # Returns
/// Tuple of (new_left, new_top, new_width, new_height)
#[allow(clippy::too_many_arguments)]
pub fn snap_glyph_to_cell(
    glyph_left: f32,
    glyph_top: f32,
    render_w: f32,
    render_h: f32,
    cell_x0: f32,
    cell_y0: f32,
    cell_x1: f32,
    cell_y1: f32,
    snap_threshold: f32,
    extension: f32,
) -> (f32, f32, f32, f32) {
    let mut new_left = glyph_left;
    let mut new_top = glyph_top;
    let mut new_w = render_w;
    let mut new_h = render_h;

    let glyph_right = glyph_left + render_w;
    let glyph_bottom = glyph_top + render_h;

    // Snap left edge
    if (glyph_left - cell_x0).abs() < snap_threshold {
        new_left = cell_x0 - extension;
        new_w = glyph_right - new_left;
    }

    // Snap right edge
    if (glyph_right - cell_x1).abs() < snap_threshold {
        new_w = cell_x1 + extension - new_left;
    }

    // Snap top edge
    if (glyph_top - cell_y0).abs() < snap_threshold {
        new_top = cell_y0 - extension;
        new_h = glyph_bottom - new_top;
    }

    // Snap bottom edge
    if (glyph_bottom - cell_y1).abs() < snap_threshold {
        new_h = cell_y1 + extension - new_top;
    }

    // Also snap to middle boundaries for half-block characters
    let cell_cx = (cell_x0 + cell_x1) / 2.0;
    let cell_cy = (cell_y0 + cell_y1) / 2.0;

    // Vertical middle snap
    if (glyph_bottom - cell_cy).abs() < snap_threshold {
        new_h = cell_cy - new_top;
    } else if (glyph_top - cell_cy).abs() < snap_threshold {
        let bottom = new_top + new_h;
        new_top = cell_cy;
        new_h = bottom - new_top;
    }

    // Horizontal middle snap
    if (glyph_right - cell_cx).abs() < snap_threshold {
        new_w = cell_cx - new_left;
    } else if (glyph_left - cell_cx).abs() < snap_threshold {
        let right = new_left + new_w;
        new_left = cell_cx;
        new_w = right - new_left;
    }

    (new_left, new_top, new_w, new_h)
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

        assert!(!should_render_geometrically(BlockCharType::None));
        assert!(!should_render_geometrically(BlockCharType::Shade));
        assert!(!should_render_geometrically(BlockCharType::Geometric));
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
        let (left, top, w, h) = snap_glyph_to_cell(
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
}
