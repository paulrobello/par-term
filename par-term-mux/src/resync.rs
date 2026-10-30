//! Reattach/resync over the command tier.
//!
//! On connect, the app needs the daemon's current world: which sessions
//! exist (create-or-attach is client-side for par-mux), which windows they
//! hold, and which panes exist to seed screens for. Reply shapes are the
//! daemon's fixed wire contract: `list-sessions` replies `+W: wname: $N: name`
//! rows (workspace-grouped), `list-windows` replies `@N: name` lines,
//! `list-panes` replies bare `%N`
//! lines — all globally scoped; geometry arrives via `%layout-change`
//! pushes, not list replies.

use crate::client::MuxSessionClient;
use par_term_tmux::{TmuxPaneId, TmuxWindowId};
use std::collections::HashMap;
use std::io::{self, ErrorKind};

/// One session from `list-sessions` (`+W: wname: $N: name`): the session
/// plus the workspace that owns it.
#[derive(Debug, Clone)]
pub struct SessionSummary {
    pub id: u64,
    pub name: String,
    pub workspace_id: u64,
    pub workspace_name: String,
}

/// One window from `list-windows` (`@N: name`).
#[derive(Debug, Clone)]
pub struct WindowSummary {
    pub id: TmuxWindowId,
    pub name: String,
}

/// One workspace from `list-workspaces` (`+N: name[ active]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSummary {
    pub id: u64,
    pub name: String,
    /// The daemon's active workspace (the one `select-workspace` resumes).
    pub active: bool,
}

/// One `client-snapshot` `git` row: `+N <branch> <ahead|-> <behind|->` —
/// one per repo workspace whose badge facts the daemon has. `0` is
/// spelled `-` so the row keeps its fixed four-field shape; a detached
/// HEAD or a non-repo chain emits no row, so a workspace without a row
/// has no badge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitStatusSummary {
    /// The workspace id, `+N`.
    pub id: u64,
    /// The checked-out branch.
    pub branch: String,
    /// Commits ahead of the upstream (0 with none).
    pub ahead: u32,
    /// Commits behind the upstream (0 with none).
    pub behind: u32,
}

impl GitStatusSummary {
    /// Parse the rest of one `git ` row; `None` when the id is not `+N`,
    /// a count is neither `-` nor digits, or a field is missing. A
    /// malformed row is skipped, never an error — the snapshot is
    /// additive and the badge is best-effort.
    fn parse(rest: &str) -> Option<Self> {
        let mut fields = rest.splitn(4, ' ');
        let id = fields
            .next()?
            .strip_prefix('+')
            .and_then(|id| id.parse().ok())?;
        let branch = fields.next()?;
        if branch.is_empty() {
            return None;
        }
        let ahead = parse_count_field(fields.next()?)?;
        let behind = parse_count_field(fields.next()?)?;
        Some(Self {
            id,
            branch: branch.to_string(),
            ahead,
            behind,
        })
    }
}

/// A snapshot count column: `0` spells `-` on the wire; a bare `0` reads
/// the same.
fn parse_count_field(field: &str) -> Option<u32> {
    match field {
        "-" => Some(0),
        digits if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) => {
            digits.parse().ok()
        }
        _ => None,
    }
}

/// What a `client-snapshot` reply carries that par-term reads: every
/// workspace and every session, with the target window's owning session
/// marked. `window`/`pane`/`agent`/`worktree` rows and any future row
/// kind are ignored here — the drain's status read needs the two rosters
/// only, and the picker's git badges read [`Self::git`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClientSnapshot {
    pub workspaces: Vec<WorkspaceSummary>,
    pub sessions: Vec<SnapshotSession>,
    /// One row per repo workspace (core 0.64+); empty from a daemon
    /// without git rows.
    pub git: Vec<GitStatusSummary>,
}

/// One `session` row: `$N <*|-> +W <name>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotSession {
    pub id: u64,
    pub name: String,
    pub workspace_id: u64,
    /// The session that owns the snapshot's target window.
    pub displayed: bool,
}

impl ClientSnapshot {
    /// Parse a `client-snapshot` reply: the `snapshot 1` header, then one
    /// typed row per line. Unknown row kinds and unknown trailing tokens
    /// are ignored; a header that is not `snapshot 1` is an error (the
    /// positional contract may have moved under a bumped version).
    pub fn parse(lines: &[String]) -> io::Result<Self> {
        let mut snapshot = Self::default();
        let mut header_seen = false;
        for line in lines {
            let mut words = line.splitn(2, ' ');
            match words.next() {
                Some("snapshot") => {
                    if words.next() != Some("1") {
                        return Err(invalid_line("snapshot version", line));
                    }
                    header_seen = true;
                }
                Some("workspace") => {
                    // `workspace +W <*|-> <name>` — the name is the rest of
                    // the line, so spaces survive.
                    let err = || invalid_line("workspace row", line);
                    let mut fields = line.splitn(4, ' ');
                    let _kind = fields.next();
                    let id = fields
                        .next()
                        .and_then(|w| w.strip_prefix('+'))
                        .and_then(|w| w.parse().ok())
                        .ok_or_else(err)?;
                    let active = match fields.next().ok_or_else(err)? {
                        "*" => true,
                        "-" => false,
                        _ => return Err(err()),
                    };
                    snapshot.workspaces.push(WorkspaceSummary {
                        id,
                        name: fields.next().unwrap_or_default().to_string(),
                        active,
                    });
                }
                Some("session") => {
                    // `session $N <*|-> +W <name>`.
                    let err = || invalid_line("session row", line);
                    let mut fields = line.splitn(5, ' ');
                    let _kind = fields.next();
                    let id = fields
                        .next()
                        .and_then(|w| w.strip_prefix('$'))
                        .and_then(|w| w.parse().ok())
                        .ok_or_else(err)?;
                    let displayed = match fields.next().ok_or_else(err)? {
                        "*" => true,
                        "-" => false,
                        _ => return Err(err()),
                    };
                    let workspace_id = fields
                        .next()
                        .and_then(|w| w.strip_prefix('+'))
                        .and_then(|w| w.parse().ok())
                        .ok_or_else(err)?;
                    snapshot.sessions.push(SnapshotSession {
                        id,
                        name: fields.next().unwrap_or_default().to_string(),
                        workspace_id,
                        displayed,
                    });
                }
                Some("git") => {
                    // `git +N <branch> <ahead|-> <behind|->` — one per repo
                    // workspace. A malformed row is skipped like an
                    // unknown kind: the badges are best-effort and the
                    // snapshot grammar is additive.
                    if let Some(row) = GitStatusSummary::parse(words.next().unwrap_or_default()) {
                        snapshot.git.push(row);
                    }
                }
                _ => {}
            }
        }
        if !header_seen {
            return Err(invalid_line(
                "snapshot header",
                lines.first().map(String::as_str).unwrap_or(""),
            ));
        }
        Ok(snapshot)
    }
}

/// One replayed daemon event: its daemon-wide monotonic sequence number,
/// its epoch-millisecond timestamp, and the control-mode wire line
/// exactly as a live client would have received it on the push stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayedEvent {
    /// Daemon-wide monotonic sequence (the first event is seq 1; the ring
    /// is daemon-lifetime, never persisted, so a restart restarts at 1).
    pub seq: u64,
    /// Epoch milliseconds when the daemon broadcast the line.
    pub at_ms: u64,
    /// The wire line (leading `%` included, trailing newline stripped).
    pub line: String,
}

/// What an `events [--since <seq>]` reply carries: the current ring head
/// (0 = nothing recorded yet) and every retained event after `since`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EventReplay {
    /// The highest sequence number the daemon has assigned (0 = none).
    pub head: u64,
    /// Retained events with a seq strictly greater than the `since` the
    /// caller sent, in sequence order.
    pub events: Vec<ReplayedEvent>,
}

/// Parse an `events` reply: the `head <seq>` header, then one
/// `<seq> <epoch-ms> <wire-line>` row per retained event. The wire line
/// is the rest of the row, so spaces (and any `%` text) inside it ride
/// whole. A reply without the header — an older daemon's
/// `unknown command: events` error body — is an error, never an empty
/// replay: the caller must not mistake "daemon cannot answer" for
/// "nothing happened".
pub fn parse_events_reply(lines: &[String]) -> io::Result<EventReplay> {
    let mut replay = EventReplay::default();
    let mut header_seen = false;
    for line in lines {
        let mut words = line.splitn(2, ' ');
        match words.next() {
            Some("head") => {
                let seq = words
                    .next()
                    .and_then(|s| s.trim().parse().ok())
                    .ok_or_else(|| invalid_line("events header", line))?;
                header_seen = true;
                replay.head = seq;
            }
            _ => {
                // Event row: `<seq> <epoch-ms> <wire-line>`. The wire line
                // is the rest of the line, so the split is fixed at three.
                let err = || invalid_line("event row", line);
                let mut fields = line.splitn(3, ' ');
                let seq = fields.next().and_then(|s| s.parse().ok()).ok_or_else(err)?;
                let at_ms = fields.next().and_then(|s| s.parse().ok()).ok_or_else(err)?;
                let line_text = fields.next().filter(|s| !s.is_empty()).ok_or_else(err)?;
                replay.events.push(ReplayedEvent {
                    seq,
                    at_ms,
                    line: line_text.to_string(),
                });
            }
        }
    }
    if !header_seen {
        return Err(invalid_line(
            "events header",
            lines.first().map(String::as_str).unwrap_or(""),
        ));
    }
    Ok(replay)
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
    /// List the daemon's sessions as `+W: wname: $N: name` rows, the
    /// session id the last `$<digits>:` marker (the daemon's documented
    /// resolution rule, so a name carrying `:` or `$` text still reads).
    pub fn list_sessions(&mut self) -> io::Result<Vec<SessionSummary>> {
        let lines = self.send("list-sessions")?;
        lines
            .iter()
            .filter(|line| !line.trim().is_empty())
            .map(|line| parse_session_row(line))
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

    /// List the daemon's workspaces as `+N: name` rows, id order; the
    /// daemon's active one ends with ` active`.
    pub fn list_workspaces(&mut self) -> io::Result<Vec<WorkspaceSummary>> {
        let lines = self.send("list-workspaces")?;
        lines
            .iter()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                let (id, name) = parse_id_name_line(line, '+')?;
                let (name, active) = match name.strip_suffix(" active") {
                    Some(name) => (name, true),
                    None => (name.as_str(), false),
                };
                Ok(WorkspaceSummary {
                    id,
                    name: name.to_string(),
                    active,
                })
            })
            .collect()
    }

    /// Create a workspace (`new-workspace [-n name]`). The daemon answers
    /// the new id (`+N`), makes it active, and spawns its first session
    /// (named after the workspace, window `1`), so it is landable the
    /// moment it exists.
    pub fn new_workspace(&mut self, name: Option<&str>) -> io::Result<WorkspaceSummary> {
        let flag = name.map_or_else(String::new, |n| {
            format!(" -n {}", crate::quote_env_value(n))
        });
        let lines = self.send(&format!("new-workspace{flag}"))?;
        let line = lines
            .iter()
            .find(|line| !line.trim().is_empty())
            .ok_or_else(|| invalid_line("workspace id", ""))?;
        let (id, name) = parse_id_name_line(line, '+')?;
        Ok(WorkspaceSummary {
            id,
            name,
            active: true,
        })
    }

    /// Select a workspace (`select-workspace -t +N`): the workspace's
    /// previously active session resumes as the daemon's displayed one.
    pub fn select_workspace(&mut self, id: u64) -> io::Result<()> {
        self.send(&format!("select-workspace -t +{id}")).map(|_| ())
    }

    /// Rename a workspace (`rename-workspace -t +N <name>`).
    pub fn rename_workspace(&mut self, id: u64, name: &str) -> io::Result<()> {
        let cmd = format!("rename-workspace -t +{id} {}", crate::quote_env_value(name));
        self.send(&cmd).map(|_| ())
    }

    /// Kill a workspace (`kill-workspace -t +N`): its sessions end and the
    /// workspace is removed; an emptied active workspace lands clients on
    /// the first survivor.
    pub fn kill_workspace(&mut self, id: par_term_tmux::TmuxWorkspaceId) -> io::Result<()> {
        self.send(&format!("kill-workspace -t +{id}")).map(|_| ())
    }

    /// The displayed session's id (bare `switch-client`): the active
    /// workspace's active session — the one a no-target attach resumes.
    /// Empty when the daemon displays nothing.
    pub fn displayed_session_id(&mut self) -> io::Result<Option<u64>> {
        let lines = self.send("switch-client")?;
        let Some(line) = lines.iter().find(|line| !line.trim().is_empty()) else {
            return Ok(None);
        };
        line.trim()
            .strip_prefix('$')
            .and_then(|rest| rest.parse().ok())
            .map(Some)
            .ok_or_else(|| invalid_line("session id", line))
    }

    /// The daemon's dispatchable commands, one `name [feature …]` line per
    /// command plus the daemon-level `features …` line. Capability
    /// discovery: send once after `version`, then gate feature-specific
    /// commands on the tokens.
    pub fn list_commands(&mut self) -> io::Result<Vec<String>> {
        Ok(self
            .send("list-commands")?
            .into_iter()
            .filter(|line| !line.trim().is_empty())
            .collect())
    }

    /// Whether `list-commands` output advertises the `client-snapshot v1`
    /// capability — the only license to send `client-snapshot` on the
    /// wire; a daemon without the token must never see the command.
    pub fn snapshot_supported(list_commands: &[String]) -> bool {
        list_commands.iter().any(|line| {
            let mut words = line.split_whitespace();
            words.next() == Some("client-snapshot") && words.any(|token| token == "v1")
        })
    }

    /// The one-round-trip status read (`client-snapshot -t <window>`):
    /// every workspace, every session (the target window's owner marked),
    /// the owning session's windows, their panes, and the agent roster in
    /// one reply. Sent only when [`Self::snapshot_supported`] passed.
    pub fn client_snapshot(&mut self, window: TmuxWindowId) -> io::Result<ClientSnapshot> {
        let lines = self.send(&format!("client-snapshot -t @{window}"))?;
        ClientSnapshot::parse(&lines)
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

    /// Replay the daemon's pane-event ring (`events [--since <seq>]`,
    /// core 0.72.0): every lifecycle notification the daemon broadcast
    /// while this client was away (or never received), under daemon-wide
    /// monotonic sequence numbers. The reply carries the current ring
    /// `head <seq>` first, then one line per retained event with a seq
    /// strictly greater than `since`, `<seq> <epoch-ms> <wire-line>`.
    ///
    /// `since 0` is spelled bare `events` — the same reply either way
    /// (the daemon reads an absent `--since` as 0) — and `--since` past
    /// the head is an empty, healthy reply naming the head as the resume
    /// point. A daemon that predates the verb answers an `%error` body,
    /// which fails the parse here; callers degrade (the events are then
    /// only the registration replay of held state) the way they do for
    /// the other additive verbs this module sends.
    pub fn events_since(&mut self, since: u64) -> io::Result<EventReplay> {
        let command = if since == 0 {
            "events".to_string()
        } else {
            format!("events --since {since}")
        };
        parse_events_reply(&self.send(&command)?)
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

/// The foreground command name in a `pane-info -t %N` reply, or `None`
/// when the daemon did not report one.
///
/// Wire contract (core `docs/MUX.md`): one line, `%N @W COLSxROWS
/// [cmd=<standard base64>]`, where `cmd` is the basename of the deepest
/// descendant of the pane's child process (an idle shell reports itself).
/// The token is optional: a daemon that predates it replies without it, and
/// one that predates `pane-info` entirely replies an `%error` body that
/// never starts with the pane id, so both read as "unknown". Unknown must
/// never be treated as idle by a caller that wants to protect a job, only
/// as "no evidence to prompt on".
pub fn pane_foreground_command(reply: &[String], pane: TmuxPaneId) -> Option<String> {
    use base64::Engine as _;
    let prefix = format!("%{pane} ");
    let line = reply.iter().find(|line| line.starts_with(&prefix))?;
    let encoded = line
        .split_whitespace()
        .find_map(|token| token.strip_prefix("cmd="))?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .ok()?;
    let name = String::from_utf8(bytes).ok()?;
    (!name.is_empty()).then_some(name)
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

/// Parse one `list-sessions` row, `+W: wname: $N: name`: the workspace id
/// after the leading `+`, and the session id the LAST `$<digits>: ` marker
/// in the line (the daemon's documented resolution rule — a name carrying
/// `:` or `$` text stays whole because only the final marker separates it).
pub fn parse_session_row(line: &str) -> io::Result<SessionSummary> {
    let rest = line
        .strip_prefix('+')
        .ok_or_else(|| invalid_line("id", line))?;
    let (ws_id, rest) = rest
        .split_once(": ")
        .ok_or_else(|| invalid_line("id", line))?;
    let ws_id = ws_id.trim().parse().map_err(|_| invalid_line("id", line))?;

    // The session marker: the last `$<digits>: ` run in the workspace part.
    let bytes = rest.as_bytes();
    let mut marker = None;
    for (i, b) in bytes.iter().enumerate() {
        if *b != b'$' {
            continue;
        }
        let mut j = i + 1;
        while j < bytes.len() && bytes[j].is_ascii_digit() {
            j += 1;
        }
        if j > i + 1 && rest[j..].starts_with(':') {
            marker = Some((i, j));
        }
    }
    let Some((start, after_digits)) = marker else {
        return Err(invalid_line("id", line));
    };
    let sess_id = rest[start + 1..after_digits]
        .parse()
        .map_err(|_| invalid_line("id", line))?;
    // Workspace name = the text before the marker, minus its `: ` join.
    let workspace_name = rest[..start]
        .strip_suffix(' ')
        .and_then(|s| s.strip_suffix(':'))
        .unwrap_or(rest[..start].trim());
    Ok(SessionSummary {
        id: sess_id,
        name: rest[after_digits + 1..].trim().to_string(),
        workspace_id: ws_id,
        workspace_name: workspace_name.to_string(),
    })
}

fn invalid_line(kind: &str, line: &str) -> io::Error {
    io::Error::new(
        ErrorKind::InvalidData,
        format!("unrecognized {kind} reply line: {line:?}"),
    )
}

/// One placement from a `refresh-client -G` seed's `%graphics` section:
/// raw RGBA pixels plus the cell the image's top-left corner sits on and
/// its pixel size. ids are the daemon's graphic ids (replace-tracking on
/// a later re-seed); par-term re-encodes the pixels client-side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedGraphic {
    pub id: u64,
    pub col: u32,
    pub row: u32,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// One graphics placement line: `%gfx <id> <col> <row> <w> <h> <base64>`.
/// A malformed line is skipped, never an error — a pane can print any
/// text, so the daemon's section always rides inside an inert reply and
/// the walk stays best-effort.
fn parse_gfx_line(line: &str) -> Option<SeedGraphic> {
    use base64::Engine as _;
    let mut fields = line.split_whitespace();
    let tag = fields.next()?;
    if tag != "%gfx" {
        return None;
    }
    let id = fields.next()?.parse().ok()?;
    let col = fields.next()?.parse().ok()?;
    let row = fields.next()?.parse().ok()?;
    let width = fields.next()?.parse().ok()?;
    let height = fields.next()?.parse().ok()?;
    let rgba = base64::engine::general_purpose::STANDARD
        .decode(fields.next()?)
        .ok()?;
    // A trailing field or missing payload is malformed (six fields exactly).
    if fields.next().is_some() {
        return None;
    }
    Some(SeedGraphic {
        id,
        col,
        row,
        width,
        height,
        rgba,
    })
}

/// Split a `refresh-client -G` reply into its screen part and graphics
/// placements. A reply without the `%graphics-begin` marker is all screen
/// (`None` graphics) — an older daemon's rejection of the flag or a
/// no-graphics pane look identical and both mean "no replay". A pane can
/// print the marker itself, so the split is on the LAST begin marker, the
/// same discipline the daemon-side mirror uses. `%gfx` rows after the
/// closing marker (a pane printing them) are screen bytes again.
pub fn split_graphics_seed(reply: &[String]) -> (Vec<String>, Vec<SeedGraphic>) {
    let Some(begin) = reply
        .iter()
        .rposition(|l| l.trim_end() == "%graphics-begin")
    else {
        return (reply.to_vec(), Vec::new());
    };
    let screen = reply[..begin].to_vec();
    let mut graphics = Vec::new();
    for line in &reply[begin + 1..] {
        if line.trim_end() == "%graphics-end" {
            break;
        }
        if let Some(graphic) = parse_gfx_line(line) {
            graphics.push(graphic);
        }
    }
    (screen, graphics)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(reply: &[&str]) -> Vec<String> {
        reply.iter().map(|l| l.to_string()).collect()
    }

    /// Criterion 1 (card "Mux reconnect: replay daemon event ring via
    /// `events --since`"): an events reply parses into the head watermark
    /// and one event per row, the wire line riding whole (a pane can print
    /// any text; the row split is fixed at three fields).
    #[test]
    fn parses_an_events_reply_into_head_and_event_rows() {
        let replay = parse_events_reply(&lines(&[
            "head 3",
            "1 1730000000000 %window-add @2",
            "2 1730000000001 %pane-exited %3 7",
            "3 1730000000002 %agent-state-changed %3 claude working hook",
        ]))
        .expect("reply parses");
        assert_eq!(replay.head, 3);
        assert_eq!(
            replay.events,
            vec![
                ReplayedEvent {
                    seq: 1,
                    at_ms: 1730000000000,
                    line: "%window-add @2".to_string(),
                },
                ReplayedEvent {
                    seq: 2,
                    at_ms: 1730000000001,
                    line: "%pane-exited %3 7".to_string(),
                },
                ReplayedEvent {
                    seq: 3,
                    at_ms: 1730000000002,
                    line: "%agent-state-changed %3 claude working hook".to_string(),
                },
            ]
        );
    }

    /// `--since` past the head (a fresh daemon behind a watermark from a
    /// dead one) is an empty, healthy reply naming the head as the resume
    /// point — not an error.
    #[test]
    fn an_empty_ring_is_a_head_only_reply() {
        let replay = parse_events_reply(&lines(&["head 0"])).expect("head-only reply parses");
        assert_eq!(replay.head, 0);
        assert!(replay.events.is_empty());
    }

    /// A reply without the header is an error — an older daemon's
    /// `unknown command: events` body must never read as "nothing
    /// happened", or the caller would re-baseline its watermark at 0 and
    /// lose the resume point on the next attach.
    #[test]
    fn a_reply_without_a_head_header_is_an_error() {
        assert!(parse_events_reply(&lines(&["unknown command: events"])).is_err());
        assert!(parse_events_reply(&lines(&["%pane-exited %3 7"])).is_err());
        assert!(parse_events_reply(&lines(&[])).is_err());
        // A malformed head value is the same class of failure.
        assert!(parse_events_reply(&lines(&["head x"])).is_err());
    }

    /// Malformed event rows (missing the timestamp or the wire line) fail
    /// loudly rather than fabricating an event.
    #[test]
    fn malformed_event_rows_are_errors() {
        assert!(parse_events_reply(&lines(&["head 2", "1 1730000000000"])).is_err());
        assert!(parse_events_reply(&lines(&["head 2", "1 1730000000000 "])).is_err());
        assert!(parse_events_reply(&lines(&["head 2", "x 1730000000000 %a"])).is_err());
        // A wire line that is itself text starting with a token the header
        // match would take must stay an event row: only the exact `head`
        // token opens the header.
        let replay = parse_events_reply(&lines(&["head 2", "1 1730000000000 head 99"]))
            .expect("a wire line that begins with `head` is event text");
        assert_eq!(replay.events.len(), 1);
        assert_eq!(replay.events[0].line, "head 99");
    }

    #[test]
    fn pane_info_reply_yields_the_foreground_command() {
        // "c2xlZXA=" is base64("sleep").
        assert_eq!(
            pane_foreground_command(&lines(&["%3 @1 80x24 cmd=c2xlZXA="]), 3).as_deref(),
            Some("sleep")
        );
    }

    #[test]
    fn pane_info_reply_without_a_command_is_unknown() {
        // A daemon that predates the cmd token, an unknown-command error,
        // a reply for a different pane, an empty reply, and bad base64 all
        // read as "no foreground command known".
        assert_eq!(pane_foreground_command(&lines(&["%3 @1 80x24"]), 3), None);
        assert_eq!(
            pane_foreground_command(&lines(&["unknown command: pane-info"]), 3),
            None
        );
        assert_eq!(
            pane_foreground_command(&lines(&["%4 @1 80x24 cmd=c2xlZXA="]), 3),
            None
        );
        assert_eq!(pane_foreground_command(&[], 3), None);
        assert_eq!(
            pane_foreground_command(&lines(&["%3 @1 80x24 cmd=!!!"]), 3),
            None
        );
        assert_eq!(
            pane_foreground_command(&lines(&["%3 @1 80x24 cmd="]), 3),
            None
        );
    }

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

    #[test]
    fn parses_workspace_grouped_session_rows() {
        let row = parse_session_row("+0: main: $0: roster").expect("row parses");
        assert_eq!(row.workspace_id, 0);
        assert_eq!(row.workspace_name, "main");
        assert_eq!(row.id, 0);
        assert_eq!(row.name, "roster");

        // Non-zero ids on both levels.
        let row = parse_session_row("+2: other: $5: dev").expect("row parses");
        assert_eq!(row.workspace_id, 2);
        assert_eq!(row.workspace_name, "other");
        assert_eq!(row.id, 5);
        assert_eq!(row.name, "dev");
    }

    #[test]
    fn workspace_names_carrying_marker_text_do_not_confuse_the_parse() {
        // The documented rule: the session id is the LAST `$<digits>:`
        // marker in the row, so a WORKSPACE name imitating one (it precedes
        // the real marker) cannot steal the parse.
        let row = parse_session_row("+0: $7: weird: $1: dev").expect("row parses");
        assert_eq!(row.workspace_id, 0);
        assert_eq!(row.workspace_name, "$7: weird");
        assert_eq!(row.id, 1);
        assert_eq!(row.name, "dev");
    }

    #[test]
    fn parses_a_client_snapshot_reply() {
        let snap = ClientSnapshot::parse(&lines(&[
            "snapshot 1",
            "workspace +0 * main",
            "workspace +1 - lab",
            "session $0 - +0 work",
            "session $1 * +1 spaced name",
            "window @0 * main",
            "pane %0 title with spaces",
            "pane %1",
            "agent %0 claude working hook",
            "future-row-kind x y z",
        ]))
        .expect("snapshot parses");
        assert_eq!(snap.workspaces.len(), 2);
        assert_eq!(snap.workspaces[0].id, 0);
        assert!(snap.workspaces[0].active);
        assert_eq!(snap.workspaces[1].name, "lab");
        assert_eq!(snap.sessions.len(), 2);
        assert_eq!(snap.sessions[0].id, 0);
        assert_eq!(snap.sessions[0].workspace_id, 0);
        assert!(!snap.sessions[0].displayed);
        // The name is the rest of the line, so spaces survive.
        assert_eq!(snap.sessions[1].name, "spaced name");
        assert!(snap.sessions[1].displayed);
    }

    #[test]
    fn snapshot_rows_the_reader_does_not_need_are_ignored() {
        // window/pane/agent/worktree rows and a future header-less
        // garbage line do not break the two rosters the status read
        // needs.
        let snap = ClientSnapshot::parse(&lines(&[
            "snapshot 1",
            "window @0 * x",
            "agent %0 a s src",
            "worktree +0 repo - main /r/x",
        ]))
        .expect("snapshot parses");
        assert!(snap.workspaces.is_empty());
        assert!(snap.sessions.is_empty());
        assert!(snap.git.is_empty());
    }

    /// Criterion 1 + 4 (card "Adopt client-snapshot git rows and
    /// #{pane_title}"): `git` rows parse into the snapshot, a body
    /// without them reads as no badges, and a malformed git row is
    /// skipped like an unknown kind — never an error.
    #[test]
    fn parses_git_rows_tolerates_their_absence_and_skips_malformed_ones() {
        let snap = ClientSnapshot::parse(&lines(&[
            "snapshot 1",
            "workspace +0 * main",
            "workspace +1 - lab",
            "git +0 main - -",
            "git +1 feat 2 3",
            "git +2 dev - 1",
            "future-row-kind x y z",
        ]))
        .expect("snapshot parses");
        assert_eq!(
            snap.git,
            vec![
                GitStatusSummary {
                    id: 0,
                    branch: "main".to_string(),
                    ahead: 0,
                    behind: 0
                },
                GitStatusSummary {
                    id: 1,
                    branch: "feat".to_string(),
                    ahead: 2,
                    behind: 3
                },
                GitStatusSummary {
                    id: 2,
                    branch: "dev".to_string(),
                    ahead: 0,
                    behind: 1
                },
            ]
        );

        // The same body without git rows: no badges, and the rosters
        // still parse.
        let snap = ClientSnapshot::parse(&lines(&[
            "snapshot 1",
            "workspace +0 * main",
            "workspace +1 - lab",
            "future-row-kind x y z",
        ]))
        .expect("snapshot parses");
        assert!(snap.git.is_empty());
        assert_eq!(snap.workspaces.len(), 2);

        // Malformed git rows skip like unknown kinds: a `%`-prefixed id,
        // a non-count field, and a missing field are all tolerated.
        let snap = ClientSnapshot::parse(&lines(&[
            "snapshot 1",
            "git %0 main - -",
            "git +0 main x -",
            "git +0 main -",
        ]))
        .expect("snapshot parses");
        assert!(snap.git.is_empty());
    }

    #[test]
    fn snapshot_replies_without_a_v1_header_are_rejected() {
        assert!(ClientSnapshot::parse(&lines(&["unknown command: client-snapshot"])).is_err());
        assert!(ClientSnapshot::parse(&lines(&["snapshot 2", "workspace +0 * m"])).is_err());
        assert!(ClientSnapshot::parse(&lines(&[])).is_err());
    }

    #[test]
    fn the_snapshot_gate_needs_the_v1_token_on_the_client_snapshot_row() {
        assert!(MuxSessionClient::snapshot_supported(&lines(&[
            "capture-pane escape",
            "client-snapshot v1",
            "refresh-client cell-pixels chrome",
            "features replay-held-state",
        ])));
        // No token, an unversioned row, or the token on another command
        // all leave the gate shut.
        assert!(!MuxSessionClient::snapshot_supported(&lines(&[
            "client-snapshot",
            "pane-info cmd v1",
        ])));
        assert!(!MuxSessionClient::snapshot_supported(&lines(&[])));
    }

    #[test]
    fn rejects_malformed_session_rows() {
        // Pre-workspace `$N: name` shape, a bare id, and a missing session
        // marker all fail loudly — the daemon never sends them.
        assert!(parse_session_row("$0: roster").is_err());
        assert!(parse_session_row("+0").is_err());
        assert!(parse_session_row("+0: main: dev").is_err());
    }

    /// A 4-byte RGBA pixel, base64-encoded.
    fn pixel_b64() -> String {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD.encode([0x80u8, 0x40, 0x20, 0xFF])
    }

    #[test]
    fn reply_without_marker_is_all_screen() {
        let reply = lines(&["restore", "lines"]);
        let (screen, graphics) = split_graphics_seed(&reply);
        assert_eq!(screen, vec!["restore".to_string(), "lines".to_string()]);
        assert!(graphics.is_empty());
    }

    #[test]
    fn graphics_section_is_extracted_after_last_marker() {
        let b64 = pixel_b64();
        let reply = lines(&[
            "%graphics-begin", // a pane printed a fake marker; the LAST one wins
            "%gfx 7 1 2 4 2 <not-base64-but-ignored-screen>", // ignored: before the real section
            "%graphics-end",
            "screen continues here",
            "%graphics-begin",
            &format!("%gfx 7 1 2 4 2 {b64}"),
            "%gfx 8 0 0 1 1 AAAA",
            "%graphics-end",
        ]);
        let (screen, graphics) = split_graphics_seed(&reply);
        assert_eq!(
            screen,
            vec![
                "%graphics-begin".to_string(),
                "%gfx 7 1 2 4 2 <not-base64-but-ignored-screen>".to_string(),
                "%graphics-end".to_string(),
                "screen continues here".to_string(),
            ]
        );
        assert_eq!(graphics.len(), 2);
        assert_eq!(graphics[0].id, 7);
        assert_eq!((graphics[0].col, graphics[0].row), (1, 2));
        assert_eq!((graphics[0].width, graphics[0].height), (4, 2));
        assert_eq!(graphics[0].rgba, vec![0x80, 0x40, 0x20, 0xFF]);
        assert_eq!(graphics[1].id, 8);
    }

    #[test]
    fn malformed_gfx_lines_are_skipped() {
        let b64 = pixel_b64();
        let reply = lines(&[
            "%graphics-begin",
            "%gfx",                                 // no fields
            "%gfx 7 1 2 4 2",                       // missing payload
            &format!("%gfx 9 0 0 1 1 {b64} extra"), // trailing field
            &format!("%gfx 10 0 0 1 1 {b64}"),      // the one good row
        ]);
        let (_, graphics) = split_graphics_seed(&reply);
        assert_eq!(graphics.len(), 1);
        assert_eq!(graphics[0].id, 10);
    }

    #[test]
    fn unclosed_section_still_parses_rows() {
        // A pane could print `%graphics-begin` without a closing marker;
        // rows up to EOF parse rather than the whole tail becoming screen.
        let b64 = pixel_b64();
        let reply = lines(&["%graphics-begin", &format!("%gfx 3 0 0 1 1 {b64}")]);
        let (screen, graphics) = split_graphics_seed(&reply);
        assert!(screen.is_empty());
        assert_eq!(graphics.len(), 1);
        assert_eq!(graphics[0].id, 3);
    }
}
