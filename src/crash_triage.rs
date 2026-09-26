//! Crash triage: capture a crashed pane's facts and hand them to the
//! default agent on an explicit user action.
//!
//! Omarchy-F5 port (card `01a0d8da6c1f`): when a pane's process dies
//! non-zero, its exit code and recent output are already par-term's data —
//! capture is local-only and automatic. *Sending* anything to an agent is
//! the user's click: the sole surface is a `Triage crash: …` command-palette
//! row (the palette opens on user command, so nothing steals focus) plus a
//! passive toast announcing the capture. Activating the row launches the
//! default agent (the first `agents:` entry with `default: true`) with a
//! prompt that names the exit status and a payload file holding the full
//! tail — always a plain launch, never the autonomous variant.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use par_term_config::{PaneId, TabId};

/// Payload-file numbering is process-global: states are per-window, and
/// same-numbered files from two states would overwrite each other in the
/// shared temp dir.
static PAYLOAD_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A crash detected before any window exists (the previous run's panic
/// snapshot, consumed at session-restore time). Stashed process-globally
/// because [`CrashTriageState`] is per-window; the first window's state
/// adopts it.
struct StashedCrash {
    label: String,
    exit_code: i32,
    facts: String,
    tail: String,
}

static STARTUP_CRASH: std::sync::Mutex<Option<StashedCrash>> = std::sync::Mutex::new(None);

/// Stash a pre-window crash for the first [`CrashTriageState`] to adopt.
pub fn stash_startup_crash(label: String, exit_code: i32, facts: String, tail: String) {
    let mut slot = STARTUP_CRASH
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    // Keep the first: the report from the run that just died outranks
    // anything a later consumer in the same process might stash.
    if slot.is_none() {
        *slot = Some(StashedCrash {
            label,
            exit_code,
            facts,
            tail,
        });
    }
}

/// Offers older than this stop appearing in the palette (the user moved on;
/// the payload file stays on disk for manual inspection).
const OFFER_TTL: Duration = Duration::from_secs(15 * 60);
/// Live offers kept at once — a crash-looping pane must not flood the palette.
const MAX_OFFERS: usize = 5;
/// Lines of exported terminal text kept in the payload file.
const TAIL_LINES: usize = 40;

/// One captured crash, pending the user's triage decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrashOffer {
    /// Dispatch id: `triage-crash:<id>`.
    pub id: u64,
    /// Human label naming the pane that died.
    pub label: String,
    /// The child's exit code (signal deaths surface as non-zero; portable-pty
    /// reports 1 for them, so any death this object exists for is non-zero).
    pub exit_code: i32,
    /// Path of the markdown payload file handed to the agent.
    pub payload_path: PathBuf,
    created: Instant,
}

/// Window-wide triage state: live offers plus the pane-exit dedupe set.
///
/// The dedupe set is keyed by `(TabId, PaneId)` because the capture pass runs
/// every frame — without it, a pane kept dead by `shell_exit_action: keep`
/// would be re-captured on every redraw until the tab closes.
#[derive(Debug, Default)]
pub struct CrashTriageState {
    offers: Vec<CrashOffer>,
    next_id: u64,
    captured: HashSet<(TabId, PaneId)>,
}

impl CrashTriageState {
    /// Whether this pane's exit was already processed (offered or judged clean).
    pub(crate) fn already_captured(&self, key: (TabId, PaneId)) -> bool {
        self.captured.contains(&key)
    }

    /// Record one pane exit. A zero exit code is marked captured without an
    /// offer — a clean `exit` is not a crash. Returns the offer when one was
    /// created.
    pub(crate) fn record(
        &mut self,
        key: (TabId, PaneId),
        label: String,
        exit_code: i32,
        shell_line: &str,
        tail_text: &str,
    ) -> Option<&CrashOffer> {
        if !self.captured.insert(key) {
            return None; // already processed on an earlier frame
        }
        if exit_code == 0 {
            return None;
        }
        let payload_path = self.write_payload(&label, exit_code, shell_line, tail_text)?;
        Some(self.push_offer(label, exit_code, payload_path))
    }

    /// Record a crash from a source other than a pane exit (plugin
    /// crash-cap, previous-run panic). Event-driven rather than
    /// frame-driven, so it never touches the pane dedupe set.
    pub(crate) fn record_external(
        &mut self,
        label: String,
        exit_code: i32,
        facts: &str,
        tail_text: &str,
    ) -> Option<&CrashOffer> {
        let payload_path = self.write_external_payload(&label, exit_code, facts, tail_text)?;
        Some(self.push_offer(label, exit_code, payload_path))
    }

    /// Bookkeeping shared by every offer source: id, TTL expiry, and the
    /// live-offer cap.
    fn push_offer(&mut self, label: String, exit_code: i32, payload_path: PathBuf) -> &CrashOffer {
        self.next_id += 1;
        self.offers.push(CrashOffer {
            id: self.next_id,
            label,
            exit_code,
            payload_path,
            created: Instant::now(),
        });
        // Drop the oldest past the cap; expire lazy entries on the same pass.
        self.expire();
        while self.offers.len() > MAX_OFFERS {
            self.offers.remove(0);
        }
        self.offers.last().expect("just pushed")
    }

    /// Drop offers past their TTL (the palette calls this; expiry is a
    /// presentation concern, so nothing else needs a timer).
    pub(crate) fn expire(&mut self) {
        let now = Instant::now();
        self.offers
            .retain(|offer| now.duration_since(offer.created) < OFFER_TTL);
    }

    /// Drop dedupe keys for tabs that no longer exist, so a recycled
    /// `TabId`-`PaneId` pair in a future tab is captured again.
    pub(crate) fn forget_tabs_except(&mut self, live: &HashSet<TabId>) {
        self.captured.retain(|(tab_id, _)| live.contains(tab_id));
    }

    /// Drop the dedupe keys of panes observed running again: a respawned
    /// pane keeps its id and terminal, so without this its next crash would
    /// be swallowed by the key from the previous episode.
    pub(crate) fn clear_captured(&mut self, keys: &[(TabId, PaneId)]) {
        for key in keys {
            self.captured.remove(key);
        }
    }

    /// Consume an offer on activation (`triage-crash:<id>` dispatch).
    pub(crate) fn take(&mut self, id: u64) -> Option<CrashOffer> {
        let index = self.offers.iter().position(|offer| offer.id == id)?;
        Some(self.offers.remove(index))
    }

    /// Record a plugin crash-cap (the supervisor gave up on a crash-looping
    /// entry) as a triage offer — the same palette row a crashed pane gets.
    pub(crate) fn record_plugin_crash_cap(
        &mut self,
        cap: &par_term_scripting::plugin_manager::PluginCrashCap,
    ) -> Option<&CrashOffer> {
        let label = format!("plugin '{}' ({} entry)", cap.plugin_id, cap.kind);
        let facts = format!(
            "- Source: plugin crash-loop ({} restart attempts, supervisor gave up)\n\
             - Entry: {}\n- Restart policy: {}",
            par_term_scripting::restart::MAX_RESTART_ATTEMPTS,
            cap.entry,
            cap.restart_mode,
        );
        let tail = cap.stderr_tail.join("\n");
        self.record_external(label, 1, &facts, &tail)
    }

    /// Adopt the stashed startup crash, if any (first caller wins).
    pub(crate) fn adopt_startup_crash(&mut self) {
        let stash = STARTUP_CRASH
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        if let Some(crash) = stash {
            self.record_external(crash.label, crash.exit_code, &crash.facts, &crash.tail);
        }
    }

    /// The live offers (TTL-pruned), newest first.
    pub(crate) fn live_offers(&mut self) -> &[CrashOffer] {
        self.expire();
        self.offers.as_slice()
    }

    fn write_external_payload(
        &mut self,
        label: &str,
        exit_code: i32,
        facts: &str,
        tail_text: &str,
    ) -> Option<PathBuf> {
        let n = PAYLOAD_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("par_term_triage_{}_{n}.md", std::process::id()));
        let tail: Vec<&str> = tail_text.lines().rev().take(TAIL_LINES).collect::<Vec<_>>();
        let tail = tail.into_iter().rev().collect::<Vec<_>>();
        let body = format!(
            "# Crash triage: {label}\n\n\
             - Captured: {time}\n\
             - Exit status: {exit_code}\n\
             {facts}\n\n\
             ## Last output\n\n```\n{tail}\n```\n",
            time = chrono::Local::now().format("%Y-%m-%d %H:%M:%S %Z"),
            tail = tail.join("\n"),
        );
        std::fs::write(&path, body)
            .map_err(|e| {
                log::warn!(
                    "crash triage: could not write payload {}: {e}",
                    path.display()
                );
                e
            })
            .ok()?;
        Some(path)
    }

    fn write_payload(
        &mut self,
        label: &str,
        exit_code: i32,
        shell_line: &str,
        tail_text: &str,
    ) -> Option<PathBuf> {
        let n = PAYLOAD_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("par_term_triage_{}_{n}.md", std::process::id()));
        let tail: Vec<&str> = tail_text.lines().rev().take(TAIL_LINES).collect::<Vec<_>>();
        let tail = tail.into_iter().rev().collect::<Vec<_>>();
        let body = format!(
            "# Crash triage: {label}\n\n\
             - Exited: {time}\n\
             - Exit status: {exit_code} (non-zero; signal deaths report 1)\n\
             - Process: {shell_line}\n\n\
             ## Last output\n\n```\n{tail}\n```\n",
            time = chrono::Local::now().format("%Y-%m-%d %H:%M:%S %Z"),
            tail = tail.join("\n"),
        );
        std::fs::write(&path, body)
            .map_err(|e| {
                log::warn!(
                    "crash triage: could not write payload {}: {e}",
                    path.display()
                );
                e
            })
            .ok()?;
        Some(path)
    }

    /// Palette rows for the live offers — the consent surface. Nothing is
    /// sent to any agent until one of these rows is activated.
    pub(crate) fn palette_entries(&mut self) -> Vec<crate::command_palette::catalog::PaletteEntry> {
        self.live_offers()
            .iter()
            .map(|offer| crate::command_palette::catalog::PaletteEntry {
                action_id: format!("triage-crash:{}", offer.id),
                label: format!("Triage crash: {} (exit {})", offer.label, offer.exit_code),
                chord: None,
                priority: 2,
            })
            .collect()
    }
}

/// Quote `text` for the POSIX shell the agent command line is typed into.
/// `'…'` quoting with the `'"'"'` escape keeps every byte literal.
pub(crate) fn shell_single_quote(text: &str) -> String {
    if text.is_empty() {
        return "''".to_string();
    }
    let mut out = String::with_capacity(text.len() + 2);
    out.push('\'');
    for ch in text.chars() {
        if ch == '\'' {
            out.push_str("'\"'\"'");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}

/// The prompt appended to the default agent's command for a triage launch.
pub(crate) fn triage_prompt(offer: &CrashOffer) -> String {
    format!(
        "A terminal process crashed: {label} exited with status {code}. \
         Diagnose it using the facts in {path} (process, exit status, recent output).",
        label = offer.label,
        code = offer.exit_code,
        path = offer.payload_path.display(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(tab: u64, pane: u64) -> (TabId, PaneId) {
        (tab, pane)
    }

    #[test]
    fn shell_single_quote_is_literal() {
        assert_eq!(shell_single_quote("plain"), "'plain'");
        assert_eq!(shell_single_quote("it's"), "'it'\"'\"'s'");
        assert_eq!(shell_single_quote("a$b `c` \"d\""), "'a$b `c` \"d\"'");
        assert_eq!(shell_single_quote(""), "''");
    }

    #[test]
    fn record_zero_exit_marks_captured_without_offer() {
        let mut state = CrashTriageState::default();
        assert!(
            state
                .record(key(1, 1), "tab pane 1".into(), 0, "zsh", "")
                .is_none()
        );
        assert!(state.offers.is_empty());
        assert!(
            state.already_captured(key(1, 1)),
            "clean exit is deduped too"
        );
    }

    #[test]
    fn record_nonzero_creates_offer_with_payload_tail() {
        let mut state = CrashTriageState::default();
        let offer = state
            .record(
                key(1, 2),
                "mytab pane 2".into(),
                42,
                "/bin/zsh",
                "l1\nl2\nl3",
            )
            .unwrap()
            .clone();
        assert_eq!(offer.exit_code, 42);
        assert_eq!(state.live_offers().len(), 1);
        let body = std::fs::read_to_string(&offer.payload_path).unwrap();
        assert!(body.contains("Exit status: 42"), "payload: {body}");
        assert!(body.contains("/bin/zsh"));
        assert!(body.contains("l3"), "tail keeps last lines: {body}");
        // Re-recording the same pane is a no-op (dedupe).
        assert!(
            state
                .record(key(1, 2), "mytab pane 2".into(), 42, "/bin/zsh", "x")
                .is_none()
        );
        assert_eq!(state.live_offers().len(), 1);
    }

    #[test]
    fn record_caps_live_offers() {
        let mut state = CrashTriageState::default();
        for pane in 0..(MAX_OFFERS + 3) as u64 {
            state.record(key(1, pane), format!("pane {pane}"), 1, "sh", "tail");
        }
        assert_eq!(state.live_offers().len(), MAX_OFFERS);
        // Newest survived.
        assert_eq!(state.live_offers().last().unwrap().label, "pane 7");
    }

    #[test]
    fn take_consumes_and_unknown_id_is_none() {
        let mut state = CrashTriageState::default();
        let offer = state
            .record(key(1, 1), "t".into(), 3, "sh", "tail")
            .unwrap()
            .clone();
        assert!(state.take(999).is_none());
        assert_eq!(state.take(offer.id).unwrap().id, offer.id);
        assert!(state.live_offers().is_empty());
        assert!(state.take(offer.id).is_none());
    }

    #[test]
    fn palette_rows_only_for_live_offers() {
        let mut state = CrashTriageState::default();
        assert!(state.palette_entries().is_empty());
        let offer = state
            .record(key(1, 1), "tab pane 1".into(), 7, "sh", "x")
            .unwrap()
            .clone();
        let rows = state.palette_entries();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].action_id, format!("triage-crash:{}", offer.id));
        assert!(rows[0].label.contains("exit 7"));
    }

    #[test]
    fn clear_captured_lets_a_respawned_pane_offer_again() {
        let mut state = CrashTriageState::default();
        let k = key(1, 1);
        state.record(k, "t".into(), 9, "sh", "x").unwrap();
        assert!(state.record(k, "t".into(), 9, "sh", "x").is_none());
        // The pane restarted and is running again.
        state.clear_captured(&[k]);
        assert!(state.record(k, "t".into(), 9, "sh", "x").is_some());
        assert_eq!(state.live_offers().len(), 2);
    }

    #[test]
    fn forget_tabs_except_prunes_dedupe_keys() {
        let mut state = CrashTriageState::default();
        state.record(key(1, 1), "t".into(), 1, "sh", "x");
        state.record(key(2, 1), "t2".into(), 1, "sh", "x");
        let mut live = HashSet::new();
        live.insert(2u64);
        state.forget_tabs_except(&live);
        assert!(!state.already_captured(key(1, 1)));
        assert!(state.already_captured(key(2, 1)));
    }

    /// The plugin crash-cap trigger, unit-level: the supervisor's give-up
    /// event becomes the same palette row a crashed pane gets.
    #[test]
    fn plugin_crash_cap_surfaces_the_same_triage_row() {
        use par_term_scripting::plugin_manager::PluginCrashCap;
        let cap = PluginCrashCap {
            plugin_id: "com.example.clock".to_string(),
            kind: "widget",
            entry: "/plugins/com.example.clock/widget.py".to_string(),
            restart_mode: "Always".to_string(),
            stderr_tail: vec!["Traceback (most recent call last):".to_string()],
        };
        let mut state = CrashTriageState::default();
        let offer = state.record_plugin_crash_cap(&cap).unwrap().clone();
        assert_eq!(offer.label, "plugin 'com.example.clock' (widget entry)");
        let rows = state.palette_entries();
        assert_eq!(rows.len(), 1, "the crash-cap surfaces a palette row");
        assert!(
            rows[0]
                .label
                .contains("Triage crash: plugin 'com.example.clock'"),
            "{}",
            rows[0].label
        );
        let body = std::fs::read_to_string(&offer.payload_path).unwrap();
        assert!(body.contains("supervisor gave up"), "payload: {body}");
        assert!(body.contains("Restart policy"));
        assert!(body.contains("Traceback"), "stderr tail kept: {body}");
        let prompt = triage_prompt(&offer);
        assert!(prompt.contains("plugin 'com.example.clock'"));
    }

    #[test]
    fn record_external_offers_without_a_pane_key() {
        let mut state = CrashTriageState::default();
        let offer = state
            .record_external(
                "plugin 'clock' (widget entry)".into(),
                1,
                "- Source: plugin crash-loop (5 restart attempts in the grace window)\n- Entry: /bin/false",
                "e1\ne2",
            )
            .unwrap()
            .clone();
        let rows = state.palette_entries();
        assert_eq!(
            rows.len(),
            1,
            "external offers surface the same palette row"
        );
        assert!(
            rows[0].label.contains("plugin 'clock'"),
            "{}",
            rows[0].label
        );
        let body = std::fs::read_to_string(&offer.payload_path).unwrap();
        assert!(body.contains("5 restart attempts"), "payload: {body}");
        assert!(body.contains("/bin/false"));
        assert!(body.contains("e2"), "tail keeps last lines: {body}");
        // External sources are event-driven: recording again is a new
        // episode, not a dedupe hit.
        assert!(
            state
                .record_external(
                    "plugin 'clock' (widget entry)".into(),
                    1,
                    "- Source: x",
                    "y"
                )
                .is_some()
        );
    }

    /// The startup stash, state-level: session restore stashes the previous
    /// run's panic and the first restored window adopts it —
    /// `adopt_startup_crash` is called from `restore_session`, not from
    /// `WindowState::new`, so ordinary window construction in other tests
    /// can never steal it. One test because the stash is process-global.
    #[test]
    fn stashed_startup_crash_becomes_a_live_offer_in_the_first_window() {
        stash_startup_crash(
            "previous par-term run".into(),
            101,
            "- Source: the previous run ended in a panic (snapshot: 2 windows)".to_string(),
            "thread 'main' panicked at 'boom'".to_string(),
        );
        let mut first = CrashTriageState::default();
        first.adopt_startup_crash();
        let offers = first.live_offers().to_vec();
        assert_eq!(offers.len(), 1, "the first window adopts the stash");
        assert_eq!(offers[0].exit_code, 101);
        assert_eq!(offers[0].label, "previous par-term run");
        let body = std::fs::read_to_string(&offers[0].payload_path).unwrap();
        assert!(body.contains("panicked at 'boom'"), "payload: {body}");
        let rows = first.palette_entries();
        assert_eq!(rows.len(), 1);
        assert!(
            rows[0]
                .label
                .contains("Triage crash: previous par-term run"),
            "{}",
            rows[0].label
        );

        let mut second = CrashTriageState::default();
        second.adopt_startup_crash();
        assert!(
            second.live_offers().is_empty(),
            "only the first window adopts the stash"
        );
    }

    #[test]
    fn triage_prompt_names_status_and_path() {
        let mut state = CrashTriageState::default();
        let offer = state
            .record(key(1, 1), "build pane 3".into(), 139, "zsh", "x")
            .unwrap()
            .clone();
        let prompt = triage_prompt(&offer);
        assert!(prompt.contains("status 139"));
        assert!(prompt.contains(offer.payload_path.display().to_string().as_str()));
        // The prompt must survive being embedded in a shell command line.
        let quoted = shell_single_quote(&prompt);
        assert!(!quoted.contains('\\'));
    }

    /// End-to-end through the real shell paths (no mux): a pane whose
    /// process exits non-zero is captured by `handle_shell_exit`, its
    /// payload keeps the tail, and activating the palette row launches the
    /// default agent with the exit status and payload path typed after its
    /// command.
    #[test]
    fn crashed_pane_is_captured_and_triaged_into_the_default_agent() {
        use std::time::{Duration, Instant};

        // A multi-thread runtime: this test pumps real local shells, and the
        // core's PTY reader needs background task progress a dormant
        // current-thread runtime never makes.
        let runtime = std::sync::Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("build test runtime"),
        );

        fn agent_config() -> crate::config::Config {
            crate::config::Config {
                agents: vec![par_term_config::agent_launcher::AgentLaunchConfig {
                    id: "probe".to_string(),
                    name: "Probe".to_string(),
                    command: "echo PAR_TERM_TRIAGE_RAN".to_string(),
                    autonomy_args: String::new(),
                    default: true,
                }],
                ..Default::default()
            }
        }

        let mut ws = crate::app::window_state::WindowState::new(agent_config(), runtime.clone());
        // A spare tab so the victim's close does not end the "window".
        let cfg = ws.config.load();
        ws.tab_manager
            .new_tab(&cfg, runtime.clone(), false, Some((80, 24)))
            .expect("spare tab");
        drop(cfg);

        // The victim: a custom shell that prints a marker and exits 42.
        let mut crash_cfg = agent_config();
        crash_cfg.shell.custom_shell = Some("/bin/sh".to_string());
        crash_cfg.shell.shell_args = Some(vec![
            "-c".to_string(),
            "echo PAR_TERM_CRASH_TAIL; exit 42".to_string(),
        ]);
        ws.config.store(std::sync::Arc::new(crash_cfg));
        let cfg = ws.config.load();
        let victim = ws
            .tab_manager
            .new_tab(&cfg, runtime.clone(), false, Some((80, 24)))
            .expect("victim tab");
        drop(cfg);

        // Wait for the victim's process to die.
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let running = ws
                .tab_manager
                .get_tab(victim)
                .and_then(|tab| tab.try_with_read_terminal(|term| term.is_running()))
                .unwrap_or(false);
            if !running {
                break;
            }
            assert!(Instant::now() < deadline, "victim pane never exited");
            std::thread::sleep(Duration::from_millis(50));
        }

        // The production close path: capture runs inside, then the tab closes.
        assert!(!ws.handle_shell_exit());
        assert_eq!(ws.tab_manager.tab_count(), 1, "victim tab closed");

        let offers = ws.crash_triage.live_offers().to_vec();
        assert_eq!(offers.len(), 1, "exactly one crash offer");
        assert_eq!(offers[0].exit_code, 42);
        let payload = std::fs::read_to_string(&offers[0].payload_path).unwrap();
        assert!(payload.contains("Exit status: 42"), "payload: {payload}");
        assert!(
            payload.contains("PAR_TERM_CRASH_TAIL"),
            "payload keeps the tail: {payload}"
        );

        // Back to a real shell so the agent tab lives to run its command.
        ws.config.store(std::sync::Arc::new(agent_config()));
        let action = format!("triage-crash:{}", offers[0].id);
        assert!(ws.execute_keybinding_action(&action));
        assert_eq!(
            ws.tab_manager.tab_count(),
            2,
            "the triage launch opened a tab"
        );
        assert!(ws.crash_triage.live_offers().is_empty(), "offer consumed");

        // The typed line = agent command + quoted prompt naming the exit
        // status; both render once the echo runs.
        let tab_id = ws.tab_manager.active_tab().unwrap().id;
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let seen = ws.tab_manager.get_tab(tab_id).and_then(|tab| {
                tab.try_with_read_terminal(|term| {
                    crate::app::window_state::search_highlight::get_all_searchable_lines(
                        term,
                        term.dimensions().1,
                    )
                    .map(|(_, line)| line)
                    .collect::<Vec<_>>()
                })
            });
            if let Some(lines) = seen {
                let text = lines.join("\n");
                if text.contains("PAR_TERM_TRIAGE_RAN") && text.contains("status 42") {
                    break;
                }
                if Instant::now() >= deadline {
                    panic!("triage command never ran or lost the status: {text:?}");
                }
            } else if Instant::now() >= deadline {
                panic!("agent tab never produced readable output");
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
