//! The par-mux session directory (UX.md A16/A22): which sessions every
//! running daemon holds, without starting a daemon to find out.
//!
//! A session is identified by the daemon socket that serves it plus its
//! name. par-term's own attach convention names the socket after the
//! session (`par-mux-<name>.sock`, `MuxClient::connect_or_spawn`), but a
//! daemon can hold several sessions, so a listed session whose name is not
//! its socket's stem is still attachable — through its socket, never
//! through a name-derived spawn that would create an empty namesake.
//!
//! Each daemon is queried on its own thread with a deadline: a hung daemon
//! costs its own row, never the rest of the list or the frame.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// How long one daemon may take to answer `list-sessions`.
pub(crate) const DAEMON_QUERY_DEADLINE: Duration = Duration::from_secs(2);

pub(crate) use crate::session_picker_mux::{
    MuxDirectory, MuxSessionRow, MuxWorkspaceRow, daemon_name_of,
};

/// The directory the default par-mux sockets live in.
pub(crate) fn default_socket_dir() -> Option<PathBuf> {
    par_term_emu_core_rust::mux::ipc::default_socket_path("x")
        .parent()
        .map(Path::to_path_buf)
}

/// Every par-mux socket file in `dir`, name-sorted.
pub(crate) fn socket_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut sockets: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| daemon_name_of(p).is_some())
        .collect();
    sockets.sort();
    sockets
}

/// Ask one daemon for its workspaces and sessions. Connect-only: a socket
/// nothing serves fails here, and nothing is spawned in its place. The two
/// lists share one connection so a scan is two round trips per daemon.
fn query_daemon(socket: &Path) -> Result<MuxDirectory, String> {
    let daemon = daemon_name_of(socket).unwrap_or_default();
    let mut client = par_term_mux::MuxSessionClient::connect(socket)
        .map_err(|e| format!("par-mux daemon '{daemon}': {e}"))?;
    // A daemon predating the workspace commands answers
    // `unknown command: list-workspaces`; its sessions still list, and an
    // empty workspace list simply hides the picker's workspaces section.
    let workspaces = match client.list_workspaces() {
        Ok(rows) => rows,
        Err(e) if is_unknown_command(&e.to_string()) => Vec::new(),
        Err(e) => return Err(format!("par-mux daemon '{daemon}': {e}")),
    };
    let workspaces = workspaces
        .into_iter()
        .map(|w| MuxWorkspaceRow {
            socket: socket.to_path_buf(),
            daemon: daemon.clone(),
            id: w.id,
            name: w.name,
            active: w.active,
            git: None,
        })
        .collect::<Vec<_>>();
    let sessions = client
        .list_sessions()
        .map_err(|e| format!("par-mux daemon '{daemon}': {e}"))?
        .into_iter()
        .map(|s| MuxSessionRow {
            socket: socket.to_path_buf(),
            daemon: daemon.clone(),
            id: s.id,
            name: s.name,
            workspace_name: s.workspace_name,
        })
        .collect();
    // The workspaces' git badges ride `client-snapshot`'s `git` rows —
    // the snapshot is their only emitter — fetched last, capability-gated,
    // so an older daemon never sees the command and a failure leaves the
    // rows bare instead of failing the scan.
    let git = git_facts(&mut client);
    Ok(MuxDirectory {
        sessions,
        workspaces: apply_git_facts(workspaces, &git),
        errors: Vec::new(),
    })
}

/// One daemon's git-badge facts: the `client-snapshot` `git` rows. Probed
/// through the v1 capability gate (a daemon without the token never sees
/// a `client-snapshot` line), target-scoped to the daemon's first listed
/// window, and best-effort: any failure — no capability, no windows, a
/// stale reply — is no facts, never a scan error.
fn git_facts(client: &mut par_term_mux::MuxSessionClient) -> Vec<par_term_mux::GitStatusSummary> {
    let supported = client
        .list_commands()
        .map(|lines| par_term_mux::MuxSessionClient::snapshot_supported(&lines))
        .unwrap_or(false);
    if !supported {
        return Vec::new();
    }
    client
        .list_windows()
        .ok()
        .and_then(|windows| windows.first().map(|w| w.id))
        .and_then(|window| client.client_snapshot(window).ok())
        .map(|snap| snap.git)
        .unwrap_or_default()
}

/// Stamp each workspace row with its own snapshot `git` row, when one
/// exists. Rows with no row — non-repo, detached HEAD, or the facts
/// query failing — keep `git: None` and draw no badge.
fn apply_git_facts(
    mut rows: Vec<MuxWorkspaceRow>,
    git: &[par_term_mux::GitStatusSummary],
) -> Vec<MuxWorkspaceRow> {
    for row in &mut rows {
        row.git =
            git.iter()
                .find(|g| g.id == row.id)
                .map(|g| crate::session_picker_mux::WorkspaceGit {
                    branch: g.branch.clone(),
                    ahead: g.ahead,
                    behind: g.behind,
                });
    }
    rows
}

/// Scan every daemon socket in `dir`, one thread per daemon, each bounded
/// by `deadline`. A dead socket file (a daemon that exited without
/// cleaning up) is skipped silently; a live daemon that errors or hangs is
/// reported in `errors`.
pub(crate) fn scan(dir: &Path, deadline: Duration) -> MuxDirectory {
    let sockets = socket_files(dir);
    let (tx, rx) = mpsc::channel();
    for socket in &sockets {
        let tx = tx.clone();
        let socket = socket.clone();
        let spawned = std::thread::Builder::new()
            .name("mux-directory".into())
            .spawn(move || {
                let _ = tx.send((socket.clone(), query_daemon(&socket)));
            });
        if spawned.is_err() {
            log::warn!("mux directory: could not start a query thread");
        }
    }
    drop(tx);
    let mut dir_out = MuxDirectory::default();
    let mut pending: Vec<PathBuf> = sockets.clone();
    let end = Instant::now() + deadline;
    while !pending.is_empty() {
        let left = end.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok((socket, result)) => {
                pending.retain(|p| p != &socket);
                match result {
                    Ok(rows) => {
                        dir_out.sessions.extend(rows.sessions);
                        dir_out.workspaces.extend(rows.workspaces);
                    }
                    Err(e) if is_dead_socket(&e) => {}
                    Err(e) => dir_out.errors.push(e),
                }
            }
            Err(_) => break,
        }
    }
    for socket in pending {
        let daemon = daemon_name_of(&socket).unwrap_or_default();
        dir_out
            .errors
            .push(format!("par-mux daemon '{daemon}' did not answer"));
    }
    dir_out
        .sessions
        .sort_by(|a, b| (&a.daemon, &a.name).cmp(&(&b.daemon, &b.name)));
    dir_out
}

/// A leftover socket file with no daemon behind it — not worth a message.
fn is_dead_socket(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("connection refused")
        || lower.contains("no such file")
        || lower.contains("not found")
}

/// The daemon replied with its `unknown command:` error line — the client
/// sent a command this daemon binary predates.
fn is_unknown_command(message: &str) -> bool {
    message.contains("unknown command:")
}

/// A background scan in flight. The picker, the palette cache, and the
/// session chip all read the same finished result; polling never blocks.
pub(crate) struct PendingScan {
    rx: mpsc::Receiver<MuxDirectory>,
}

impl PendingScan {
    /// Start a scan of `dir` off the calling thread.
    pub(crate) fn start(dir: PathBuf) -> Option<Self> {
        let (tx, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("mux-directory-scan".into())
            .spawn(move || {
                let _ = tx.send(scan(&dir, DAEMON_QUERY_DEADLINE));
            })
            .ok()?;
        Some(Self { rx })
    }

    /// The finished scan, once it lands; `Err(())` when the worker died.
    pub(crate) fn poll(&self) -> Option<Result<MuxDirectory, ()>> {
        match self.rx.try_recv() {
            Ok(dir) => Some(Ok(dir)),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(Err(())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An empty or missing directory scans to nothing, with no error.
    #[test]
    fn a_missing_directory_scans_empty() {
        let dir = std::env::temp_dir().join(format!("par-term-no-mux-{}", std::process::id()));
        assert_eq!(
            scan(&dir, Duration::from_millis(200)),
            MuxDirectory::default()
        );
    }

    /// Criterion 2 + 4: each snapshot `git` row stamps only its own
    /// workspace's row; a workspace without one — non-repo, detached
    /// HEAD, or the facts query failing — keeps `git: None` and no badge.
    #[test]
    fn git_facts_stamp_their_own_workspace_rows_only() {
        let ws = |id: u64| MuxWorkspaceRow {
            socket: PathBuf::from("/s/x.sock"),
            daemon: "x".to_string(),
            id,
            name: format!("repo-{id}"),
            active: false,
            git: None,
        };
        let rows = vec![ws(0), ws(1), ws(2)];
        let git = vec![
            par_term_mux::GitStatusSummary {
                id: 0,
                branch: "main".to_string(),
                ahead: 0,
                behind: 0,
            },
            par_term_mux::GitStatusSummary {
                id: 2,
                branch: "feat".to_string(),
                ahead: 1,
                behind: 0,
            },
        ];
        let rows = apply_git_facts(rows, &git);
        assert_eq!(
            rows[0]
                .git
                .as_ref()
                .map(|g| (g.branch.as_str(), g.ahead, g.behind)),
            Some(("main", 0, 0)),
            "the +0 row reaches workspace +0"
        );
        assert_eq!(rows[1].git, None, "workspace +1 has no git row");
        assert_eq!(
            rows[2]
                .git
                .as_ref()
                .map(|g| (g.branch.as_str(), g.ahead, g.behind)),
            Some(("feat", 1, 0))
        );
    }
}
