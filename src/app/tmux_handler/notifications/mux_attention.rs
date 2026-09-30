//! Agent attention across tabs (UX.md A17, V1, V2, V3): the roster cache
//! projected onto tabs and panes each frame, the session chip snapshot,
//! and "jump to the next agent needing attention".
//!
//! Every surface reads the one roster cache (`TmuxState::agent_roster`),
//! so tab badges, pane badges, the status widget, the palette rows, and the
//! A17 cycle cannot disagree about who is blocked.

use crate::app::window_state::WindowState;
use crate::pane::PaneId;
use crate::session_chip::{MuxHealth, SessionChip};
use crate::tab::TabId;
use crate::tab::pane_badges::{AgentAttention, TabMuxView};
use par_term_tmux::TmuxPaneId;

/// One agent that owes the user a look, in cycle order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AttentionTarget {
    pub(crate) tab: TabId,
    pub(crate) pane: PaneId,
    pub(crate) mux_pane: TmuxPaneId,
    pub(crate) attention: AgentAttention,
}

/// The A17 cycle order: blocked agents first, then done-unseen, each group
/// in tab order then pane order. `tab_order` is the window's tab order;
/// `candidates` are mapped roster panes with their attention.
pub(crate) fn attention_order(
    tab_order: &[TabId],
    mut candidates: Vec<AttentionTarget>,
) -> Vec<AttentionTarget> {
    candidates.retain(|c| {
        matches!(
            c.attention,
            AgentAttention::Blocked | AgentAttention::DoneUnseen
        )
    });
    let tab_pos = |tab: TabId| {
        tab_order
            .iter()
            .position(|t| *t == tab)
            .unwrap_or(usize::MAX)
    };
    candidates.sort_by_key(|c| (std::cmp::Reverse(c.attention), tab_pos(c.tab), c.mux_pane));
    candidates
}

/// The target after `current` in `order`, wrapping; the first when
/// `current` is not in the list (the user is elsewhere).
pub(crate) fn next_attention(
    order: &[AttentionTarget],
    current: Option<TmuxPaneId>,
) -> Option<AttentionTarget> {
    if order.is_empty() {
        return None;
    }
    let next = current
        .and_then(|cur| order.iter().position(|t| t.mux_pane == cur))
        .map_or(0, |i| (i + 1) % order.len());
    Some(order[next])
}

impl WindowState {
    /// Every mapped roster agent with its attention state.
    fn roster_attention(&self) -> Vec<AttentionTarget> {
        let roster = &self.tmux_state.agent_roster;
        roster
            .iter()
            .filter_map(|entry| {
                let (tab, pane) = self.tmux_state.tmux_pane_owner(entry.pane)?;
                let attention =
                    AgentAttention::from_roster(&entry.state, roster.is_done_unseen(entry.pane))?;
                Some(AttentionTarget {
                    tab,
                    pane,
                    mux_pane: entry.pane,
                    attention,
                })
            })
            .collect()
    }

    /// Refresh every tab's par-mux view from the roster (UX.md V2/V3).
    /// Cheap (the roster is a handful of entries); runs once per frame
    /// before the tab bar and pane titles draw.
    pub(crate) fn refresh_tab_mux_views(&mut self) {
        let attention = self.roster_attention();
        let session = self.tmux_state.tmux_session_name.clone();
        let attached_windows: Vec<TabId> = self
            .tab_manager
            .tabs()
            .iter()
            .map(|t| t.id)
            .filter(|id| self.mux_window_for_tab(*id).is_some())
            .collect();
        for tab in self.tab_manager.tabs_mut() {
            let attached = attached_windows.contains(&tab.id);
            let agents = attention
                .iter()
                .filter(|a| a.tab == tab.id)
                .map(|a| (a.pane, a.attention))
                .collect();
            let view = TabMuxView {
                attached,
                session: attached.then(|| session.clone()).flatten(),
                agents,
            };
            if tab.mux_view != view {
                tab.mux_view = view;
            }
        }
    }

    /// Hidden daemon tabs of this window (UX.md M4/D7), for the chip. The
    /// tmux gateway tab also hides itself; only mux-mapped tabs count.
    pub(crate) fn hidden_mux_tab_count(&self) -> usize {
        self.tab_manager
            .tabs()
            .iter()
            .filter(|t| t.is_hidden && self.mux_window_for_tab(t.id).is_some())
            .count()
    }

    /// This frame's session chip (UX.md V1).
    pub(crate) fn session_chip(&self) -> SessionChip {
        let attaching = self.tmux_state.mux_attach_pending.as_ref();
        let session = match (&self.tmux_state.transport, attaching) {
            (Some(_), _) => self.tmux_state.tmux_session_name.clone(),
            (None, Some(pending)) => Some(pending.name.clone()),
            (None, None) => None,
        };
        let health = if self.tmux_state.transport.is_none() && attaching.is_some() {
            MuxHealth::Attaching
        } else {
            self.tmux_state.mux_health
        };
        SessionChip {
            session,
            health,
            hidden_tabs: self.hidden_mux_tab_count(),
            last_error: self.tmux_state.mux_last_error.clone(),
        }
    }

    /// Apply a chip click.
    pub(crate) fn handle_session_chip_action(
        &mut self,
        action: crate::session_chip::SessionChipAction,
    ) {
        match action {
            crate::session_chip::SessionChipAction::OpenPicker => {
                if !self.overlay_ui.tmux_session_picker_ui.visible {
                    self.overlay_ui.tmux_session_picker_ui.show_picker();
                    self.refresh_mux_directory();
                }
            }
            crate::session_chip::SessionChipAction::DismissError => {
                self.tmux_state.mux_last_error = None;
            }
        }
        self.focus_state.needs_redraw = true;
        self.request_redraw();
    }

    /// A17 `focus_next_attention_agent`: jump to the next roster agent that
    /// is blocked, then done-unseen, cycling across tabs. Focusing a
    /// done-unseen agent marks it seen, so the order is recomputed on every
    /// press. Returns false when no agent needs attention.
    pub(crate) fn focus_next_attention_agent(&mut self) -> bool {
        let tab_order: Vec<TabId> = self.tab_manager.tabs().iter().map(|t| t.id).collect();
        let order = attention_order(&tab_order, self.roster_attention());
        let current = self.focused_mux_pane_from_native();
        match next_attention(&order, current) {
            Some(target) => self.focus_agent_roster_pane(target.mux_pane),
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use AgentAttention as A;

    fn t(tab: TabId, mux_pane: TmuxPaneId, attention: AgentAttention) -> AttentionTarget {
        AttentionTarget {
            tab,
            pane: mux_pane as PaneId,
            mux_pane,
            attention,
        }
    }

    #[test]
    fn blocked_agents_come_before_done_unseen_in_tab_order() {
        let order = attention_order(
            &[3, 1, 2],
            vec![
                t(1, 10, A::DoneUnseen),
                t(2, 20, A::Blocked),
                t(3, 30, A::Working),
                t(3, 31, A::Blocked),
                t(1, 11, A::Blocked),
            ],
        );
        let panes: Vec<TmuxPaneId> = order.iter().map(|o| o.mux_pane).collect();
        assert_eq!(
            panes,
            vec![31, 11, 20, 10],
            "blocked in tab order (tab 3, 1, 2), then done-unseen; working never"
        );
    }

    #[test]
    fn the_cycle_advances_and_wraps() {
        let order = attention_order(&[1, 2], vec![t(1, 1, A::Blocked), t(2, 2, A::Blocked)]);
        assert_eq!(next_attention(&order, None).map(|o| o.mux_pane), Some(1));
        assert_eq!(next_attention(&order, Some(1)).map(|o| o.mux_pane), Some(2));
        assert_eq!(next_attention(&order, Some(2)).map(|o| o.mux_pane), Some(1));
        assert_eq!(
            next_attention(&order, Some(99)).map(|o| o.mux_pane),
            Some(1),
            "from a pane not in the list, start at the top"
        );
        assert_eq!(next_attention(&[], Some(1)), None);
    }
}
