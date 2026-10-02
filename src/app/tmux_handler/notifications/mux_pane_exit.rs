//! par-mux held-pane lifecycle: `%pane-exited %N [code]` marks a daemon
//! pane whose process exited (the daemon HOLDS it — remain-on-exit — with
//! its frozen screen), `%pane-respawned %N` clears that state after a
//! `respawn-pane`. The exited-pane overlay and the restart key read
//! `TmuxState::mux_exited_panes`; restart sends `respawn-pane`.
//!
//! Both pushes are consumed string-level so the code compiles against a
//! core that predates them: the published pin parses them as
//! `Unknown { line }`, a newer core as named variants that `emit`
//! serializes back to the same wire line. An older daemon never sends
//! either, so the feature is dormant there.

use crate::app::window_state::WindowState;
use par_term_emu_core_rust::tmux_control::TmuxNotification as CoreNotification;
use par_term_tmux::TmuxPaneId;

/// One held-pane lifecycle push, parsed from its wire line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PaneLifecycle {
    /// The pane's process exited; `code` is `None` for a signal death or
    /// a code the daemon could not read.
    Exited { pane: TmuxPaneId, code: Option<i32> },
    /// The pane's process was restarted in place by `respawn-pane`.
    Respawned { pane: TmuxPaneId },
}

/// Parse a `%pane-exited %N [code]` / `%pane-respawned %N` wire line.
/// Anything else (including a malformed pane id) is `None`.
pub(crate) fn parse_pane_lifecycle_line(line: &str) -> Option<PaneLifecycle> {
    let mut fields = line.split_whitespace();
    let kind = fields.next()?;
    let pane = fields.next()?.strip_prefix('%')?.parse().ok()?;
    match kind {
        "%pane-exited" => Some(PaneLifecycle::Exited {
            pane,
            code: fields.next().and_then(|c| c.parse().ok()),
        }),
        "%pane-respawned" => Some(PaneLifecycle::Respawned { pane }),
        _ => None,
    }
}

/// The lifecycle push a core notification carries, if any: the raw line
/// of an `Unknown` (published pin), else the named variant re-serialized
/// by the core's own emitter (newer core). `emit` yields an empty string
/// for every variant it does not serialize, which parses to `None`.
pub(crate) fn pane_lifecycle_of(notification: &CoreNotification) -> Option<PaneLifecycle> {
    if let CoreNotification::Unknown { line } = notification {
        return parse_pane_lifecycle_line(line);
    }
    match notification.notification_type() {
        "pane-exited" | "pane-respawned" => {
            parse_pane_lifecycle_line(&par_term_emu_core_rust::mux::emit(notification))
        }
        _ => None,
    }
}

/// The `respawn-pane` command for a restart: a held (dead) pane restarts
/// with no flag; a live pane needs `-k` or the daemon refuses it as
/// "still running".
pub(crate) fn respawn_command(pane: TmuxPaneId, dead: bool) -> String {
    if dead {
        format!("respawn-pane -t %{pane}")
    } else {
        format!("respawn-pane -k -t %{pane}")
    }
}

impl WindowState {
    /// Apply lifecycle pushes. Runs before the output group of the same
    /// drain: a respawned pane's mirror reset must land before the new
    /// process's first output, or the reset would wipe it.
    pub(super) fn apply_pane_lifecycle(&mut self, events: Vec<PaneLifecycle>) -> bool {
        let mut needs_redraw = false;
        for event in events {
            match event {
                PaneLifecycle::Exited { pane, code } => {
                    log::info!("MUX: pane %{pane} exited (code {code:?}); held daemon-side");
                    self.tmux_state.mux_exited_panes.insert(pane, code);
                }
                PaneLifecycle::Respawned { pane } => {
                    log::info!("MUX: pane %{pane} respawned");
                    self.tmux_state.mux_exited_panes.remove(&pane);
                    self.reset_respawned_mux_mirror(pane);
                }
            }
            needs_redraw = true;
        }
        needs_redraw
    }

    /// A respawned pane runs a fresh daemon terminal, but the native
    /// mirror still holds the dead process's state (alt screen, cursor and
    /// keypad modes, mouse reporting). RIS resets it, then the daemon's
    /// current screen is re-seeded: output the new process wrote before
    /// this push arrived was already applied to the old state and is lost
    /// to the reset, so the seed is what restores it.
    fn reset_respawned_mux_mirror(&mut self, pane: TmuxPaneId) {
        if let Some((tab_id, native)) = self.tmux_state.tmux_pane_owner(pane)
            && let Some(tab) = self.tab_manager.get_tab_mut(tab_id)
            && let Some(pm) = tab.pane_manager_mut()
            && let Some(pane_obj) = pm.get_pane_mut(native)
        {
            // blocking_read, not try_read: a skipped reset would leave the
            // dead TUI's modes on the new shell's screen for good.
            pane_obj
                .terminal
                .blocking_read()
                .process_mux_output(b"\x1bc");
        }
        let Some(transport) = &self.tmux_state.transport else {
            return;
        };
        match transport.send_command(&format!("refresh-client -t %{pane}")) {
            Ok(reply) => {
                let mut bytes = b"\x1b[H\x1b[2J".to_vec();
                bytes.extend_from_slice(reply.join("\n").as_bytes());
                self.tmux_state.mux_screen_seeds.insert(pane, bytes);
                self.deliver_pending_mux_seed(pane);
            }
            Err(e) => log::warn!("par-mux: re-seed of respawned pane %{pane} failed: {e}"),
        }
    }

    /// Restart a daemon pane's process in place via `respawn-pane`. The
    /// exited chrome is NOT cleared here — only the `%pane-respawned`
    /// push clears it, so a rejected restart leaves the overlay truthful.
    /// Returns whether a command was sent.
    pub(crate) fn restart_mux_pane(&mut self, pane: TmuxPaneId) -> bool {
        let Some(transport) = &self.tmux_state.transport else {
            return false;
        };
        let dead = self.tmux_state.mux_exited_panes.contains_key(&pane);
        let cmd = respawn_command(pane, dead);
        let result = transport.send_command(&cmd);
        // Success is an empty body; an %error block (unknown command on a
        // pre-respawn daemon, "still running", a bad target) arrives as an
        // Ok body — non-empty IS the failure signal.
        match result {
            Ok(body) if body.is_empty() => {
                log::info!("MUX: sent {cmd}");
                true
            }
            Ok(body) => {
                let text = body.join("\n");
                log::error!("par-mux {cmd} rejected: {text}");
                self.record_mux_error(format!("par-mux: restart failed — {text}"));
                true
            }
            Err(e) => {
                log::error!("par-mux {cmd} failed: {e}");
                self.record_mux_error(format!("par-mux: restart failed — {e}"));
                true
            }
        }
    }

    /// Restart the focused daemon pane (palette row, Enter on a held pane).
    /// False when no mux pane is focused.
    pub(crate) fn restart_focused_mux_pane(&mut self) -> bool {
        match self.focused_mux_pane_from_native() {
            Some(pane) => self.restart_mux_pane(pane),
            None => false,
        }
    }

    /// Key routing for a focused HELD daemon pane: Enter restarts it and
    /// every other key is swallowed (the daemon's pane has no process to
    /// receive it) — the same contract as the local `RestartWithPrompt`
    /// prompt. `None` when the focused pane is not a held mux pane, so the
    /// caller routes the key normally.
    pub(crate) fn handle_key_for_exited_mux_pane(&mut self, bytes: &[u8]) -> Option<()> {
        let pane = self.focused_mux_pane_from_native()?;
        if !self.tmux_state.mux_exited_panes.contains_key(&pane) {
            return None;
        }
        if matches!(bytes, b"\r" | b"\n" | b"\r\n") {
            self.restart_mux_pane(pane);
        }
        Some(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::mux_test_seams::{SettledSend, quiesce, wait_until};
    use super::*;

    fn unknown(line: &str) -> CoreNotification {
        CoreNotification::Unknown {
            line: line.to_string(),
        }
    }

    #[test]
    fn parses_exited_with_and_without_a_code_and_respawned() {
        assert_eq!(
            parse_pane_lifecycle_line("%pane-exited %3 7"),
            Some(PaneLifecycle::Exited {
                pane: 3,
                code: Some(7)
            })
        );
        assert_eq!(
            parse_pane_lifecycle_line("%pane-exited %12\n"),
            Some(PaneLifecycle::Exited {
                pane: 12,
                code: None
            })
        );
        assert_eq!(
            parse_pane_lifecycle_line("%pane-exited %1 -1"),
            Some(PaneLifecycle::Exited {
                pane: 1,
                code: Some(-1)
            })
        );
        assert_eq!(
            parse_pane_lifecycle_line("%pane-respawned %4"),
            Some(PaneLifecycle::Respawned { pane: 4 })
        );
    }

    #[test]
    fn rejects_other_lines_and_malformed_ids() {
        for line in [
            "",
            "%pane-exited",
            "%pane-exited 3 0",
            "%pane-exited %x 0",
            "%pane-respawned",
            "%window-add @1",
            "%pane-exitedx %1",
        ] {
            assert_eq!(parse_pane_lifecycle_line(line), None, "{line:?}");
        }
    }

    /// The published pin delivers both pushes as `Unknown { line }`.
    #[test]
    fn unknown_notifications_carry_the_lifecycle_pushes() {
        assert_eq!(
            pane_lifecycle_of(&unknown("%pane-exited %0 42")),
            Some(PaneLifecycle::Exited {
                pane: 0,
                code: Some(42)
            })
        );
        assert_eq!(
            pane_lifecycle_of(&unknown("%pane-respawned %0")),
            Some(PaneLifecycle::Respawned { pane: 0 })
        );
        assert_eq!(pane_lifecycle_of(&unknown("%something-else %0")), None);
        assert_eq!(pane_lifecycle_of(&CoreNotification::SessionsChanged), None);
    }

    #[test]
    fn respawn_uses_k_only_for_a_live_pane() {
        assert_eq!(respawn_command(5, true), "respawn-pane -t %5");
        assert_eq!(respawn_command(5, false), "respawn-pane -k -t %5");
    }

    /// Pump the drain until `done` holds.
    fn pump_until(
        ws: &mut crate::app::window_state::WindowState,
        what: &str,
        done: impl Fn(&crate::app::window_state::WindowState) -> bool,
    ) {
        wait_until(what, || {
            done(ws) || {
                ws.check_mux_notifications();
                done(ws)
            }
        });
    }

    /// Criteria 1+2 end to end against the daemon surface that HOLDS an
    /// exited pane (core fea7bdc+): the shell exits with a known code, the
    /// `%pane-exited` push lands in the held state with that code while
    /// the pane stays mapped (the frozen screen is kept), the overlay
    /// gather reports "Process exited (code 7)", Enter sends `respawn-pane`
    /// without `-k`, `%pane-respawned` clears the chrome, and a restart of
    /// the now-live pane needs (and uses) `-k`.
    #[test]
    fn exited_pane_shows_code_and_restart_respawns_it() {
        use super::super::mux::MuxAttachPending;
        use super::super::mux::tests::{manners_state, socket_path, spawn_daemon};
        use super::super::mux_test_seams::attach_until_installed;

        let path = socket_path("pane-exit");
        spawn_daemon(&path);
        let mut ws = manners_state();
        attach_until_installed(
            &mut ws,
            || {
                let core_client =
                    par_term_emu_core_rust::mux::MuxClient::connect(&path).expect("connect");
                let (tx, rx) = std::sync::mpsc::channel();
                tx.send(Ok(core_client)).unwrap();
                drop(tx);
                MuxAttachPending {
                    name: "exitme".to_string(),
                    rx,
                }
            },
            &path,
        );
        ws.handle_tmux_window_add(0);
        wait_until("the layout consumer maps %0", || {
            ws.tmux_state
                .transport
                .as_ref()
                .unwrap()
                .send_command_no_wait("refresh-client -t %0 -C 80x24")
                .expect("size push");
            ws.check_mux_notifications();
            ws.tmux_state.tmux_pane_owners.contains_key(&0)
        });
        let send = |ws: &crate::app::window_state::WindowState, cmd: &str| {
            ws.tmux_state
                .transport
                .as_ref()
                .unwrap()
                .send_settled(cmd)
                .unwrap_or_else(|e| panic!("{cmd}: {e}"))
        };

        // The shell exits with a known code.
        send(&ws, "send-keys -t %0 -l 'exit 7'");
        send(&ws, "send-keys -t %0 Enter");
        pump_until(&mut ws, "%pane-exited for %0", |ws| {
            ws.tmux_state.mux_exited_panes.contains_key(&0)
        });
        assert_eq!(
            ws.tmux_state.mux_exited_panes.get(&0),
            Some(&Some(7)),
            "the held state carries the daemon-reported exit code"
        );
        // HELD, not removed: the daemon keeps the pane and the client keeps
        // its mapping (the frozen screen stays under the overlay).
        assert!(ws.tmux_state.tmux_pane_owners.contains_key(&0));
        let panes = send(&ws, "list-panes");
        assert!(
            panes.iter().any(|l| l.contains("%0")),
            "the daemon holds the dead pane: {panes:?}"
        );
        let banners = crate::app::render_pipeline::exited_pane_overlay::gather_exited_pane_banners(
            &ws.tmux_state,
            ws.tab_manager.active_tab(),
            1.0,
        );
        assert_eq!(banners.len(), 1, "one banner for the held pane");
        assert_eq!(banners[0].tmux_pane, 0);
        assert_eq!(
            crate::app::render_pipeline::exited_pane_overlay::exited_label(banners[0].code),
            "Process exited (code 7)"
        );

        // Clear the attach toast so a restart rejection toast is visible.
        ws.overlay_state.toasts.clear();

        // A non-Enter key on the held pane is swallowed, not routed.
        assert_eq!(ws.handle_key_for_exited_mux_pane(b"x"), Some(()));
        assert!(ws.tmux_state.mux_exited_panes.contains_key(&0));

        // Enter restarts: respawn-pane WITHOUT -k (the pane is dead). The
        // chrome clears only on the %pane-respawned push.
        quiesce(&mut ws);
        assert_eq!(ws.handle_key_for_exited_mux_pane(b"\r"), Some(()));
        assert_eq!(
            ws.last_toast_text(),
            None,
            "the dead-pane respawn was accepted"
        );
        pump_until(&mut ws, "%pane-respawned clears the chrome", |ws| {
            ws.tmux_state.mux_exited_panes.is_empty()
        });
        assert!(
            ws.tmux_state.tmux_pane_owners.contains_key(&0),
            "the respawned pane keeps its id and mapping"
        );
        assert_eq!(
            ws.handle_key_for_exited_mux_pane(b"\r"),
            None,
            "a live pane's keys route normally again"
        );

        // A live pane refuses a bare respawn (negative control for -k)...
        let refused = send(&ws, "respawn-pane -t %0");
        assert!(
            !refused.is_empty(),
            "a running pane must refuse respawn without -k"
        );
        // ...and the restart path uses -k for it.
        quiesce(&mut ws);
        assert!(ws.restart_focused_mux_pane());
        assert_eq!(
            ws.last_toast_text(),
            None,
            "the live-pane restart (-k) was accepted"
        );

        let _ = std::fs::remove_file(&path);
    }

    /// Attach through the app's install path and map window @0's %0 from
    /// the drain alone: the created session's %window-add waits in the
    /// client channel, and its adoption maps %0. A manual
    /// `handle_tmux_window_add(0)` (the ladder above) would add a second
    /// tab for @0 and make every window count wrong.
    fn attached_with_pane_zero(
        tag: &str,
    ) -> (crate::app::window_state::WindowState, std::path::PathBuf) {
        use super::super::mux::MuxAttachPending;
        use super::super::mux::tests::{manners_state, socket_path, spawn_daemon};

        let path = socket_path(tag);
        spawn_daemon(&path);
        let core_client = par_term_emu_core_rust::mux::MuxClient::connect(&path).expect("connect");
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(Ok(core_client)).unwrap();
        drop(tx);
        let mut ws = manners_state();
        ws.tmux_state.mux_attach_pending = Some(MuxAttachPending {
            name: tag.to_string(),
            rx,
        });
        ws.poll_mux_attach();
        assert!(ws.tmux_state.transport.is_some(), "attach must install");
        pump_until(&mut ws, "the %window-add adoption maps %0", |ws| {
            ws.tmux_state.tmux_pane_owners.contains_key(&0)
        });
        assert_eq!(ws.tab_manager.tab_count(), 1, "one tab for @0");
        (ws, path)
    }

    fn send_ok(ws: &crate::app::window_state::WindowState, cmd: &str) -> Vec<String> {
        ws.tmux_state
            .transport
            .as_ref()
            .unwrap()
            .send_settled(cmd)
            .unwrap_or_else(|e| panic!("{cmd}: {e}"))
    }

    /// Exit `pane`'s shell and pump until the daemon's `%pane-exited`
    /// holds it.
    fn exit_and_hold(ws: &mut crate::app::window_state::WindowState, pane: u64) {
        send_ok(ws, &format!("send-keys -t %{pane} -l 'exit 7'"));
        send_ok(ws, &format!("send-keys -t %{pane} Enter"));
        pump_until(ws, "%pane-exited", |ws| {
            ws.tmux_state.mux_exited_panes.contains_key(&pane)
        });
    }

    fn daemon_windows(ws: &crate::app::window_state::WindowState) -> Vec<u64> {
        send_ok(ws, "list-windows")
            .iter()
            .filter_map(|l| l.strip_prefix('@')?.split(':').next()?.parse().ok())
            .collect()
    }

    /// Card 01a0ef3b criterion 1: Cmd+W (close pane) on a tab whose only
    /// pane is HELD ends its daemon window — the last-pane close normally
    /// hides the tab and keeps the window running, which for a window of
    /// dead panes is a leak nobody can see. The tab then closes through the
    /// ordinary %window-close teardown.
    #[test]
    fn closing_a_held_last_pane_tab_kills_its_daemon_window() {
        let (mut ws, path) = attached_with_pane_zero("held-close");

        // A second window, so closing the held one does not end the session.
        send_ok(&ws, "new-window");
        pump_until(&mut ws, "window @1 mapped", |ws| {
            ws.tmux_state.tmux_pane_owners.contains_key(&1)
        });
        assert_eq!(daemon_windows(&ws), vec![0, 1]);

        exit_and_hold(&mut ws, 0);
        let (held_tab, _) = ws.tmux_state.tmux_pane_owner(0).unwrap();
        ws.tab_manager.switch_to(held_tab);
        assert!(ws.tmux_state.all_tab_panes_held(held_tab));

        quiesce(&mut ws);
        assert!(!ws.close_focused_pane(), "the window stays open");
        assert!(
            !ws.overlay_ui.mux_last_tab_ui.is_visible(),
            "another window survives, so no last-tab dialog"
        );
        assert_eq!(
            daemon_windows(&ws),
            vec![1],
            "the held window is killed daemon-side, not left running"
        );
        pump_until(&mut ws, "%window-close tears the held tab down", |ws| {
            ws.tab_manager.get_tab(held_tab).is_none()
        });
        assert!(
            ws.tmux_state.tmux_sync.get_tab(0).is_none(),
            "no stale window mapping"
        );
        let _ = std::fs::remove_file(&path);
    }

    /// The held-window kill must leave a VISIBLE tab active. With the only
    /// other tab hidden (its live last pane was closed, D7), the hide path
    /// used to re-show it; the kill path closes the held tab through the
    /// %window-close teardown, whose index-based pick knows nothing of
    /// hidden tabs.
    #[test]
    fn killing_a_held_tab_beside_a_hidden_tab_leaves_a_visible_tab_active() {
        let (mut ws, path) = attached_with_pane_zero("held-beside-hidden");
        send_ok(&ws, "new-window");
        pump_until(&mut ws, "window @1 mapped", |ws| {
            ws.tmux_state.tmux_pane_owners.contains_key(&1)
        });
        let (tab_a, _) = ws.tmux_state.tmux_pane_owner(0).unwrap();
        let (tab_b, _) = ws.tmux_state.tmux_pane_owner(1).unwrap();

        // Hide B: its (live) last pane closes, the D7 shape.
        ws.tab_manager.switch_to(tab_b);
        quiesce(&mut ws);
        assert!(!ws.close_focused_pane());
        assert!(ws.tab_manager.get_tab(tab_b).is_some_and(|t| t.is_hidden));
        assert_eq!(ws.tab_manager.active_tab_id(), Some(tab_a));

        // A's shell exits; Cmd+W on the held tab kills its window.
        exit_and_hold(&mut ws, 0);
        quiesce(&mut ws);
        assert!(!ws.close_focused_pane());
        pump_until(&mut ws, "the held tab tears down", |ws| {
            ws.tab_manager.get_tab(tab_a).is_none()
        });
        let active = ws.tab_manager.active_tab().expect("a tab stays active");
        assert_eq!(active.id, tab_b);
        assert!(
            !active.is_hidden,
            "the surviving tab must be re-shown, not left active-but-hidden"
        );
        let _ = std::fs::remove_file(&path);
    }

    /// The negative half: a held tab that is the session's ONLY window is
    /// not killed by the close key (that would end the session, which is
    /// kept and restored as fresh shells) — the last-tab dialog decides.
    /// A live last pane still hides as before (D7).
    #[test]
    fn closing_the_sessions_only_held_tab_asks_instead_of_killing() {
        let (mut ws, path) = attached_with_pane_zero("held-last");
        let (tab, _) = ws.tmux_state.tmux_pane_owner(0).unwrap();
        ws.tab_manager.switch_to(tab);
        assert!(
            !ws.tmux_state.all_tab_panes_held(tab),
            "a live pane is not held"
        );

        exit_and_hold(&mut ws, 0);
        quiesce(&mut ws);
        assert!(!ws.close_focused_pane(), "the dialog decides");
        assert!(
            ws.overlay_ui.mux_last_tab_ui.is_visible(),
            "closing the session's last (held) window asks first"
        );
        assert_eq!(daemon_windows(&ws), vec![0], "nothing was killed");
        assert!(ws.tab_manager.get_tab(tab).is_some_and(|t| !t.is_hidden));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn exited_then_respawned_sets_and_clears_the_held_state() {
        let mut ws = super::super::mux::tests::manners_state();
        assert!(ws.apply_pane_lifecycle(vec![PaneLifecycle::Exited {
            pane: 2,
            code: Some(3)
        }]));
        assert_eq!(ws.tmux_state.mux_exited_panes.get(&2), Some(&Some(3)));
        assert!(ws.apply_pane_lifecycle(vec![PaneLifecycle::Respawned { pane: 2 }]));
        assert!(ws.tmux_state.mux_exited_panes.is_empty());
    }
}
