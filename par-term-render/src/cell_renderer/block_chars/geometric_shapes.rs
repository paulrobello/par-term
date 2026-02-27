//! Geometric shape character rendering (U+25A0–U+25FF).
//!
//! Provides pixel-perfect rectangles for filled geometric shapes such as
//! black squares (■), small squares (▪), rectangles (▬▮), and medium squares (◼◾).
//! Outline/hollow shapes return `None` and fall through to font rendering.

use super::types::PixelRect;

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
