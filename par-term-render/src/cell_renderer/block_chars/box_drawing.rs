//! Box drawing character rendering.
//!
//! Provides geometric representations of Unicode box drawing characters (U+2500–U+257F)
//! using the 7-position grid system for precise positioning of light, heavy, and double lines.
//!
//! The raw character data lives in [`super::box_drawing_data`].  This module owns the
//! `LazyLock<HashMap>` built from that data and exposes the public lookup function.

use super::types::{BoxDrawingGeometry, LineSegment};
use std::collections::HashMap;
use std::sync::LazyLock;

/// Static lookup map: Unicode char → `&'static [LineSegment]`.
///
/// Populated once on first access from the raw data in `super::box_drawing_data`.
static BOX_DRAWING_MAP: LazyLock<HashMap<char, &'static [LineSegment]>> = LazyLock::new(|| {
    super::box_drawing_data::BOX_DRAWING_ENTRIES
        .iter()
        .copied()
        .collect()
});

/// Get geometric representation of a box drawing character.
///
/// `aspect_ratio` = cell_height / cell_width (used to make lines visually equal thickness).
/// Returns `None` if the character is not a recognised box drawing character.
pub fn get_box_drawing_geometry(ch: char, aspect_ratio: f32) -> Option<BoxDrawingGeometry> {
    let segments = BOX_DRAWING_MAP.get(&ch)?;
    if segments.is_empty() {
        return None;
    }
    Some(BoxDrawingGeometry::from_lines(segments, aspect_ratio))
}
