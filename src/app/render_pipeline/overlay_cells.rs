//! Final text overlay helpers for pane-rendered cells.
//!
//! These run after terminal cell generation so transient overlays
//! (URL hover/underline, search highlights) are applied as the final text layer.

use crate::cell_renderer::Cell;
use crate::url_detection::{DetectedItemType, DetectedUrl};

/// Parameters for [`apply_url_overlays_to_cells`].
pub(super) struct UrlOverlayParams<'a> {
    /// Mutable cell grid for the current pane/frame.
    pub(super) cells: &'a mut [Cell],
    /// Number of columns in the pane grid.
    pub(super) cols: usize,
    /// URLs detected in the viewport snapshot.
    pub(super) detected_urls: &'a [DetectedUrl],
    /// Scroll offset used when `detected_urls` were computed.
    pub(super) url_scroll_offset: usize,
    /// Optional hovered URL bounds: `(absolute_row, start_col, end_col)`.
    pub(super) hovered_bounds: Option<(usize, usize, usize)>,
    /// Foreground color to apply to the hovered URL when enabled.
    pub(super) url_color: [u8; 4],
    /// Whether to color the hovered URL foreground.
    pub(super) do_color: bool,
    /// Whether to underline detected URL cells.
    pub(super) do_underline: bool,
}

/// Apply URL underline/hover styling to the final pane cell buffer.
pub(super) fn apply_url_overlays_to_cells(params: UrlOverlayParams<'_>) {
    let UrlOverlayParams {
        cells,
        cols,
        detected_urls,
        url_scroll_offset,
        hovered_bounds,
        url_color,
        do_color,
        do_underline,
    } = params;

    if detected_urls.is_empty() || cols == 0 {
        return;
    }

    for url in detected_urls {
        if url.row < url_scroll_offset {
            continue;
        }
        let viewport_row = url.row - url_scroll_offset;
        let is_hovered = hovered_bounds == Some((url.row, url.start_col, url.end_col));
        for col in url.start_col..url.end_col {
            if col >= cols {
                break;
            }
            let cell_idx = viewport_row * cols + col;
            if cell_idx < cells.len() {
                if do_color && is_hovered && matches!(url.item_type, DetectedItemType::Url) {
                    cells[cell_idx].fg_color = url_color;
                }
                if do_underline {
                    cells[cell_idx].underline = true;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{UrlOverlayParams, apply_url_overlays_to_cells};
    use crate::url_detection::{DetectedItemType, DetectedUrl};

    fn detected_url(row: usize, start_col: usize, end_col: usize) -> DetectedUrl {
        DetectedUrl {
            url: "https://example.com".to_string(),
            start_col,
            end_col,
            row,
            hyperlink_id: None,
            item_type: DetectedItemType::Url,
        }
    }

    fn detected_path(row: usize, start_col: usize, end_col: usize) -> DetectedUrl {
        DetectedUrl {
            url: "~/Repos/par-term".to_string(),
            start_col,
            end_col,
            row,
            hyperlink_id: None,
            item_type: DetectedItemType::FilePath {
                line: None,
                column: None,
            },
        }
    }

    #[test]
    fn url_overlay_applies_to_final_cells_after_substitution() {
        let mut cells = vec![crate::cell_renderer::Cell::default(); 10];
        for (idx, ch) in "rendered".chars().enumerate() {
            cells[idx].grapheme = ch.to_string();
        }

        apply_url_overlays_to_cells(UrlOverlayParams {
            cells: &mut cells,
            cols: 10,
            detected_urls: &[detected_url(5, 2, 6)],
            url_scroll_offset: 5,
            hovered_bounds: Some((5, 2, 6)),
            url_color: [1, 2, 3, 255],
            do_color: true,
            do_underline: true,
        });

        for (idx, cell) in cells.iter().enumerate().take(6).skip(2) {
            assert!(cell.underline, "cell {idx} should be underlined");
            assert_eq!(cell.fg_color, [1, 2, 3, 255]);
        }
        assert_eq!(cells[0].grapheme, "r");
    }

    #[test]
    fn url_overlay_uses_detection_scroll_offset() {
        let mut cells = vec![crate::cell_renderer::Cell::default(); 20];

        apply_url_overlays_to_cells(UrlOverlayParams {
            cells: &mut cells,
            cols: 10,
            detected_urls: &[detected_url(7, 1, 3)],
            url_scroll_offset: 6,
            hovered_bounds: None,
            url_color: [1, 2, 3, 255],
            do_color: false,
            do_underline: true,
        });

        assert!(cells[11].underline);
        assert!(cells[12].underline);
        assert!(!cells[1].underline);
    }

    #[test]
    fn file_path_hover_does_not_overwrite_existing_prompt_color() {
        let mut cells = vec![crate::cell_renderer::Cell::default(); 10];
        cells[2].fg_color = [9, 8, 7, 255];

        apply_url_overlays_to_cells(UrlOverlayParams {
            cells: &mut cells,
            cols: 10,
            detected_urls: &[detected_path(5, 2, 5)],
            url_scroll_offset: 5,
            hovered_bounds: Some((5, 2, 5)),
            url_color: [1, 2, 3, 255],
            do_color: true,
            do_underline: true,
        });

        assert!(cells[2].underline);
        assert_eq!(cells[2].fg_color, [9, 8, 7, 255]);
    }
}
