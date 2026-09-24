//! Built-in pane-hint selection mode (tmux `display-panes` style).
//!
//! Invoking the `select_pane_hint` keybinding arms the mode: a letter badge
//! (letter on a circle background) is drawn centered in every pane of the
//! focused tab; typing a pane's letter focuses that pane and exits the mode;
//! Escape or any non-matching key cancels without changing focus.
//!
//! Mode-stack contract (docs/plans/2026-09-24-overlay-plugin-design.md):
//! badges render above every plugin overlay and, while active, the mode
//! captures keys regardless of overlay focus. It is built into par-term, not
//! a plugin.

use par_term_config::TabId;

use super::window_state::WindowState;
use crate::pane::PaneId;

/// Home-row letter pool for pane badges. Assigned in pane-tree order, so the
/// same layout always yields the same letters.
pub(crate) const PANE_HINT_LETTERS: &[char] = &[
    'a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l', 'q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p',
    'z', 'x', 'c', 'v', 'b', 'n', 'm',
];

#[derive(Default)]
pub(crate) enum PaneHintSelectState {
    #[default]
    Idle,
    Selecting {
        /// Tab whose panes the badges describe; leaving that tab cancels.
        tab_id: TabId,
        /// Letter → pane, in tree order at arm time.
        assignments: Vec<(char, PaneId)>,
    },
}

impl PaneHintSelectState {
    pub(crate) fn is_active(&self) -> bool {
        matches!(self, Self::Selecting { .. })
    }
}

impl WindowState {
    /// Arm the pane-hint selection mode for the focused tab. Does nothing for
    /// tabs without a multi-pane layout — there is nothing to choose.
    pub fn enter_pane_hint_select(&mut self) {
        let Some(tab_id) = self.tab_manager.active_tab_id() else {
            return;
        };
        let Some(tab) = self.tab_manager.get_tab(tab_id) else {
            return;
        };
        if !tab.has_multiple_panes() {
            return;
        }
        let Some(pm) = tab.pane_manager() else {
            return;
        };
        let assignments: Vec<(char, PaneId)> = pm
            .all_panes()
            .iter()
            .zip(PANE_HINT_LETTERS.iter())
            .map(|(pane, &letter)| (letter, pane.id))
            .collect();
        if assignments.len() < 2 {
            return;
        }
        self.pane_hint_select = PaneHintSelectState::Selecting {
            tab_id,
            assignments,
        };
        self.focus_state.needs_redraw = true;
        self.request_redraw();
    }

    fn exit_pane_hint_select(&mut self) {
        if self.pane_hint_select.is_active() {
            self.pane_hint_select = PaneHintSelectState::Idle;
            self.focus_state.needs_redraw = true;
            self.request_redraw();
        }
    }

    /// Handle a key press while the mode is armed. Returns true if the key
    /// was consumed. Any press resolves the mode: a matching letter focuses
    /// its pane, anything else (including Escape) cancels.
    pub(crate) fn handle_pane_hint_select_key(&mut self, event: &winit::event::KeyEvent) -> bool {
        if event.state != winit::event::ElementState::Pressed {
            return false;
        }
        if let winit::keyboard::Key::Character(ch) = &event.logical_key {
            self.resolve_pane_hint_select(ch.chars().next())
        } else {
            self.resolve_pane_hint_select(None)
        }
    }

    /// Resolve the armed mode against a typed character (lowercase matched;
    /// `None` = non-character key such as Escape — always a cancel).
    fn resolve_pane_hint_select(&mut self, typed: Option<char>) -> bool {
        let PaneHintSelectState::Selecting {
            tab_id,
            assignments,
        } = std::mem::replace(&mut self.pane_hint_select, PaneHintSelectState::Idle)
        else {
            return false;
        };

        if let Some(c) = typed.map(|c| c.to_ascii_lowercase())
            && let Some(&(_, pane_id)) = assignments.iter().find(|(letter, _)| *letter == c)
            && self.tab_manager.active_tab_id() == Some(tab_id)
            && let Some(tab) = self.tab_manager.get_tab_mut(tab_id)
            && let Some(pm) = tab.pane_manager_mut()
        {
            pm.focus_pane(pane_id);
        }
        // Matching letter or not, the mode ends here; redraw either way.
        self.focus_state.needs_redraw = true;
        self.request_redraw();
        true
    }

    /// Cancel the mode when its tab is no longer the active one (switch or
    /// close) — called from the tab-change path.
    pub(crate) fn cancel_pane_hint_select_if_stale(&mut self) {
        let stale = match &self.pane_hint_select {
            PaneHintSelectState::Idle => false,
            PaneHintSelectState::Selecting { tab_id, .. } => {
                self.tab_manager.active_tab_id() != Some(*tab_id)
            }
        };
        if stale {
            self.exit_pane_hint_select();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::pane::{Pane, PaneNode, SplitDirection};
    use crate::tab::Tab;
    use par_term_terminal::TerminalManager;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use tokio::sync::RwLock;

    fn test_runtime() -> Arc<tokio::runtime::Runtime> {
        Arc::new(
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build test runtime"),
        )
    }

    /// A pane whose terminal has no shell spawned, so it runs without a PTY
    /// on every supported platform. `working_directory` carries the marker.
    fn stub_pane(id: PaneId, marker: &str) -> Pane {
        let terminal = TerminalManager::new_with_scrollback(80, 24, 100)
            .expect("stub terminal creation without a shell");
        Pane::new_wrapping_terminal(
            id,
            Arc::new(RwLock::new(terminal)),
            Some(marker.to_string()),
            Arc::new(AtomicBool::new(false)),
        )
    }

    /// A `WindowState` whose active tab holds two shell-less panes (ids 1
    /// and 2, focused on 1) — the smallest layout the mode targets.
    fn window_with_two_panes() -> WindowState {
        let mut state = WindowState::new(Config::default(), test_runtime());

        let mut tab = Tab::new_stub(1, 1);
        let pm = tab
            .pane_manager_mut()
            .expect("a stub tab always has a pane manager");
        pm.set_root(PaneNode::split(
            SplitDirection::Vertical,
            0.5,
            PaneNode::leaf(stub_pane(1, "/one")),
            PaneNode::leaf(stub_pane(2, "/two")),
        ));
        pm.focus_pane(1);

        state.tab_manager.push_tab_for_test(tab);
        state
    }

    fn focused_pane_id(state: &WindowState) -> Option<PaneId> {
        state
            .tab_manager
            .active_tab()
            .and_then(|t| t.pane_manager())
            .and_then(|pm| pm.focused_pane_id())
    }

    #[test]
    fn arming_draws_badges_and_a_letter_focuses_its_pane() {
        let mut state = window_with_two_panes();

        state.enter_pane_hint_select();
        assert!(
            state.pane_hint_select.is_active(),
            "mode arms on a multi-pane tab"
        );

        // Home-row letters in tree order: pane 1 gets 'a', pane 2 gets 's'.
        state.resolve_pane_hint_select(Some('s'));

        assert!(
            !state.pane_hint_select.is_active(),
            "a match resolves the mode"
        );
        assert_eq!(focused_pane_id(&state), Some(2), "'s' focuses pane 2");
    }

    #[test]
    fn uppercase_letters_match_their_pane() {
        let mut state = window_with_two_panes();
        state.enter_pane_hint_select();
        state.resolve_pane_hint_select(Some('A'));
        assert_eq!(focused_pane_id(&state), Some(1), "'A' matches pane 1's 'a'");
        assert!(!state.pane_hint_select.is_active());
    }

    #[test]
    fn escape_or_a_non_matching_key_cancels_without_changing_focus() {
        let mut state = window_with_two_panes();
        state.enter_pane_hint_select();

        // 'z' is not in the two-pane assignment set ('a', 's'); Escape is a
        // non-character key — both must cancel, leaving focus untouched.
        state.resolve_pane_hint_select(Some('z'));
        assert!(
            !state.pane_hint_select.is_active(),
            "a miss cancels the mode"
        );
        assert_eq!(focused_pane_id(&state), Some(1), "a miss keeps focus");

        state.enter_pane_hint_select();
        state.resolve_pane_hint_select(None);
        assert!(
            !state.pane_hint_select.is_active(),
            "Escape cancels the mode"
        );
        assert_eq!(focused_pane_id(&state), Some(1));
    }

    #[test]
    fn a_letter_for_a_pane_that_no_longer_exists_resolves_without_panicking() {
        // Badges resolve from live pane state each frame, but the key press
        // resolves against arm-time assignments. A pane closed while the
        // mode was armed names an id no lookup can return — the resolution
        // must be a no-op focus, not a panic.
        let mut state = window_with_two_panes();
        state.enter_pane_hint_select();

        let tab = state
            .tab_manager
            .get_tab_mut(1)
            .and_then(|t| t.pane_manager_mut())
            .expect("stub tab has a pane manager");
        let Ok(remap) = tab.insert_subtree_at(
            1,
            PaneNode::leaf(stub_pane(9, "/new")),
            SplitDirection::Vertical,
            0.5,
        ) else {
            panic!("inserting into pane 1 of a two-pane tab succeeds");
        };
        let _ = remap;

        state.resolve_pane_hint_select(Some('s'));
        assert!(!state.pane_hint_select.is_active());
    }

    #[test]
    fn a_single_pane_tab_does_not_arm_the_mode() {
        let mut state = WindowState::new(Config::default(), test_runtime());
        state.tab_manager.push_tab_for_test(Tab::new_stub(1, 1));

        state.enter_pane_hint_select();
        assert!(!state.pane_hint_select.is_active(), "nothing to choose");
    }

    #[test]
    fn leaving_the_tab_cancels_the_mode() {
        let mut state = window_with_two_panes();
        state.enter_pane_hint_select();

        state.tab_manager.push_tab_for_test(Tab::new_stub(2, 2));
        state.tab_manager.switch_to(2);
        // The about-to-wait sweep is what observes the switch.
        state.cancel_pane_hint_select_if_stale();
        assert!(
            !state.pane_hint_select.is_active(),
            "switching tabs cancels the mode"
        );
    }

    #[test]
    fn letters_are_deterministic_per_layout() {
        // Tree order drives assignment, so the same layout must always arm
        // with the same letters — 'a' for pane 1, 's' for pane 2.
        for _ in 0..3 {
            let mut state = window_with_two_panes();
            state.enter_pane_hint_select();
            let PaneHintSelectState::Selecting { assignments, .. } = &state.pane_hint_select else {
                panic!("mode arms");
            };
            assert_eq!(
                assignments,
                &vec![('a', PaneId::from(1u64)), ('s', PaneId::from(2u64))],
            );
        }
    }
}
