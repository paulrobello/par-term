//! Local-pane agent detection for the agent roster (the mux-by-default
//! follow-up: daemon panes report through the wire, LOCAL (non-mux) panes
//! have no daemon — their agents are detected from the OS process tree).
//!
//! A pane counts as local when the daemon does not map it: its
//! `(tab id, pane id)` pair is not a `tmux_pane_owners` value. Detection
//! reuses [`TerminalManager::get_running_child_processes`] — the same OS
//! process-table inspection the close-confirmation path uses — over the
//! pane shell's direct children, matched by exact executable name against
//! [`par_term_mux::ADOPTABLE_KINDS`] plus the hook agents `pi`/`omp`
//! (their binaries are as recognizable by name as the scrape-tier kinds).
//!
//! ## Honesty contract
//!
//! Process detection sees a running agent binary, nothing else. A local
//! row is always `working` + scrape-sourced, never blocked/question/done
//! (those are hook states only daemon panes carry) and never done-unseen
//! (the synthetic namespace never enters `done_unseen`). The surfaces
//! render the limit in plain words instead of faking a state.
//!
//! ## Cadence and placement
//!
//! A scan starts at most once every [`DETECTION_INTERVAL`], off the UI
//! thread (one worker per scan); at most one scan is in flight. The UI
//! thread only polls the landed result, filters it to panes still local,
//! and reconciles the roster's synthetic namespace via
//! [`AgentRoster::set_local_agents`].

use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use par_term_terminal::TerminalManager;

use super::agent_roster::AgentRoster;
use crate::pane::PaneId;
use crate::tab::{TabId, TabManager};
use par_term_tmux::TmuxPaneId;

/// How often a detection scan may start. Process-table inspection is not
/// free, so detection is eventually consistent (seconds), never per-frame.
const DETECTION_INTERVAL: Duration = Duration::from_secs(3);

/// The per-window detector: the throttle and the one in-flight scan.
pub(crate) struct LocalRosterDetector {
    last_scan_start: Option<Instant>,
    pending: Option<mpsc::Receiver<Vec<(u64, u64, String)>>>,
}

impl LocalRosterDetector {
    pub(crate) fn new() -> Self {
        Self {
            last_scan_start: None,
            pending: None,
        }
    }

    /// Whether a scan is out and has not landed yet (test seam).
    #[cfg(test)]
    pub(crate) fn scan_in_flight(&self) -> bool {
        self.pending.is_some()
    }

    /// One upkeep tick: land a finished scan, then start a new one when the
    /// interval has elapsed and none is in flight. Process-table inspection
    /// and terminal-lock waits never run here — the caller is the event
    /// loop's upkeep, so everything is `try`/non-blocking by construction.
    ///
    /// `owners` is the daemon pane map; its values are the `(tab, pane)`
    /// pairs the daemon feeds, which detection must skip (a mapped pane's
    /// agent is the daemon row's content — detection never shadows it).
    /// Returns whether the roster's local namespace changed (a redraw
    /// signal for the surfaces).
    pub(crate) fn poll(
        &mut self,
        tabs: &TabManager,
        owners: &std::collections::HashMap<TmuxPaneId, (TabId, PaneId)>,
        roster: &mut AgentRoster,
    ) -> bool {
        let local: Vec<(u64, u64, Arc<tokio::sync::RwLock<TerminalManager>>)> = tabs
            .tabs()
            .iter()
            .filter(|tab| !tab.tmux.tmux_gateway_active)
            .flat_map(|tab| {
                tab.pane_manager().into_iter().flat_map(move |pm| {
                    pm.all_panes().into_iter().filter_map(move |pane| {
                        if owners.values().any(|owner| *owner == (tab.id, pane.id)) {
                            None
                        } else {
                            Some((tab.id, pane.id, Arc::clone(&pane.terminal)))
                        }
                    })
                })
            })
            .collect();
        let mut changed = false;
        if let Some(rx) = self.pending.take() {
            match rx.try_recv() {
                Ok(mut detected) => {
                    // The scan started against an earlier pane set: keep only
                    // rows whose pane is still local right now, then apply
                    // wholesale (the absent-means-absent rule, local side).
                    detected.retain(|(tab, pane, _)| {
                        local.iter().any(|(t, p, _)| (*t, *p) == (*tab, *pane))
                    });
                    changed = roster.set_local_agents(detected);
                }
                // The worker died without sending — the scan is simply over.
                Err(mpsc::TryRecvError::Disconnected) => {}
                Err(mpsc::TryRecvError::Empty) => self.pending = Some(rx),
            }
        }
        if self.pending.is_some() {
            return changed;
        }
        // Nothing local: the whole synthetic namespace dies now, without a
        // worker (an in-flight result, if any, would be retained to nothing).
        if local.is_empty() {
            if roster.has_local() {
                roster.set_local_agents(Vec::new());
                return true;
            }
            return changed;
        }
        if self
            .last_scan_start
            .is_some_and(|last| last.elapsed() < DETECTION_INTERVAL)
        {
            return changed;
        }
        self.last_scan_start = Some(Instant::now());
        let (tx, rx) = mpsc::channel();
        self.pending = Some(rx);
        std::thread::spawn(move || {
            let mut detected: Vec<(u64, u64, String)> = Vec::new();
            for (tab, pane, terminal) in local {
                // Blocking reads are confined to this worker. try_read is
                // not used here: the renderer may hold the lock briefly on
                // every frame, and a skipped pane would report nothing for
                // that whole cycle.
                let children: Vec<String> =
                    terminal.blocking_read().get_running_child_processes(&[]);
                if let Some(agent) = detected_agent(&children) {
                    detected.push((tab, pane, agent.to_string()));
                }
            }
            let _ = tx.send(detected);
        });
        changed
    }
}

/// Match a pane's shell children against the agent executables the product
/// knows. Exact basename only: `claude-helper` is not Claude.
fn detected_agent(children: &[String]) -> Option<&'static str> {
    par_term_mux::ADOPTABLE_KINDS
        .iter()
        .copied()
        .chain(["pi", "omp"])
        .find(|kind| {
            children.iter().any(|name| {
                name.strip_suffix(".exe")
                    .unwrap_or(name)
                    .eq_ignore_ascii_case(kind)
            })
        })
}

/// Focus the local pane behind a synthetic roster id (`agent-roster-focus`
/// dispatch, local branch). Mirrors the daemon branch's flow — unhide a
/// hidden tab (UX.md M4), switch over visible-tab order (UX.md M13), focus
/// the pane — without the daemon-facing input sync a local pane needs
/// nothing of.
pub(crate) fn focus_local_roster_pane(
    ws: &mut crate::app::window_state::WindowState,
    encoded: par_term_tmux::TmuxPaneId,
) -> bool {
    let Some((tab_id, pane_id)) = AgentRoster::decode_local_pane(encoded) else {
        return false;
    };
    let owner_tab_id = ws.tab_manager.tabs().iter().find_map(|tab| {
        let owns = tab.id == tab_id
            && tab
                .pane_manager()
                .is_some_and(|pm| pm.get_pane(pane_id).is_some());
        owns.then_some(tab.id)
    });
    let Some(owner_tab_id) = owner_tab_id else {
        return false;
    };
    // D7 (UX.md M4): the owning tab may be hidden; selecting the agent is a
    // request to see that pane again, so the tab re-shows.
    if let Some(tab) = ws.tab_manager.get_tab_mut(owner_tab_id) {
        tab.is_hidden = false;
    }
    // 1-based over VISIBLE tabs (the switch_to_tab_index contract, UX.md M13).
    let Some(visible_index) = ws
        .tab_manager
        .visible_tabs()
        .iter()
        .position(|t| t.id == owner_tab_id)
    else {
        return false;
    };
    ws.switch_to_tab_index(visible_index + 1);
    if let Some(tab) = ws.tab_manager.active_tab_mut()
        && let Some(pm) = tab.pane_manager_mut()
    {
        pm.focus_pane(pane_id);
    }
    ws.focus_state.needs_redraw = true;
    ws.request_redraw();
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_agent_executables_by_exact_name() {
        assert_eq!(detected_agent(&["claude".to_string()]), Some("claude"));
        assert_eq!(detected_agent(&["OMP.exe".to_string()]), Some("omp"));
        assert_eq!(
            detected_agent(&["claude-helper".to_string(), "node".to_string()]),
            None,
            "substring matches would fake agents that are not there"
        );
        assert_eq!(detected_agent(&[]), None);
    }
}
