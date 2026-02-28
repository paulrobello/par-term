//! `PaneBounds` — pixel-space bounding box for a pane.

/// Bounds of a pane in pixels
#[derive(Debug, Clone, Copy, Default)]
pub struct PaneBounds {
    /// X position in pixels from left edge of content area
    pub x: f32,
    /// Y position in pixels from top of content area (below tab bar)
    pub y: f32,
    /// Width in pixels
    pub width: f32,
    /// Height in pixels
    pub height: f32,
}

impl PaneBounds {
    /// Create new bounds
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Check if a point is inside these bounds
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }

    /// Get the center point of the bounds
    pub fn center(&self) -> (f32, f32) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    /// Calculate grid dimensions (cols, rows) given cell dimensions
    pub fn grid_size(&self, cell_width: f32, cell_height: f32) -> (usize, usize) {
        let cols = (self.width / cell_width).floor() as usize;
        let rows = (self.height / cell_height).floor() as usize;
        (cols.max(1), rows.max(1))
    }
}
