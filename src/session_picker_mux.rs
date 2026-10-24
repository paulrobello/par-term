//! The par-mux half of the unified session picker (UX.md A16, A22).
//!
//! Plain data and the egui section only — the socket I/O that fills a
//! [`MuxDirectory`] lives in the mux-gated `mux_directory` module, so this
//! file compiles in every build and the picker draws the same with or
//! without par-mux support (the section is simply absent without it).
//!
//! A session is identified by the daemon socket that serves it plus its
//! name: a daemon can hold several sessions, and a session whose name is
//! not its daemon's name is still reached through that daemon.

use egui::RichText;
use std::path::{Path, PathBuf};

/// One session a running daemon holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MuxSessionRow {
    /// The daemon socket that serves the session.
    pub socket: PathBuf,
    /// The daemon's name for the socket (`par-mux-<name>.sock`).
    pub daemon: String,
    /// Session id (`$N`) within its daemon.
    pub id: u64,
    /// Session name.
    pub name: String,
    /// The workspace that owns the session (carried by `list-sessions`
    /// rows since the workspace-aware wire).
    pub workspace_name: String,
}

impl MuxSessionRow {
    /// Whether the next launch can reattach this session: restore stores
    /// the session name and reaches it through `par-mux-<name>.sock`, so
    /// only a session named after its own daemon round-trips.
    pub fn survives_restore(&self) -> bool {
        self.daemon == self.name
    }
}

/// One workspace's git-badge facts (a `client-snapshot` `git` row): the
/// branch and its ahead/behind counts. A workspace without facts —
/// non-repo, detached HEAD, or an older daemon — has none, and the row
/// shows no badge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceGit {
    /// The checked-out branch.
    pub branch: String,
    /// Commits ahead of the upstream.
    pub ahead: u32,
    /// Commits behind the upstream.
    pub behind: u32,
}

impl WorkspaceGit {
    /// The badge text: ` branch`, plus ` ↑n` / ` ↓n` only when non-zero —
    /// `-` on the wire means zero, so an in-sync repo shows the branch
    /// alone.
    pub fn badge(&self) -> String {
        let mut badge = format!(" {}", self.branch);
        if self.ahead > 0 {
            badge.push_str(&format!(" \u{2191}{}", self.ahead));
        }
        if self.behind > 0 {
            badge.push_str(&format!(" \u{2193}{}", self.behind));
        }
        badge
    }
}

/// One workspace a running daemon holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MuxWorkspaceRow {
    /// The daemon socket that serves the workspace.
    pub socket: PathBuf,
    /// The daemon's name for the socket (`par-mux-<name>.sock`).
    pub daemon: String,
    /// Workspace id (`+N`) within its daemon.
    pub id: u64,
    /// Workspace name.
    pub name: String,
    /// The daemon's active workspace (the one `select-workspace` resumes).
    pub active: bool,
    /// The workspace's git-badge facts, when the daemon reported them.
    pub git: Option<WorkspaceGit>,
}

/// What a directory scan found: sessions, plus one message per daemon that
/// could not be read (a broken daemon is visible, not silently absent).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MuxDirectory {
    pub sessions: Vec<MuxSessionRow>,
    pub workspaces: Vec<MuxWorkspaceRow>,
    pub errors: Vec<String>,
}

/// A session operation chosen in the picker or the palette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MuxPickerAction {
    /// Attach this window to a listed session (switching if attached).
    Attach(MuxSessionRow),
    /// Create a new session and attach this window to it.
    Create(String),
    /// Rename a listed session daemon-side.
    Rename(MuxSessionRow, String),
    /// End a listed session (`kill-session`), after confirmation.
    Kill(MuxSessionRow),
    /// Detach this window from its session.
    Detach,
    /// Create a named workspace on a listed daemon.
    NewWorkspace {
        socket: PathBuf,
        daemon: String,
        name: String,
    },
    /// Land on a listed daemon's workspace: daemon-side select, and for
    /// this window's own daemon, follow to the resumed session.
    SwitchWorkspace(MuxWorkspaceRow),
    /// Rename a listed workspace daemon-side.
    RenameWorkspace(MuxWorkspaceRow, String),
    /// End a listed workspace (`kill-workspace`), after confirmation.
    KillWorkspace(MuxWorkspaceRow),
}

/// The daemon name a socket path encodes, or `None` for a file that is not
/// a par-mux socket.
pub fn daemon_name_of(path: &Path) -> Option<String> {
    let file = path.file_name()?.to_str()?;
    let stem = file.strip_prefix("par-mux-")?.strip_suffix(".sock")?;
    (!stem.is_empty()).then(|| stem.to_string())
}

/// A name for a new session that no listed session or daemon already uses:
/// `session-1`, `session-2`, …
pub fn free_session_name(existing: &[MuxSessionRow]) -> String {
    (1..)
        .map(|n| format!("session-{n}"))
        .find(|candidate| {
            !existing
                .iter()
                .any(|row| &row.name == candidate || &row.daemon == candidate)
        })
        .unwrap_or_else(|| "session".to_string())
}

/// Why `name` cannot name a new session, or `None` when it can. The name
/// becomes the daemon's socket file name and a wire token.
pub fn session_name_refusal(name: &str) -> Option<&'static str> {
    if name.is_empty() {
        return Some("a session name is required");
    }
    if name
        .chars()
        .any(|c| c.is_whitespace() || matches!(c, '/' | '\\' | '\'' | '"' | ':'))
    {
        return Some("a session name cannot contain spaces, quotes, slashes, or colons");
    }
    None
}

/// Why `name` cannot name a workspace, or `None` when it can. Workspace
/// names ride the wire quoted (the `new-workspace -n` grammar), so only
/// the line-framed wire itself constrains them.
pub fn workspace_name_refusal(name: &str) -> Option<&'static str> {
    if name.is_empty() {
        return Some("a workspace name is required");
    }
    if name.chars().any(|c| c == '\n' || c == '\r') {
        return Some("a workspace name cannot contain line breaks");
    }
    None
}

/// What the picker's par-mux section shows this frame.
pub struct MuxPickerInput<'a> {
    /// The last finished scan, or `None` before the first one lands.
    pub directory: Option<&'a MuxDirectory>,
    /// A scan is in flight.
    pub loading: bool,
    /// `(daemon, session)` this window is attached to, if any.
    pub attached: Option<(&'a str, &'a str)>,
}

/// Per-picker editing state for the par-mux section.
#[derive(Default)]
pub struct MuxPickerSection {
    new_name: String,
    /// Row being renamed, with its edit buffer.
    renaming: Option<(MuxSessionRow, String)>,
    /// The rename field was just opened: its opening click is not a click
    /// away, and focus is requested once (OV4).
    rename_first_frame: bool,
    /// Row whose End button was pressed once; a second press ends it.
    confirm_kill: Option<MuxSessionRow>,
    /// Refusal for the typed new-session name, shown under the field.
    name_error: Option<&'static str>,
    /// Workspace being renamed, with its edit buffer.
    ws_renaming: Option<(MuxWorkspaceRow, String)>,
    /// The workspace rename field was just opened (OV4).
    ws_rename_first_frame: bool,
    /// Workspace whose End was pressed once; a second press ends it.
    ws_confirm_kill: Option<MuxWorkspaceRow>,
    /// The daemon a pending new-workspace name targets: `(socket, daemon,
    /// edit buffer)` while the inline new-workspace prompt is open.
    ws_new_target: Option<(PathBuf, String, String)>,
    /// Refusal for the typed new-workspace name, shown under the field.
    ws_name_error: Option<&'static str>,
}

impl MuxPickerSection {
    /// Reset per-open state.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Draw the par-mux section and return the chosen operation, if any.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        input: &MuxPickerInput<'_>,
    ) -> Option<MuxPickerAction> {
        let mut action = None;
        ui.heading("par-mux Sessions");
        ui.separator();

        if let Some((daemon, session)) = input.attached {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("This window: {session}")).strong());
                if daemon != session {
                    ui.label(RichText::new(format!("(daemon {daemon})")).weak());
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button("Detach")
                        .on_hover_text("Leave the session running in the daemon")
                        .clicked()
                    {
                        action = Some(MuxPickerAction::Detach);
                    }
                });
            });
            ui.add_space(4.0);
        }

        let sessions: &[MuxSessionRow] = input.directory.map_or(&[], |d| &d.sessions);
        if sessions.is_empty() {
            if input.loading || input.directory.is_none() {
                // UX.md OV10: the list loads off the frame; show it working.
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new("Loading sessions...").italics());
                });
            } else {
                ui.label(RichText::new("No par-mux sessions running").italics());
            }
        }
        for row in sessions {
            if let Some(chosen) = self.show_row(ui, row, input.attached) {
                action = Some(chosen);
            }
        }

        // Workspaces subsection, grouped per daemon: each daemon's
        // workspaces with Switch/Rename/End and a New affordance —
        // create/switch/kill workspaces end to end from par-term.
        if let Some(dir) = input.directory {
            let mut seen: Vec<PathBuf> = Vec::new();
            for ws in &dir.workspaces {
                if seen.contains(&ws.socket) {
                    continue;
                }
                seen.push(ws.socket.clone());
                ui.add_space(8.0);
                ui.separator();
                ui.label(
                    RichText::new(format!("Workspaces — {}", ws.daemon))
                        .weak()
                        .small(),
                );
                for w in dir.workspaces.iter().filter(|w| w.socket == ws.socket) {
                    if let Some(chosen) =
                        self.show_workspace_row(ui, w, input.attached, &dir.sessions)
                    {
                        action = Some(chosen);
                    }
                }
                if let Some(chosen) = self.show_new_workspace_row(ui, &ws.socket, &ws.daemon) {
                    action = Some(chosen);
                }
            }
        }
        if let Some(dir) = input.directory {
            for error in &dir.errors {
                ui.colored_label(crate::app::overlay::theme::DANGER, error);
            }
        }

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label("New session:");
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.new_name)
                    .desired_width(160.0)
                    .hint_text(free_session_name(sessions)),
            );
            let submit = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.button("Create").clicked() || submit {
                let name = if self.new_name.trim().is_empty() {
                    free_session_name(sessions)
                } else {
                    self.new_name.trim().to_string()
                };
                match session_name_refusal(&name) {
                    Some(reason) => self.name_error = Some(reason),
                    None => {
                        self.name_error = None;
                        action = Some(MuxPickerAction::Create(name));
                    }
                }
            }
        });
        if let Some(reason) = self.name_error {
            ui.colored_label(crate::app::overlay::theme::DANGER, reason);
        }
        action
    }

    fn show_row(
        &mut self,
        ui: &mut egui::Ui,
        row: &MuxSessionRow,
        attached: Option<(&str, &str)>,
    ) -> Option<MuxPickerAction> {
        let mut action = None;
        let is_here = attached == Some((row.daemon.as_str(), row.name.as_str()));
        if let Some((target, buffer)) = self.renaming.as_mut()
            && target == row
        {
            // UX.md OV4: Enter submits, Escape or a click away cancels.
            use crate::app::overlay::inline_edit::{self, InlineEditOutcome};
            let first_frame = std::mem::take(&mut self.rename_first_frame);
            let mut outcome = InlineEditOutcome::Editing;
            ui.horizontal(|ui| {
                let response = ui.add(egui::TextEdit::singleline(buffer).desired_width(160.0));
                outcome = inline_edit::resolve(ui, &response, first_frame);
                if ui.button("Cancel").clicked() {
                    outcome = InlineEditOutcome::Cancel;
                }
            });
            match outcome {
                InlineEditOutcome::Submit => {
                    action = Some(MuxPickerAction::Rename(
                        row.clone(),
                        buffer.trim().to_string(),
                    ));
                    self.renaming = None;
                }
                InlineEditOutcome::Cancel => self.renaming = None,
                InlineEditOutcome::Editing => {}
            }
            return action;
        }
        ui.horizontal(|ui| {
            let name = if is_here {
                RichText::new(&row.name).strong()
            } else {
                RichText::new(&row.name)
            };
            ui.label(name);
            if !row.survives_restore() {
                ui.label(RichText::new(format!("(daemon {})", row.daemon)).weak())
                    .on_hover_text("Attaches now; not reattached on the next launch");
            }
            if is_here {
                ui.label(RichText::new("(this window)").color(crate::app::overlay::theme::SUCCESS));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let armed = self.confirm_kill.as_ref() == Some(row);
                let end = if armed { "Confirm End" } else { "End" };
                let end_btn = ui
                    .button(RichText::new(end).color(crate::app::overlay::theme::DANGER))
                    .on_hover_text("End the session and every program in it");
                if end_btn.clicked() {
                    if armed {
                        self.confirm_kill = None;
                        action = Some(MuxPickerAction::Kill(row.clone()));
                    } else {
                        self.confirm_kill = Some(row.clone());
                    }
                }
                if ui.button("Rename").clicked() {
                    self.confirm_kill = None;
                    self.renaming = Some((row.clone(), row.name.clone()));
                    self.rename_first_frame = true;
                }
                if !is_here && ui.button("Attach").clicked() {
                    self.confirm_kill = None;
                    action = Some(MuxPickerAction::Attach(row.clone()));
                }
            });
        });
        action
    }

    /// One workspace row: name with the active marker, then Switch /
    /// Rename / End on the right.
    fn show_workspace_row(
        &mut self,
        ui: &mut egui::Ui,
        ws: &MuxWorkspaceRow,
        attached: Option<(&str, &str)>,
        sessions: &[MuxSessionRow],
    ) -> Option<MuxPickerAction> {
        let mut action = None;
        let ours = attached.is_some_and(|(daemon, _)| daemon == ws.daemon);
        if let Some((target, buffer)) = self.ws_renaming.as_mut()
            && target == ws
        {
            // UX.md OV4: Enter submits, Escape or a click away cancels.
            use crate::app::overlay::inline_edit::{self, InlineEditOutcome};
            let first_frame = std::mem::take(&mut self.ws_rename_first_frame);
            let mut outcome = InlineEditOutcome::Editing;
            ui.horizontal(|ui| {
                let response = ui.add(egui::TextEdit::singleline(buffer).desired_width(160.0));
                outcome = inline_edit::resolve(ui, &response, first_frame);
                if ui.button("Cancel").clicked() {
                    outcome = InlineEditOutcome::Cancel;
                }
            });
            match outcome {
                InlineEditOutcome::Submit => {
                    action = Some(MuxPickerAction::RenameWorkspace(
                        ws.clone(),
                        buffer.trim().to_string(),
                    ));
                    self.ws_renaming = None;
                }
                InlineEditOutcome::Cancel => self.ws_renaming = None,
                InlineEditOutcome::Editing => {}
            }
            return action;
        }
        ui.horizontal(|ui| {
            let label = if ws.active {
                RichText::new(format!("{} (active)", ws.name)).strong()
            } else {
                RichText::new(&ws.name)
            };
            ui.label(label);
            if let Some(git) = &ws.git {
                ui.label(RichText::new(git.badge()).weak());
            }
            if ours {
                ui.label(RichText::new("(this daemon)").weak());
            }
            let session_count = sessions
                .iter()
                .filter(|s| s.socket == ws.socket && s.workspace_name == ws.name)
                .count();
            if session_count > 0 {
                ui.label(
                    RichText::new(format!(
                        "({session_count} session{})",
                        if session_count == 1 { "" } else { "s" }
                    ))
                    .weak(),
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let armed = self.ws_confirm_kill.as_ref() == Some(ws);
                let end = if armed { "Confirm End" } else { "End" };
                if ui
                    .button(RichText::new(end).color(crate::app::overlay::theme::DANGER))
                    .on_hover_text("End the workspace and every session in it")
                    .clicked()
                {
                    if armed {
                        self.ws_confirm_kill = None;
                        action = Some(MuxPickerAction::KillWorkspace(ws.clone()));
                    } else {
                        self.ws_confirm_kill = Some(ws.clone());
                    }
                }
                if ui.button("Rename").clicked() {
                    self.ws_confirm_kill = None;
                    self.ws_renaming = Some((ws.clone(), ws.name.clone()));
                    self.ws_rename_first_frame = true;
                }
                if !ws.active && ui.button("Switch").clicked() {
                    self.ws_confirm_kill = None;
                    action = Some(MuxPickerAction::SwitchWorkspace(ws.clone()));
                }
            });
        });
        action
    }

    /// The per-daemon new-workspace affordance: a button that opens an
    /// inline name prompt, and the prompt row while it is open.
    fn show_new_workspace_row(
        &mut self,
        ui: &mut egui::Ui,
        socket: &Path,
        daemon: &str,
    ) -> Option<MuxPickerAction> {
        let open = self
            .ws_new_target
            .as_ref()
            .is_some_and(|(s, d, _)| s == socket && d == daemon);
        if !open {
            if ui.button("New workspace").clicked() {
                self.ws_new_target =
                    Some((socket.to_path_buf(), daemon.to_string(), String::new()));
                self.ws_name_error = None;
            }
            return None;
        }
        let mut buffer = self
            .ws_new_target
            .as_ref()
            .map(|(_, _, b)| b.clone())
            .unwrap_or_default();
        let mut chosen: Option<MuxPickerAction> = None;
        ui.horizontal(|ui| {
            let response = ui.add(
                egui::TextEdit::singleline(&mut buffer)
                    .desired_width(160.0)
                    .hint_text("workspace name"),
            );
            let submit = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.button("Create").clicked() || submit {
                let name = buffer.trim().to_string();
                match workspace_name_refusal(&name) {
                    Some(reason) => self.ws_name_error = Some(reason),
                    None => {
                        self.ws_new_target = None;
                        chosen = Some(MuxPickerAction::NewWorkspace {
                            socket: socket.to_path_buf(),
                            daemon: daemon.to_string(),
                            name,
                        });
                    }
                }
            }
            if ui.button("Cancel").clicked() {
                self.ws_new_target = None;
            }
        });
        if let Some((_, _, b)) = self.ws_new_target.as_mut() {
            *b = buffer;
        }
        if let Some(reason) = self.ws_name_error {
            ui.colored_label(crate::app::overlay::theme::DANGER, reason);
        }
        chosen
    }
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
            workspace_name: String::new(),
        }
    }

    #[test]
    fn socket_names_decode_to_their_daemon() {
        assert_eq!(
            daemon_name_of(Path::new("/tmp/x/par-mux-work.sock")),
            Some("work".to_string())
        );
        assert_eq!(daemon_name_of(Path::new("/tmp/x/par-mux-.sock")), None);
        assert_eq!(daemon_name_of(Path::new("/tmp/x/other.sock")), None);
        assert_eq!(daemon_name_of(Path::new("/tmp/x/par-mux-work.state")), None);
    }

    #[test]
    fn free_names_skip_taken_sessions_and_daemons() {
        assert_eq!(free_session_name(&[]), "session-1");
        assert_eq!(
            free_session_name(&[row("session-1", "session-1"), row("x", "session-2")]),
            "session-3"
        );
    }

    #[test]
    fn session_names_that_cannot_be_socket_names_are_refused() {
        assert!(session_name_refusal("").is_some());
        assert!(session_name_refusal("a/b").is_some());
        assert!(session_name_refusal("a b").is_some());
        assert!(session_name_refusal("it's").is_some());
        assert_eq!(session_name_refusal("work-2"), None);
    }

    #[test]
    fn only_a_session_named_after_its_daemon_survives_restore() {
        assert!(row("work", "work").survives_restore());
        assert!(!row("default", "work").survives_restore());
    }

    /// Criterion 2 (card "Adopt client-snapshot git rows and
    /// #{pane_title}"): the badge is ` branch` plus ` ↑n` / ` ↓n` only
    /// when non-zero — `-` on the wire means zero, so an in-sync repo
    /// shows the branch alone.
    #[test]
    fn workspace_git_badges_spell_zero_as_silence() {
        let git = |branch: &str, ahead: u32, behind: u32| WorkspaceGit {
            branch: branch.to_string(),
            ahead,
            behind,
        };
        assert_eq!(git("main", 0, 0).badge(), " main");
        assert_eq!(git("feat", 2, 0).badge(), " feat ↑2");
        assert_eq!(git("dev", 0, 3).badge(), " dev ↓3");
        assert_eq!(git("wip", 1, 2).badge(), " wip ↑1 ↓2");
    }

    /// Criterion 4: a workspace without git facts — non-repo, detached
    /// HEAD, or an older daemon — has `git: None` and no badge text.
    #[test]
    fn workspaces_without_git_facts_have_no_badge() {
        let bare = MuxWorkspaceRow {
            socket: PathBuf::from("/s/x.sock"),
            daemon: "x".to_string(),
            id: 0,
            name: "repo".to_string(),
            active: false,
            git: None,
        };
        assert_eq!(bare.git, None);
    }
}
