/// Powerline fringe-extension helpers.
///
/// When a colored background run is adjacent to a powerline separator glyph whose
/// background is the terminal default color, anti-aliased glyph pixels blend against
/// the viewport fill (dark) rather than the colored run, producing a visible fringe.
///
/// These helpers extend the colored quad by 1 px to sit under the glyph so the blend
/// becomes:  `fg * alpha + colored * (1 - alpha)`  →  seamless transition.
use par_term_config::{Cell, color_u8x4_rgb_to_f32};

/// All Nerd Font powerline codepoints that may produce a visible fringe when
/// adjacent to a differently-colored background run.
const POWERLINE_GLYPHS: &[&str] = &[
    "\u{E0B0}", "\u{E0B1}", "\u{E0B2}", "\u{E0B3}", "\u{E0B4}", "\u{E0B5}", "\u{E0B6}", "\u{E0B7}",
];

/// Right-pointing powerline separator glyphs that are rendered in the RLE path
/// in background-image mode — requiring the special trim of their own x0.
const POWERLINE_RIGHT_POINTING: &[&str] = &["\u{E0B0}", "\u{E0B1}", "\u{E0B4}", "\u{E0B5}"];

/// Return true if `bg` is equal to `background_color` (within tolerance).
///
/// `background_color` is the renderer's `[f32; 4]` field — only the RGB channels are compared.
pub(super) fn is_default_bg(bg: [u8; 4], background_color: [f32; 4]) -> bool {
    let f = color_u8x4_rgb_to_f32(bg);
    (f[0] - background_color[0]).abs() < 0.001
        && (f[1] - background_color[1]).abs() < 0.001
        && (f[2] - background_color[2]).abs() < 0.001
}

/// Parameters for `extend_powerline_fringes`.
pub(super) struct PowerlineFringeParams<'a> {
    /// The full row of cells.
    pub row_cells: &'a [Cell],
    /// Start column of the current RLE run.
    pub start_col: usize,
    /// Column index just past the end of the run (i.e. the next-cell index).
    pub col: usize,
    /// Left pixel edge of the run (pre-adjustment).
    pub x0: f32,
    /// Right pixel edge of the run (pre-adjustment).
    pub x1: f32,
    /// True when a custom shader or background image is active (no viewport fill drawn).
    pub skip_solid_background: bool,
    /// True when the current run's background equals the terminal default color.
    pub is_default_bg: bool,
    /// Terminal default background color (`CellRenderer::background_color`, `[f32; 4]`).
    /// Only the RGB channels are used for comparison.
    pub background_color: [f32; 4],
}

/// Adjust `(x0, x1)` pixel edges to eliminate the anti-aliased fringe at
/// powerline separator boundaries.
///
/// Returns `(adjusted_x0, adjusted_x1)`.
///
/// Three adjustments are applied, in order:
///
/// 1. **Extend right** by 1 px if the next cell (at `col`) is a powerline glyph with
///    default background — covers the glyph's anti-aliased left edge.
/// 2. **Extend left** by 1 px if the previous cell is a powerline glyph with default
///    background — covers its anti-aliased right edge.
/// 3. **Trim left** by 1 px (only in bg-image mode) when *this* run starts on a
///    right-pointing separator whose left neighbor is a colored run — prevents the
///    separator's own BG quad from covering the 1 px extension from step 1.
pub(super) fn extend_powerline_fringes(p: PowerlineFringeParams<'_>) -> (f32, f32) {
    let PowerlineFringeParams {
        row_cells,
        start_col,
        col,
        x0,
        x1,
        skip_solid_background,
        is_default_bg: run_is_default_bg,
        background_color,
    } = p;

    let is_def = |bg: [u8; 4]| self::is_default_bg(bg, background_color);
    let is_powerline = |g: &str| POWERLINE_GLYPHS.contains(&g);

    // 1. Extend right: next cell is a powerline glyph with default bg.
    let x1 = if col < row_cells.len()
        && is_powerline(row_cells[col].grapheme.as_str())
        && is_def(row_cells[col].bg_color)
    {
        x1 + 1.0
    } else {
        x1
    };

    // 2. Extend left: previous cell is a powerline glyph with default bg.
    let x0 = if start_col > 0
        && is_powerline(row_cells[start_col - 1].grapheme.as_str())
        && is_def(row_cells[start_col - 1].bg_color)
    {
        x0 - 1.0
    } else {
        x0
    };

    // 3. In bg-image mode: trim x0 of a right-pointing separator's own BG quad so the
    //    1 px extension from the colored run to its left remains visible.
    let x0 = if skip_solid_background
        && run_is_default_bg
        && POWERLINE_RIGHT_POINTING.contains(&row_cells[start_col].grapheme.as_str())
        && start_col > 0
        && !is_def(row_cells[start_col - 1].bg_color)
    {
        x0 + 1.0
    } else {
        x0
    };

    (x0, x1)
}
