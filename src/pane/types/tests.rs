//! Tests for PaneBounds and SplitDirection.

use super::bounds::PaneBounds;
use super::common::SplitDirection;

#[test]
fn test_pane_bounds_contains() {
    let bounds = PaneBounds::new(10.0, 20.0, 100.0, 50.0);

    // Inside
    assert!(bounds.contains(50.0, 40.0));
    assert!(bounds.contains(10.0, 20.0)); // Top-left corner

    // Outside
    assert!(!bounds.contains(5.0, 40.0)); // Left of bounds
    assert!(!bounds.contains(150.0, 40.0)); // Right of bounds
    assert!(!bounds.contains(50.0, 10.0)); // Above bounds
    assert!(!bounds.contains(50.0, 80.0)); // Below bounds
}

#[test]
fn test_pane_bounds_grid_size() {
    let bounds = PaneBounds::new(0.0, 0.0, 800.0, 600.0);
    let (cols, rows) = bounds.grid_size(10.0, 20.0);
    assert_eq!(cols, 80);
    assert_eq!(rows, 30);
}

#[test]
fn test_split_direction_clone() {
    let dir = SplitDirection::Horizontal;
    let cloned = dir;
    assert_eq!(dir, cloned);
}
