//! tmux notification handling and gateway session management.
//!
//! ## Sub-modules
//!
//! - `notifications`: `check_tmux_notifications` and all notification event handlers
//!   (session-changed, window-add/close/rename, layout-change, output, pane-focus,
//!   session-ended, pause/continue, sync-action dispatch).
//! - `gateway`: Gateway session lifecycle (initiate, attach, disconnect), input routing
//!   (send_input_via_tmux, paste_via_tmux, prefix key), pane operations
//!   (split/close), and clipboard/resize sync.
//! - `gateway_profile`: Profile auto-application on tmux session connect.

mod gateway;
mod gateway_input;
mod gateway_profile;
mod notifications;
mod pane_write;
pub(crate) mod tmux_state;

/// Columns reserved at the window's right edge for the scrollbar in a
/// par-mux tab. The daemon's layout knows nothing about par-term's
/// scrollbar, so the width pushed to it excludes this strip and the native
/// pane area is shrunk to match; otherwise the right-most pane's last
/// columns draw under the scrollbar.
pub(crate) fn mux_scrollbar_reserved_cols(scrollbar_width: f32, cell_width: f32) -> usize {
    if scrollbar_width <= 0.0 || cell_width <= 0.0 {
        return 0;
    }
    (scrollbar_width / cell_width).ceil() as usize
}

#[cfg(test)]
mod tests {
    use super::mux_scrollbar_reserved_cols;

    /// The strip reserved for the scrollbar is whole cells, rounded UP: a
    /// partial cell under the scrollbar would still hide that column.
    #[test]
    fn scrollbar_strip_rounds_up_to_whole_cells() {
        assert_eq!(mux_scrollbar_reserved_cols(15.0, 8.0), 2);
        assert_eq!(mux_scrollbar_reserved_cols(16.0, 8.0), 2);
        assert_eq!(mux_scrollbar_reserved_cols(17.0, 8.0), 3);
        assert_eq!(
            mux_scrollbar_reserved_cols(0.0, 8.0),
            0,
            "no scrollbar, no strip"
        );
        assert_eq!(
            mux_scrollbar_reserved_cols(15.0, 0.0),
            0,
            "degenerate cell width"
        );
    }
}
