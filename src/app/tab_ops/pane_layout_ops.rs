//! Pane layout actions: arrow resize (PN3), equalize (A3), layout presets
//! (A4), and split left / up (A5), for local and par-mux tabs.
//!
//! Local tabs change their own tree. Attached tabs send the daemon command
//! and let its `%layout-change` drive the mirror, the pattern split, swap
//! and close already use: a local edit would be overwritten by the next
//! layout push, and a second client would never see it (M7).

use crate::app::window_state::WindowState;
use crate::pane::{LayoutPreset, NavigationDirection, SplitDirection};

/// Fraction of the enclosing split an arrow resize moves the divider by.
const RESIZE_STEP: f32 = 0.05;

impl WindowState {
    /// Move the divider nearest the focused pane in the arrow's direction
    /// (PN3), never past `pane_min_size` (PN4).
    ///
    /// Attached tabs run the same resize on the mirror and send the result
    /// as absolute pane sizes (the divider-drag path), which the daemon
    /// echoes back as `%layout-change` (M7). The daemon's relative
    /// `resize-pane -L/-R` grows the TARGET pane whichever side of the
    /// divider it sits on and reaches only its direct parent split, so it
    /// cannot express "move the divider in the arrow's direction" for a
    /// right-hand pane or a nested layout.
    pub fn resize_pane(&mut self, direction: NavigationDirection) {
        let moved = self
            .tab_manager
            .active_tab_mut()
            .and_then(|tab| tab.pane_manager_mut())
            .and_then(|pm| {
                let focused = pm.focused_pane_id()?;
                Some(pm.resize_toward(focused, direction, RESIZE_STEP))
            });
        if moved == Some(true) {
            let horizontal_divider = matches!(
                direction,
                NavigationDirection::Up | NavigationDirection::Down
            );
            self.sync_pane_resize_to_tmux(horizontal_divider);
            self.after_pane_layout_change();
        }
    }

    /// Give every pane in the tab an equal area (A3).
    pub(crate) fn equalize_panes(&mut self) {
        if self.mux_relayout(LayoutPreset::EvenHorizontal, true) {
            return;
        }
        if self.refuse_in_tmux_gateway("Equalize") {
            return;
        }
        let kept = self
            .tab_manager
            .active_tab_mut()
            .and_then(|tab| tab.pane_manager_mut())
            .map(|pm| pm.equalize());
        if kept == Some(false) {
            self.show_toast("Equalize refused: a pane would be below the minimum size");
        }
        self.after_pane_layout_change();
    }

    /// Equalize the split a double-clicked divider belongs to (PN8).
    pub(crate) fn equalize_divider(&mut self, divider_index: usize) {
        if let Some(pm) = self
            .tab_manager
            .active_tab_mut()
            .and_then(|tab| tab.pane_manager_mut())
        {
            pm.equalize_divider(divider_index);
        }
        // The divider-drag sync: a no-op for local tabs, absolute sizes to
        // the tmux gateway or par-mux daemon otherwise.
        let horizontal = self
            .tab_manager
            .active_tab()
            .and_then(|t| t.get_divider(divider_index))
            .map(|d| d.is_horizontal);
        if let Some(horizontal) = horizontal {
            self.sync_pane_resize_to_tmux(horizontal);
        }
        self.after_pane_layout_change();
    }

    /// Rearrange the tab's panes into a preset (A4).
    pub(crate) fn apply_layout_preset(&mut self, preset: LayoutPreset) {
        if self.mux_relayout(preset, false) {
            return;
        }
        if self.refuse_in_tmux_gateway("Layout presets") {
            return;
        }
        let kept = self
            .tab_manager
            .active_tab_mut()
            .and_then(|tab| tab.pane_manager_mut())
            .map(|pm| pm.apply_preset(preset));
        if kept == Some(false) {
            self.show_toast(format!(
                "Layout {} refused: a pane would be below the minimum size",
                preset.name()
            ));
        }
        self.after_pane_layout_change();
    }

    /// Step to the next preset in [`LayoutPreset::ALL`] (A4 `cycle_layout`).
    pub(crate) fn cycle_layout_preset(&mut self) {
        let current = self
            .tab_manager
            .active_tab()
            .and_then(|t| t.pane_manager())
            .and_then(|pm| pm.layout_preset());
        let next = match current {
            Some(p) => {
                let i = LayoutPreset::ALL.iter().position(|q| *q == p).unwrap_or(0);
                LayoutPreset::ALL[(i + 1) % LayoutPreset::ALL.len()]
            }
            None => LayoutPreset::ALL[0],
        };
        self.apply_layout_preset(next);
    }

    /// Split with the new pane left of (`Vertical`) or above
    /// (`Horizontal`) the focused one (A5).
    pub(crate) fn split_pane_before(&mut self, direction: SplitDirection) {
        #[cfg(feature = "mux")]
        if self.split_pane_via_mux_placed(direction == SplitDirection::Vertical, true) {
            return;
        }
        if self.refuse_in_tmux_gateway("Split left/up") {
            return;
        }
        self.split_pane_placed(direction, true, true, None, 50);
    }

    /// Attached tabs: the daemon has no `select-layout`, so a preset or
    /// equalize is expressed as absolute pane sizes computed from the local
    /// mirror, sent with `resize-pane -x/-y`. The structural presets
    /// (`main-*`, `tiled`, a different chain axis) would need panes moved
    /// daemon-side; those are refused with a toast. Returns whether the
    /// request was consumed (an attached tab).
    fn mux_relayout(&mut self, preset: LayoutPreset, equalize_only: bool) -> bool {
        #[cfg(feature = "mux")]
        if self.focused_mux_pane_from_native().is_some() {
            if !equalize_only {
                self.show_toast(format!(
                    "Layout {}: not available in par-mux tabs yet (equalize is)",
                    preset.name()
                ));
                return true;
            }
            let equalized = self
                .tab_manager
                .active_tab_mut()
                .and_then(|tab| tab.pane_manager_mut())
                .is_some_and(|pm| pm.equalize());
            if equalized {
                // Both axes: equalize can change widths and heights.
                self.sync_pane_resize_to_tmux(false);
                self.sync_pane_resize_to_tmux(true);
            }
            self.after_pane_layout_change();
            return true;
        }
        let _ = (preset, equalize_only);
        false
    }
}
