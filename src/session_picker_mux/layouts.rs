//! The session picker's named-layouts subsection (core 0.70.0's
//! `save-layout` / `load-layout` / `list-layouts` / `kill-layout`): the
//! data rows, the wire-shape parsers, and the [`MuxPickerSection`] methods
//! that draw a per-daemon Save/Load/Delete list. Split out of the picker
//! module to hold it under the line-count gate.

use super::{MuxPickerAction, MuxPickerSection};
use egui::RichText;
use std::path::{Path, PathBuf};

/// One saved named layout (`save-layout`) a daemon holds: the pane-tree
/// snapshot is daemon-wide, so the row only carries where it lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MuxLayoutRow {
    /// The daemon socket that holds the layout.
    pub socket: PathBuf,
    /// The daemon's name for the socket (`par-mux-<name>.sock`).
    pub daemon: String,
    /// The layout's name.
    pub name: String,
}

/// Why `name` cannot name a saved layout, or `None` when it can. The
/// daemon's grammar is `[A-Za-z0-9._-]+`; mirrors it so the picker can
/// refuse before the round trip.
pub fn layout_name_refusal(name: &str) -> Option<&'static str> {
    if name.is_empty() {
        return Some("a layout name is required");
    }
    if name
        .chars()
        .any(|c| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')))
    {
        return Some("a layout name can only use letters, digits, dots, dashes, and underscores");
    }
    None
}

/// The saved-layout names in a `list-layouts` reply: the non-empty body
/// lines, one name each (an empty store replies no names).
pub fn layout_names_from_reply(reply: &[String]) -> Vec<String> {
    reply
        .iter()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

impl MuxPickerSection {
    /// One saved-layout row: the name, then Delete and Load on the right.
    /// Both buttons confirm with a second press; Load's second press sends
    /// the replace-with-force the daemon demands while live panes exist.
    pub(super) fn show_layout_row(
        &mut self,
        ui: &mut egui::Ui,
        row: &MuxLayoutRow,
    ) -> Option<MuxPickerAction> {
        let mut action = None;
        ui.horizontal(|ui| {
            ui.label(&row.name);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let armed = self.confirm_load.as_ref() == Some(row);
                let load = if armed { "Confirm Replace" } else { "Load" };
                if ui
                    .button(load)
                    .on_hover_text("Replace the daemon's panes with this layout")
                    .clicked()
                {
                    if armed {
                        self.confirm_load = None;
                        action = Some(MuxPickerAction::LoadLayout {
                            socket: row.socket.clone(),
                            daemon: row.daemon.clone(),
                            name: row.name.clone(),
                            force: true,
                        });
                    } else {
                        self.confirm_load = Some(row.clone());
                    }
                }
                let armed = self.confirm_kill_layout.as_ref() == Some(row);
                let delete = if armed { "Confirm Delete" } else { "Delete" };
                if ui
                    .button(RichText::new(delete).color(crate::app::overlay::theme::DANGER))
                    .on_hover_text("Delete the saved layout")
                    .clicked()
                {
                    if armed {
                        self.confirm_kill_layout = None;
                        action = Some(MuxPickerAction::KillLayout {
                            socket: row.socket.clone(),
                            daemon: row.daemon.clone(),
                            name: row.name.clone(),
                        });
                    } else {
                        self.confirm_kill_layout = Some(row.clone());
                    }
                }
            });
        });
        action
    }

    /// The per-daemon save-layout affordance: a button that opens an
    /// inline name prompt, and the prompt row while it is open.
    pub(super) fn show_save_layout_row(
        &mut self,
        ui: &mut egui::Ui,
        socket: &Path,
        daemon: &str,
    ) -> Option<MuxPickerAction> {
        let open = self
            .layout_save_target
            .as_ref()
            .is_some_and(|(s, d, _)| s == socket && d == daemon);
        if !open {
            if ui.button("Save layout").clicked() {
                self.layout_save_target =
                    Some((socket.to_path_buf(), daemon.to_string(), String::new()));
                self.layout_name_error = None;
                self.confirm_load = None;
                self.confirm_kill_layout = None;
            }
            return None;
        }
        let mut buffer = self
            .layout_save_target
            .as_ref()
            .map(|(_, _, b)| b.clone())
            .unwrap_or_default();
        let mut chosen: Option<MuxPickerAction> = None;
        ui.horizontal(|ui| {
            let response = ui.add(
                egui::TextEdit::singleline(&mut buffer)
                    .desired_width(160.0)
                    .hint_text("layout name"),
            );
            let submit = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.button("Save").clicked() || submit {
                let name = buffer.trim().to_string();
                match layout_name_refusal(&name) {
                    Some(reason) => self.layout_name_error = Some(reason),
                    None => {
                        self.layout_save_target = None;
                        chosen = Some(MuxPickerAction::SaveLayout {
                            socket: socket.to_path_buf(),
                            daemon: daemon.to_string(),
                            name,
                        });
                    }
                }
            }
            if ui.button("Cancel").clicked() {
                self.layout_save_target = None;
            }
        });
        if let Some((_, _, b)) = self.layout_save_target.as_mut() {
            *b = buffer;
        }
        if let Some(reason) = self.layout_name_error {
            ui.colored_label(crate::app::overlay::theme::DANGER, reason);
        }
        chosen
    }
}
