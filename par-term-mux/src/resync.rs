//! Reattach/resync over the command tier.
//!
//! On connect, the app needs the daemon's current world: which sessions
//! exist (create-or-attach is client-side for par-mux), which windows they
//! hold, and which panes exist to seed screens for. Reply shapes are the
//! daemon's fixed wire contract: `list-sessions` replies `$N: name` lines,
//! `list-windows` replies `@N: name` lines, `list-panes` replies bare `%N`
//! lines — all globally scoped; geometry arrives via `%layout-change`
//! pushes, not list replies.

use crate::client::MuxSessionClient;
use par_term_tmux::{TmuxPaneId, TmuxWindowId};
use std::collections::HashMap;
use std::io::{self, ErrorKind};

/// One session from `list-sessions` (`$N: name`).
#[derive(Debug, Clone)]
pub struct SessionSummary {
    pub id: u64,
    pub name: String,
}

/// One window from `list-windows` (`@N: name`).
#[derive(Debug, Clone)]
pub struct WindowSummary {
    pub id: TmuxWindowId,
    pub name: String,
}

/// What [`MuxSessionClient::create_or_attach`] did.
#[derive(Debug, Clone)]
pub enum AttachOutcome {
    /// The named session already existed; the client attached to it.
    Attached(SessionSummary),
    /// No such session existed; one was created.
    Created(SessionSummary),
}

impl MuxSessionClient {
    /// List the daemon's sessions as `$N: name` lines.
    pub fn list_sessions(&mut self) -> io::Result<Vec<SessionSummary>> {
        let lines = self.send("list-sessions")?;
        lines
            .iter()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                let (id, name) = parse_id_name_line(line, '$')?;
                Ok(SessionSummary { id, name })
            })
            .collect()
    }

    /// List the daemon's windows (all sessions) as `@N: name` lines.
    pub fn list_windows(&mut self) -> io::Result<Vec<WindowSummary>> {
        let lines = self.send("list-windows")?;
        lines
            .iter()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                let (id, name) = parse_id_name_line(line, '@')?;
                Ok(WindowSummary { id, name })
            })
            .collect()
    }

    /// List the daemon's panes (all windows) as bare `%N` lines.
    pub fn list_panes(&mut self) -> io::Result<Vec<TmuxPaneId>> {
        let lines = self.send("list-panes")?;
        lines
            .iter()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                line.strip_prefix('%')
                    .and_then(|id| id.trim().parse().ok())
                    .ok_or_else(|| invalid_line("pane", line))
            })
            .collect()
    }

    /// A pane's effective title (`pane-title -t %N`): the user `-T` title
    /// when one is set, else the pane terminal's current OSC 0/2 title.
    /// An empty reply means neither is set. One line; a title cannot
    /// carry a newline on the line-framed wire.
    pub fn pane_title(&mut self, pane: TmuxPaneId) -> io::Result<String> {
        let lines = self.send(&format!("pane-title -t %{pane}"))?;
        Ok(lines
            .into_iter()
            .find(|line| !line.trim().is_empty())
            .unwrap_or_default())
    }

    /// Attach to `name`, creating the session when it does not exist —
    /// par-mux's client-side analogue of `new-session -A`.
    ///
    /// The daemon's command grammar is a documented whitespace split (no
    /// quoting outside `send-keys` payloads — core mux command.rs: "tmux's
    /// real argument grammar ... is not a goal"), so a name that cannot
    /// survive that split is rejected here instead of silently creating a
    /// mangled session: `Par Mux Test` would arrive as `-s Par` and create
    /// a session named `Par`.
    pub fn create_or_attach(&mut self, name: &str) -> io::Result<AttachOutcome> {
        self.create_or_attach_with_env(name, &HashMap::new())
    }

    /// [`Self::create_or_attach`], seeding the session environment: a
    /// created session gets `env` through `new-session -e` (its first pane
    /// spawns inside that command); an existing one gets it through
    /// `set-environment`, so panes spawned after this attach see the
    /// client's current values. Env failures never fail the attach: a
    /// daemon that predates session env ignores `-e` and refuses
    /// `set-environment`.
    pub fn create_or_attach_with_env(
        &mut self,
        name: &str,
        env: &HashMap<String, String>,
    ) -> io::Result<AttachOutcome> {
        if name.is_empty()
            || name
                .chars()
                .any(|c| c.is_whitespace() || matches!(c, '\'' | '"' | '\\'))
        {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                "session name must be non-empty and cannot contain spaces, quotes, \
                 or backslashes (the daemon command grammar does not support quoting)",
            ));
        }
        let existing = self.list_sessions()?.into_iter().find(|s| s.name == name);
        match existing {
            Some(session) => {
                if !env.is_empty() {
                    let (skipped, refused) = self.push_session_env(session.id, env)?;
                    warn_skipped(&skipped);
                    if !refused.is_empty() {
                        log::warn!(
                            "par-mux session env: daemon refused {} of {} variables ({}); \
                             the daemon likely predates set-environment, restart it",
                            refused.len(),
                            env.len(),
                            refused.join(", ")
                        );
                    }
                }
                Ok(AttachOutcome::Attached(session))
            }
            None => {
                let (env_args, skipped) = crate::env::new_session_env_args(env);
                warn_skipped(&skipped);
                self.send(&format!("new-session -s {name}{env_args}"))?;
                let created = self
                    .list_sessions()?
                    .into_iter()
                    .find(|s| s.name == name)
                    .ok_or_else(|| {
                        io::Error::new(
                            ErrorKind::InvalidData,
                            format!("session {name:?} missing after new-session"),
                        )
                    })?;
                Ok(AttachOutcome::Created(created))
            }
        }
    }

    /// Ask the daemon to replay a pane's visible screen — the per-pane
    /// seeding step of reattach. The replay arrives as the COMMAND REPLY
    /// (the pane's screen content), not a `%output` push, so callers feed
    /// the returned lines to their pane terminal. Size-driven refits go
    /// through [`MuxSessionClient::set_client_size`].
    pub fn refresh_pane(&mut self, pane: TmuxPaneId) -> io::Result<Vec<String>> {
        self.send(&format!("refresh-client -t %{pane}"))
    }
}

/// Report env variables the line-framed wire cannot carry (a newline or
/// NUL in a `shell_env` value). Names only, never values.
fn warn_skipped(skipped: &[String]) {
    if !skipped.is_empty() {
        log::warn!(
            "par-mux session env: skipped variables the wire cannot carry \
             (newline or NUL in the value): {}",
            skipped.join(", ")
        );
    }
}

/// Parse `sigil`N`: `name` (the `@N: name` / `$N: name` wire shapes).
fn parse_id_name_line(line: &str, sigil: char) -> io::Result<(u64, String)> {
    let rest = line
        .strip_prefix(sigil)
        .ok_or_else(|| invalid_line("id", line))?;
    let (id, name) = rest
        .split_once(": ")
        .ok_or_else(|| invalid_line("id", line))?;
    let id = id.trim().parse().map_err(|_| invalid_line("id", line))?;
    Ok((id, name.to_string()))
}

fn invalid_line(kind: &str, line: &str) -> io::Error {
    io::Error::new(
        ErrorKind::InvalidData,
        format!("unrecognized {kind} reply line: {line:?}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_session_and_window_reply_lines() {
        let (id, name) = parse_id_name_line("$3: dev", '$').expect("session line parses");
        assert_eq!((id, name.as_str()), (3, "dev"));

        let (id, name) = parse_id_name_line("@7: editor", '@').expect("window line parses");
        assert_eq!((id, name.as_str()), (7, "editor"));
    }

    #[test]
    fn rejects_malformed_reply_lines() {
        assert!(parse_id_name_line("3: dev", '$').is_err());
        assert!(parse_id_name_line("$3:dev", '$').is_err());
        assert!(parse_id_name_line("$x: dev", '$').is_err());
    }
}
