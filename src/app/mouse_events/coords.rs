use crate::app::window_state::WindowState;
use std::sync::Arc;

// ── Pure coordinate math (extracted for unit testing) ────────────────────────

/// Pane bounds and cell metrics for `pixel_to_pane_cell_raw`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PaneBoundsRaw {
    pub bx: f64,
    pub by: f64,
    pub bw: f64,
    pub bh: f64,
    pub cell_width: f64,
    pub cell_height: f64,
    pub pane_padding: f64,
    pub title_offset: f64,
}

/// Convert pixel coordinates to terminal cell coordinates given renderer metrics.
///
/// Returns `None` if the resulting row or column would be negative (caller should
/// treat as "outside the terminal grid").
///
/// This is the core math from `WindowState::pixel_to_cell` extracted so it can
/// be tested independently without a GPU renderer.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn pixel_to_cell_raw(
    x: f64,
    y: f64,
    cell_width: f64,
    cell_height: f64,
    padding: f64,
    content_offset_x: f64,
    content_offset_y: f64,
) -> (usize, usize) {
    let adjusted_x = (x - padding - content_offset_x).max(0.0);
    let adjusted_y = (y - padding - content_offset_y).max(0.0);
    let col = (adjusted_x / cell_width) as usize;
    let row = (adjusted_y / cell_height) as usize;
    (col, row)
}

/// Convert pixel coordinates to pane-local cell coordinates.
///
/// Returns `None` if `(x, y)` is outside the pane bounds described by `pane`.
/// This is the core math from `WindowState::pixel_to_pane_cell` extracted for
/// unit testing without a live renderer or pane configuration.
///
/// The centering offsets mirror `gather_pane_render_data` so that the integer
/// cell grid centred within the content area maps back correctly.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn pixel_to_pane_cell_raw(
    x: f64,
    y: f64,
    pane: PaneBoundsRaw,
) -> Option<(usize, usize)> {
    let PaneBoundsRaw {
        bx,
        by,
        bw,
        bh,
        cell_width,
        cell_height,
        pane_padding,
        title_offset,
    } = pane;
    if x < bx || x >= bx + bw || y < by || y >= by + bh {
        return None;
    }

    // Compute the same centering offsets as gather_pane_render_data.
    let viewport_h = bh - title_offset;
    let content_h = (viewport_h - pane_padding * 2.0).max(cell_height);
    let rows_fit = ((content_h / cell_height).floor() as usize).max(1);
    let center_offset_y = ((content_h - rows_fit as f64 * cell_height) / 2.0).floor();

    let content_w = (bw - pane_padding * 2.0).max(cell_width);
    let cols_fit = ((content_w / cell_width).floor() as usize).max(1);
    let center_offset_x = ((content_w - cols_fit as f64 * cell_width) / 2.0).floor();

    let local_x = (x - bx - pane_padding - center_offset_x).max(0.0);
    let local_y = (y - by - pane_padding - title_offset - center_offset_y).max(0.0);
    let col = (local_x / cell_width) as usize;
    let row = (local_y / cell_height) as usize;
    Some((col, row))
}

impl WindowState {
    /// Convert pixel coordinates to terminal cell coordinates
    pub(crate) fn pixel_to_cell(&self, x: f64, y: f64) -> Option<(usize, usize)> {
        if let Some(renderer) = &self.renderer {
            // Use actual cell dimensions from renderer for accurate coordinate mapping
            let cell_width = renderer.cell_width() as f64;
            let cell_height = renderer.cell_height() as f64;
            let padding = renderer.window_padding() as f64;
            let content_offset_y = renderer.content_offset_y() as f64;
            let content_offset_x = renderer.content_offset_x() as f64;

            // Account for window padding (all sides) and content offsets (tab bar)
            let adjusted_x = (x - padding - content_offset_x).max(0.0);
            let adjusted_y = (y - padding - content_offset_y).max(0.0);

            let col = (adjusted_x / cell_width) as usize;
            let row = (adjusted_y / cell_height) as usize;

            Some((col, row))
        } else {
            None
        }
    }

    /// Convert pixel coordinates to cell coordinates relative to a specific pane
    ///
    /// Accounts for pane bounds, pane padding, and pane title bar offset.
    /// Returns None if the pixel coordinates are outside the pane's bounds
    /// or if the renderer is not available.
    pub(crate) fn pixel_to_pane_cell(
        &self,
        x: f64,
        y: f64,
        pane_bounds: &crate::pane::PaneBounds,
    ) -> Option<(usize, usize)> {
        let renderer = self.renderer.as_ref()?;

        // Check if the point is inside the pane's bounds
        let bx = pane_bounds.x as f64;
        let by = pane_bounds.y as f64;
        let bw = pane_bounds.width as f64;
        let bh = pane_bounds.height as f64;
        if x < bx || x >= bx + bw || y < by || y >= by + bh {
            return None;
        }

        let cell_width = renderer.cell_width() as f64;
        let cell_height = renderer.cell_height() as f64;

        // Calculate physical pane padding (config is logical, scale for DPI).
        // Suppress padding when there is only one pane — no dividers means no padding needed.
        let scale = renderer.scale_factor() as f64;
        let pane_count = self
            .tab_manager
            .active_tab()
            .and_then(|t| t.pane_manager.as_ref())
            .map(|pm| pm.pane_count())
            .unwrap_or(0);
        // In split mode: half divider width (to avoid overlap) + user padding, scaled to physical.
        // Single-pane and tmux-gateway: zero padding.
        let pane_padding = if self.is_gateway_active() || pane_count <= 1 {
            0.0
        } else {
            (self.config.load().pane_divider_width.unwrap_or(2.0) / 2.0
                + self.config.load().pane_padding) as f64
                * scale
        };

        // Account for pane title bar if enabled
        let title_offset = if self.config.load().show_pane_titles {
            self.config.load().pane_title_height as f64 * scale
        } else {
            0.0
        };

        // Mirror the centering offsets from gather_pane_render_data so that the
        // coordinate mapping matches the renderer's pixel layout.
        // The renderer centres the integer cell grid within the content area;
        // without subtracting these offsets the bottom ~center_offset_y pixels of
        // each row map to the *next* row — up to cell_height/2 - 1 pixels off on
        // HiDPI displays (visible as "half a cell off" during drag-selection).
        let viewport_h = bh - title_offset;
        let content_h = (viewport_h - pane_padding * 2.0).max(cell_height);
        let rows_fit = ((content_h / cell_height).floor() as usize).max(1);
        let center_offset_y = ((content_h - rows_fit as f64 * cell_height) / 2.0).floor();

        let content_w = (bw - pane_padding * 2.0).max(cell_width);
        let cols_fit = ((content_w / cell_width).floor() as usize).max(1);
        let center_offset_x = ((content_w - cols_fit as f64 * cell_width) / 2.0).floor();

        // Convert to pane-local coordinates, accounting for padding and centering.
        let local_x = (x - bx - pane_padding - center_offset_x).max(0.0);
        let local_y = (y - by - pane_padding - title_offset - center_offset_y).max(0.0);

        let col = (local_x / cell_width) as usize;
        let row = (local_y / cell_height) as usize;

        Some((col, row))
    }

    /// Convert pixel coordinates to cell coordinates for selection purposes.
    ///
    /// In split-pane mode, returns pane-relative coordinates for the focused pane.
    /// In single-pane mode, returns global terminal coordinates (same as `pixel_to_cell`).
    /// Returns `None` if the point is outside the active pane's bounds (split mode)
    /// or if no renderer is available.
    pub(crate) fn pixel_to_selection_cell(&self, x: f64, y: f64) -> Option<(usize, usize)> {
        if let Some(tab) = self.tab_manager.active_tab()
            && let Some(ref pm) = tab.pane_manager
            && let Some(focused_pane) = pm.focused_pane()
        {
            self.pixel_to_pane_cell(x, y, &focused_pane.bounds)
        } else {
            self.pixel_to_cell(x, y)
        }
    }

    /// Handle a file being dropped into the terminal window.
    ///
    /// Quotes the file path according to the configured style and writes it
    /// to the terminal session under the drop position. In split-pane and tmux
    /// modes the pane under the cursor is focused first so the text lands in
    /// the correct pane.
    pub(crate) fn handle_dropped_file(&mut self, path: std::path::PathBuf) {
        use crate::shell_quote::quote_path;

        // Quote the path according to the configured style
        let quoted_path = quote_path(&path, self.config.load().dropped_file_quote_style);

        log::debug!(
            "File dropped: {:?} -> {} (style: {:?})",
            path,
            quoted_path,
            self.config.load().dropped_file_quote_style
        );

        // Use the last known cursor position to focus the pane under the drop.
        // winit keeps CursorMoved firing during the drag, so position is current.
        let drop_pos = self
            .tab_manager
            .active_tab()
            .map(|tab| tab.active_mouse().position);

        if let Some((mx, my)) = drop_pos
            && let Some(tab) = self.tab_manager.active_tab_mut()
            && tab.has_multiple_panes()
            && let Some(pane_id) = tab.focus_pane_at(mx as f32, my as f32)
        {
            log::debug!("File drop focused pane {} at ({}, {})", pane_id, mx, my);
            // Update tmux focused pane so send-keys targets it
            self.set_tmux_focused_pane_from_native(pane_id);
            self.focus_state.needs_redraw = true;
        }

        // In tmux gateway mode, route through send-keys so the text reaches
        // the (now-focused) tmux pane instead of the gateway PTY.
        if self.is_tmux_connected() && self.paste_via_tmux(&quoted_path) {
            self.request_redraw();
            return;
        }

        // Native mode: write to the focused pane's terminal (or the tab's
        // primary terminal for single-pane tabs).
        if let Some(tab) = self.tab_manager.active_tab() {
            let terminal_clone = if tab.has_multiple_panes() {
                // Use the focused pane's terminal
                tab.pane_manager()
                    .and_then(|pm| pm.focused_pane_id())
                    .and_then(|id| tab.pane_manager().and_then(|pm| pm.get_pane(id)))
                    .map(|pane| Arc::clone(&pane.terminal))
            } else {
                None
            }
            .unwrap_or_else(|| Arc::clone(&tab.terminal));

            let runtime = Arc::clone(&self.runtime);

            runtime.spawn(async move {
                let term = terminal_clone.write().await;
                let bytes = quoted_path.as_bytes().to_vec();
                if let Err(e) = term.write(&bytes) {
                    log::error!("Failed to write dropped file path to terminal: {}", e);
                }
            });

            // Request redraw in case terminal needs to update
            self.request_redraw();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PaneBoundsRaw, pixel_to_cell_raw, pixel_to_pane_cell_raw};

    // ── pixel_to_cell_raw ─────────────────────────────────────────────────

    #[test]
    fn test_pixel_to_cell_origin_no_offset() {
        // (0,0) with no padding or offsets → cell (0,0)
        let (col, row) = pixel_to_cell_raw(0.0, 0.0, 8.0, 16.0, 0.0, 0.0, 0.0);
        assert_eq!(col, 0);
        assert_eq!(row, 0);
    }

    #[test]
    fn test_pixel_to_cell_exact_cell_boundary() {
        // Exactly one cell width & height from origin → cell (1, 1)
        let (col, row) = pixel_to_cell_raw(8.0, 16.0, 8.0, 16.0, 0.0, 0.0, 0.0);
        assert_eq!(col, 1);
        assert_eq!(row, 1);
    }

    #[test]
    fn test_pixel_to_cell_with_padding() {
        // Padding of 4px; pixel (12, 20) → adjusted (8, 16) → cell (1, 1)
        let (col, row) = pixel_to_cell_raw(12.0, 20.0, 8.0, 16.0, 4.0, 0.0, 0.0);
        assert_eq!(col, 1);
        assert_eq!(row, 1);
    }

    #[test]
    fn test_pixel_to_cell_with_content_offsets() {
        // content_offset_x = 20 (e.g. side panel), content_offset_y = 30 (tab bar)
        // pixel (28, 46) → adjusted (8, 16) → cell (1, 1)
        let (col, row) = pixel_to_cell_raw(28.0, 46.0, 8.0, 16.0, 0.0, 20.0, 30.0);
        assert_eq!(col, 1);
        assert_eq!(row, 1);
    }

    #[test]
    fn test_pixel_to_cell_clamped_to_zero() {
        // Pixel inside padding/offset region → adjusted negative → clamped to 0
        let (col, row) = pixel_to_cell_raw(1.0, 1.0, 8.0, 16.0, 4.0, 0.0, 0.0);
        assert_eq!(col, 0);
        assert_eq!(row, 0);
    }

    #[test]
    fn test_pixel_to_cell_large_grid() {
        // 1920 px wide / 8 px cells = column 240
        let (col, _) = pixel_to_cell_raw(1920.0, 0.0, 8.0, 16.0, 0.0, 0.0, 0.0);
        assert_eq!(col, 240);
    }

    // ── pixel_to_pane_cell_raw ────────────────────────────────────────────

    #[test]
    fn test_pane_cell_inside_bounds_no_padding() {
        // Pane at (100, 200), 400x300, cell 8×16, no padding or title.
        // content_h=300 → rows_fit=18 (18×16=288), center_offset_y=floor(12/2)=6.
        // content_w=400 → cols_fit=50 (50×8=400), center_offset_x=0.
        // Point (140, 248): local_x=140-100-0=40 → col=5; local_y=248-200-6=42 → row=2
        let result = pixel_to_pane_cell_raw(
            140.0,
            248.0,
            PaneBoundsRaw {
                bx: 100.0,
                by: 200.0,
                bw: 400.0,
                bh: 300.0,
                cell_width: 8.0,
                cell_height: 16.0,
                pane_padding: 0.0,
                title_offset: 0.0,
            },
        );
        assert_eq!(result, Some((5, 2)));
    }

    #[test]
    fn test_pane_cell_outside_left_edge() {
        let result = pixel_to_pane_cell_raw(
            99.9,
            250.0,
            PaneBoundsRaw {
                bx: 100.0,
                by: 200.0,
                bw: 400.0,
                bh: 300.0,
                cell_width: 8.0,
                cell_height: 16.0,
                pane_padding: 0.0,
                title_offset: 0.0,
            },
        );
        assert_eq!(result, None);
    }

    #[test]
    fn test_pane_cell_outside_right_edge() {
        // x == bx + bw is exclusive
        let result = pixel_to_pane_cell_raw(
            500.0,
            250.0,
            PaneBoundsRaw {
                bx: 100.0,
                by: 200.0,
                bw: 400.0,
                bh: 300.0,
                cell_width: 8.0,
                cell_height: 16.0,
                pane_padding: 0.0,
                title_offset: 0.0,
            },
        );
        assert_eq!(result, None);
    }

    #[test]
    fn test_pane_cell_outside_top_edge() {
        let result = pixel_to_pane_cell_raw(
            150.0,
            199.9,
            PaneBoundsRaw {
                bx: 100.0,
                by: 200.0,
                bw: 400.0,
                bh: 300.0,
                cell_width: 8.0,
                cell_height: 16.0,
                pane_padding: 0.0,
                title_offset: 0.0,
            },
        );
        assert_eq!(result, None);
    }

    #[test]
    fn test_pane_cell_with_pane_padding() {
        // Pane at (0,0), 200x200. Pane padding 8px.
        // Point (16, 32) → local_x = 16-8 = 8, local_y = 32-8 = 24 → cell (1, 1)
        let result = pixel_to_pane_cell_raw(
            16.0,
            32.0,
            PaneBoundsRaw {
                bx: 0.0,
                by: 0.0,
                bw: 200.0,
                bh: 200.0,
                cell_width: 8.0,
                cell_height: 16.0,
                pane_padding: 8.0,
                title_offset: 0.0,
            },
        );
        assert_eq!(result, Some((1, 1)));
    }

    #[test]
    fn test_pane_cell_with_title_offset() {
        // Pane 200×200, title_offset=20, cell 8×16, no padding.
        // viewport_h=180 → rows_fit=11 (11×16=176), center_offset_y=floor(4/2)=2.
        // Point (8, 52): local_x=8 → col=1; local_y=52-20-2=30 → row=1
        let result = pixel_to_pane_cell_raw(
            8.0,
            52.0,
            PaneBoundsRaw {
                bx: 0.0,
                by: 0.0,
                bw: 200.0,
                bh: 200.0,
                cell_width: 8.0,
                cell_height: 16.0,
                pane_padding: 0.0,
                title_offset: 20.0,
            },
        );
        assert_eq!(result, Some((1, 1)));
    }

    #[test]
    fn test_pane_cell_padding_clamps_to_zero() {
        // Point inside bounds but inside padding region → local coords negative → clamped to 0
        let result = pixel_to_pane_cell_raw(
            5.0,
            5.0,
            PaneBoundsRaw {
                bx: 0.0,
                by: 0.0,
                bw: 200.0,
                bh: 200.0,
                cell_width: 8.0,
                cell_height: 16.0,
                pane_padding: 8.0,
                title_offset: 0.0,
            },
        );
        assert_eq!(result, Some((0, 0)));
    }

    #[test]
    fn test_pane_cell_centering_offset_row_alignment() {
        // Regression test: the bottom fraction of each displayed cell must map to the
        // SAME row, not the next one.  Without the center_offset correction this failed
        // for pane heights that leave a non-zero remainder after dividing by cell_height.
        //
        // bh=300, cell_height=20 → rows_fit=15 (15×20=300), center_offset_y=0.
        // Clicking at the very bottom of row 0 (y=by+19) must still yield row 0.
        let result = pixel_to_pane_cell_raw(
            0.0,
            19.0,
            PaneBoundsRaw {
                bx: 0.0,
                by: 0.0,
                bw: 200.0,
                bh: 300.0,
                cell_width: 8.0,
                cell_height: 20.0,
                pane_padding: 0.0,
                title_offset: 0.0,
            },
        );
        assert_eq!(result, Some((0, 0)));

        // bh=308, cell_height=20 → rows_fit=15 (15×20=300), center_offset_y=floor(8/2)=4.
        // Rendered row 0 starts at y=4.  Clicking at y=3 (inside the centering gap)
        // clamps to row 0.  Clicking at the last pixel of row 0 (y=4+19=23) → row 0.
        // Clicking at the first pixel of row 1 (y=4+20=24) → row 1.
        let pane = PaneBoundsRaw {
            bx: 0.0,
            by: 0.0,
            bw: 200.0,
            bh: 308.0,
            cell_width: 8.0,
            cell_height: 20.0,
            pane_padding: 0.0,
            title_offset: 0.0,
        };
        // Last pixel of row 0
        assert_eq!(pixel_to_pane_cell_raw(0.0, 23.0, pane), Some((0, 0)));
        // First pixel of row 1
        assert_eq!(pixel_to_pane_cell_raw(0.0, 24.0, pane), Some((0, 1)));
    }
}
