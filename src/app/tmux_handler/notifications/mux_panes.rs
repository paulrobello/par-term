//! par-mux daemon-side pane operations: split, split-with-command, close,
//! swap, and agent launch, each sent through the transport so the
//! `%layout-change` consumer creates or removes the native pane. Split from
//! `mux.rs`, which owns the attach/detach lifecycle and input routing.

use super::mux_attach::stamp_pane_session_id;
use crate::app::window_state::WindowState;
use crate::pane::NavigationDirection;

/// What [`WindowState::launch_agent_via_mux`] did with a launch attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MuxLaunchOutcome {
    /// No transport attached / no daemon pane resolvable — the caller
    /// falls through to the local launch path.
    NotMux,
    /// The agent command was typed into a fresh daemon pane.
    Launched,
    /// A daemon target resolved but a wire step failed (toast shown) —
    /// consumed, never fall through to a local tab beside the daemon.
    Failed,
}

impl WindowState {
    /// Split the focused par-mux pane daemon-side: targeted `split-window`
    /// via the transport (the command the tmux gateway writes to its PTY),
    /// then let the %layout-change consumer create the native pane and
    /// mapping — the same flow the gateway split path relies on. Returns
    /// false when no transport is attached or the command fails, so
    /// callers fall through rather than leaving a daemon/local mismatch.
    /// On success the reply's new pane id becomes the focused pane, so
    /// subsequent input lands in the freshly split pane.
    pub(crate) fn split_pane_via_mux(&mut self, vertical: bool) -> bool {
        self.split_pane_via_mux_placed(vertical, false)
    }

    /// [`Self::split_pane_via_mux`] with `-b` when `before` is set: the new
    /// pane lands left of / above the focused one (UX.md A5; the daemon has
    /// parsed `split-window -b` since core 0.57).
    pub(crate) fn split_pane_via_mux_placed(&mut self, vertical: bool, before: bool) -> bool {
        let Some(transport) = &self.tmux_state.transport else {
            return false;
        };
        // The daemon REQUIRES -t on split-window (untargeted is a wire
        // error), and mux_focused_pane is unset after a fresh attach until
        // a click or focus push — fall back to the focused native pane,
        // exactly as input routing does. No mux pane focused (a local tab)
        // returns false so the caller proceeds with a local split.
        let Some(target) = self.focused_mux_pane_from_native() else {
            crate::debug_trace!("MUX", "split skipped — no mux pane focused (local tab?)");
            return false;
        };
        // tmux's -h is a side-by-side split (par-term "vertical"); -v stacks.
        stamp_pane_session_id(transport.as_ref(), self.tmux_state.mux_session_id);
        let flag = if vertical { "-h" } else { "-v" };
        let place = if before { " -b" } else { "" };
        let cmd = format!("split-window {flag}{place} -t %{target}");
        match transport.send_command(&cmd) {
            Ok(reply) => {
                if let Some(id) = reply
                    .iter()
                    .find_map(|line| line.trim().strip_prefix('%').and_then(|s| s.parse().ok()))
                {
                    self.tmux_state.mux_focused_pane = Some(id);
                    true
                } else {
                    // The daemon rejects bad splits with an %error block,
                    // which arrives as an Ok reply body — a missing pane id
                    // IS the failure signal. Consume (a daemon pane was the
                    // target; a local split would strand it) and surface it.
                    let body = reply.join("\n");
                    log::error!("par-mux split-window rejected: {body}");
                    self.record_persistent_mux_error(format!("par-mux: split failed — {body}"));
                    true
                }
            }
            Err(e) => {
                log::error!("par-mux split-window failed: {e}");
                self.record_persistent_mux_error(format!("par-mux: split failed — {e}"));
                // M6: a resolved daemon target means the split was
                // consumed — false here falls through to a LOCAL split and
                // strands an unmapped pane inside the mux tab.
                true
            }
        }
    }

    /// Snippet/trigger SplitPane actions' mux arm: split the focused
    /// daemon pane and type the command into the new pane's shell, the
    /// same launch mechanism [`Self::launch_agent_via_mux`] uses (the
    /// daemon wire has no initial-command, split-percent, or focus form —
    /// the daemon's defaults apply and the new pane becomes focused, as
    /// in [`Self::split_pane_via_mux`]). Delayed commands send
    /// immediately: the delay exists to let a local shell start, while
    /// the daemon buffers `send-keys` for a pane whose shell is not yet
    /// ready (proven by the agent-launch e2e). Returns false when no
    /// transport is attached or the focused native pane is not a daemon
    /// mirror (a local tab), so the caller keeps its local split. Once a
    /// daemon target has resolved the call is consumed regardless of
    /// outcome — a native fallback split would strand an unmapped local
    /// pane inside the mux tab.
    pub(crate) fn split_pane_with_command_via_mux(
        &mut self,
        vertical: bool,
        command: Option<&str>,
    ) -> bool {
        let Some(transport) = &self.tmux_state.transport else {
            return false;
        };
        let Some(target) = self.focused_mux_pane_from_native() else {
            return false;
        };
        stamp_pane_session_id(transport.as_ref(), self.tmux_state.mux_session_id);
        let flag = if vertical { "-h" } else { "-v" };
        let split = transport.send_command(&format!("split-window {flag} -t %{target}"));
        let pane = match &split {
            Ok(reply) => reply.iter().find_map(|line| {
                line.trim()
                    .strip_prefix('%')
                    .and_then(|s| s.parse::<u64>().ok())
            }),
            Err(e) => {
                log::error!("par-mux split-window failed: {e}");
                self.record_persistent_mux_error(format!("par-mux: split failed — {e}"));
                return true;
            }
        };
        let Some(pane) = pane else {
            // An %error block arrives as an Ok reply body with no pane id.
            let body = split
                .as_ref()
                .ok()
                .map(|r| r.join("\n"))
                .unwrap_or_default();
            log::error!("par-mux split-window rejected: {body}");
            self.record_persistent_mux_error(format!("par-mux: split failed — {body}"));
            return true;
        };
        self.tmux_state.mux_focused_pane = Some(pane);
        if let Some(command) = command.filter(|c| !c.is_empty()) {
            let keys = [
                format!(
                    "send-keys -t %{pane} -l {}",
                    par_term_mux::quote_env_value(command)
                ),
                format!("send-keys -t %{pane} Enter"),
            ];
            for cmd in keys {
                if let Err(e) = transport.send_command(&cmd) {
                    log::error!("par-mux split send-keys failed: {e}");
                    self.record_mux_error(format!("par-mux: split command failed — {e}"));
                    break;
                }
            }
        }
        log::info!("MUX: split pane %{pane} for a split action");
        true
    }

    /// Close the focused par-mux pane daemon-side — the mirror of
    /// [`Self::split_pane_via_mux`]: targeted `kill-pane` via the
    /// transport, then the daemon's `%layout-change` broadcast drives the
    /// layout consumer's removal path (`handle_pane_removal`) exactly as
    /// the gateway close flow does. Returns true when the close was
    /// consumed here (a daemon pane was the target — a native close would
    /// delete the local pane and dangle the tmux→native mapping while the
    /// daemon pane lives on); false when no transport is attached or no
    /// mux pane is focused, so the caller falls through to the local
    /// close.
    ///
    /// Unlike the split path, a TRANSPORT-level failure still consumes: a
    /// resolved daemon mapping means the pane's identity is owned
    /// daemon-side, so locally closing it on a dead connection races the
    /// SessionEnded teardown and can strand the mapping either way.
    pub(crate) fn close_pane_via_mux(&mut self) -> bool {
        let Some(transport) = &self.tmux_state.transport else {
            return false;
        };
        // Same resolution as input routing and the split path: mux focus
        // when set, else the focused native pane's mapping.
        let Some(target) = self.focused_mux_pane_from_native() else {
            crate::debug_trace!("MUX", "close skipped — no mux pane focused (local tab?)");
            return false;
        };
        let cmd = format!("kill-pane -t %{target}");
        let result = transport.send_command(&cmd);
        let ok = result.as_ref().map(|body| body.is_empty()).unwrap_or(false);
        match result {
            // kill-pane's success reply is an empty body; the daemon
            // rejects a bad TARGET with an %error block, which also
            // arrives as an Ok body — non-empty IS the failure signal,
            // same shape as the split path. A valid target is always
            // killed, including a window's last pane (which closes the
            // window — the last-pane guard in `close_focused_pane` keeps
            // par-term from sending that).
            Ok(body) if !ok => {
                let text = body.join("\n");
                log::error!("par-mux kill-pane rejected: {text}");
                self.record_mux_error(format!("par-mux: close failed — {text}"));
                true
            }
            Ok(_) => true,
            Err(e) => {
                log::error!("par-mux kill-pane failed: {e}");
                self.record_mux_error(format!("par-mux: close failed — {e}"));
                true
            }
        }
    }

    /// Swap the focused par-mux pane with its directional neighbor
    /// daemon-side: targeted `swap-pane -t %f -s %n` via the transport.
    /// The neighbor is resolved from the native mirror tree's bounds — the
    /// same `find_pane_in_direction` the local swap path uses — and both
    /// ids map through `tmux_pane_in_tab` to daemon pane ids. The daemon's
    /// swap reply carries a layout broadcast, so the %layout-change
    /// consumer re-lays-out the mirror exactly as split/close flows do.
    ///
    /// Returns true when the swap was consumed here (a daemon pane was the
    /// target — a local swap of the mirror would fight the daemon's next
    /// layout push and desync the pane map). False when no transport is
    /// attached, no pane is focused, or no neighbor lies that way — the
    /// caller then falls through to the local swap.
    pub(crate) fn swap_pane_via_mux(&mut self, direction: NavigationDirection) -> bool {
        let Some(transport) = self.tmux_state.transport.as_ref() else {
            return false;
        };
        // Resolve everything under immutable borrows first: `send_command`
        // and `show_toast` need `self` again below.
        let resolved = (|| {
            let tab = self.tab_manager.active_tab()?;
            let pm = tab.pane_manager()?;
            let focused = pm.focused_pane_id()?;
            let neighbor = pm.neighbor_in_direction(focused, direction)?;
            let f_mux = self.tmux_state.tmux_pane_in_tab(tab.id, focused)?;
            let n_mux = self.tmux_state.tmux_pane_in_tab(tab.id, neighbor)?;
            Some((f_mux, n_mux))
        })();
        let Some((f_mux, n_mux)) = resolved else {
            return false;
        };
        // tmux's swap-pane takes the neighbor as `-s`, the focused pane as
        // the target. A success reply carries only the layout broadcast, so
        // non-empty IS the failure signal (same shape as kill-pane's).
        let cmd = format!("swap-pane -t %{f_mux} -s %{n_mux}");
        let result = transport.send_command(&cmd);
        let ok = result.as_ref().is_ok_and(|body| body.is_empty());
        match result {
            Ok(body) if !ok => {
                let text = body.join("\n");
                log::error!("par-mux swap-pane rejected: {text}");
                self.record_mux_error(format!("par-mux: swap failed — {text}"));
            }
            Ok(_) => log::info!("MUX: swapped panes %{f_mux} and %{n_mux}"),
            Err(e) => {
                log::error!("par-mux swap-pane failed: {e}");
                self.record_mux_error(format!("par-mux: swap failed — {e}"));
            }
        }
        true
    }

    /// then types the command line into the new pane's shell: the daemon
    /// spawns shells on `split-window` (no command argument exists on the
    /// wire), so typing is the launch mechanism, and the pane lands in the
    /// current daemon window where the agent roster's hooks report it.
    ///
    /// The split's reply must carry the new pane id before any keys are
    /// sent — `mux_focused_pane` may hold a stale id from an earlier
    /// interaction, so it is never trusted as the send target here.
    pub(crate) fn launch_agent_via_mux(&mut self, command_line: &str) -> MuxLaunchOutcome {
        let Some(transport) = &self.tmux_state.transport else {
            return MuxLaunchOutcome::NotMux;
        };
        let Some(target) = self.any_mux_pane() else {
            crate::debug_trace!("MUX", "agent launch skipped — no daemon pane resolvable");
            return MuxLaunchOutcome::NotMux;
        };
        stamp_pane_session_id(transport.as_ref(), self.tmux_state.mux_session_id);
        let reply = match transport.send_command(&format!("split-window -h -t %{target}")) {
            Ok(reply) => reply,
            Err(e) => {
                log::error!("par-mux agent-launch split failed: {e}");
                self.record_mux_error(format!("par-mux: launch failed — {e}"));
                return MuxLaunchOutcome::Failed;
            }
        };
        let Some(pane) = reply.iter().find_map(|line| {
            line.trim()
                .strip_prefix('%')
                .and_then(|s| s.parse::<u64>().ok())
        }) else {
            // An %error block arrives as an Ok reply body with no pane id —
            // the same failure signal as the split path. The daemon target
            // resolved, so this is consumed-failed, never fall-through.
            let body = reply.join("\n");
            log::error!("par-mux agent-launch split rejected: {body}");
            self.record_mux_error(format!("par-mux: launch failed — {body}"));
            return MuxLaunchOutcome::Failed;
        };
        let keys = [
            format!(
                "send-keys -t %{pane} -l {}",
                par_term_mux::quote_env_value(command_line)
            ),
            format!("send-keys -t %{pane} Enter"),
        ];
        for cmd in keys {
            if let Err(e) = transport.send_command(&cmd) {
                log::error!("par-mux agent-launch {cmd:?} failed: {e}");
                self.record_mux_error(format!("par-mux: launch failed — {e}"));
                return MuxLaunchOutcome::Failed;
            }
        }
        log::info!("MUX: agent launched in %{pane}");
        MuxLaunchOutcome::Launched
    }

    /// Resolve any daemon pane to act as a launch anchor: the explicit mux
    /// focus, then the focused native pane's mapping (input routing's
    /// resolution), then the first mux tab's focused pane in tab order —
    /// so launching from a local tab while attached still lands daemon-side
    /// where the roster sees it, instead of a local tab whose command write
    /// would miss the daemon pane.
    fn any_mux_pane(&self) -> Option<u64> {
        if let Some(pane) = self.tmux_state.mux_focused_pane {
            return Some(pane);
        }
        if let Some(pane) = self.focused_mux_pane_from_native() {
            return Some(pane);
        }
        self.tab_manager.tabs().iter().find_map(|tab| {
            let pane_id = tab.pane_manager()?.focused_pane()?.id;
            self.tmux_state.tmux_pane_in_tab(tab.id, pane_id)
        })
    }
}
