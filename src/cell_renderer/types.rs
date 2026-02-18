/// Vertex for cell rendering
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Vertex {
    pub position: [f32; 2],
    pub tex_coords: [f32; 2],
}

/// Instance data for background rendering
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct BackgroundInstance {
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub color: [f32; 4],
}

/// Instance data for text rendering
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct TextInstance {
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub tex_offset: [f32; 2],
    pub tex_size: [f32; 2],
    pub color: [f32; 4],
    pub is_colored: u32, // 1 for emoji/colored glyphs, 0 for regular text
}

// Re-export Cell from par-term-config (shared type used by terminal and renderer)
pub use par_term_config::Cell;

/// Glyph info for atlas
#[derive(Clone, Debug)]
pub(crate) struct GlyphInfo {
    #[allow(dead_code)]
    pub key: u64,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    #[allow(dead_code)]
    pub bearing_x: f32,
    #[allow(dead_code)]
    pub bearing_y: f32,
    pub is_colored: bool,
    pub prev: Option<u64>,
    pub next: Option<u64>,
}

/// Row cache entry
pub(crate) struct RowCacheEntry {}

/// Viewport for rendering a single pane
///
/// All coordinates are in pixels relative to the window surface.
#[derive(Clone, Copy, Debug, Default)]
pub struct PaneViewport {
    /// X position in pixels from left edge of window
    pub x: f32,
    /// Y position in pixels from top edge of window
    pub y: f32,
    /// Width in pixels
    pub width: f32,
    /// Height in pixels
    pub height: f32,
    /// Whether this pane is focused (affects focus indicator rendering)
    pub focused: bool,
    /// Opacity multiplier for inactive pane dimming (0.0-1.0)
    pub opacity: f32,
    /// Padding inside the pane (content inset from edges)
    pub padding: f32,
}

impl PaneViewport {
    /// Create a new pane viewport
    pub fn new(x: f32, y: f32, width: f32, height: f32, focused: bool, opacity: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
            focused,
            opacity,
            padding: 0.0,
        }
    }

    /// Create a new pane viewport with padding
    pub fn with_padding(
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        focused: bool,
        opacity: f32,
        padding: f32,
    ) -> Self {
        Self {
            x,
            y,
            width,
            height,
            focused,
            opacity,
            padding,
        }
    }

    /// Convert to wgpu scissor rect (u32 values) - uses full bounds
    pub fn to_scissor_rect(&self) -> (u32, u32, u32, u32) {
        (
            self.x.max(0.0) as u32,
            self.y.max(0.0) as u32,
            self.width.max(1.0) as u32,
            self.height.max(1.0) as u32,
        )
    }

    /// Get the content area origin (with padding applied)
    pub fn content_origin(&self) -> (f32, f32) {
        (self.x + self.padding, self.y + self.padding)
    }

    /// Get the content area size (with padding applied)
    pub fn content_size(&self) -> (f32, f32) {
        (
            (self.width - self.padding * 2.0).max(1.0),
            (self.height - self.padding * 2.0).max(1.0),
        )
    }

    /// Calculate grid dimensions (cols, rows) given cell dimensions
    /// Uses content size (with padding) for calculation
    pub fn grid_size(&self, cell_width: f32, cell_height: f32) -> (usize, usize) {
        let (content_width, content_height) = self.content_size();
        let cols = (content_width / cell_width).floor() as usize;
        let rows = (content_height / cell_height).floor() as usize;
        (cols.max(1), rows.max(1))
    }
}
