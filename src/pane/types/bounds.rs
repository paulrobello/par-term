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

#[cfg(test)]
mod tests {
    use super::*;

    // ── PaneBounds::contains ──────────────────────────────────────────────

    #[test]
    fn test_contains_interior_point() {
        let b = PaneBounds::new(10.0, 20.0, 100.0, 80.0);
        assert!(b.contains(50.0, 50.0));
    }

    #[test]
    fn test_contains_top_left_corner() {
        let b = PaneBounds::new(10.0, 20.0, 100.0, 80.0);
        // Inclusive lower bound
        assert!(b.contains(10.0, 20.0));
    }

    #[test]
    fn test_contains_exclusive_right_edge() {
        let b = PaneBounds::new(0.0, 0.0, 100.0, 100.0);
        // Right/bottom edge is exclusive
        assert!(!b.contains(100.0, 50.0));
        assert!(!b.contains(50.0, 100.0));
    }

    #[test]
    fn test_contains_outside_left() {
        let b = PaneBounds::new(10.0, 10.0, 50.0, 50.0);
        assert!(!b.contains(9.9, 30.0));
    }

    #[test]
    fn test_contains_outside_above() {
        let b = PaneBounds::new(10.0, 10.0, 50.0, 50.0);
        assert!(!b.contains(30.0, 9.9));
    }

    // ── PaneBounds::center ────────────────────────────────────────────────

    #[test]
    fn test_center_unit_box() {
        let b = PaneBounds::new(0.0, 0.0, 2.0, 2.0);
        assert_eq!(b.center(), (1.0, 1.0));
    }

    #[test]
    fn test_center_offset_box() {
        let b = PaneBounds::new(10.0, 20.0, 40.0, 60.0);
        assert_eq!(b.center(), (30.0, 50.0));
    }

    // ── PaneBounds::grid_size ─────────────────────────────────────────────

    #[test]
    fn test_grid_size_exact_division() {
        // 800 x 600 pixels at 8x16 cell size → 100 cols × 37 rows
        let b = PaneBounds::new(0.0, 0.0, 800.0, 600.0);
        let (cols, rows) = b.grid_size(8.0, 16.0);
        assert_eq!(cols, 100);
        assert_eq!(rows, 37);
    }

    #[test]
    fn test_grid_size_fractional_truncated() {
        // 85 / 8 = 10.625 → truncated to 10
        let b = PaneBounds::new(0.0, 0.0, 85.0, 100.0);
        let (cols, _) = b.grid_size(8.0, 16.0);
        assert_eq!(cols, 10);
    }

    #[test]
    fn test_grid_size_minimum_one() {
        // Very small pane — must return at least 1×1
        let b = PaneBounds::new(0.0, 0.0, 1.0, 1.0);
        let (cols, rows) = b.grid_size(8.0, 16.0);
        assert_eq!(cols, 1);
        assert_eq!(rows, 1);
    }

    #[test]
    fn test_grid_size_zero_dimensions_minimum_one() {
        let b = PaneBounds::new(0.0, 0.0, 0.0, 0.0);
        let (cols, rows) = b.grid_size(8.0, 16.0);
        assert_eq!(cols, 1);
        assert_eq!(rows, 1);
    }

    // ── PaneBounds::default ───────────────────────────────────────────────

    #[test]
    fn test_default_is_zero() {
        let b = PaneBounds::default();
        assert_eq!(b.x, 0.0);
        assert_eq!(b.y, 0.0);
        assert_eq!(b.width, 0.0);
        assert_eq!(b.height, 0.0);
    }
}
