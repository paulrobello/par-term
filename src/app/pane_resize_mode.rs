//! Keyboard resize mode (UX.md A6, action `enter_resize_mode`).
//!
//! While armed, the mode owns the key stream (mode-stack contract, the same
//! slot as pane-hint selection): arrows move the focused pane's divider by
//! `pane_resize_step` percent, Shift+arrows by one cell, and Escape or Enter
//! exits. Other keys are swallowed so nothing leaks to the shell. The mode
//! ends when its tab stops being active or no longer has two panes.

use par_term_config::TabId;

use super::window_state::WindowState;
use crate::pane::{NavigationDirection, ResizeStep};

/// Resize mode state for one window.
#[derive(Default)]
pub(crate) enum PaneResizeModeState {
    #[default]
    Idle,
    /// Armed on this tab; leaving it ends the mode.
    Resizing { tab_id: TabId },
}

impl PaneResizeModeState {
    pub(crate) fn is_active(&self) -> bool {
        matches!(self, Self::Resizing { .. })
    }
}

/// A key as resize mode sees it. winit `KeyEvent` cannot be built in
/// tests, so the key handler and the `--ui-test` injector translate into
/// this first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResizeModeKey {
    Arrow {
        direction: NavigationDirection,
        fine: bool,
    },
    Exit,
    Other,
}

impl ResizeModeKey {
    /// Translate a logical key and the Shift state.
    pub(crate) fn from_key(key: &winit::keyboard::Key, shift: bool) -> Self {
        use winit::keyboard::{Key, NamedKey};
        let direction = match key {
            Key::Named(NamedKey::ArrowLeft) => NavigationDirection::Left,
            Key::Named(NamedKey::ArrowRight) => NavigationDirection::Right,
            Key::Named(NamedKey::ArrowUp) => NavigationDirection::Up,
            Key::Named(NamedKey::ArrowDown) => NavigationDirection::Down,
            Key::Named(NamedKey::Escape | NamedKey::Enter) => return Self::Exit,
            _ => return Self::Other,
        };
        Self::Arrow {
            direction,
            fine: shift,
        }
    }
}

/// The status line shown while the mode is armed.
const RESIZE_MODE_HINT: &str =
    "Resize mode: arrows move the divider, Shift+arrows by one cell, Esc or Enter to finish";

impl WindowState {
    /// Arm resize mode on the active tab (A6). Does nothing without two
    /// panes, or in a tmux gateway tab whose layout tmux owns.
    pub(crate) fn enter_pane_resize_mode(&mut self) {
        let Some(tab) = self.tab_manager.active_tab() else {
            return;
        };
        if !tab.has_multiple_panes() {
            self.show_toast("Resize mode: the tab has only one pane");
            return;
        }
        let tab_id = tab.id;
        if self.refuse_in_tmux_gateway("Resize mode") {
            return;
        }
        self.pane_resize_mode = PaneResizeModeState::Resizing { tab_id };
        // A mode banner, not a toast (UX.md OV8).
        self.overlay_state.mode_banner = Some(RESIZE_MODE_HINT);
        self.focus_state.needs_redraw = true;
        self.request_redraw();
    }

    /// Leave resize mode and clear its status line.
    pub(crate) fn exit_pane_resize_mode(&mut self) {
        if !self.pane_resize_mode.is_active() {
            return;
        }
        self.pane_resize_mode = PaneResizeModeState::Idle;
        if self.overlay_state.mode_banner == Some(RESIZE_MODE_HINT) {
            self.overlay_state.mode_banner = None;
        }
        self.focus_state.needs_redraw = true;
        self.request_redraw();
    }

    /// Handle one key while the mode is armed. Every key is consumed.
    /// `cell` is the (width, height) of a cell in physical pixels, the
    /// Shift+arrow step.
    pub(crate) fn resolve_pane_resize_key(&mut self, key: ResizeModeKey, cell: (f32, f32)) {
        match key {
            ResizeModeKey::Exit => self.exit_pane_resize_mode(),
            ResizeModeKey::Other => {}
            ResizeModeKey::Arrow { direction, fine } => {
                if fine {
                    let px = match direction {
                        NavigationDirection::Left | NavigationDirection::Right => cell.0,
                        NavigationDirection::Up | NavigationDirection::Down => cell.1,
                    };
                    self.resize_pane_by(direction, ResizeStep::Pixels(px));
                } else {
                    self.resize_pane(direction);
                }
            }
        }
    }

    /// Key-event entry: translate and resolve, using the renderer's cell
    /// size. Called from `handle_key_event` while the mode is armed.
    pub(crate) fn handle_pane_resize_mode_key(&mut self, event: &winit::event::KeyEvent) {
        if event.state != winit::event::ElementState::Pressed {
            return;
        }
        let shift = self.input_handler.modifiers.state().shift_key();
        let key = ResizeModeKey::from_key(&event.logical_key, shift);
        let cell = self.resize_mode_cell_size();
        self.resolve_pane_resize_key(key, cell);
    }

    /// The renderer's cell size, or a nominal one before it exists.
    pub(crate) fn resize_mode_cell_size(&self) -> (f32, f32) {
        self.renderer
            .as_ref()
            .map(|r| (r.cell_width(), r.cell_height()))
            .unwrap_or((8.0, 16.0))
    }

    /// End the mode when its tab is no longer active or lost its split —
    /// called from the about-to-wait sweep, beside the pane-hint sweep.
    pub(crate) fn cancel_pane_resize_mode_if_stale(&mut self) {
        let PaneResizeModeState::Resizing { tab_id } = self.pane_resize_mode else {
            return;
        };
        let still_valid = self.tab_manager.active_tab_id() == Some(tab_id)
            && self
                .tab_manager
                .get_tab(tab_id)
                .is_some_and(|t| t.has_multiple_panes());
        if !still_valid {
            self.exit_pane_resize_mode();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::pane::{Pane, PaneBounds, PaneNode, SplitDirection};
    use crate::tab::Tab;
    use par_term_terminal::TerminalManager;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use tokio::sync::RwLock;
    use winit::keyboard::{Key, NamedKey};

    fn stub(id: crate::pane::PaneId) -> Pane {
        Pane::new_wrapping_terminal(
            id,
            Arc::new(RwLock::new(
                TerminalManager::new_with_scrollback(20, 5, 0).expect("terminal"),
            )),
            None,
            Arc::new(AtomicBool::new(false)),
        )
    }

    /// Panes 1 | 2 side by side over 1000x400, focused on 1.
    fn window(config: Config) -> WindowState {
        let runtime = Arc::new(
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("runtime"),
        );
        let mut ws = WindowState::new(config, runtime);
        let mut tab = Tab::new_stub(1, 1);
        let pm = tab.pane_manager_mut().expect("pm");
        pm.set_root(PaneNode::split(
            SplitDirection::Vertical,
            0.5,
            PaneNode::leaf(stub(1)),
            PaneNode::leaf(stub(2)),
        ));
        pm.focus_pane(1);
        pm.set_bounds(PaneBounds::new(0.0, 0.0, 1000.0, 400.0));
        ws.tab_manager.push_tab_for_test(tab);
        ws
    }

    fn left_width(ws: &WindowState) -> f32 {
        ws.tab_manager
            .active_tab()
            .and_then(|t| t.pane_manager())
            .and_then(|pm| pm.get_pane(1))
            .map(|p| p.bounds.width)
            .expect("pane 1")
    }

    fn arrow(direction: NavigationDirection, fine: bool) -> ResizeModeKey {
        ResizeModeKey::Arrow { direction, fine }
    }

    #[test]
    fn keys_translate_to_resize_mode_keys() {
        assert_eq!(
            ResizeModeKey::from_key(&Key::Named(NamedKey::ArrowRight), false),
            arrow(NavigationDirection::Right, false)
        );
        assert_eq!(
            ResizeModeKey::from_key(&Key::Named(NamedKey::ArrowUp), true),
            arrow(NavigationDirection::Up, true)
        );
        assert_eq!(
            ResizeModeKey::from_key(&Key::Named(NamedKey::Escape), false),
            ResizeModeKey::Exit
        );
        assert_eq!(
            ResizeModeKey::from_key(&Key::Named(NamedKey::Enter), false),
            ResizeModeKey::Exit
        );
        assert_eq!(
            ResizeModeKey::from_key(&Key::Character("x".into()), false),
            ResizeModeKey::Other
        );
    }

    /// A6: arrows move the divider by `pane_resize_step` percent, Shift+
    /// arrows by one cell, other keys do nothing, and Escape exits.
    #[test]
    fn arrows_step_by_the_configured_percent_and_shift_by_one_cell() {
        let mut config = Config::default();
        config.panes.pane_resize_step = 10.0;
        let mut ws = window(config);
        ws.enter_pane_resize_mode();
        assert!(ws.pane_resize_mode.is_active());
        assert_eq!(
            ws.overlay_state.mode_banner,
            Some(RESIZE_MODE_HINT),
            "the hint is a mode banner (OV8)"
        );
        assert!(
            ws.overlay_state.toasts.toasts().is_empty(),
            "the hint takes no toast slot"
        );

        let before = left_width(&ws);
        ws.resolve_pane_resize_key(arrow(NavigationDirection::Right, false), (8.0, 16.0));
        let after_step = left_width(&ws);
        let available = 1000.0 - 2.0; // total minus one default divider
        assert!(
            (after_step - before - available * 0.10).abs() < 1.0,
            "10% of the split: {before} -> {after_step}"
        );

        ws.resolve_pane_resize_key(arrow(NavigationDirection::Left, true), (8.0, 16.0));
        assert!(
            (after_step - left_width(&ws) - 8.0).abs() < 0.5,
            "one cell: {after_step} -> {}",
            left_width(&ws)
        );

        let steady = left_width(&ws);
        ws.resolve_pane_resize_key(ResizeModeKey::Other, (8.0, 16.0));
        assert_eq!(left_width(&ws), steady, "other keys are swallowed");
        assert!(ws.pane_resize_mode.is_active());

        ws.resolve_pane_resize_key(ResizeModeKey::Exit, (8.0, 16.0));
        assert!(!ws.pane_resize_mode.is_active());
        assert_eq!(ws.overlay_state.mode_banner, None, "the hint clears");
    }

    #[test]
    fn a_single_pane_tab_does_not_arm_and_leaving_the_tab_cancels() {
        let mut ws = window(Config::default());
        ws.enter_pane_resize_mode();
        ws.tab_manager.push_tab_for_test(Tab::new_stub(2, 2));
        ws.tab_manager.switch_to(2);
        ws.cancel_pane_resize_mode_if_stale();
        assert!(!ws.pane_resize_mode.is_active(), "switching tabs cancels");

        ws.enter_pane_resize_mode();
        assert!(
            !ws.pane_resize_mode.is_active(),
            "one pane: nothing to resize"
        );
    }
}
