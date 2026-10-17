//! OSC 22 pointer-shape application (card 01a1476e).
//!
//! The core parses OSC 22 as the pointer shape (kitty/ghostty/iTerm2
//! meaning) and exposes it as [`par_term_emu_core_rust::terminal::Terminal::pointer_shape`]
//! (actually `title_badge` module). This module polls the focused pane's
//! terminal once per event-loop iteration (mirroring `check_bell`) and
//! applies the requested shape to the winit cursor while it is active;
//! an empty/absent shape resets to the app's default (`Text`).

use super::WindowState;

/// Map a requested shape name to a winit cursor icon.
///
/// CSS names (`text`, `ew-resize`, …) parse through winit's re-exported
/// `cursor-icon` crate directly. The X11 cursor-font synonyms the mux
/// client emits alongside the CSS name (`sb_h_double_arrow` /
/// `sb_v_double_arrow` and close relatives) are matched explicitly —
/// winit's `FromStr` knows only the kebab-case CSS vocabulary. An
/// unmappable shape yields `None` and leaves the current cursor alone.
pub(crate) fn cursor_icon_for_shape(shape: &str) -> Option<winit::window::CursorIcon> {
    if let Ok(icon) = shape.parse() {
        return Some(icon);
    }
    match shape {
        "sb_h_double_arrow" | "h_double_arrow" | "size_hor" | "split_h" => {
            Some(winit::window::CursorIcon::EwResize)
        }
        "sb_v_double_arrow" | "v_double_arrow" | "size_ver" | "split_v" => {
            Some(winit::window::CursorIcon::NsResize)
        }
        _ => None,
    }
}

impl WindowState {
    /// Apply the focused terminal's OSC 22 pointer-shape request to the
    /// window cursor (card 01a1476e).
    ///
    /// While a shape is active it is re-asserted every iteration: the
    /// app's own event-driven cursor writes (URL hover `Pointer`,
    /// divider `ColResize`, plain `Text` in `mouse_move.rs`) would
    /// otherwise clobber the requested shape between mouse events. The
    /// egui overlay still wins when it requests a cursor the same frame
    /// (it applies after `about_to_wait`), which is the right precedence
    /// for tab drags. On the reset edge (empty/absent shape → `None`)
    /// the default `Text` cursor is restored once and the app's normal
    /// hover logic takes over again.
    pub(crate) fn check_pointer_shape(&mut self) {
        let requested: Option<String> = {
            let Some(tab) = self.tab_manager.active_tab() else {
                return;
            };
            let terminal = tab
                .pane_manager
                .as_ref()
                .and_then(|pm| pm.focused_pane())
                .map(|pane| std::sync::Arc::clone(&pane.terminal))
                .unwrap_or_else(|| std::sync::Arc::clone(&tab.terminal));
            // try_read: per-frame polling in about_to_wait; a missed lock
            // skips one frame, matching check_bell's tolerance.
            let Ok(term) = terminal.try_read() else {
                return;
            };
            term.pointer_shape().map(|s| s.to_string())
        };

        let changed = requested != self.osc22_pointer_shape;
        if let Some(shape) = requested.as_deref()
            && let Some(icon) = cursor_icon_for_shape(shape)
            && let Some(window) = &self.window
        {
            window.set_cursor(icon);
        }
        if changed && requested.is_none() {
            // Reset edge: restore the app default immediately so the
            // requested icon cannot stick when no mouse event follows.
            if let Some(window) = &self.window {
                window.set_cursor(winit::window::CursorIcon::Text);
            }
        }
        self.osc22_pointer_shape = requested;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_names_parse_directly() {
        assert_eq!(
            cursor_icon_for_shape("ew-resize"),
            Some(winit::window::CursorIcon::EwResize)
        );
        assert_eq!(
            cursor_icon_for_shape("ns-resize"),
            Some(winit::window::CursorIcon::NsResize)
        );
        assert_eq!(
            cursor_icon_for_shape("text"),
            Some(winit::window::CursorIcon::Text)
        );
    }

    #[test]
    fn mux_x11_synonyms_map() {
        assert_eq!(
            cursor_icon_for_shape("sb_h_double_arrow"),
            Some(winit::window::CursorIcon::EwResize)
        );
        assert_eq!(
            cursor_icon_for_shape("sb_v_double_arrow"),
            Some(winit::window::CursorIcon::NsResize)
        );
    }

    #[test]
    fn unknown_shape_is_unmapped() {
        assert_eq!(cursor_icon_for_shape("definitely-not-a-cursor"), None);
    }
}
