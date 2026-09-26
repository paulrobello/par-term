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
        self.offers.last()
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

    /// Consume an offer on activation (`triage-crash:<id>` dispatch).
    pub(crate) fn take(&mut self, id: u64) -> Option<CrashOffer> {
        let index = self.offers.iter().position(|offer| offer.id == id)?;
        Some(self.offers.remove(index))
    }

    /// The live offers (TTL-pruned), newest first.
    pub(crate) fn live_offers(&mut self) -> &[CrashOffer] {
        self.expire();
        self.offers.as_slice()
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
}
