//! Shared types for block character rendering.

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

impl PixelRect {
    /// Snap rectangle coordinates to the pixel grid for crisp, consistent rendering.
    ///
    /// Without snapping, thin geometric lines (e.g. box-drawing characters) can
    /// straddle pixel boundaries and appear as 1 pixel in some positions and 2 pixels
    /// in others, causing visual inconsistency (e.g. tmux pane borders flickering
    /// between single and double lines).
    pub fn snap_to_pixels(self) -> Self {
        let x = self.x.round();
        let y = self.y.round();
        let w = self.width.round().max(1.0);
        let h = self.height.round().max(1.0);
        Self {
            x,
            y,
            width: w,
            height: h,
        }
    }
}

/// Represents line segments for box drawing characters
#[derive(Debug, Clone)]
pub struct BoxDrawingGeometry {
    pub segments: Vec<GeometricBlock>,
}

impl BoxDrawingGeometry {
    pub(super) fn from_lines(lines: &[LineSegment], aspect_ratio: f32) -> Self {
        Self {
            segments: lines.iter().map(|l| l.to_block(aspect_ratio)).collect(),
        }
    }
}

/// A line segment for box drawing.
/// Coordinates are normalized (0.0-1.0) within the cell.
#[derive(Debug, Clone, Copy)]
pub(super) struct LineSegment {
    pub(super) x1: f32,
    pub(super) y1: f32,
    pub(super) x2: f32,
    pub(super) y2: f32,
    pub(super) thickness: f32,
}

impl LineSegment {
    pub(super) const fn new(x1: f32, y1: f32, x2: f32, y2: f32, thickness: f32) -> Self {
        Self {
            x1,
            y1,
            x2,
            y2,
            thickness,
        }
    }

    /// Create a horizontal line segment
    pub(super) const fn horizontal(y: f32, x1: f32, x2: f32, thickness: f32) -> Self {
        Self::new(x1, y, x2, y, thickness)
    }

    /// Create a vertical line segment
    pub(super) const fn vertical(x: f32, y1: f32, y2: f32, thickness: f32) -> Self {
        Self::new(x, y1, x, y2, thickness)
    }

    /// Convert to a geometric block (rectangle)
    pub(super) fn to_block(self, aspect_ratio: f32) -> GeometricBlock {
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
pub(super) mod grid {
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
