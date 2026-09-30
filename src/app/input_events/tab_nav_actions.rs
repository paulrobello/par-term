//! Tab and window navigation actions (UX.md P4: A10–A14, A21).
//!
//! Split out of `keybinding_actions` to keep that table under the line
//! gate. Every function here is an `ActionHandler`: it returns `true`
//! because the name is claimed even when the action has nothing to do
//! (a single tab, no previous tab).

use crate::app::window_state::WindowState;
use crate::tab::TabId;

/// A10 `last_tab`: toggle to the previously active tab (tmux
/// `last-window`). A no-op with a toast when there is none.
pub(super) fn last_tab(s: &mut WindowState) -> bool {
    match s.tab_manager.previous_tab_id() {
        Some(id) => s.switch_to_tab_id(id),
        None => s.show_toast("No previous tab"),
    }
    true
}

/// A11 `go_to_last_tab`: the rightmost visible tab.
pub(super) fn go_to_last_tab(s: &mut WindowState) -> bool {
    if let Some(id) = s.tab_manager.last_visible_tab_id() {
        s.switch_to_tab_id(id);
    }
    true
}

/// A12 `rename_tab`: open the inline rename field on the active tab. The
/// submit goes through `TabBarAction::RenameTab`, so a par-mux tab's
/// rename still reaches the daemon (`rename-window`).
pub(super) fn rename_tab(s: &mut WindowState) -> bool {
    let Some((id, title)) = s.tab_manager.active_tab().map(|t| (t.id, t.title.clone())) else {
        return true;
    };
    let frame = s
        .egui
        .ctx
        .as_ref()
        .map(|ctx| ctx.cumulative_frame_nr())
        .unwrap_or(0);
    let pos = s.tab_bar_ui.rename_anchor(id);
    s.tab_bar_ui.begin_rename(id, &title, pos, frame);
    s.focus_state.needs_redraw = true;
    s.request_redraw();
    true
}

/// A13 `next_window` / `prev_window` and `switch_to_window_1..9`: these
/// need every window, so they queue on the menu bridge the `WindowManager`
/// drains (the same route `new_window` takes).
pub(super) fn next_window(_s: &mut WindowState) -> bool {
    crate::menu::dispatch(crate::menu::MenuAction::CycleWindow(1));
    true
}

pub(super) fn prev_window(_s: &mut WindowState) -> bool {
    crate::menu::dispatch(crate::menu::MenuAction::CycleWindow(-1));
    true
}

/// A13 `switch_to_window_N`: the window holding number N (iTerm2's
/// Cmd+Opt+N, unbound here; see the actions table).
fn focus_window_number(n: usize) -> bool {
    crate::menu::dispatch(crate::menu::MenuAction::FocusWindowNumber(n));
    true
}

pub(super) fn switch_to_window_1(_s: &mut WindowState) -> bool {
    focus_window_number(1)
}
pub(super) fn switch_to_window_2(_s: &mut WindowState) -> bool {
    focus_window_number(2)
}
pub(super) fn switch_to_window_3(_s: &mut WindowState) -> bool {
    focus_window_number(3)
}
pub(super) fn switch_to_window_4(_s: &mut WindowState) -> bool {
    focus_window_number(4)
}
pub(super) fn switch_to_window_5(_s: &mut WindowState) -> bool {
    focus_window_number(5)
}
pub(super) fn switch_to_window_6(_s: &mut WindowState) -> bool {
    focus_window_number(6)
}
pub(super) fn switch_to_window_7(_s: &mut WindowState) -> bool {
    focus_window_number(7)
}
pub(super) fn switch_to_window_8(_s: &mut WindowState) -> bool {
    focus_window_number(8)
}
pub(super) fn switch_to_window_9(_s: &mut WindowState) -> bool {
    focus_window_number(9)
}

/// A14 `close_window` (real): close the whole window with all its tabs,
/// through the same confirmation the title-bar close runs (D6).
pub(super) fn close_window(_s: &mut WindowState) -> bool {
    crate::menu::dispatch(crate::menu::MenuAction::CloseWholeWindow);
    true
}

/// A14 `close_tab_or_window`: the old smart close under its new id — the
/// active tab when the window holds several, else the window.
pub(super) fn close_tab_or_window(_s: &mut WindowState) -> bool {
    crate::menu::dispatch(crate::menu::MenuAction::CloseWindow);
    true
}

/// A21 `close_other_tabs`: close every visible tab except the active one.
pub(super) fn close_other_tabs(s: &mut WindowState) -> bool {
    let Some(keep) = s.tab_manager.active_tab_id() else {
        return true;
    };
    let targets: Vec<TabId> = s
        .tab_manager
        .visible_tabs()
        .iter()
        .map(|t| t.id)
        .filter(|id| *id != keep)
        .collect();
    s.close_tabs_keeping(&targets, keep);
    true
}

/// A21 `close_tabs_to_right`: close every visible tab right of the active
/// one.
pub(super) fn close_tabs_to_right(s: &mut WindowState) -> bool {
    let Some(keep) = s.tab_manager.active_tab_id() else {
        return true;
    };
    let visible: Vec<TabId> = s.tab_manager.visible_tabs().iter().map(|t| t.id).collect();
    let targets: Vec<TabId> = visible
        .iter()
        .skip_while(|id| **id != keep)
        .skip(1)
        .copied()
        .collect();
    s.close_tabs_keeping(&targets, keep);
    true
}

/// A18 `move_tab_to_window_picker`: open the palette on the Move Tab
/// rows — one per other window plus New Window (the keyboard version of
/// the tab context submenu).
pub(super) fn move_tab_to_window_picker(s: &mut WindowState) -> bool {
    if let Some(id) = s.tab_manager.active_tab_id()
        && s.mux_window_for_tab(id).is_some()
    {
        s.show_toast(
            "par-mux: tabs attached to a par-mux session can't move between windows — \
             detach first",
        );
        return true;
    }
    let rows = s.move_tab_palette_rows();
    s.overlay_ui
        .command_palette
        .open(rows, &s.keybinding_registry);
    s.overlay_ui.command_palette.set_query("Move Tab to");
    s.focus_state.needs_redraw = true;
    s.request_redraw();
    true
}

/// `move_tab_to_window:<n>`: the runtime row id for "move the active tab
/// to window N".
pub(crate) fn move_tab_row_id(window_number: usize) -> String {
    format!("move_tab_to_window:{window_number}")
}

impl WindowState {
    /// A18 palette rows: one per other window, labelled with its number and
    /// active tab (`move_tab_candidates`, refreshed every frame by the
    /// manager). Empty for a par-mux tab, which cannot move.
    pub(crate) fn move_tab_palette_rows(
        &self,
    ) -> Vec<crate::command_palette::catalog::PaletteEntry> {
        let movable = self
            .tab_manager
            .active_tab_id()
            .is_some_and(|id| self.mux_window_for_tab(id).is_none());
        if !movable {
            return Vec::new();
        }
        self.overlay_ui
            .move_tab_candidates
            .iter()
            .filter_map(|(_, label)| {
                let number: usize = label
                    .strip_prefix("Window ")?
                    .split(|c: char| !c.is_ascii_digit())
                    .next()?
                    .parse()
                    .ok()?;
                Some(crate::command_palette::catalog::PaletteEntry {
                    action_id: move_tab_row_id(number),
                    label: format!("Move Tab to {label}"),
                    chord: None,
                    priority: 0,
                })
            })
            .collect()
    }

    /// Dispatch a `move_tab_to_window:<n>` row: queue the move of the
    /// active tab to the window holding number N.
    pub(crate) fn dispatch_move_tab_to_window(&mut self, action: &str) -> bool {
        let Some(number) = action
            .strip_prefix("move_tab_to_window:")
            .and_then(|n| n.parse::<usize>().ok())
        else {
            return false;
        };
        let Some(tab_id) = self.tab_manager.active_tab_id() else {
            return false;
        };
        if self.mux_window_for_tab(tab_id).is_some() {
            self.show_toast(
                "par-mux: tabs attached to a par-mux session can't move between windows — \
                 detach first",
            );
            return true;
        }
        let dest = self
            .overlay_ui
            .move_tab_candidates
            .iter()
            .find(|(_, label)| {
                label
                    .strip_prefix("Window ")
                    .and_then(|rest| rest.split(|c: char| !c.is_ascii_digit()).next())
                    .and_then(|n| n.parse::<usize>().ok())
                    == Some(number)
            })
            .map(|(id, _)| *id);
        match dest {
            Some(dest) => {
                self.overlay_ui.pending_move_tab_request =
                    Some(crate::app::window_manager::MoveTabRequest {
                        tab_id,
                        destination: crate::app::window_manager::MoveDestination::ExistingWindow(
                            dest,
                        ),
                    });
                true
            }
            None => {
                self.show_toast(format!("No window {number}"));
                false
            }
        }
    }

    /// Switch to `id` through the user tab-switch tail (copy mode exit,
    /// render invalidation, `select-window` for an attached tab).
    pub(crate) fn switch_to_tab_id(&mut self, id: TabId) {
        self.copy_mode.exit();
        self.tab_manager.switch_to(id);
        self.clear_and_invalidate();
        self.after_user_tab_switch();
    }

    /// Close `targets` one by one through the ordinary tab close — each
    /// keeps its running-job confirmation, par-mux `kill-window`, and undo
    /// entry — then return to `keep`. A close that stops for a
    /// confirmation dialog ends the batch there: one dialog at a time, and
    /// the tabs after it stay open.
    pub(crate) fn close_tabs_keeping(&mut self, targets: &[TabId], keep: TabId) {
        let mut closed = 0usize;
        for &id in targets {
            if self.tab_manager.get_tab(id).is_none() {
                continue;
            }
            self.tab_manager.switch_to(id);
            let before = self.tab_manager.tab_count();
            self.close_current_tab();
            if self.overlay_ui.close_confirmation_ui.is_visible()
                || self.overlay_ui.mux_last_tab_ui.is_visible()
            {
                break;
            }
            if self.tab_manager.tab_count() < before || self.mux_window_for_tab(id).is_some() {
                closed += 1;
            }
        }
        if self.tab_manager.get_tab(keep).is_some()
            && !self.overlay_ui.close_confirmation_ui.is_visible()
        {
            self.tab_manager.switch_to(keep);
        }
        self.clear_and_invalidate();
        if closed > 1 {
            // An action toast (UX.md OV7): the button reopens the last
            // closed tab; the hint names the live chord, never a raw
            // config string.
            let hint = self
                .live_chord_hint("reopen_closed_tab")
                .map(|chord| format!(" ({chord} reopens)"))
                .unwrap_or_default();
            self.post_toast(
                crate::app::overlay::toast::ToastKind::Info,
                format!("Closed {closed} tabs{hint}"),
                Some(crate::app::overlay::toast::ToastAction {
                    label: "Reopen".to_string(),
                    action_id: "reopen_closed_tab".to_string(),
                }),
            );
        }
        self.focus_state.needs_redraw = true;
        self.request_redraw();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window_with_tabs(n: usize) -> WindowState {
        let runtime = std::sync::Arc::new(
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("runtime"),
        );
        let mut s = WindowState::new(crate::config::Config::default(), runtime);
        for i in 0..n {
            s.tab_manager
                .push_tab_for_test(crate::tab::Tab::new_stub(i as TabId + 1, i + 1));
        }
        assert_eq!(s.tab_manager.tab_count(), n);
        s
    }

    fn active_index(s: &WindowState) -> usize {
        s.tab_manager.active_tab_index().expect("active tab")
    }

    /// A10: last_tab toggles between the two most recent tabs, like
    /// tmux `last-window`.
    #[test]
    fn last_tab_toggles_between_the_two_most_recent_tabs() {
        let mut s = window_with_tabs(4);
        s.switch_to_tab_index(1);
        s.switch_to_tab_index(3);
        assert!(last_tab(&mut s));
        assert_eq!(active_index(&s), 0, "back to tab 1");
        assert!(last_tab(&mut s));
        assert_eq!(active_index(&s), 2, "and forward to tab 3 again");
    }

    /// A10: a previous tab that has since closed is not a target.
    #[test]
    fn last_tab_ignores_a_closed_previous_tab() {
        let mut s = window_with_tabs(3);
        s.switch_to_tab_index(1);
        let first = s.tab_manager.active_tab_id().unwrap();
        s.switch_to_tab_index(3);
        s.tab_manager.close_tab(first);
        assert_eq!(s.tab_manager.previous_tab_id(), None);
        let before = s.tab_manager.active_tab_id();
        last_tab(&mut s);
        assert_eq!(s.tab_manager.active_tab_id(), before);
    }

    /// A11: go_to_last_tab lands on the rightmost visible tab.
    #[test]
    fn go_to_last_tab_lands_on_the_rightmost_tab() {
        let mut s = window_with_tabs(5);
        s.switch_to_tab_index(2);
        go_to_last_tab(&mut s);
        assert_eq!(active_index(&s), 4);
    }

    /// A11: a hidden rightmost tab is skipped.
    #[test]
    fn go_to_last_tab_skips_a_hidden_tab() {
        let mut s = window_with_tabs(3);
        let last = s.tab_manager.tabs().last().unwrap().id;
        s.tab_manager.get_tab_mut(last).unwrap().is_hidden = true;
        s.switch_to_tab_index(1);
        go_to_last_tab(&mut s);
        assert_eq!(active_index(&s), 1);
    }

    /// A12: rename_tab opens the inline rename on the active tab.
    #[test]
    fn rename_tab_opens_the_inline_rename_on_the_active_tab() {
        let mut s = window_with_tabs(2);
        s.switch_to_tab_index(2);
        assert!(!s.tab_bar_ui.is_renaming());
        rename_tab(&mut s);
        assert!(s.tab_bar_ui.is_renaming());
        assert_eq!(
            s.tab_bar_ui.context_menu_tab_id(),
            s.tab_manager.active_tab_id()
        );
    }

    /// A18: one Move Tab row per other window; a row queues the move of
    /// the active tab to that window.
    #[test]
    fn move_tab_rows_list_other_windows_and_queue_the_move() {
        let mut s = window_with_tabs(2);
        let other = winit::window::WindowId::from(42u64);
        s.overlay_ui.move_tab_candidates = vec![(other, "Window 3 - logs".to_string())];
        let rows = s.move_tab_palette_rows();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].action_id, "move_tab_to_window:3");
        assert_eq!(rows[0].label, "Move Tab to Window 3 - logs");

        assert!(s.execute_keybinding_action("move_tab_to_window:3"));
        let req = s.overlay_ui.pending_move_tab_request.expect("queued");
        assert_eq!(Some(req.tab_id), s.tab_manager.active_tab_id());
        assert!(matches!(
            req.destination,
            crate::app::window_manager::MoveDestination::ExistingWindow(id) if id == other
        ));
        assert!(
            !s.execute_keybinding_action("move_tab_to_window:9"),
            "no window 9"
        );
    }

    /// A21: close_other_tabs keeps only the active tab.
    #[test]
    fn close_other_tabs_keeps_only_the_active_tab() {
        let mut s = window_with_tabs(4);
        s.switch_to_tab_index(2);
        let keep = s.tab_manager.active_tab_id().unwrap();
        close_other_tabs(&mut s);
        assert_eq!(s.tab_manager.tab_count(), 1);
        assert_eq!(s.tab_manager.active_tab_id(), Some(keep));
    }

    /// A21: close_tabs_to_right keeps the active tab and everything left
    /// of it.
    #[test]
    fn close_tabs_to_right_keeps_the_left_side() {
        let mut s = window_with_tabs(5);
        s.switch_to_tab_index(2);
        let keep = s.tab_manager.active_tab_id().unwrap();
        close_tabs_to_right(&mut s);
        assert_eq!(s.tab_manager.tab_count(), 2);
        assert_eq!(s.tab_manager.active_tab_id(), Some(keep));
        assert_eq!(active_index(&s), 1);
    }
}
