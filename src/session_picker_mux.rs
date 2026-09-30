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
}

impl MuxSessionRow {
    /// Whether the next launch can reattach this session: restore stores
    /// the session name and reaches it through `par-mux-<name>.sock`, so
    /// only a session named after its own daemon round-trips.
    pub fn survives_restore(&self) -> bool {
        self.daemon == self.name
    }
}

/// What a directory scan found: sessions, plus one message per daemon that
/// could not be read (a broken daemon is visible, not silently absent).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MuxDirectory {
    pub sessions: Vec<MuxSessionRow>,
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
}
