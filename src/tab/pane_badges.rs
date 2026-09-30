//! Tab-bar and pane-title indicators for pane modes (UX.md V5, V6).
//!
//! Pure functions over tab state so the indicator text is unit-testable:
//! `--screenshot` captures skip the egui overlay these are drawn in.

use super::Tab;

/// Zoom indicator glyph (V6). `⤢` is not in egui's default fonts, so the
/// "maximize" frame glyph stands in for it.
pub(crate) const ZOOM_BADGE: &str = "⛶";

/// Broadcast indicator glyph (V5).
pub(crate) const BROADCAST_BADGE: &str = "📡";

impl Tab {
    /// The pane-mode badge the tab bar shows before the title, or `None`.
    /// Broadcast wins over zoom: it changes where typing goes.
    pub(crate) fn pane_mode_badge(&self) -> Option<&'static str> {
        if self.broadcast_input {
            return Some(BROADCAST_BADGE);
        }
        self.pane_manager
            .as_ref()
            .is_some_and(|pm| pm.is_zoomed())
            .then_some(ZOOM_BADGE)
    }

    /// The hover text explaining [`Self::pane_mode_badge`].
    pub(crate) fn pane_mode_badge_tooltip(&self) -> &'static str {
        if self.broadcast_input {
            "Broadcast input: typing and pastes go to every pane in this tab"
        } else {
            "A pane is zoomed to fill this tab"
        }
    }

    /// The panes that receive broadcast input: every pane not opted out.
    pub(crate) fn broadcast_receivers(&self) -> Vec<crate::pane::PaneId> {
        self.pane_manager
            .as_ref()
            .map(|pm| {
                pm.all_panes()
                    .iter()
                    .filter(|p| !p.broadcast_excluded)
                    .map(|p| p.id)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Bounds of the panes to outline as broadcast receivers (V5), or
    /// empty when broadcast is off. Zoom-aware: only visible panes.
    pub(crate) fn broadcast_outline_bounds(&self) -> Vec<crate::pane::PaneBounds> {
        if !self.broadcast_input {
            return Vec::new();
        }
        let Some(pm) = self.pane_manager.as_ref() else {
            return Vec::new();
        };
        if !pm.has_multiple_panes() {
            return Vec::new();
        }
        pm.visible_panes()
            .iter()
            .filter(|p| !p.broadcast_excluded)
            .map(|p| p.bounds)
            .collect()
    }
}

/// Attached-tab badge glyph (UX.md V2).
pub(crate) const ATTACHED_BADGE: &str = "🔗";

/// An agent's attention state as badges show it (UX.md V3). Ordered by
/// urgency: a tab badge shows its most urgent pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum AgentAttention {
    /// Working — a subtle marker.
    Working,
    /// Finished and not looked at since.
    DoneUnseen,
    /// Waiting on the user.
    Blocked,
}

impl AgentAttention {
    /// Map a roster entry's verbatim state (plus par-term's own
    /// done-unseen mark) onto a badge. `None`: no badge (idle, unknown).
    pub(crate) fn from_roster(state: &str, done_unseen: bool) -> Option<Self> {
        if done_unseen {
            return Some(Self::DoneUnseen);
        }
        if state.eq_ignore_ascii_case("blocked") {
            Some(Self::Blocked)
        } else if state.eq_ignore_ascii_case("working") {
            Some(Self::Working)
        } else {
            None
        }
    }

    /// The badge glyph: amber dot, green dot, working marker.
    pub(crate) fn glyph(self) -> &'static str {
        match self {
            Self::Blocked => "🟠",
            Self::DoneUnseen => "🟢",
            Self::Working => "⋯",
        }
    }

    pub(crate) fn describe(self) -> &'static str {
        match self {
            Self::Blocked => "an agent is waiting for you",
            Self::DoneUnseen => "an agent finished (not yet seen)",
            Self::Working => "an agent is working",
        }
    }
}

/// A tab's par-mux view for one frame (UX.md V2/V3), filled from the
/// roster cache by the window.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TabMuxView {
    /// The tab mirrors a daemon window (decided per tab, not per window).
    pub(crate) attached: bool,
    /// The attached session's name, for the badge tooltip.
    pub(crate) session: Option<String>,
    /// Agent attention per native pane of this tab.
    pub(crate) agents: Vec<(crate::pane::PaneId, AgentAttention)>,
}

impl TabMuxView {
    /// The tab's agent badge: its most urgent pane's state.
    pub(crate) fn agent_badge(&self) -> Option<AgentAttention> {
        self.agents.iter().map(|(_, a)| *a).max()
    }

    /// One pane's agent state, for its title bar.
    pub(crate) fn pane_agent(&self, pane: crate::pane::PaneId) -> Option<AgentAttention> {
        self.agents
            .iter()
            .find(|(id, _)| *id == pane)
            .map(|(_, a)| *a)
    }

    /// The attached badge's hover text.
    pub(crate) fn attached_tooltip(&self) -> String {
        match &self.session {
            Some(name) => format!("Attached to par-mux session {name}; survives quit"),
            None => "Attached to a par-mux session; survives quit".to_string(),
        }
    }
}

/// A pane title with its mode markers (V6: zoom) and, when
/// `show_pane_numbers` is on, its 1-based tree-order number (V7).
pub(crate) fn decorate_pane_title(title: String, zoomed: bool, number: Option<usize>) -> String {
    let title = match number {
        Some(n) => format!("{n}: {title}"),
        None => title,
    };
    if zoomed {
        format!("{ZOOM_BADGE} {title}")
    } else {
        title
    }
}

/// [`decorate_pane_title`] plus the pane's agent badge (UX.md V3), which
/// leads so a blocked agent reads first.
pub(crate) fn decorate_pane_title_with_agent(
    title: String,
    zoomed: bool,
    number: Option<usize>,
    agent: Option<AgentAttention>,
) -> String {
    let title = decorate_pane_title(title, zoomed, number);
    match agent {
        Some(agent) => format!("{} {title}", agent.glyph()),
        None => title,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_zoomed_pane_title_carries_the_zoom_marker() {
        assert_eq!(decorate_pane_title("vim".into(), true, None), "⛶ vim");
        assert_eq!(decorate_pane_title("vim".into(), false, None), "vim");
    }

    #[test]
    fn roster_states_map_to_badges_with_done_unseen_winning() {
        use AgentAttention as A;
        assert_eq!(
            AgentAttention::from_roster("blocked", false),
            Some(A::Blocked)
        );
        assert_eq!(
            AgentAttention::from_roster("Working", false),
            Some(A::Working)
        );
        assert_eq!(AgentAttention::from_roster("idle", false), None);
        assert_eq!(
            AgentAttention::from_roster("idle", true),
            Some(A::DoneUnseen)
        );
    }

    #[test]
    fn a_tab_badge_shows_its_most_urgent_pane() {
        use AgentAttention as A;
        let view = TabMuxView {
            attached: true,
            session: None,
            agents: vec![(1, A::Working), (2, A::Blocked), (3, A::DoneUnseen)],
        };
        assert_eq!(view.agent_badge(), Some(A::Blocked));
        assert_eq!(view.pane_agent(3), Some(A::DoneUnseen));
        assert_eq!(view.pane_agent(9), None);
        assert_eq!(TabMuxView::default().agent_badge(), None);
    }

    #[test]
    fn a_pane_title_leads_with_its_agent_badge() {
        assert_eq!(
            decorate_pane_title_with_agent(
                "claude".into(),
                false,
                None,
                Some(AgentAttention::Blocked)
            ),
            "🟠 claude"
        );
        assert_eq!(
            decorate_pane_title_with_agent("vim".into(), true, None, None),
            "⛶ vim"
        );
    }

    /// V7: `show_pane_numbers` prefixes the tree-order number, after the
    /// zoom marker.
    #[test]
    fn a_numbered_pane_title_leads_with_its_number() {
        assert_eq!(decorate_pane_title("vim".into(), false, Some(3)), "3: vim");
        assert_eq!(decorate_pane_title("vim".into(), true, Some(1)), "⛶ 1: vim");
    }
}
