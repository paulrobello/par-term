//! The profile editor's Keyboard Shortcut field: a recorder that targets
//! the profile's `open_profile:<id>` registry binding (UX.md PR3).
//!
//! The editor has no access to `config.keybindings`, so it stages the
//! recorded chord and, on Done, hands it to the Profile Management section
//! as [`PendingBinding`]. That section reads the profile's current chord
//! into `config_shortcut` each frame, runs the conflict check, and writes
//! the binding.

use super::ProfileModalUI;
use crate::input_tab::{capture_key_combo, display_key_combo};
use par_term_config::{KeyBinding, ProfileId};

/// A binding change for one profile, applied to `config.keybindings` by the
/// owner of the config. `chord: None` removes the binding.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PendingBinding {
    pub(crate) profile: ProfileId,
    pub(crate) chord: Option<String>,
}

/// The registry action that opens `profile` in a new tab.
pub(crate) fn open_profile_action(profile: ProfileId) -> String {
    format!("open_profile:{profile}")
}

/// Point `profile`'s `open_profile` binding at `chord`, replacing any
/// existing one; `None` removes it.
pub(crate) fn apply_binding(bindings: &mut Vec<KeyBinding>, change: &PendingBinding) {
    let action = open_profile_action(change.profile);
    bindings.retain(|b| b.action != action);
    if let Some(chord) = &change.chord {
        bindings.push(KeyBinding {
            key: chord.clone(),
            action,
        });
    }
}

/// The chord bound to `profile`'s `open_profile` action, if any.
pub(crate) fn bound_chord(bindings: &[KeyBinding], profile: ProfileId) -> Option<String> {
    let action = open_profile_action(profile);
    bindings
        .iter()
        .find(|b| b.action == action)
        .map(|b| b.key.clone())
}

impl ProfileModalUI {
    /// The profile open in the editor, if any.
    pub(crate) fn editing_profile(&self) -> Option<ProfileId> {
        self.editing_id
    }

    /// Set by the Settings window each frame: the chord currently bound to
    /// the profile being edited.
    pub(crate) fn set_config_shortcut(&mut self, chord: Option<String>) {
        self.config_shortcut = chord;
    }

    /// The staged binding change, once Done has saved the profile.
    pub(crate) fn take_pending_binding(&mut self) -> Option<PendingBinding> {
        self.pending_binding.take()
    }

    /// A freshly recorded chord the owner has not conflict-checked yet,
    /// with the action it would bind.
    pub(crate) fn take_chord_to_check(&mut self) -> Option<(String, String)> {
        let chord = self.unchecked_chord.take()?;
        let id = self.editing_id?;
        Some((chord, open_profile_action(id)))
    }

    pub(crate) fn set_shortcut_conflict(&mut self, conflict: Option<String>) {
        self.shortcut_conflict = conflict;
    }

    /// Forget any staged shortcut edit (editor closed or form cleared).
    pub(super) fn reset_shortcut_state(&mut self) {
        self.staged_shortcut = None;
        self.shortcut_recording = false;
        self.shortcut_conflict = None;
        self.unchecked_chord = None;
    }

    /// Move the staged edit into the pending binding when the profile is
    /// saved.
    pub(super) fn commit_shortcut(&mut self, profile: ProfileId) {
        if let Some(chord) = self.staged_shortcut.take() {
            self.pending_binding = Some(PendingBinding { profile, chord });
        }
        self.reset_shortcut_state();
    }

    /// The chord the field shows: the staged edit, else the bound chord,
    /// else a legacy `keyboard_shortcut` string not yet migrated.
    fn shown_shortcut(&self) -> Option<String> {
        match &self.staged_shortcut {
            Some(staged) => staged.clone(),
            None => self.config_shortcut.clone().or_else(|| {
                (!self.temp_keyboard_shortcut.is_empty())
                    .then(|| self.temp_keyboard_shortcut.clone())
            }),
        }
    }

    /// One form-grid row: label, current chord, Record and Clear.
    pub(super) fn render_shortcut_row(&mut self, ui: &mut egui::Ui) {
        ui.label("Keyboard Shortcut:");
        ui.horizontal(|ui| {
            if self.shortcut_recording {
                if let Some(chord) = capture_key_combo(ui) {
                    self.staged_shortcut = Some(Some(chord.clone()));
                    self.temp_keyboard_shortcut.clear();
                    self.unchecked_chord = Some(chord);
                    self.shortcut_recording = false;
                } else if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.shortcut_recording = false;
                }
            }

            if self.shortcut_recording {
                ui.colored_label(egui::Color32::YELLOW, "Press key combo... (Esc to cancel)");
            } else {
                match self.shown_shortcut() {
                    Some(chord) => ui.monospace(display_key_combo(&chord)),
                    None => ui.weak("(not set)"),
                };
            }

            let label = if self.shortcut_recording {
                "Cancel"
            } else {
                "Record"
            };
            if ui.button(label).clicked() {
                self.shortcut_recording = !self.shortcut_recording;
                self.shortcut_conflict = None;
            }
            if !self.shortcut_recording
                && self.shown_shortcut().is_some()
                && ui.button("Clear").clicked()
            {
                self.staged_shortcut = Some(None);
                self.temp_keyboard_shortcut.clear();
                self.shortcut_conflict = None;
            }
        });
        ui.end_row();
        if let Some(conflict) = &self.shortcut_conflict {
            ui.label("");
            ui.colored_label(
                egui::Color32::RED,
                format!("Keybinding conflict: {conflict}"),
            );
            ui.end_row();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kb(key: &str, action: &str) -> KeyBinding {
        KeyBinding {
            key: key.to_string(),
            action: action.to_string(),
        }
    }

    #[test]
    fn recording_replaces_the_profiles_open_binding() {
        let id = uuid::Uuid::new_v4();
        let other = uuid::Uuid::new_v4();
        let mut bindings = vec![
            kb("Cmd+1", &open_profile_action(id)),
            kb("Cmd+2", &open_profile_action(other)),
            kb("Cmd+3", "new_tab"),
        ];
        apply_binding(
            &mut bindings,
            &PendingBinding {
                profile: id,
                chord: Some("Cmd+9".to_string()),
            },
        );
        assert_eq!(bound_chord(&bindings, id).as_deref(), Some("Cmd+9"));
        assert_eq!(
            bindings
                .iter()
                .filter(|b| b.action == open_profile_action(id))
                .count(),
            1,
            "replaced, not duplicated"
        );
        assert_eq!(bound_chord(&bindings, other).as_deref(), Some("Cmd+2"));
        assert_eq!(bindings.len(), 3);
    }

    #[test]
    fn clearing_removes_only_that_profiles_binding() {
        let id = uuid::Uuid::new_v4();
        let mut bindings = vec![
            kb("Cmd+1", &open_profile_action(id)),
            kb("Cmd+3", "new_tab"),
        ];
        apply_binding(
            &mut bindings,
            &PendingBinding {
                profile: id,
                chord: None,
            },
        );
        assert_eq!(bound_chord(&bindings, id), None);
        assert_eq!(bindings.len(), 1);
    }

    /// The recorder flow through the editor: record a chord in the field,
    /// press Done, and the owner's config carries exactly one
    /// `open_profile:<id>` binding for it.
    #[test]
    fn editor_recording_lands_as_the_profiles_open_binding() {
        let profile = par_term_config::Profile::new("work");
        let id = profile.id;
        let mut modal = ProfileModalUI::new();
        modal.add_profile_for_test(profile);
        modal.start_edit_for_test(id);
        let mut bindings = vec![kb("Cmd+1", &open_profile_action(id))];

        modal.set_config_shortcut(bound_chord(&bindings, id));
        modal.staged_shortcut = Some(Some("Cmd+Shift+9".to_string()));
        modal.unchecked_chord = Some("Cmd+Shift+9".to_string());
        assert_eq!(
            modal.take_chord_to_check(),
            Some(("Cmd+Shift+9".to_string(), open_profile_action(id)))
        );

        modal.save_form();
        let change = modal.take_pending_binding().expect("Done stages a binding");
        apply_binding(&mut bindings, &change);
        assert_eq!(bindings.len(), 1);
        assert_eq!(bound_chord(&bindings, id).as_deref(), Some("Cmd+Shift+9"));
        assert!(modal.take_pending_binding().is_none());
    }

    /// The footer Save with the editor still open finishes the form and
    /// writes the recorded chord into the config it hands back, without a
    /// frame of the Profiles section in between.
    #[test]
    fn footer_save_with_the_editor_open_keeps_the_recorded_shortcut() {
        let profile = par_term_config::Profile::new("work");
        let id = profile.id;
        let mut settings =
            crate::settings_ui::SettingsUI::new_for_tests(par_term_config::Config::default());
        settings.sync_profiles(vec![profile]);
        settings.profile_modal_ui.start_edit_for_test(id);
        settings.profile_modal_ui.staged_shortcut = Some(Some("Cmd+Shift+9".to_string()));

        let saved = settings.request_save().expect("the form is valid");
        assert_eq!(
            bound_chord(&saved.keybindings, id).as_deref(),
            Some("Cmd+Shift+9")
        );
        settings.take_profile_save_request();
        assert!(!settings.has_unsaved_changes());
    }
}
