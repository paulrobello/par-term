//! The cached agent roster (A2b task 1): the ONE owner of par-mux agent
//! state on the app side.
//!
//! Filled wholesale by `list-agents` on attach/reattach, updated
//! incrementally by each `%agent-state-changed` push (intercepted in
//! [`super::mux_drain`]'s `check_mux_notifications` before the `ParserBridge`
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
//!
//! The cache mirrors the daemon, and the daemon is cross-session: its
//! `list-agents` and `%agent-state-changed` pushes cover every session it
//! owns, not just the attached one. The SURFACES are scoped instead — every
//! render method takes a `visible` predicate, and the call sites pass "the
//! app currently maps this pane to a native pane". A mapped pane is by
//! construction a pane of the attached session, so other sessions' agents
//! never render, and a closed pane drops out the moment the layout
//! reconciliation drops its mapping. `remove_panes` keeps the cache itself
//! honest for the close signal the app does receive.

use par_term_mux::{AgentEntry, AgentSource};
use par_term_tmux::TmuxPaneId;

/// The cached roster, keyed by pane for deterministic surface order (the
/// daemon's own reply is pane-sorted).
pub(crate) struct AgentRoster {
    entries: std::collections::BTreeMap<TmuxPaneId, AgentEntry>,
    /// Panes whose agent finished (`working` → `idle`) and that the user
    /// has not focused since — herdr's "Done, unseen" signal. Cleared by
    /// [`Self::mark_seen`] at the focus seam; every removal path drops it
    /// with the entry.
    done_unseen: std::collections::BTreeSet<TmuxPaneId>,
}

impl AgentRoster {
    pub(crate) fn new() -> Self {
        Self {
            entries: std::collections::BTreeMap::new(),
            done_unseen: std::collections::BTreeSet::new(),
        }
    }

    /// Replace the whole roster from a `list-agents` reply. Wholesale
    /// replacement IS the absent-means-absent rule: a pane that no longer
    /// reports is not in the reply and therefore not in the roster. Viewed
    /// state is par-term's own (the daemon's reply carries none), so a
    /// fresh fill starts every pane seen — the reattach view is new.
    pub(crate) fn fill_from_list(&mut self, agents: Vec<AgentEntry>) {
        self.entries.clear();
        self.done_unseen.clear();
        for entry in agents {
            self.entries.insert(entry.pane, entry);
        }
    }

    /// Apply one `%agent-state-changed` push: an upsert, because the push
    /// is per pane and a later push replaces the earlier state. A
    /// `working` → `idle` transition marks the pane done-unseen; the drain
    /// clears the mark for the pane the user is watching (the watching
    /// case has no focus event to clear it later).
    pub(crate) fn apply_push(&mut self, entry: AgentEntry) {
        let finished = self.entries.get(&entry.pane).is_some_and(|prev| {
            prev.state.eq_ignore_ascii_case("working") && entry.state.eq_ignore_ascii_case("idle")
        });
        if finished {
            self.done_unseen.insert(entry.pane);
        }
        self.entries.insert(entry.pane, entry);
    }

    /// The user focused `pane` — a done agent there has been seen. A no-op
    /// for panes with no unseen mark.
    pub(crate) fn mark_seen(&mut self, pane: TmuxPaneId) {
        self.done_unseen.remove(&pane);
    }

    /// Apply one `%agent-released` push: the claiming agent announced it is
    /// gone (pane.release_agent, core aed41c2), so the pane leaves the
    /// roster now instead of showing a dead agent working until the next
    /// fill or close. Label-guarded: a release for an agent that no longer
    /// owns the pane (the claim was superseded between release and
    /// delivery) must not drop the successor's entry.
    pub(crate) fn apply_release(&mut self, pane: TmuxPaneId, agent: &str) {
        if self
            .entries
            .get(&pane)
            .is_some_and(|entry| entry.agent == agent)
        {
            self.entries.remove(&pane);
            self.done_unseen.remove(&pane);
        }
    }

    /// Drop the entries for closed panes — the daemon sends no removal
    /// push, so the layout reconciliation's `panes_to_remove` is the app's
    /// only close signal. Without this, a closed pane's entry lingers in
    /// the cache until the next attach fill (its surface row hides with
    /// the mapping gone, but the cache would be lying about the daemon).
    pub(crate) fn remove_panes(&mut self, panes: &[TmuxPaneId]) {
        for pane in panes {
            self.entries.remove(pane);
            self.done_unseen.remove(pane);
        }
    }

    /// Drop every entry — the transport died or the session ended, and a
    /// roster that outlives its daemon renders ghosts.
    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.done_unseen.clear();
    }

    /// The rostered entries, pane-ordered, for surfaces to render.
    /// A2b tasks 2/3 (the status widget, the palette picker) are the callers.
    pub(crate) fn iter(&self) -> impl Iterator<Item = &AgentEntry> {
        self.entries.values()
    }

    /// One-line summary for the status-bar widget (A2b task 2):
    /// `\u{1f465} 2 blocked, 1~ working`. States are grouped verbatim (nothing
    /// inferred), ordered by count desc then state asc for determinism. A `~`
    /// marks scrape-sourced counts (detected, not reported) — one character
    /// per affected group; mixed groups split as `2+1~`. `None` hides the
    /// widget (empty roster / no mux session). `visible` scopes the count to
    /// the attached session (see the module doc: the cache is cross-session,
    /// the surfaces are not).
    pub(crate) fn summary_line(&self, visible: &dyn Fn(TmuxPaneId) -> bool) -> Option<String> {
        let mut groups: std::collections::BTreeMap<&str, (u32, u32)> =
            std::collections::BTreeMap::new();
        for entry in self.iter().filter(|e| visible(e.pane)) {
            // done-unseen renders as its own group, herdr-style — the
            // "who owes you a look" count the widget exists for.
            let state = if self.done_unseen.contains(&entry.pane) {
                "done"
            } else {
                entry.state.as_str()
            };
            let counts = groups.entry(state).or_insert((0, 0));
            match entry.source {
                AgentSource::Hook => counts.0 += 1,
                AgentSource::Scrape => counts.1 += 1,
            }
        }
        let mut parts: Vec<(u32, &str, String)> = groups
            .into_iter()
            .map(|(state, (hook, scrape))| {
                let count_text = match (hook, scrape) {
                    (hook, 0) => hook.to_string(),
                    (0, scrape) => format!("{scrape}~"),
                    (hook, scrape) => format!("{hook}+{scrape}~"),
                };
                (hook + scrape, state, count_text)
            })
            .collect();
        parts.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(b.1)));
        if parts.is_empty() {
            // Every entry is out of scope (other sessions / unmapped panes):
            // same hiding as an empty roster, not a bare glyph with no count.
            return None;
        }
        let breakdown = parts
            .into_iter()
            .map(|(_, state, count_text)| format!("{count_text} {state}"))
            .collect::<Vec<_>>()
            .join(", ");
        Some(format!("\u{1f465} {breakdown}"))
    }

    /// Multi-line hover text for the status-bar widget: one line per agent
    /// with pane, state, and provenance ("reported" = hook claim, "detected"
    /// = scrape guess), plus the reason when the wire carries one. `None`
    /// hides the tooltip. `visible` scopes the lines to the attached
    /// session, like [`Self::summary_line`].
    pub(crate) fn tooltip_text(&self, visible: &dyn Fn(TmuxPaneId) -> bool) -> Option<String> {
        let lines: Vec<String> = self
            .iter()
            .filter(|e| visible(e.pane))
            .map(|entry| {
                let source = match entry.source {
                    AgentSource::Hook => "reported",
                    AgentSource::Scrape => "detected",
                };
                let state = if self.done_unseen.contains(&entry.pane) {
                    "done (unseen)"
                } else {
                    entry.state.as_str()
                };
                let mut line = format!(
                    "{} · {} · {} (pane {})",
                    entry.agent, state, source, entry.pane
                );
                if let Some(reason) = entry.reason.as_ref() {
                    line.push_str(&format!(" — {reason}"));
                }
                line
            })
            .collect();
        if lines.is_empty() {
            return None;
        }
        Some(lines.join("\n"))
    }

    /// Palette rows for rostered agents (A2b task 3): one runtime
    /// [`PaletteEntry`] per agent, following `plugin_palette_entries`'s
    /// runtime-rows shape rather than the static dispatch tables.
    ///
    /// - label — `agent: state`, `~`-suffixed when scrape-detected (the same
    ///   marker convention as the status widget), plus ` — reason` when the
    ///   wire carries one (absent degrades cleanly — claude/codex/grok send
    ///   no message at all).
    /// - action_id — `agent-roster-focus:<pane>`, dispatched by the
    ///   miss-path in `execute_keybinding_action` to focus that pane.
    /// - priority — 2 for blocked agents and unseen-done finishers, 1 for
    ///   the rest, so the picker answers "who owes you a look" first; pane
    ///   order is preserved within each tier (stable sort over the
    ///   pane-ordered cache). Unseen-done rows read `done ✓`.
    ///
    /// `visible` scopes the rows to the attached session (the module doc's
    /// rule): a row the picker offers must be a pane `focus_agent_roster_pane`
    /// can actually focus, and an unmapped pane — another session's agent, a
    /// closed pane mid-reconciliation — is not focusable.
    pub(crate) fn palette_rows(
        &self,
        visible: &dyn Fn(TmuxPaneId) -> bool,
    ) -> Vec<crate::command_palette::catalog::PaletteEntry> {
        let mut rows: Vec<crate::command_palette::catalog::PaletteEntry> = self
            .iter()
            .filter(|e| visible(e.pane))
            .map(|entry| {
                let unseen_done = self.done_unseen.contains(&entry.pane);
                let mut state = if unseen_done {
                    "done ✓".to_string()
                } else {
                    entry.state.clone()
                };
                if matches!(entry.source, AgentSource::Scrape) {
                    state.push('~');
                }
                let mut label = format!("{}: {}", entry.agent, state);
                if let Some(reason) = entry.reason.as_ref() {
                    label.push_str(&format!(" — {reason}"));
                }
                crate::command_palette::catalog::PaletteEntry {
                    action_id: format!("agent-roster-focus:{}", entry.pane),
                    label,
                    chord: None,
                    // Blocked and unseen-done are the two "owes you a
                    // look" states; everything else keeps pane order.
                    priority: if entry.state.eq_ignore_ascii_case("blocked") || unseen_done {
                        2
                    } else {
                        1
                    },
                }
            })
            .collect();
        // Stable: blocked rows rise to the front, everything else keeps the
        // cache's pane order.
        rows.sort_by_key(|r| std::cmp::Reverse(r.priority));
        rows
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

    /// The `visible` every pre-scoping test used implicitly: all panes in
    /// scope. The scoping behavior itself has its own tests below.
    fn all_panes(_: TmuxPaneId) -> bool {
        true
    }

    /// Whether `pane` renders as done-unseen, read through the tooltip the
    /// way a user would (the cache keeps no queryable flag — the mark only
    /// exists to change what the surfaces say).
    fn marked_done_unseen(roster: &AgentRoster, pane: TmuxPaneId) -> bool {
        roster.tooltip_text(&all_panes).is_some_and(|text| {
            text.lines().any(|line| {
                line.contains(&format!("(pane {pane})")) && line.contains("done (unseen)")
            })
        })
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
    fn release_drops_the_pane_its_agent_claimed() {
        let mut roster = AgentRoster::new();
        roster.fill_from_list(vec![
            entry(0, "pi", "working", AgentSource::Hook),
            entry(3, "omp", "idle", AgentSource::Hook),
        ]);
        roster.apply_release(0, "pi");
        let panes: Vec<_> = roster.iter().map(|e| e.pane).collect();
        assert_eq!(panes, vec![3], "the released pane leaves, others stay");
    }

    #[test]
    fn release_for_a_superseded_claim_keeps_the_successor() {
        let mut roster = AgentRoster::new();
        roster.fill_from_list(vec![entry(2, "claude", "working", AgentSource::Hook)]);
        // A late release from the agent the pane no longer runs.
        roster.apply_release(2, "pi");
        let snapshot: Vec<_> = roster.iter().map(|e| e.pane).collect();
        assert_eq!(
            snapshot,
            vec![2],
            "a label mismatch must not drop the entry"
        );
    }

    #[test]
    fn release_for_an_unrostered_pane_is_a_noop() {
        let mut roster = AgentRoster::new();
        roster.fill_from_list(vec![entry(0, "kimi", "working", AgentSource::Hook)]);
        roster.apply_release(9, "kimi");
        let panes: Vec<_> = roster.iter().map(|e| e.pane).collect();
        assert_eq!(panes, vec![0]);
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

    #[test]
    fn summary_and_tooltip_hide_when_empty() {
        let roster = AgentRoster::new();
        assert_eq!(roster.summary_line(&all_panes), None);
        assert_eq!(roster.tooltip_text(&all_panes), None);
    }

    #[test]
    fn summary_groups_states_with_scrape_marker() {
        let mut roster = AgentRoster::new();
        roster.fill_from_list(vec![
            entry(0, "kimi", "working", AgentSource::Hook),
            entry(1, "claude", "blocked", AgentSource::Hook),
            entry(2, "pi", "blocked", AgentSource::Hook),
            entry(3, "omp", "working", AgentSource::Scrape),
        ]);
        // Largest group first; `~` marks the scrape-sourced count within a
        // group without doubling the widget's width.
        assert_eq!(
            roster.summary_line(&all_panes).as_deref(),
            Some("\u{1f465} 2 blocked, 1+1~ working")
        );
    }

    #[test]
    fn summary_pure_scrape_group_gets_bare_marker() {
        let mut roster = AgentRoster::new();
        roster.apply_push(entry(5, "grok", "waiting", AgentSource::Scrape));
        assert_eq!(
            roster.summary_line(&all_panes).as_deref(),
            Some("\u{1f465} 1~ waiting")
        );
    }

    #[test]
    fn summary_orders_same_size_groups_by_state() {
        let mut roster = AgentRoster::new();
        roster.fill_from_list(vec![
            entry(0, "kimi", "working", AgentSource::Hook),
            entry(1, "claude", "blocked", AgentSource::Hook),
        ]);
        assert_eq!(
            roster.summary_line(&all_panes).as_deref(),
            Some("\u{1f465} 1 blocked, 1 working")
        );
    }

    #[test]
    fn tooltip_lists_provenance_and_reason() {
        let mut roster = AgentRoster::new();
        roster.apply_push(entry(0, "kimi", "working", AgentSource::Hook));
        roster.apply_push(AgentEntry {
            pane: 4,
            agent: "pi".to_string(),
            state: "blocked".to_string(),
            source: AgentSource::Scrape,
            reason: Some("waiting on approval".to_string()),
        });
        assert_eq!(
            roster.tooltip_text(&all_panes).as_deref(),
            Some(
                "kimi · working · reported (pane 0)\npi · blocked · detected (pane 4) — waiting on approval"
            )
        );
    }

    #[test]
    fn palette_rows_empty_roster_yields_none() {
        assert!(AgentRoster::new().palette_rows(&all_panes).is_empty());
    }

    #[test]
    fn palette_rows_blocked_first_then_pane_order() {
        let mut roster = AgentRoster::new();
        roster.fill_from_list(vec![
            entry(0, "kimi", "working", AgentSource::Hook),
            entry(1, "claude", "blocked", AgentSource::Hook),
            entry(2, "omp", "idle", AgentSource::Hook),
            entry(3, "grok", "blocked", AgentSource::Hook),
        ]);
        let rows = roster.palette_rows(&all_panes);
        let ids: Vec<&str> = rows.iter().map(|r| r.action_id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "agent-roster-focus:1",
                "agent-roster-focus:3",
                "agent-roster-focus:0",
                "agent-roster-focus:2"
            ],
            "blocked agents lead; working/idle keep pane order behind them"
        );
    }

    #[test]
    fn palette_rows_label_carries_marker_and_reason() {
        let mut roster = AgentRoster::new();
        roster.apply_push(entry(0, "kimi", "working", AgentSource::Hook));
        roster.apply_push(AgentEntry {
            pane: 4,
            agent: "pi".to_string(),
            state: "blocked".to_string(),
            source: AgentSource::Scrape,
            reason: Some("waiting on approval".to_string()),
        });
        let rows = roster.palette_rows(&all_panes);
        // Blocked row leads; scrape marker and reason both render; hook rows
        // and reason-less rows degrade cleanly to the plain label.
        assert_eq!(rows[0].label, "pi: blocked~ — waiting on approval");
        assert_eq!(rows[1].label, "kimi: working");
        assert!(rows.iter().all(|r| r.chord.is_none()));
    }

    #[test]
    fn remove_panes_drops_only_the_listed() {
        let mut roster = AgentRoster::new();
        roster.fill_from_list(vec![
            entry(0, "kimi", "working", AgentSource::Hook),
            entry(1, "claude", "blocked", AgentSource::Hook),
            entry(2, "omp", "idle", AgentSource::Scrape),
        ]);
        roster.remove_panes(&[1]);
        let panes: Vec<_> = roster.iter().map(|e| e.pane).collect();
        assert_eq!(
            panes,
            vec![0, 2],
            "the closed pane's entry dies with it; survivors keep theirs"
        );
        // A pane that re-reports later re-enters normally (upsert).
        roster.apply_push(entry(1, "claude", "working", AgentSource::Hook));
        assert_eq!(roster.iter().count(), 3);
    }

    /// AC of the stale-entries card: the daemon's roster is cross-session
    /// and its pushes have no removal, so the SURFACES must scope to what
    /// the app can see and focus. Pane 5 is unmapped (another session's
    /// agent, or a pane closed before reconciliation) — it must not count,
    /// hover, or offer a focus row; pane 0 must render exactly as before.
    #[test]
    fn surfaces_scope_to_panes_the_app_maps() {
        let mut roster = AgentRoster::new();
        roster.fill_from_list(vec![
            entry(0, "kimi", "working", AgentSource::Hook),
            entry(5, "claude", "blocked", AgentSource::Hook),
        ]);
        let visible = |pane: TmuxPaneId| pane == 0;
        assert_eq!(
            roster.summary_line(&visible).as_deref(),
            Some("\u{1f465} 1 working"),
            "the unmapped pane must not count even when blocked"
        );
        assert_eq!(
            roster.tooltip_text(&visible).as_deref(),
            Some("kimi · working · reported (pane 0)")
        );
        let rows = roster.palette_rows(&visible);
        let ids: Vec<&str> = rows.iter().map(|r| r.action_id.as_str()).collect();
        assert_eq!(ids, ["agent-roster-focus:0"]);

        // Nothing in scope hides the widget/tooltip like an empty roster
        // — a bare glyph with no count would be a lie, not a scope.
        let none_visible = |_: TmuxPaneId| false;
        assert_eq!(roster.summary_line(&none_visible), None);
        assert_eq!(roster.tooltip_text(&none_visible), None);
        assert!(roster.palette_rows(&none_visible).is_empty());
    }

    /// AC of the done-unseen card: a `working` → `idle` transition in a
    /// pane the user has not focused shows as done/unseen in the widget
    /// and the picker, and clears when the pane is focused.
    #[test]
    fn working_to_idle_shows_done_unseen_until_seen() {
        let mut roster = AgentRoster::new();
        roster.apply_push(entry(0, "kimi", "working", AgentSource::Hook));
        roster.apply_push(entry(1, "omp", "idle", AgentSource::Hook));
        // The transition: kimi finishes while pane 1 (not kimi's) is focused.
        roster.apply_push(entry(0, "kimi", "idle", AgentSource::Hook));
        assert!(
            marked_done_unseen(&roster, 0),
            "the unseen finisher is marked"
        );
        assert!(
            !marked_done_unseen(&roster, 1),
            "plain idle was never working"
        );
        assert_eq!(
            roster.summary_line(&all_panes).as_deref(),
            Some("\u{1f465} 1 done, 1 idle"),
            "the widget groups the unseen finisher as done"
        );
        assert_eq!(
            roster.tooltip_text(&all_panes).as_deref(),
            Some("kimi · done (unseen) · reported (pane 0)\nomp · idle · reported (pane 1)")
        );
        let rows = roster.palette_rows(&all_panes);
        assert_eq!(
            rows[0].label, "kimi: done ✓",
            "the picker leads with the unseen finisher"
        );
        assert_eq!(rows[0].action_id, "agent-roster-focus:0");
        assert_eq!(rows[0].priority, 2, "done-unseen is an attention tier");

        // Focusing the pane is seeing it: every surface back to plain idle.
        roster.mark_seen(0);
        assert!(!marked_done_unseen(&roster, 0));
        assert_eq!(
            roster.summary_line(&all_panes).as_deref(),
            Some("\u{1f465} 2 idle")
        );
        assert_eq!(
            roster.palette_rows(&all_panes)[0].label,
            "kimi: idle",
            "pane order resumes once the mark clears (both idle, pane order)"
        );
    }

    #[test]
    fn only_a_real_working_to_idle_transition_marks() {
        let mut roster = AgentRoster::new();
        // blocked → idle is a release from attention, not a finish.
        roster.apply_push(entry(0, "pi", "blocked", AgentSource::Hook));
        roster.apply_push(entry(0, "pi", "idle", AgentSource::Hook));
        assert!(!marked_done_unseen(&roster, 0));
        // A fresh working push after being seen can finish again.
        roster.apply_push(entry(0, "pi", "working", AgentSource::Hook));
        roster.apply_push(entry(0, "pi", "idle", AgentSource::Hook));
        assert!(marked_done_unseen(&roster, 0));
        // working → blocked is attention, not done.
        roster.mark_seen(0);
        roster.apply_push(entry(0, "pi", "working", AgentSource::Hook));
        roster.apply_push(entry(0, "pi", "blocked", AgentSource::Hook));
        assert!(!marked_done_unseen(&roster, 0));
    }

    /// Every removal path drops the mark with the entry — a mark that
    /// outlived its pane would ghost the next agent on that pane id.
    #[test]
    fn removal_paths_drop_the_done_unseen_mark() {
        let mut roster = AgentRoster::new();
        roster.apply_push(entry(0, "kimi", "working", AgentSource::Hook));
        roster.apply_push(entry(0, "kimi", "idle", AgentSource::Hook));
        roster.remove_panes(&[0]);
        assert!(!marked_done_unseen(&roster, 0));
        roster.apply_push(entry(0, "kimi", "working", AgentSource::Hook));
        roster.apply_push(entry(0, "kimi", "idle", AgentSource::Hook));
        roster.apply_release(0, "kimi");
        assert!(!marked_done_unseen(&roster, 0));
        roster.apply_push(entry(0, "kimi", "working", AgentSource::Hook));
        roster.apply_push(entry(0, "kimi", "idle", AgentSource::Hook));
        roster.fill_from_list(vec![entry(0, "kimi", "idle", AgentSource::Hook)]);
        assert!(
            !marked_done_unseen(&roster, 0),
            "a fresh fill starts every pane seen"
        );
        roster.apply_push(entry(0, "kimi", "working", AgentSource::Hook));
        roster.apply_push(entry(0, "kimi", "idle", AgentSource::Hook));
        roster.clear();
        assert!(!marked_done_unseen(&roster, 0));
    }
}
