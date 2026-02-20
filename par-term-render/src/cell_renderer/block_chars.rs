//! Block character detection and geometric rendering utilities.
//!
//! This module provides utilities for detecting Unicode block drawing characters
//! and rendering them geometrically to avoid gaps between adjacent cells.
//!
//! The box drawing implementation uses a 7-position grid system inspired by iTerm2,
//! where lines share exact endpoint coordinates at corners and junctions.

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

    /// Miscellaneous Symbols (U+2600–U+26FF) — includes ballot boxes (☐☑☒)
    pub const MISC_SYMBOLS_START: u32 = 0x2600;
    pub const MISC_SYMBOLS_END: u32 = 0x26FF;

    /// Dingbats (U+2700–U+27BF) — includes check marks (✓✔✗✘)
    pub const DINGBATS_START: u32 = 0x2700;
    pub const DINGBATS_END: u32 = 0x27BF;
}

/// 7-position grid system for box drawing (normalized 0.0-1.0)
///
/// This grid provides precise positioning for light, heavy, and double lines:
/// - Light lines use the center position (D/V4)
/// - Heavy lines use two parallel strokes at C+E / V3+V5
/// - Double lines use two strokes at B+F / V2+V6
///
/// ```text
///     A     B     C     D     E     F     G
///    0.0  0.25  0.40  0.50  0.60  0.75  1.0
///    left  |     |   center  |     |   right
///          heavy-outer       heavy-outer
///                heavy-inner
/// ```
mod grid {
    // Horizontal positions
    pub const A: f32 = 0.0; // Left edge
    pub const C: f32 = 0.40; // Heavy/double inner left
    pub const D: f32 = 0.50; // Center (light lines)
    pub const E: f32 = 0.60; // Heavy/double inner right
    pub const G: f32 = 1.0; // Right edge

    // Vertical positions (same values)
    pub const V1: f32 = 0.0; // Top edge
    pub const V3: f32 = 0.40; // Heavy/double inner top
    pub const V4: f32 = 0.50; // Center (light lines)
    pub const V5: f32 = 0.60; // Heavy/double inner bottom
    pub const V7: f32 = 1.0; // Bottom edge

    // Line thicknesses (as fraction of cell dimension)
    pub const LIGHT_THICKNESS: f32 = 0.12;
    pub const HEAVY_THICKNESS: f32 = 0.20;
    pub const DOUBLE_THICKNESS: f32 = 0.08;
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
    /// Miscellaneous symbols (ballot boxes, check marks, etc.) - snap to boundaries
    Symbol,
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

/// A line segment for box drawing
/// Coordinates are normalized (0.0-1.0) within the cell
#[derive(Debug, Clone, Copy)]
struct LineSegment {
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    thickness: f32,
}

impl LineSegment {
    const fn new(x1: f32, y1: f32, x2: f32, y2: f32, thickness: f32) -> Self {
        Self {
            x1,
            y1,
            x2,
            y2,
            thickness,
        }
    }

    /// Create a horizontal line segment
    const fn horizontal(y: f32, x1: f32, x2: f32, thickness: f32) -> Self {
        Self::new(x1, y, x2, y, thickness)
    }

    /// Create a vertical line segment
    const fn vertical(x: f32, y1: f32, y2: f32, thickness: f32) -> Self {
        Self::new(x, y1, x, y2, thickness)
    }

    /// Convert to a geometric block (rectangle)
    fn to_block(self, aspect_ratio: f32) -> GeometricBlock {
        let is_horizontal = (self.y1 - self.y2).abs() < 0.001;
        let is_vertical = (self.x1 - self.x2).abs() < 0.001;

        if is_horizontal {
            // Horizontal line: thickness applies to height
            let x = self.x1.min(self.x2);
            let width = (self.x2 - self.x1).abs();
            let height = self.thickness;
            let y = self.y1 - height / 2.0;
            GeometricBlock::new(x, y, width, height)
        } else if is_vertical {
            // Vertical line: thickness adjusted by aspect ratio for visual consistency
            let y = self.y1.min(self.y2);
            let height = (self.y2 - self.y1).abs();
            let width = self.thickness * aspect_ratio;
            let x = self.x1 - width / 2.0;
            GeometricBlock::new(x, y, width, height)
        } else {
            // Diagonal or other - treat as rectangle from corner to corner
            let x = self.x1.min(self.x2);
            let y = self.y1.min(self.y2);
            let width = (self.x2 - self.x1).abs().max(self.thickness);
            let height = (self.y2 - self.y1).abs().max(self.thickness);
            GeometricBlock::new(x, y, width, height)
        }
    }
}

/// Represents line segments for box drawing characters
#[derive(Debug, Clone)]
pub struct BoxDrawingGeometry {
    pub segments: Vec<GeometricBlock>,
}

impl BoxDrawingGeometry {
    fn from_lines(lines: &[LineSegment], aspect_ratio: f32) -> Self {
        Self {
            segments: lines.iter().map(|l| l.to_block(aspect_ratio)).collect(),
        }
    }
}

/// Get geometric representation of a box drawing character
/// aspect_ratio = cell_height / cell_width (used to make lines visually equal thickness)
/// Returns None if the character should use font rendering
pub fn get_box_drawing_geometry(ch: char, aspect_ratio: f32) -> Option<BoxDrawingGeometry> {
    use grid::*;

    let lt = LIGHT_THICKNESS;
    let ht = HEAVY_THICKNESS;
    let dt = DOUBLE_THICKNESS;

    let lines: &[LineSegment] = match ch {
        // ═══════════════════════════════════════════════════════════════════
        // LIGHT LINES
        // ═══════════════════════════════════════════════════════════════════

        // ─ Light horizontal
        '\u{2500}' => &[LineSegment::horizontal(V4, A, G, lt)],

        // │ Light vertical
        '\u{2502}' => &[LineSegment::vertical(D, V1, V7, lt)],

        // ┌ Light down and right - lines meet at center
        '\u{250C}' => &[
            LineSegment::horizontal(V4, D, G, lt),
            LineSegment::vertical(D, V4, V7, lt),
        ],

        // ┐ Light down and left
        '\u{2510}' => &[
            LineSegment::horizontal(V4, A, D, lt),
            LineSegment::vertical(D, V4, V7, lt),
        ],

        // └ Light up and right
        '\u{2514}' => &[
            LineSegment::horizontal(V4, D, G, lt),
            LineSegment::vertical(D, V1, V4, lt),
        ],

        // ┘ Light up and left
        '\u{2518}' => &[
            LineSegment::horizontal(V4, A, D, lt),
            LineSegment::vertical(D, V1, V4, lt),
        ],

        // ├ Light vertical and right
        '\u{251C}' => &[
            LineSegment::vertical(D, V1, V7, lt),
            LineSegment::horizontal(V4, D, G, lt),
        ],

        // ┤ Light vertical and left
        '\u{2524}' => &[
            LineSegment::vertical(D, V1, V7, lt),
            LineSegment::horizontal(V4, A, D, lt),
        ],

        // ┬ Light down and horizontal
        '\u{252C}' => &[
            LineSegment::horizontal(V4, A, G, lt),
            LineSegment::vertical(D, V4, V7, lt),
        ],

        // ┴ Light up and horizontal
        '\u{2534}' => &[
            LineSegment::horizontal(V4, A, G, lt),
            LineSegment::vertical(D, V1, V4, lt),
        ],

        // ┼ Light vertical and horizontal
        '\u{253C}' => &[
            LineSegment::horizontal(V4, A, G, lt),
            LineSegment::vertical(D, V1, V7, lt),
        ],

        // ═══════════════════════════════════════════════════════════════════
        // HEAVY LINES (two parallel strokes)
        // ═══════════════════════════════════════════════════════════════════

        // ━ Heavy horizontal
        '\u{2501}' => &[LineSegment::horizontal(V4, A, G, ht)],

        // ┃ Heavy vertical
        '\u{2503}' => &[LineSegment::vertical(D, V1, V7, ht)],

        // ┏ Heavy down and right
        '\u{250F}' => &[
            LineSegment::horizontal(V4, D, G, ht),
            LineSegment::vertical(D, V4, V7, ht),
        ],

        // ┓ Heavy down and left
        '\u{2513}' => &[
            LineSegment::horizontal(V4, A, D, ht),
            LineSegment::vertical(D, V4, V7, ht),
        ],

        // ┗ Heavy up and right
        '\u{2517}' => &[
            LineSegment::horizontal(V4, D, G, ht),
            LineSegment::vertical(D, V1, V4, ht),
        ],

        // ┛ Heavy up and left
        '\u{251B}' => &[
            LineSegment::horizontal(V4, A, D, ht),
            LineSegment::vertical(D, V1, V4, ht),
        ],

        // ┣ Heavy vertical and right
        '\u{2523}' => &[
            LineSegment::vertical(D, V1, V7, ht),
            LineSegment::horizontal(V4, D, G, ht),
        ],

        // ┫ Heavy vertical and left
        '\u{252B}' => &[
            LineSegment::vertical(D, V1, V7, ht),
            LineSegment::horizontal(V4, A, D, ht),
        ],

        // ┳ Heavy down and horizontal
        '\u{2533}' => &[
            LineSegment::horizontal(V4, A, G, ht),
            LineSegment::vertical(D, V4, V7, ht),
        ],

        // ┻ Heavy up and horizontal
        '\u{253B}' => &[
            LineSegment::horizontal(V4, A, G, ht),
            LineSegment::vertical(D, V1, V4, ht),
        ],

        // ╋ Heavy vertical and horizontal
        '\u{254B}' => &[
            LineSegment::horizontal(V4, A, G, ht),
            LineSegment::vertical(D, V1, V7, ht),
        ],

        // ═══════════════════════════════════════════════════════════════════
        // MIXED LIGHT/HEAVY LINES
        // ═══════════════════════════════════════════════════════════════════

        // ┍ Down light and right heavy
        '\u{250D}' => &[
            LineSegment::horizontal(V4, D, G, ht),
            LineSegment::vertical(D, V4, V7, lt),
        ],

        // ┎ Down heavy and right light
        '\u{250E}' => &[
            LineSegment::horizontal(V4, D, G, lt),
            LineSegment::vertical(D, V4, V7, ht),
        ],

        // ┑ Down light and left heavy
        '\u{2511}' => &[
            LineSegment::horizontal(V4, A, D, ht),
            LineSegment::vertical(D, V4, V7, lt),
        ],

        // ┒ Down heavy and left light
        '\u{2512}' => &[
            LineSegment::horizontal(V4, A, D, lt),
            LineSegment::vertical(D, V4, V7, ht),
        ],

        // ┕ Up light and right heavy
        '\u{2515}' => &[
            LineSegment::horizontal(V4, D, G, ht),
            LineSegment::vertical(D, V1, V4, lt),
        ],

        // ┖ Up heavy and right light
        '\u{2516}' => &[
            LineSegment::horizontal(V4, D, G, lt),
            LineSegment::vertical(D, V1, V4, ht),
        ],

        // ┙ Up light and left heavy
        '\u{2519}' => &[
            LineSegment::horizontal(V4, A, D, ht),
            LineSegment::vertical(D, V1, V4, lt),
        ],

        // ┚ Up heavy and left light
        '\u{251A}' => &[
            LineSegment::horizontal(V4, A, D, lt),
            LineSegment::vertical(D, V1, V4, ht),
        ],

        // ┝ Vertical light and right heavy
        '\u{251D}' => &[
            LineSegment::vertical(D, V1, V7, lt),
            LineSegment::horizontal(V4, D, G, ht),
        ],

        // ┞ Up heavy and right down light
        '\u{251E}' => &[
            LineSegment::vertical(D, V1, V4, ht),
            LineSegment::vertical(D, V4, V7, lt),
            LineSegment::horizontal(V4, D, G, lt),
        ],

        // ┟ Down heavy and right up light
        '\u{251F}' => &[
            LineSegment::vertical(D, V1, V4, lt),
            LineSegment::vertical(D, V4, V7, ht),
            LineSegment::horizontal(V4, D, G, lt),
        ],

        // ┠ Vertical heavy and right light
        '\u{2520}' => &[
            LineSegment::vertical(D, V1, V7, ht),
            LineSegment::horizontal(V4, D, G, lt),
        ],

        // ┡ Down light and right up heavy
        '\u{2521}' => &[
            LineSegment::vertical(D, V1, V4, ht),
            LineSegment::vertical(D, V4, V7, lt),
            LineSegment::horizontal(V4, D, G, ht),
        ],

        // ┢ Up light and right down heavy
        '\u{2522}' => &[
            LineSegment::vertical(D, V1, V4, lt),
            LineSegment::vertical(D, V4, V7, ht),
            LineSegment::horizontal(V4, D, G, ht),
        ],

        // ┥ Vertical light and left heavy
        '\u{2525}' => &[
            LineSegment::vertical(D, V1, V7, lt),
            LineSegment::horizontal(V4, A, D, ht),
        ],

        // ┦ Up heavy and left down light
        '\u{2526}' => &[
            LineSegment::vertical(D, V1, V4, ht),
            LineSegment::vertical(D, V4, V7, lt),
            LineSegment::horizontal(V4, A, D, lt),
        ],

        // ┧ Down heavy and left up light
        '\u{2527}' => &[
            LineSegment::vertical(D, V1, V4, lt),
            LineSegment::vertical(D, V4, V7, ht),
            LineSegment::horizontal(V4, A, D, lt),
        ],

        // ┨ Vertical heavy and left light
        '\u{2528}' => &[
            LineSegment::vertical(D, V1, V7, ht),
            LineSegment::horizontal(V4, A, D, lt),
        ],

        // ┩ Down light and left up heavy
        '\u{2529}' => &[
            LineSegment::vertical(D, V1, V4, ht),
            LineSegment::vertical(D, V4, V7, lt),
            LineSegment::horizontal(V4, A, D, ht),
        ],

        // ┪ Up light and left down heavy
        '\u{252A}' => &[
            LineSegment::vertical(D, V1, V4, lt),
            LineSegment::vertical(D, V4, V7, ht),
            LineSegment::horizontal(V4, A, D, ht),
        ],

        // ┭ Left heavy and right down light
        '\u{252D}' => &[
            LineSegment::horizontal(V4, A, D, ht),
            LineSegment::horizontal(V4, D, G, lt),
            LineSegment::vertical(D, V4, V7, lt),
        ],

        // ┮ Right heavy and left down light
        '\u{252E}' => &[
            LineSegment::horizontal(V4, A, D, lt),
            LineSegment::horizontal(V4, D, G, ht),
            LineSegment::vertical(D, V4, V7, lt),
        ],

        // ┯ Down light and horizontal heavy
        '\u{252F}' => &[
            LineSegment::horizontal(V4, A, G, ht),
            LineSegment::vertical(D, V4, V7, lt),
        ],

        // ┰ Down heavy and horizontal light
        '\u{2530}' => &[
            LineSegment::horizontal(V4, A, G, lt),
            LineSegment::vertical(D, V4, V7, ht),
        ],

        // ┱ Right light and left down heavy
        '\u{2531}' => &[
            LineSegment::horizontal(V4, A, D, ht),
            LineSegment::horizontal(V4, D, G, lt),
            LineSegment::vertical(D, V4, V7, ht),
        ],

        // ┲ Left light and right down heavy
        '\u{2532}' => &[
            LineSegment::horizontal(V4, A, D, lt),
            LineSegment::horizontal(V4, D, G, ht),
            LineSegment::vertical(D, V4, V7, ht),
        ],

        // ┵ Left heavy and right up light
        '\u{2535}' => &[
            LineSegment::horizontal(V4, A, D, ht),
            LineSegment::horizontal(V4, D, G, lt),
            LineSegment::vertical(D, V1, V4, lt),
        ],

        // ┶ Right heavy and left up light
        '\u{2536}' => &[
            LineSegment::horizontal(V4, A, D, lt),
            LineSegment::horizontal(V4, D, G, ht),
            LineSegment::vertical(D, V1, V4, lt),
        ],

        // ┷ Up light and horizontal heavy
        '\u{2537}' => &[
            LineSegment::horizontal(V4, A, G, ht),
            LineSegment::vertical(D, V1, V4, lt),
        ],

        // ┸ Up heavy and horizontal light
        '\u{2538}' => &[
            LineSegment::horizontal(V4, A, G, lt),
            LineSegment::vertical(D, V1, V4, ht),
        ],

        // ┹ Right light and left up heavy
        '\u{2539}' => &[
            LineSegment::horizontal(V4, A, D, ht),
            LineSegment::horizontal(V4, D, G, lt),
            LineSegment::vertical(D, V1, V4, ht),
        ],

        // ┺ Left light and right up heavy
        '\u{253A}' => &[
            LineSegment::horizontal(V4, A, D, lt),
            LineSegment::horizontal(V4, D, G, ht),
            LineSegment::vertical(D, V1, V4, ht),
        ],

        // ┽ Left heavy and right vertical light
        '\u{253D}' => &[
            LineSegment::horizontal(V4, A, D, ht),
            LineSegment::horizontal(V4, D, G, lt),
            LineSegment::vertical(D, V1, V7, lt),
        ],

        // ┾ Right heavy and left vertical light
        '\u{253E}' => &[
            LineSegment::horizontal(V4, A, D, lt),
            LineSegment::horizontal(V4, D, G, ht),
            LineSegment::vertical(D, V1, V7, lt),
        ],

        // ┿ Vertical light and horizontal heavy
        '\u{253F}' => &[
            LineSegment::horizontal(V4, A, G, ht),
            LineSegment::vertical(D, V1, V7, lt),
        ],

        // ╀ Up heavy and down horizontal light
        '\u{2540}' => &[
            LineSegment::horizontal(V4, A, G, lt),
            LineSegment::vertical(D, V1, V4, ht),
            LineSegment::vertical(D, V4, V7, lt),
        ],

        // ╁ Down heavy and up horizontal light
        '\u{2541}' => &[
            LineSegment::horizontal(V4, A, G, lt),
            LineSegment::vertical(D, V1, V4, lt),
            LineSegment::vertical(D, V4, V7, ht),
        ],

        // ╂ Vertical heavy and horizontal light
        '\u{2542}' => &[
            LineSegment::horizontal(V4, A, G, lt),
            LineSegment::vertical(D, V1, V7, ht),
        ],

        // ╃ Left up heavy and right down light
        '\u{2543}' => &[
            LineSegment::horizontal(V4, A, D, ht),
            LineSegment::horizontal(V4, D, G, lt),
            LineSegment::vertical(D, V1, V4, ht),
            LineSegment::vertical(D, V4, V7, lt),
        ],

        // ╄ Right up heavy and left down light
        '\u{2544}' => &[
            LineSegment::horizontal(V4, A, D, lt),
            LineSegment::horizontal(V4, D, G, ht),
            LineSegment::vertical(D, V1, V4, ht),
            LineSegment::vertical(D, V4, V7, lt),
        ],

        // ╅ Left down heavy and right up light
        '\u{2545}' => &[
            LineSegment::horizontal(V4, A, D, ht),
            LineSegment::horizontal(V4, D, G, lt),
            LineSegment::vertical(D, V1, V4, lt),
            LineSegment::vertical(D, V4, V7, ht),
        ],

        // ╆ Right down heavy and left up light
        '\u{2546}' => &[
            LineSegment::horizontal(V4, A, D, lt),
            LineSegment::horizontal(V4, D, G, ht),
            LineSegment::vertical(D, V1, V4, lt),
            LineSegment::vertical(D, V4, V7, ht),
        ],

        // ╇ Down light and up horizontal heavy
        '\u{2547}' => &[
            LineSegment::horizontal(V4, A, G, ht),
            LineSegment::vertical(D, V1, V4, ht),
            LineSegment::vertical(D, V4, V7, lt),
        ],

        // ╈ Up light and down horizontal heavy
        '\u{2548}' => &[
            LineSegment::horizontal(V4, A, G, ht),
            LineSegment::vertical(D, V1, V4, lt),
            LineSegment::vertical(D, V4, V7, ht),
        ],

        // ╉ Right light and left vertical heavy
        '\u{2549}' => &[
            LineSegment::horizontal(V4, A, D, ht),
            LineSegment::horizontal(V4, D, G, lt),
            LineSegment::vertical(D, V1, V7, ht),
        ],

        // ╊ Left light and right vertical heavy
        '\u{254A}' => &[
            LineSegment::horizontal(V4, A, D, lt),
            LineSegment::horizontal(V4, D, G, ht),
            LineSegment::vertical(D, V1, V7, ht),
        ],

        // ═══════════════════════════════════════════════════════════════════
        // DOUBLE LINES (two parallel strokes at 1/4 and 3/4)
        // ═══════════════════════════════════════════════════════════════════

        // ═ Double horizontal
        '\u{2550}' => &[
            LineSegment::horizontal(V3, A, G, dt),
            LineSegment::horizontal(V5, A, G, dt),
        ],

        // ║ Double vertical
        '\u{2551}' => &[
            LineSegment::vertical(C, V1, V7, dt),
            LineSegment::vertical(E, V1, V7, dt),
        ],

        // ╔ Double down and right
        '\u{2554}' => &[
            LineSegment::horizontal(V3, E, G, dt),
            LineSegment::horizontal(V5, C, G, dt),
            LineSegment::vertical(C, V3, V7, dt),
            LineSegment::vertical(E, V5, V7, dt),
        ],

        // ╗ Double down and left
        '\u{2557}' => &[
            LineSegment::horizontal(V3, A, C, dt),
            LineSegment::horizontal(V5, A, E, dt),
            LineSegment::vertical(C, V5, V7, dt),
            LineSegment::vertical(E, V3, V7, dt),
        ],

        // ╚ Double up and right
        '\u{255A}' => &[
            LineSegment::horizontal(V3, C, G, dt),
            LineSegment::horizontal(V5, E, G, dt),
            LineSegment::vertical(C, V1, V3, dt),
            LineSegment::vertical(E, V1, V5, dt),
        ],

        // ╝ Double up and left
        '\u{255D}' => &[
            LineSegment::horizontal(V3, A, E, dt),
            LineSegment::horizontal(V5, A, C, dt),
            LineSegment::vertical(C, V1, V5, dt),
            LineSegment::vertical(E, V1, V3, dt),
        ],

        // ╠ Double vertical and right
        '\u{2560}' => &[
            LineSegment::vertical(C, V1, V7, dt),
            LineSegment::vertical(E, V1, V3, dt),
            LineSegment::vertical(E, V5, V7, dt),
            LineSegment::horizontal(V3, E, G, dt),
            LineSegment::horizontal(V5, E, G, dt),
        ],

        // ╣ Double vertical and left
        '\u{2563}' => &[
            LineSegment::vertical(E, V1, V7, dt),
            LineSegment::vertical(C, V1, V3, dt),
            LineSegment::vertical(C, V5, V7, dt),
            LineSegment::horizontal(V3, A, C, dt),
            LineSegment::horizontal(V5, A, C, dt),
        ],

        // ╦ Double down and horizontal
        '\u{2566}' => &[
            LineSegment::horizontal(V3, A, G, dt),
            LineSegment::horizontal(V5, A, C, dt),
            LineSegment::horizontal(V5, E, G, dt),
            LineSegment::vertical(C, V5, V7, dt),
            LineSegment::vertical(E, V5, V7, dt),
        ],

        // ╩ Double up and horizontal
        '\u{2569}' => &[
            LineSegment::horizontal(V5, A, G, dt),
            LineSegment::horizontal(V3, A, C, dt),
            LineSegment::horizontal(V3, E, G, dt),
            LineSegment::vertical(C, V1, V3, dt),
            LineSegment::vertical(E, V1, V3, dt),
        ],

        // ╬ Double vertical and horizontal
        '\u{256C}' => &[
            LineSegment::horizontal(V3, A, C, dt),
            LineSegment::horizontal(V3, E, G, dt),
            LineSegment::horizontal(V5, A, C, dt),
            LineSegment::horizontal(V5, E, G, dt),
            LineSegment::vertical(C, V1, V3, dt),
            LineSegment::vertical(C, V5, V7, dt),
            LineSegment::vertical(E, V1, V3, dt),
            LineSegment::vertical(E, V5, V7, dt),
        ],

        // ═══════════════════════════════════════════════════════════════════
        // MIXED SINGLE/DOUBLE LINES
        // ═══════════════════════════════════════════════════════════════════

        // ╒ Down single and right double
        '\u{2552}' => &[
            LineSegment::horizontal(V3, D, G, dt),
            LineSegment::horizontal(V5, D, G, dt),
            LineSegment::vertical(D, V4, V7, lt),
        ],

        // ╓ Down double and right single
        '\u{2553}' => &[
            LineSegment::horizontal(V4, D, G, lt),
            LineSegment::vertical(C, V4, V7, dt),
            LineSegment::vertical(E, V4, V7, dt),
        ],

        // ╕ Down single and left double
        '\u{2555}' => &[
            LineSegment::horizontal(V3, A, D, dt),
            LineSegment::horizontal(V5, A, D, dt),
            LineSegment::vertical(D, V4, V7, lt),
        ],

        // ╖ Down double and left single
        '\u{2556}' => &[
            LineSegment::horizontal(V4, A, D, lt),
            LineSegment::vertical(C, V4, V7, dt),
            LineSegment::vertical(E, V4, V7, dt),
        ],

        // ╘ Up single and right double
        '\u{2558}' => &[
            LineSegment::horizontal(V3, D, G, dt),
            LineSegment::horizontal(V5, D, G, dt),
            LineSegment::vertical(D, V1, V4, lt),
        ],

        // ╙ Up double and right single
        '\u{2559}' => &[
            LineSegment::horizontal(V4, D, G, lt),
            LineSegment::vertical(C, V1, V4, dt),
            LineSegment::vertical(E, V1, V4, dt),
        ],

        // ╛ Up single and left double
        '\u{255B}' => &[
            LineSegment::horizontal(V3, A, D, dt),
            LineSegment::horizontal(V5, A, D, dt),
            LineSegment::vertical(D, V1, V4, lt),
        ],

        // ╜ Up double and left single
        '\u{255C}' => &[
            LineSegment::horizontal(V4, A, D, lt),
            LineSegment::vertical(C, V1, V4, dt),
            LineSegment::vertical(E, V1, V4, dt),
        ],

        // ╞ Vertical single and right double
        '\u{255E}' => &[
            LineSegment::vertical(D, V1, V7, lt),
            LineSegment::horizontal(V3, D, G, dt),
            LineSegment::horizontal(V5, D, G, dt),
        ],

        // ╟ Vertical double and right single
        '\u{255F}' => &[
            LineSegment::vertical(C, V1, V7, dt),
            LineSegment::vertical(E, V1, V7, dt),
            LineSegment::horizontal(V4, E, G, lt),
        ],

        // ╡ Vertical single and left double
        '\u{2561}' => &[
            LineSegment::vertical(D, V1, V7, lt),
            LineSegment::horizontal(V3, A, D, dt),
            LineSegment::horizontal(V5, A, D, dt),
        ],

        // ╢ Vertical double and left single
        '\u{2562}' => &[
            LineSegment::vertical(C, V1, V7, dt),
            LineSegment::vertical(E, V1, V7, dt),
            LineSegment::horizontal(V4, A, C, lt),
        ],

        // ╤ Down single and horizontal double
        '\u{2564}' => &[
            LineSegment::horizontal(V3, A, G, dt),
            LineSegment::horizontal(V5, A, G, dt),
            LineSegment::vertical(D, V5, V7, lt),
        ],

        // ╥ Down double and horizontal single
        '\u{2565}' => &[
            LineSegment::horizontal(V4, A, G, lt),
            LineSegment::vertical(C, V4, V7, dt),
            LineSegment::vertical(E, V4, V7, dt),
        ],

        // ╧ Up single and horizontal double
        '\u{2567}' => &[
            LineSegment::horizontal(V3, A, G, dt),
            LineSegment::horizontal(V5, A, G, dt),
            LineSegment::vertical(D, V1, V3, lt),
        ],

        // ╨ Up double and horizontal single
        '\u{2568}' => &[
            LineSegment::horizontal(V4, A, G, lt),
            LineSegment::vertical(C, V1, V4, dt),
            LineSegment::vertical(E, V1, V4, dt),
        ],

        // ╪ Vertical single and horizontal double
        '\u{256A}' => &[
            LineSegment::horizontal(V3, A, G, dt),
            LineSegment::horizontal(V5, A, G, dt),
            LineSegment::vertical(D, V1, V7, lt),
        ],

        // ╫ Vertical double and horizontal single
        '\u{256B}' => &[
            LineSegment::vertical(C, V1, V7, dt),
            LineSegment::vertical(E, V1, V7, dt),
            LineSegment::horizontal(V4, A, G, lt),
        ],

        // ═══════════════════════════════════════════════════════════════════
        // DASHED AND DOTTED LINES
        // ═══════════════════════════════════════════════════════════════════

        // ┄ Light triple dash horizontal
        '\u{2504}' => &[LineSegment::horizontal(V4, A, G, lt)],

        // ┅ Heavy triple dash horizontal
        '\u{2505}' => &[LineSegment::horizontal(V4, A, G, ht)],

        // ┆ Light triple dash vertical
        '\u{2506}' => &[LineSegment::vertical(D, V1, V7, lt)],

        // ┇ Heavy triple dash vertical
        '\u{2507}' => &[LineSegment::vertical(D, V1, V7, ht)],

        // ┈ Light quadruple dash horizontal
        '\u{2508}' => &[LineSegment::horizontal(V4, A, G, lt)],

        // ┉ Heavy quadruple dash horizontal
        '\u{2509}' => &[LineSegment::horizontal(V4, A, G, ht)],

        // ┊ Light quadruple dash vertical
        '\u{250A}' => &[LineSegment::vertical(D, V1, V7, lt)],

        // ┋ Heavy quadruple dash vertical
        '\u{250B}' => &[LineSegment::vertical(D, V1, V7, ht)],

        // ═══════════════════════════════════════════════════════════════════
        // ROUNDED CORNERS (rendered as sharp corners for now)
        // ═══════════════════════════════════════════════════════════════════

        // ╭ Light arc down and right
        '\u{256D}' => &[
            LineSegment::horizontal(V4, D, G, lt),
            LineSegment::vertical(D, V4, V7, lt),
        ],

        // ╮ Light arc down and left
        '\u{256E}' => &[
            LineSegment::horizontal(V4, A, D, lt),
            LineSegment::vertical(D, V4, V7, lt),
        ],

        // ╯ Light arc up and left
        '\u{256F}' => &[
            LineSegment::horizontal(V4, A, D, lt),
            LineSegment::vertical(D, V1, V4, lt),
        ],

        // ╰ Light arc up and right
        '\u{2570}' => &[
            LineSegment::horizontal(V4, D, G, lt),
            LineSegment::vertical(D, V1, V4, lt),
        ],

        // ═══════════════════════════════════════════════════════════════════
        // DIAGONAL LINES
        // ═══════════════════════════════════════════════════════════════════

        // ╱ Light diagonal upper right to lower left
        '\u{2571}' => &[LineSegment::new(G, V1, A, V7, lt)],

        // ╲ Light diagonal upper left to lower right
        '\u{2572}' => &[LineSegment::new(A, V1, G, V7, lt)],

        // ╳ Light diagonal cross
        '\u{2573}' => &[
            LineSegment::new(A, V1, G, V7, lt),
            LineSegment::new(G, V1, A, V7, lt),
        ],

        // ═══════════════════════════════════════════════════════════════════
        // HALF LINES
        // ═══════════════════════════════════════════════════════════════════

        // ╴ Light left
        '\u{2574}' => &[LineSegment::horizontal(V4, A, D, lt)],

        // ╵ Light up
        '\u{2575}' => &[LineSegment::vertical(D, V1, V4, lt)],

        // ╶ Light right
        '\u{2576}' => &[LineSegment::horizontal(V4, D, G, lt)],

        // ╷ Light down
        '\u{2577}' => &[LineSegment::vertical(D, V4, V7, lt)],

        // ╸ Heavy left
        '\u{2578}' => &[LineSegment::horizontal(V4, A, D, ht)],

        // ╹ Heavy up
        '\u{2579}' => &[LineSegment::vertical(D, V1, V4, ht)],

        // ╺ Heavy right
        '\u{257A}' => &[LineSegment::horizontal(V4, D, G, ht)],

        // ╻ Heavy down
        '\u{257B}' => &[LineSegment::vertical(D, V4, V7, ht)],

        // ╼ Light left and heavy right
        '\u{257C}' => &[
            LineSegment::horizontal(V4, A, D, lt),
            LineSegment::horizontal(V4, D, G, ht),
        ],

        // ╽ Light up and heavy down
        '\u{257D}' => &[
            LineSegment::vertical(D, V1, V4, lt),
            LineSegment::vertical(D, V4, V7, ht),
        ],

        // ╾ Heavy left and light right
        '\u{257E}' => &[
            LineSegment::horizontal(V4, A, D, ht),
            LineSegment::horizontal(V4, D, G, lt),
        ],

        // ╿ Heavy up and light down
        '\u{257F}' => &[
            LineSegment::vertical(D, V1, V4, ht),
            LineSegment::vertical(D, V4, V7, lt),
        ],

        _ => return None,
    };

    if lines.is_empty() {
        None
    } else {
        Some(BoxDrawingGeometry::from_lines(lines, aspect_ratio))
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

/// Get pixel-perfect rectangle for geometric shape characters (U+25A0–U+25FF).
///
/// Unlike block elements which fill the cell, geometric shapes like squares
/// preserve their aspect ratio by using `cell_w` as the base dimension and
/// centering vertically within the cell. Returns `None` for outline/hollow
/// shapes, circles, triangles, and other characters that can't be represented
/// as simple filled rectangles — those fall through to font rendering.
pub fn get_geometric_shape_rect(
    ch: char,
    cell_x: f32,
    cell_y: f32,
    cell_w: f32,
    cell_h: f32,
) -> Option<PixelRect> {
    match ch {
        // ■ U+25A0 BLACK SQUARE — full cell width square
        '\u{25A0}' => {
            let size = cell_w;
            Some(PixelRect {
                x: cell_x,
                y: cell_y + (cell_h - size) / 2.0,
                width: size,
                height: size,
            })
        }
        // ▪ U+25AA BLACK SMALL SQUARE — 0.5× cell width
        '\u{25AA}' => {
            let size = cell_w * 0.5;
            Some(PixelRect {
                x: cell_x + (cell_w - size) / 2.0,
                y: cell_y + (cell_h - size) / 2.0,
                width: size,
                height: size,
            })
        }
        // ▬ U+25AC BLACK RECTANGLE — horizontal rectangle, full width, 1/3 height
        '\u{25AC}' => {
            let h = cell_h * 0.33;
            Some(PixelRect {
                x: cell_x,
                y: cell_y + (cell_h - h) / 2.0,
                width: cell_w,
                height: h,
            })
        }
        // ▮ U+25AE BLACK VERTICAL RECTANGLE — half width, full height
        '\u{25AE}' => {
            let w = cell_w * 0.5;
            Some(PixelRect {
                x: cell_x + (cell_w - w) / 2.0,
                y: cell_y,
                width: w,
                height: cell_h,
            })
        }
        // ◼ U+25FC BLACK MEDIUM SQUARE — 0.75× cell width
        '\u{25FC}' => {
            let size = cell_w * 0.75;
            Some(PixelRect {
                x: cell_x + (cell_w - size) / 2.0,
                y: cell_y + (cell_h - size) / 2.0,
                width: size,
                height: size,
            })
        }
        // ◾ U+25FE BLACK MEDIUM SMALL SQUARE — 0.625× cell width
        '\u{25FE}' => {
            let size = cell_w * 0.625;
            Some(PixelRect {
                x: cell_x + (cell_w - size) / 2.0,
                y: cell_y + (cell_h - size) / 2.0,
                width: size,
                height: size,
            })
        }
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
