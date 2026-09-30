//! Profile-free par-mux session management (UX.md A16, A22, M15): attach,
//! create, switch, rename, and end a session from the picker, the palette,
//! or the command line — no profile edit required.
//!
//! Listed sessions attach through the socket that serves them
//! ([`WindowState::begin_mux_attach_at`]), never through the name-derived
//! spawn a profile uses, so a session held by a differently named daemon
//! is reached rather than shadowed by a new empty namesake.

use super::mux::MuxAttachPending;
use super::mux_directory::{self, MuxDirectory, MuxSessionRow, PendingScan};
use crate::app::window_state::WindowState;
use std::path::{Path, PathBuf};

pub(crate) use crate::session_picker_mux::MuxPickerAction as MuxSessionRequest;

impl WindowState {
    /// The socket directory this window scans and attaches in.
    ///
    /// Under `cfg(test)` only the override counts: a test that attaches or
    /// detaches without setting it must not list (or dial) the developer's
    /// live daemons through the refresh those paths trigger.
    pub(crate) fn mux_socket_dir(&self) -> Option<PathBuf> {
        let override_dir = self.tmux_state.mux_socket_dir_override.clone();
        if cfg!(test) {
            return override_dir;
        }
        override_dir.or_else(mux_directory::default_socket_dir)
    }

    /// The socket a new session named `name` gets: par-term's convention
    /// is one daemon per session, named after it.
    fn mux_socket_for_new(&self, name: &str) -> Option<PathBuf> {
        Some(self.mux_socket_dir()?.join(format!("par-mux-{name}.sock")))
    }

    /// Start a background rescan of the session directory unless one is
    /// already in flight. Called when the picker or palette opens and after
    /// attach, detach, and `%sessions-changed`, so the cached list the
    /// palette reads is fresh without ever blocking a frame.
    pub(crate) fn refresh_mux_directory(&mut self) {
        if self.tmux_state.mux_directory_scan.is_some() {
            return;
        }
        if let Some(dir) = self.mux_socket_dir() {
            self.tmux_state.mux_directory_scan = PendingScan::start(dir);
        }
    }

    /// Apply a finished scan. Returns true when one landed.
    pub(crate) fn poll_mux_directory(&mut self) -> bool {
        let Some(scan) = self.tmux_state.mux_directory_scan.as_ref() else {
            return false;
        };
        match scan.poll() {
            None => false,
            Some(result) => {
                self.tmux_state.mux_directory_scan = None;
                self.tmux_state.mux_directory = Some(result.unwrap_or_else(|()| MuxDirectory {
                    sessions: Vec::new(),
                    errors: vec!["par-mux session scan stopped unexpectedly".to_string()],
                }));
                true
            }
        }
    }

    /// Attach this window to the session `name` served by `socket` —
    /// connect-only for a listed session (`spawn` false), connect-or-spawn
    /// for a new one. The worker thread owns the connect; the existing
    /// `poll_mux_attach` finishes on the main thread.
    pub(crate) fn begin_mux_attach_at(&mut self, socket: &Path, name: &str, spawn: bool) {
        if self.tmux_state.mux_attach_pending.is_some() {
            self.show_toast("par-mux: an attach is already in progress");
            return;
        }
        if let Some(reason) = super::mux::mux_attach_refusal(socket) {
            log::error!("par-mux attach to '{name}' refused: {reason}");
            self.record_mux_error(format!("par-mux: attach to '{name}' refused — {reason}"));
            return;
        }
        self.tmux_state.mux_daemon_rx = None;
        let (tx, rx) = std::sync::mpsc::channel();
        let socket = socket.to_path_buf();
        let spawned = std::thread::Builder::new()
            .name("mux-attach".into())
            .spawn(move || {
                let client = if spawn {
                    par_term_emu_core_rust::mux::MuxClient::connect_or_spawn_at(&socket)
                } else {
                    par_term_emu_core_rust::mux::MuxClient::connect(&socket)
                };
                let _ = tx.send(client);
            });
        match spawned {
            Ok(_) => {
                self.tmux_state.mux_attach_pending = Some(MuxAttachPending {
                    name: name.to_string(),
                    rx,
                });
            }
            Err(e) => {
                log::error!("mux attach worker could not start: {e}");
                self.record_mux_error("par-mux: attach failed (worker thread unavailable)");
            }
        }
    }

    /// Leave the current par-mux session (if any) so another can attach
    /// here, keeping the window alive: a window whose tabs were all
    /// daemon tabs would otherwise be left with none. The first daemon tab
    /// of the next session replaces the placeholder shell.
    fn detach_for_switch(&mut self) {
        if self.tmux_state.transport.is_none() {
            return;
        }
        self.detach_mux_session();
        // The transport is gone, so new_tab() spawns a local shell rather
        // than asking the old daemon for a window. A window that kept
        // local tabs keeps them and needs no placeholder.
        if self.tab_manager.tab_count() == 0 {
            self.new_tab();
            if let Some(tab) = self.tab_manager.tabs().first() {
                self.tmux_state.mux_restore_placeholder_tab = Some(tab.id);
            }
        }
    }

    /// Apply a session request from the picker or the palette.
    pub(crate) fn handle_mux_session_request(&mut self, request: MuxSessionRequest) {
        match request {
            MuxSessionRequest::Attach(row) => {
                let already_here = self.tmux_state.transport.is_some()
                    && self.tmux_state.tmux_session_name.as_deref() == Some(row.name.as_str())
                    && self.tmux_state.mux_daemon.as_deref() == Some(row.daemon.as_str());
                if already_here {
                    self.show_toast(format!("par-mux: already attached to '{}'", row.name));
                    return;
                }
                self.detach_for_switch();
                self.tmux_state.mux_daemon = Some(row.daemon.clone());
                self.begin_mux_attach_at(&row.socket, &row.name, false);
                if !row.survives_restore() {
                    self.show_toast(format!(
                        "par-mux: attaching to '{}' (daemon '{}'); it is not reattached on \
                         the next launch",
                        row.name, row.daemon
                    ));
                }
            }
            MuxSessionRequest::Create(name) => {
                if let Some(reason) = crate::session_picker_mux::session_name_refusal(&name) {
                    self.show_toast(format!("par-mux: {reason}"));
                    return;
                }
                let taken = self
                    .tmux_state
                    .mux_directory
                    .as_ref()
                    .is_some_and(|d| d.sessions.iter().any(|s| s.name == name));
                if taken {
                    self.show_toast(format!(
                        "par-mux: a session named '{name}' already exists — attach to it instead"
                    ));
                    return;
                }
                let Some(socket) = self.mux_socket_for_new(&name) else {
                    self.show_toast("par-mux: no socket directory is available");
                    return;
                };
                self.detach_for_switch();
                self.tmux_state.mux_daemon = Some(name.clone());
                self.begin_mux_attach_at(&socket, &name, true);
            }
            MuxSessionRequest::Rename(row, new_name) => {
                self.rename_mux_session(&row, &new_name);
            }
            MuxSessionRequest::Kill(row) => {
                self.kill_mux_session(&row);
            }
            MuxSessionRequest::Detach => {
                if !self.detach_mux_session() {
                    self.show_toast("Not attached to a par-mux session");
                }
            }
        }
        self.focus_state.needs_redraw = true;
        self.request_redraw();
    }

    /// Run one command against the daemon serving `row` — through this
    /// window's transport when that is the daemon, else a short-lived
    /// connect (never a spawn).
    fn send_to_session_daemon(&mut self, row: &MuxSessionRow, command: &str) -> Result<(), String> {
        let ours = self.tmux_state.transport.is_some()
            && self.tmux_state.mux_daemon.as_deref() == Some(row.daemon.as_str());
        let reply = if ours {
            self.tmux_state
                .transport
                .as_ref()
                .map(|t| t.send_command(command))
                .unwrap_or_else(|| Err(std::io::Error::other("no transport")))
        } else {
            par_term_mux::MuxSessionClient::connect(&row.socket).and_then(|mut c| c.send(command))
        };
        match reply {
            Ok(body) if body.iter().any(|l| l.starts_with("%error")) => Err(body.join(" ")),
            Ok(_) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    /// Rename a listed session (UX.md UP1, `rename-session`).
    fn rename_mux_session(&mut self, row: &MuxSessionRow, new_name: &str) {
        if let Some(reason) = crate::session_picker_mux::session_name_refusal(new_name) {
            self.show_toast(format!("par-mux: {reason}"));
            return;
        }
        let command = format!(
            "rename-session -t ${} {}",
            row.id,
            par_term_mux::quote_env_value(new_name)
        );
        match self.send_to_session_daemon(row, &command) {
            Ok(()) => {
                self.show_toast(format!("par-mux: renamed '{}' to '{new_name}'", row.name));
                self.refresh_mux_directory();
            }
            Err(e) => self.record_mux_error(format!("par-mux: rename failed — {e}")),
        }
    }

    /// End a listed session (UX.md UP2, `kill-session`). When it is this
    /// window's own session, the daemon's `%window-close` /
    /// `%sessions-changed` pushes tear the view down, as for End session
    /// in the last-tab dialog.
    fn kill_mux_session(&mut self, row: &MuxSessionRow) {
        match self.send_to_session_daemon(row, &format!("kill-session -t ${}", row.id)) {
            Ok(()) => {
                self.show_toast(format!("par-mux: ended session '{}'", row.name));
                self.refresh_mux_directory();
            }
            Err(e) => self.record_mux_error(format!("par-mux: kill-session failed — {e}")),
        }
    }

    /// M15 / A22: attach this window to `name` without a profile — the CLI
    /// `--attach` and the `mux_auto_attach` config key land here. Which
    /// daemon serves `name` is decided on the attach worker, off the event
    /// loop: a listed session attaches through its socket; an unknown name
    /// creates the session in its own daemon (par-term's
    /// one-daemon-per-session convention).
    pub(crate) fn attach_mux_session_by_name(&mut self, name: &str) {
        self.attach_by_name(name, true);
    }

    /// [`Self::attach_mux_session_by_name`]'s worker, with the create
    /// fallback optional: a hand-bound `attach_mux_session:<name>` finds a
    /// running session this way when the cached list is not filled yet,
    /// and must never create one.
    fn attach_by_name(&mut self, name: &str, create_missing: bool) {
        if let Some(reason) = crate::session_picker_mux::session_name_refusal(name) {
            self.show_toast(format!("par-mux: cannot attach to '{name}' — {reason}"));
            return;
        }
        if self.tmux_state.mux_attach_pending.is_some() {
            self.show_toast("par-mux: an attach is already in progress");
            return;
        }
        let Some(dir) = self.mux_socket_dir() else {
            self.show_toast("par-mux: no socket directory is available");
            return;
        };
        self.detach_for_switch();
        let (tx, rx) = std::sync::mpsc::channel();
        let (daemon_tx, daemon_rx) = std::sync::mpsc::channel();
        let wanted = name.to_string();
        let spawned = std::thread::Builder::new()
            .name("mux-attach".into())
            .spawn(move || {
                let listed = mux_directory::scan(&dir, mux_directory::DAEMON_QUERY_DEADLINE)
                    .sessions
                    .into_iter()
                    .find(|s| s.name == wanted);
                let client = match listed {
                    Some(row) => {
                        let _ = daemon_tx.send(row.daemon.clone());
                        par_term_emu_core_rust::mux::MuxClient::connect(&row.socket)
                    }
                    None if create_missing => {
                        let _ = daemon_tx.send(wanted.clone());
                        par_term_emu_core_rust::mux::MuxClient::connect_or_spawn_at(
                            &dir.join(format!("par-mux-{wanted}.sock")),
                        )
                    }
                    None => Err(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        format!("no running session named '{wanted}'"),
                    )),
                };
                let _ = tx.send(client);
            });
        match spawned {
            Ok(_) => {
                self.tmux_state.mux_daemon_rx = Some(daemon_rx);
                self.tmux_state.mux_attach_pending = Some(MuxAttachPending {
                    name: name.to_string(),
                    rx,
                });
            }
            Err(e) => {
                log::error!("mux attach worker could not start: {e}");
                self.record_mux_error("par-mux: attach failed (worker thread unavailable)");
            }
        }
    }

    /// Dispatch an `attach_mux_session:<daemon>/<name>` palette row or
    /// binding against the cached directory. A name no daemon lists is
    /// refused with a toast rather than silently creating a session.
    pub(crate) fn dispatch_attach_mux_session(&mut self, action: &str) -> bool {
        let resolved =
            resolve_attach_id(action, self.tmux_state.mux_directory.as_ref()).map(|r| r.cloned());
        match resolved {
            Some(Ok(row)) => {
                self.handle_mux_session_request(MuxSessionRequest::Attach(row));
                true
            }
            Some(Err(name)) => {
                // Not in the cached list — which may simply be unfilled (a
                // binding pressed right after launch). Look it up on the
                // attach worker; a name no daemon lists fails there with a
                // chip error, and nothing is created.
                self.attach_by_name(&name, false);
                true
            }
            None => false,
        }
    }

    /// Palette rows for the cached directory (A22): one attach row per
    /// listed session plus New Session. Reads the cache only — the palette
    /// never waits on a daemon.
    pub(crate) fn mux_session_palette_rows(
        tmux_state: &crate::app::tmux_handler::tmux_state::TmuxState,
    ) -> Vec<crate::command_palette::catalog::PaletteEntry> {
        let mut rows = Vec::new();
        let attached = tmux_state
            .transport
            .as_ref()
            .and(tmux_state.tmux_session_name.as_deref());
        if let Some(dir) = &tmux_state.mux_directory {
            for session in &dir.sessions {
                if attached == Some(session.name.as_str())
                    && tmux_state.mux_daemon.as_deref() == Some(session.daemon.as_str())
                {
                    continue;
                }
                let label = if session.survives_restore() {
                    format!("Attach par-mux Session: {}", session.name)
                } else {
                    format!(
                        "Attach par-mux Session: {} (daemon {})",
                        session.name, session.daemon
                    )
                };
                rows.push(crate::command_palette::catalog::PaletteEntry {
                    action_id: attach_action_id(session),
                    label,
                    chord: None,
                    priority: 0,
                });
            }
        }
        rows
    }
}

/// The runtime palette id that attaches `row`: `attach_mux_session:<daemon>/<name>`.
pub(crate) fn attach_action_id(row: &MuxSessionRow) -> String {
    format!("attach_mux_session:{}/{}", row.daemon, row.name)
}

/// Resolve an `attach_mux_session:` id against the cached directory.
/// `<name>` alone (no daemon) is accepted for hand-written bindings and
/// resolves by session name.
pub(crate) fn resolve_attach_id<'a>(
    id: &str,
    directory: Option<&'a MuxDirectory>,
) -> Option<Result<&'a MuxSessionRow, String>> {
    let spec = id.strip_prefix("attach_mux_session:")?;
    let (daemon, name) = match spec.split_once('/') {
        Some((daemon, name)) => (Some(daemon), name),
        None => (None, spec),
    };
    let found = directory.and_then(|d| {
        d.sessions
            .iter()
            .find(|s| s.name == name && daemon.is_none_or(|dn| dn == s.daemon))
    });
    Some(found.ok_or_else(|| name.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(daemon: &str, name: &str) -> MuxSessionRow {
        MuxSessionRow {
            socket: PathBuf::from(format!("/s/par-mux-{daemon}.sock")),
            daemon: daemon.to_string(),
            id: 0,
            name: name.to_string(),
        }
    }

    #[test]
    fn attach_ids_round_trip_through_the_directory() {
        let dir = MuxDirectory {
            sessions: vec![row("work", "work"), row("default", "scratch")],
            errors: vec![],
        };
        let id = attach_action_id(&dir.sessions[1]);
        assert_eq!(id, "attach_mux_session:default/scratch");
        assert_eq!(
            resolve_attach_id(&id, Some(&dir)),
            Some(Ok(&dir.sessions[1]))
        );
        assert_eq!(
            resolve_attach_id("attach_mux_session:work", Some(&dir)),
            Some(Ok(&dir.sessions[0])),
            "a bare name resolves by session name"
        );
        assert_eq!(
            resolve_attach_id("attach_mux_session:gone", Some(&dir)),
            Some(Err("gone".to_string()))
        );
        assert_eq!(resolve_attach_id("new_tab", Some(&dir)), None);
    }
}
