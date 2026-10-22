//! par-mux auto-reconnect (`mux_auto_reconnect`): when the daemon
//! connection is lost, re-launch the daemon (`connect_or_spawn` inside the
//! ordinary attach), re-create the session, and restore the window set —
//! instead of the default behavior of closing the mux tabs and reporting.
//! Split from `mux.rs`, which owns the attach/detach lifecycle this
//! composes with: capture happens from the drain's daemon-death site, the
//! trigger re-runs [`WindowState::begin_mux_session_attach`], and the
//! install tail restores the windows.

use crate::app::window_state::WindowState;
use par_term_mux::MuxSessionClient;
use std::time::{Duration, Instant};

/// The pending reconnect: the session to recreate, the window titles to
/// restore (empty string = create an unnamed window at this position),
/// and the reconnect timestamps driving the crash-loop guard.
#[derive(Clone)]
pub(crate) struct MuxReconnect {
    pub(crate) session: String,
    pub(crate) windows: Vec<String>,
    pub(crate) attempts: Vec<Instant>,
}

/// Max auto-reconnect attempts allowed inside [`ATTEMPT_WINDOW`] — a
/// crash-looping daemon must not spin the attach machinery forever.
pub(crate) const MAX_ATTEMPTS: usize = 3;
/// Sliding window for [`MAX_ATTEMPTS`].
pub(crate) const ATTEMPT_WINDOW: Duration = Duration::from_secs(60);

impl WindowState {
    /// Capture what a reconnect will restore, from the drain's
    /// daemon-death site (`disconnected`) — BEFORE the synthesized
    /// `SessionEnded` tears the view down and closes the tabs. No-op when
    /// the option is off or no session is attached. A previous pending
    /// reconnect contributes only its attempt timestamps (the crash-loop
    /// guard survives), so a repeated loss cannot reset it.
    pub(crate) fn capture_mux_reconnect(&mut self) {
        if !self.config.load().tmux.mux_auto_reconnect {
            return;
        }
        let Some(session) = self.tmux_state.tmux_session_name.clone() else {
            return;
        };
        // Window titles in tab-bar order for the tabs mirroring daemon
        // windows. A placeholder or empty title ("par-mux @N" — the tab
        // was never named) restores as an unnamed window.
        let windows: Vec<String> = self
            .tab_manager
            .tabs()
            .iter()
            .filter_map(|tab| self.tmux_state.tmux_sync.get_window(tab.id).map(|_| tab))
            .map(|tab| {
                let title = tab.title.trim();
                let placeholder = title.is_empty()
                    || title.starts_with("par-mux @")
                    || title.starts_with("tmux @");
                (if placeholder { "" } else { title }).to_string()
            })
            .collect();
        if windows.is_empty() {
            return;
        }
        let attempts = self
            .tmux_state
            .mux_reconnect
            .take()
            .map(|pending| pending.attempts)
            .unwrap_or_default();
        crate::debug_info!(
            "MUX",
            "daemon lost — captured reconnect: session '{session}', {} window(s)",
            windows.len()
        );
        self.tmux_state.mux_reconnect = Some(MuxReconnect {
            session,
            windows,
            attempts,
        });
    }

    /// Fire the captured reconnect, called at the end of
    /// [`Self::check_mux_notifications`] — after the teardown has closed
    /// the tabs and cleared the attach state, so
    /// [`Self::begin_mux_session_attach`] sees a free window. Guarded
    /// against a crash-looping daemon: at most [`MAX_ATTEMPTS`] triggers
    /// inside [`ATTEMPT_WINDOW`], then give up and report.
    pub(crate) fn trigger_mux_reconnect(&mut self) {
        let Some(pending) = self.tmux_state.mux_reconnect.as_ref() else {
            return;
        };
        let now = Instant::now();
        let mut attempts: Vec<Instant> = pending
            .attempts
            .iter()
            .copied()
            .filter(|t| now.duration_since(*t) < ATTEMPT_WINDOW)
            .collect();
        if attempts.len() >= MAX_ATTEMPTS {
            let session = pending.session.clone();
            self.tmux_state.mux_reconnect = None;
            log::error!(
                "MUX: auto-reconnect for '{session}' gave up after {MAX_ATTEMPTS} attempts in 60 s"
            );
            self.record_mux_error(format!(
                "par-mux: auto-reconnect gave up after {MAX_ATTEMPTS} attempts in 60 s — \
                 start the daemon and attach to '{session}' manually"
            ));
            return;
        }
        attempts.push(now);
        let session = pending.session.clone();
        if let Some(pending) = self.tmux_state.mux_reconnect.as_mut() {
            pending.attempts = attempts;
        }
        crate::debug_info!("MUX", "auto-reconnect: re-attaching to '{session}'");
        self.begin_mux_session_attach(&session);
    }

    /// The attach install-time decision: take the pending reconnect when
    /// this attach superseded it (any name — a manual attach wins) and,
    /// when the session was CREATED (a dead daemon's sessions die with it,
    /// so only then is the window set gone), return the captured titles to
    /// restore. Also clears the "connection lost" chip error: the
    /// connection is back.
    pub(crate) fn consume_mux_reconnect_windows(
        &mut self,
        name: &str,
        outcome: &par_term_mux::AttachOutcome,
    ) -> Option<Vec<String>> {
        let pending = self.tmux_state.mux_reconnect.take()?;
        if pending.session != name {
            log::info!(
                "MUX: manual attach to '{name}' superseded the pending reconnect to '{}'",
                pending.session
            );
            return None;
        }
        self.tmux_state.mux_last_error = None;
        // Attached: the session survived (the daemon never died) — its
        // windows are already tabbed by the attach's existing-windows loop.
        if !matches!(outcome, par_term_mux::AttachOutcome::Created(_)) {
            return None;
        }
        Some(pending.windows)
    }
}

/// Recreate the window set on the fresh daemon: the created session
/// already holds one window (the session's first), so rename it to the
/// first captured title and create the rest. Names travel with the window
/// (`-n`), and a follow-up `rename-window` makes the daemon's canonical
/// name match. Returns the `(window id, title)` pairs the client must
/// apply to its own tabs: a rename push lands in the same drain batch as
/// its window's `%window-add`, before the tab maps, and `TmuxSync` drops
/// renames for unmapped windows (the reason the ordinary attach carries
/// its `window_names` at install instead of relying on pushes).
pub(crate) fn restore_windows(
    client: &mut MuxSessionClient,
    windows: &[String],
) -> Vec<(u64, String)> {
    let existing = match client.list_windows() {
        Ok(existing) => existing,
        Err(e) => {
            log::error!("MUX: reconnect window restore — list-windows failed: {e}");
            return Vec::new();
        }
    };
    let mut titled = Vec::new();
    for (i, name) in windows.iter().enumerate() {
        if let Some(window) = existing.get(i) {
            // The fresh session's own first window: give it the captured
            // title. Its daemon-side name is the session name until told.
            if !name.is_empty() {
                send_restore(client, &format!("rename-window -t @{} {}", window.id, name));
                titled.push((window.id, name.clone()));
            }
            continue;
        }
        let command = if name.is_empty() {
            "new-window".to_string()
        } else {
            format!("new-window -n {}", quote(name))
        };
        let reply = match client.send(&command) {
            Ok(reply) => reply,
            Err(e) => {
                log::error!("MUX: reconnect window restore — {command:?} failed: {e}");
                break;
            }
        };
        // The daemon answers with the window target form `@N`.
        let Some(id) = reply.iter().find_map(|line| {
            line.trim()
                .strip_prefix('@')
                .and_then(|rest| rest.parse::<u64>().ok())
        }) else {
            continue; // window created; the tab keeps its placeholder title
        };
        if !name.is_empty() {
            send_restore(client, &format!("rename-window -t @{id} {name}"));
            titled.push((id, name.clone()));
        }
    }
    titled
}

/// Send one restore command, logging (never propagating) failures — a
/// failed rename costs a placeholder title, not the restoration.
fn send_restore(client: &mut MuxSessionClient, command: &str) {
    if let Err(e) = client.send(command) {
        log::error!("MUX: reconnect window restore — {command:?} failed: {e}");
    }
}

/// Quote a name for a daemon value flag: `quoted_flag` reads the value
/// as a shell word and strips the quotes. Not for `rename-window` — its
/// name is the RAW trailing text of the line, so quoting would become
/// part of the name.
fn quote(name: &str) -> String {
    par_term_mux::quote_env_value(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pending(session: &str, windows: &[&str]) -> MuxReconnect {
        MuxReconnect {
            session: session.to_string(),
            windows: windows.iter().map(|w| w.to_string()).collect(),
            attempts: Vec::new(),
        }
    }

    #[test]
    fn guard_prunes_attempts_outside_the_window_and_counts_inside_it() {
        let now = Instant::now();
        let mut attempts = vec![
            now - Duration::from_secs(120), // outside the window
            now - Duration::from_secs(10),
        ];
        attempts.retain(|t| now.duration_since(*t) < ATTEMPT_WINDOW);
        assert_eq!(attempts.len(), 1);
        attempts.push(now);
        assert_eq!(attempts.len(), 2);
    }

    #[test]
    fn guard_gives_up_at_max_attempts() {
        let now = Instant::now();
        let attempts: Vec<Instant> = (0..MAX_ATTEMPTS)
            .map(|i| now - Duration::from_secs(10 * (i as u64 + 1)))
            .collect();
        assert!(attempts.len() >= MAX_ATTEMPTS);
    }

    #[test]
    fn pending_state_carries_session_windows_and_attempts() {
        let p = pending("dev", &["claude", "", "zsh"]);
        assert_eq!(p.session, "dev");
        assert_eq!(
            p.windows,
            vec!["claude".to_string(), String::new(), "zsh".to_string()]
        );
        assert!(p.attempts.is_empty());
    }

    // The daemon-driven tests below reuse the wiring tests' in-process
    // daemon and attach seams. Gated on unix like every daemon-driven test.
    #[cfg(unix)]
    mod daemon_driven {
        use super::super::super::mux::tests::{manners_state, socket_path, spawn_daemon};
        use super::super::super::mux_test_seams::{
            SettledSend, attached_window_with_tabs, wait_until,
        };
        use super::super::*;

        /// Enable `mux_auto_reconnect` on a test window.
        fn enable_reconnect(ws: &mut crate::app::window_state::WindowState) {
            let mut cfg = crate::config::Config::default();
            cfg.tmux.mux_auto_reconnect = true;
            ws.config.store(std::sync::Arc::new(cfg));
        }

        /// Rename every mapped daemon window to the given names and pump
        /// until the tabs carry them — the real titles a daemon loss
        /// captures. The rename travels raw: `rename-window` takes its
        /// name as trailing text.
        fn name_windows_and_wait(ws: &mut crate::app::window_state::WindowState, names: &[&str]) {
            let rows = ws
                .tmux_state
                .transport
                .as_ref()
                .expect("attached")
                .send_settled("list-windows")
                .expect("list-windows");
            let ids: Vec<u64> = rows
                .iter()
                .filter_map(|row| {
                    row.trim()
                        .strip_prefix('@')
                        .and_then(|rest| rest.split(':').next())
                        .and_then(|id| id.parse().ok())
                })
                .collect();
            assert_eq!(ids.len(), names.len(), "the seam's window count");
            for (id, name) in ids.iter().zip(names) {
                ws.tmux_state
                    .transport
                    .as_ref()
                    .expect("attached")
                    .send_settled(&format!("rename-window -t @{id} {name}"))
                    .expect("rename");
            }
            wait_until("tabs carry the renamed titles", || {
                ws.check_tmux_notifications();
                let tabs = ws.tab_manager.tabs();
                tabs.len() == names.len()
                    && tabs.iter().zip(names).all(|(t, name)| t.title == *name)
            });
        }

        /// The capture side of a daemon loss, against a real attached
        /// window: three daemon windows named through `rename-window` are
        /// captured in tab order with their titles.
        #[test]
        fn capture_holds_the_window_set_and_titles() {
            let (mut ws, _path) = attached_window_with_tabs("recon-capture", 3);
            enable_reconnect(&mut ws);
            name_windows_and_wait(&mut ws, &["title-0", "title-1", "title-2"]);

            // The drain calls this at its `disconnected` site, before the
            // teardown closes the tabs; the loss itself is the drain's
            // tested input.
            ws.capture_mux_reconnect();
            let rc = ws
                .tmux_state
                .mux_reconnect
                .as_ref()
                .expect("the daemon loss is captured")
                .clone();
            assert_eq!(rc.session, "recon-capture");
            assert_eq!(
                rc.windows,
                vec![
                    "title-0".to_string(),
                    "title-1".to_string(),
                    "title-2".to_string()
                ]
            );
            assert!(rc.attempts.is_empty());
        }

        /// The reconnect install: a FRESH daemon (a dead daemon's sessions
        /// die with it, so the reconnect lands as Created), the teardown,
        /// and the window set restored — count and titles. The two honest
        /// seams: the capture runs directly (its `disconnected` call site
        /// is one reviewed line) and the fresh daemon is a second
        /// in-process server rather than `connect_or_spawn`'s child (the
        /// worker talks to the user's runtime dir; tests never do).
        #[test]
        fn the_reconnect_restores_the_window_set_on_a_fresh_daemon() {
            let (mut ws, _old) = attached_window_with_tabs("recon-flow", 3);
            enable_reconnect(&mut ws);
            name_windows_and_wait(&mut ws, &["title-0", "title-1", "title-2"]);

            // Daemon loss, in the drain's order: the dead socket drops,
            // capture, then the shared teardown (the synthesized
            // SessionEnded routes here) closes the tabs and records the
            // chip error.
            let _ = ws.tmux_state.transport.take();
            ws.capture_mux_reconnect();
            ws.tmux_state.mux_last_error = Some("par-mux: daemon connection lost".to_string());
            ws.handle_tmux_session_ended_for_test();
            assert_eq!(
                ws.tab_manager.tab_count(),
                0,
                "the teardown closes the daemon tabs"
            );
            assert!(ws.tmux_state.transport.is_none());

            // The reconnect attach: a FRESH daemon, armed the same way the
            // by-name worker delivers its client.
            let fresh = socket_path("recon-flow-fresh");
            spawn_daemon(&fresh);
            let core = par_term_emu_core_rust::mux::MuxClient::connect(&fresh).expect("connect");
            let (tx, rx) = std::sync::mpsc::channel();
            tx.send(Ok(core)).unwrap();
            drop(tx);
            ws.tmux_state.mux_attach_pending = Some(super::super::super::mux::MuxAttachPending {
                name: "recon-flow".to_string(),
                rx,
            });
            ws.poll_mux_attach();

            // The install consumed the pending reconnect (Created →
            // restore) and cleared the chip error.
            assert!(ws.tmux_state.transport.is_some());
            assert!(ws.tmux_state.mux_reconnect.is_none());
            assert!(ws.tmux_state.mux_last_error.is_none());

            // The restored window set: three tabs with the captured
            // titles, arriving through the ordinary %window-add/rename
            // pushes.
            wait_until("the restored window set", || {
                ws.check_tmux_notifications();
                let tabs = ws.tab_manager.tabs();
                tabs.len() == 3
                    && tabs
                        .iter()
                        .filter(|t| t.title.starts_with("title-"))
                        .count()
                        == 3
            });
        }

        /// The restore round trip keeps names verbatim: `rename-window`
        /// takes its name as RAW trailing text and the daemon never strips
        /// quotes, so a shell-quoted name would come back with the quote
        /// characters as part of the restored window's name. Pinned here
        /// with a spaced name and one holding a single quote.
        #[test]
        fn restored_window_names_survive_the_round_trip_without_quotes() {
            let names = ["title with spaces", "it's quoted"];
            let (mut ws, _old) = attached_window_with_tabs("recon-quote", names.len());
            enable_reconnect(&mut ws);
            name_windows_and_wait(&mut ws, &names);

            // Daemon loss, in the drain's order: drop the dead socket,
            // capture, then the shared teardown closes the tabs.
            let _ = ws.tmux_state.transport.take();
            ws.capture_mux_reconnect();
            ws.tmux_state.mux_last_error = Some("par-mux: daemon connection lost".to_string());
            ws.handle_tmux_session_ended_for_test();

            // The reconnect attach against a FRESH daemon, armed the same
            // way the by-name worker delivers its client.
            let fresh = socket_path("recon-quote-fresh");
            spawn_daemon(&fresh);
            let core = par_term_emu_core_rust::mux::MuxClient::connect(&fresh).expect("connect");
            let (tx, rx) = std::sync::mpsc::channel();
            tx.send(Ok(core)).unwrap();
            drop(tx);
            ws.tmux_state.mux_attach_pending = Some(super::super::super::mux::MuxAttachPending {
                name: "recon-quote".to_string(),
                rx,
            });
            ws.poll_mux_attach();

            // The captured titles come back exactly as they left — no
            // quote characters added around or inside them. The daemon's
            // canonical window names are the oracle: the tabs take their
            // titles from the restore's return value (the rename pushes
            // drop before the tab maps), so only `list-windows` shows
            // what the daemon actually stored.
            wait_until("the restored names round-trip verbatim", || {
                ws.check_tmux_notifications();
                let tabs = ws.tab_manager.tabs();
                if !(tabs.len() == names.len()
                    && tabs
                        .iter()
                        .zip(names.iter())
                        .all(|(t, name)| t.title == *name))
                {
                    return false;
                }
                let rows = ws
                    .tmux_state
                    .transport
                    .as_ref()
                    .expect("attached")
                    .send_settled("list-windows")
                    .expect("list-windows");
                let daemon_names: Vec<&str> = rows
                    .iter()
                    .filter_map(|row| row.trim().split_once(": ").map(|(_, name)| name))
                    .collect();
                daemon_names.len() == names.len()
                    && daemon_names
                        .iter()
                        .zip(names)
                        .all(|(got, want)| *got == want)
            });
        }

        /// Below the attempt ceiling the trigger keeps the pending state,
        /// records the attempt, and arms the attach — here the busy guard
        /// of `begin_mux_session_attach` absorbs the call (a pending
        /// attach is already armed), so no daemon is spawned by-name.
        #[test]
        fn a_trigger_below_the_ceiling_records_the_attempt() {
            let mut ws = manners_state();
            ws.tmux_state.mux_reconnect = Some(MuxReconnect {
                session: "recon-guard".to_string(),
                windows: vec!["a".to_string()],
                attempts: Vec::new(),
            });
            let (_tx, rx) = std::sync::mpsc::channel::<
                std::io::Result<par_term_emu_core_rust::mux::MuxClient>,
            >();
            drop(_tx); // the busy guard absorbs the attach before any poll reads this
            ws.tmux_state.mux_attach_pending = Some(
                crate::app::tmux_handler::notifications::mux::MuxAttachPending {
                    name: "recon-guard".to_string(),
                    rx,
                },
            );

            ws.trigger_mux_reconnect();

            let rc = ws
                .tmux_state
                .mux_reconnect
                .as_ref()
                .expect("below the ceiling the state survives")
                .clone();
            assert_eq!(rc.attempts.len(), 1, "the attempt is recorded");
            assert!(ws.tmux_state.mux_attach_pending.is_some());
        }

        /// At the ceiling the guard gives up: the pending state drops, the
        /// chip error names the give-up, and no attach is armed.
        #[test]
        fn the_guard_gives_up_instead_of_looping() {
            let mut ws = manners_state();
            let now = std::time::Instant::now();
            ws.tmux_state.mux_reconnect = Some(MuxReconnect {
                session: "recon-loop".to_string(),
                windows: vec!["a".to_string()],
                attempts: (0..MAX_ATTEMPTS).map(|_| now).collect(),
            });

            ws.trigger_mux_reconnect();

            assert!(ws.tmux_state.mux_reconnect.is_none());
            assert!(ws.tmux_state.mux_attach_pending.is_none());
            assert!(
                ws.tmux_state
                    .mux_last_error
                    .as_deref()
                    .is_some_and(|e| e.contains("gave up")),
                "the chip error names the give-up, got {:?}",
                ws.tmux_state.mux_last_error
            );
        }
    }
}
