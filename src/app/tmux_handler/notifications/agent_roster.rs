//! The cached agent roster (A2b task 1): the ONE owner of par-mux agent
//! state on the app side.
//!
//! Filled wholesale by `list-agents` on attach/reattach, updated
//! incrementally by each `%agent-state-changed` push (intercepted in
//! [`super::mux::check_mux_notifications`] before the `ParserBridge`
//! drop), and cleared when the transport dies or the session ends. Every
//! roster surface — the status-bar widget, the palette picker — reads
//! this cache; none queries the daemon directly, because three surfaces
//! polling one socket is three chances to disagree about what the roster
//! says.
//!
//! Semantics carried from the Phase 5 ruling: a pane with no state report
//! is ABSENT, never present-and-idle (the fill replaces wholesale, so a
//! pane that stops reporting drops out at the next fill), states are kept
//! verbatim with nothing inferred, and each entry keeps the provenance
//! (`hook` claim vs `scrape` guess) the daemon recorded.

use par_term_mux::AgentEntry;
use par_term_tmux::TmuxPaneId;

/// The cached roster, keyed by pane for deterministic surface order (the
/// daemon's own reply is pane-sorted).
pub(crate) struct AgentRoster {
    entries: std::collections::BTreeMap<TmuxPaneId, AgentEntry>,
}

impl AgentRoster {
    pub(crate) fn new() -> Self {
        Self {
            entries: std::collections::BTreeMap::new(),
        }
    }

    /// Replace the whole roster from a `list-agents` reply. Wholesale
    /// replacement IS the absent-means-absent rule: a pane that no longer
    /// reports is not in the reply and therefore not in the roster.
    pub(crate) fn fill_from_list(&mut self, agents: Vec<AgentEntry>) {
        self.entries.clear();
        for entry in agents {
            self.entries.insert(entry.pane, entry);
        }
    }

    /// Apply one `%agent-state-changed` push: an upsert, because the push
    /// is per pane and a later push replaces the earlier state. There is
    /// no removal push — a pane leaves the roster at the next fill or
    /// when the roster clears.
    pub(crate) fn apply_push(&mut self, entry: AgentEntry) {
        self.entries.insert(entry.pane, entry);
    }

    /// Drop every entry — the transport died or the session ended, and a
    /// roster that outlives its daemon renders ghosts.
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }

    /// The rostered entries, pane-ordered, for surfaces to render.
    /// A2b tasks 2/3 (the status widget, the palette picker) are the
    /// callers; until they land there is no in-crate reader, hence the
    /// scoped allow.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn iter(&self) -> impl Iterator<Item = &AgentEntry> {
        self.entries.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use par_term_mux::AgentSource;

    fn entry(pane: u64, agent: &str, state: &str, source: AgentSource) -> AgentEntry {
        AgentEntry {
            pane,
            agent: agent.to_string(),
            state: state.to_string(),
            source,
            reason: None,
        }
    }

    #[test]
    fn fill_replaces_wholesale_so_absent_drops_out() {
        let mut roster = AgentRoster::new();
        roster.fill_from_list(vec![entry(0, "kimi", "working", AgentSource::Hook)]);
        roster.fill_from_list(vec![entry(1, "claude", "blocked", AgentSource::Scrape)]);
        let panes: Vec<_> = roster.iter().map(|e| e.pane).collect();
        assert_eq!(
            panes,
            vec![1],
            "pane 0 stopped reporting and must be absent, not idle"
        );
    }

    #[test]
    fn push_upserts_its_pane_only() {
        let mut roster = AgentRoster::new();
        roster.fill_from_list(vec![
            entry(0, "kimi", "working", AgentSource::Hook),
            entry(3, "omp", "idle", AgentSource::Hook),
        ]);
        roster.apply_push(entry(0, "kimi", "blocked", AgentSource::Hook));
        let snapshot: Vec<_> = roster.iter().map(|e| (e.pane, e.state.clone())).collect();
        assert_eq!(
            snapshot,
            vec![(0, "blocked".to_string()), (3, "idle".to_string())]
        );
    }

    #[test]
    fn entries_keep_provenance_and_absent_reason() {
        let mut roster = AgentRoster::new();
        roster.apply_push(entry(2, "pi", "blocked", AgentSource::Scrape));
        let rendered = roster.iter().next().unwrap();
        assert_eq!(rendered.source, AgentSource::Scrape);
        assert_eq!(rendered.reason, None, "no reason on the wire yet");
    }

    #[test]
    fn clear_empties_for_dead_transports() {
        let mut roster = AgentRoster::new();
        roster.apply_push(entry(0, "kimi", "working", AgentSource::Hook));
        roster.clear();
        assert_eq!(roster.iter().count(), 0);
    }
}
