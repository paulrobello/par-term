//! Baseline snapshot, Revert, and the close-with-unsaved-changes prompt.
//!
//! Edits apply live to every terminal window (the host forwards the working
//! config each frame), so the working config alone cannot tell what is saved.
//! The baseline is the config as it was when Settings opened or last saved:
//!
//! - **Revert** copies the baseline back into the working config; the next
//!   live update carries it to every window.
//! - **Save** writes the working config and moves the baseline to it.
//! - **Close** with unsaved changes shows a Save / Revert / Cancel prompt.
//!   Closing never persists unsaved edits: the host restores the baseline and
//!   persists only the collapsed-section state.

use std::collections::HashSet;

use par_term_config::Config;

use super::SettingsUI;

/// True when two configs serialize to the same YAML value.
///
/// `Config` has no `PartialEq`; comparing the serialized form covers every
/// field. A serialization failure counts as a difference so callers err on
/// the side of prompting instead of silently dropping an edit.
pub fn configs_equal(a: &Config, b: &Config) -> bool {
    match (serde_yaml_ng::to_value(a), serde_yaml_ng::to_value(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// Collapsed-section list to persist on close, or `None` when it matches the
/// persisted list. Compared as sets: the live state is a `HashSet`, so its
/// iteration order is arbitrary and a list comparison would write every close.
pub fn collapsed_sections_to_persist(
    persisted: &[String],
    current: &HashSet<String>,
) -> Option<Vec<String>> {
    let persisted: HashSet<&str> = persisted.iter().map(String::as_str).collect();
    let current_refs: HashSet<&str> = current.iter().map(String::as_str).collect();
    if persisted == current_refs {
        None
    } else {
        let mut list: Vec<String> = current.iter().cloned().collect();
        list.sort();
        Some(list)
    }
}

/// A choice made in the close-with-unsaved-changes prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClosePromptChoice {
    /// Save config (and profiles when they changed), then close.
    Save,
    /// Restore the baseline, then close.
    Revert,
    /// Keep the window open with the edits in place.
    Cancel,
}

impl SettingsUI {
    /// The config as last opened or saved.
    pub fn baseline_config(&self) -> &Config {
        &self.baseline_config
    }

    /// Record shader on/off states that changed outside Settings (keybinding
    /// toggles) in the baseline, so they are neither unsaved edits nor undone
    /// by Revert.
    pub fn set_baseline_shader_states(&mut self, custom: Option<bool>, cursor: Option<bool>) {
        if let Some(enabled) = custom {
            self.baseline_config.shader.custom_shader_enabled = enabled;
        }
        if let Some(enabled) = cursor {
            self.baseline_config.shader.cursor_shader_enabled = enabled;
        }
    }

    /// Whether anything differs from what was last opened or saved: the
    /// working config, staged font fields, or the profile editor.
    pub fn has_unsaved_changes(&self) -> bool {
        self.font_pending_changes
            || self.profile_modal_ui.has_unsaved_changes()
            || !configs_equal(&self.config, &self.baseline_config)
    }

    /// Finish a Save: apply staged font fields, capture collapsed sections,
    /// regenerate snippet/action keybindings, and move the baseline to the
    /// result. Returns the config to write.
    pub fn commit_save(&mut self) -> Config {
        if self.font_pending_changes {
            self.apply_font_changes();
        }
        self.has_changes = false;
        self.sync_collapsed_sections_to_config();
        self.config.generate_snippet_action_keybindings();
        self.baseline_config = self.config.clone();
        self.config.clone()
    }

    /// Restore the baseline config and profiles, dropping every unsaved edit.
    ///
    /// Open inline editors are closed as well: their indices point into the
    /// lists that were just replaced, so saving one afterwards could write
    /// to the wrong row or past the end.
    pub fn revert_to_baseline(&mut self) {
        self.config = self.baseline_config.clone();
        self.sync_all_temps_from_config();
        self.has_changes = false;
        self.profile_modal_ui.cancel_list_changes();
        self.cancel_open_editors();
    }

    fn cancel_open_editors(&mut self) {
        self.keybinding_recording_index = None;
        self.keybinding_recorded_combo = None;

        self.snippets_tab.editing_snippet_index = None;
        self.snippets_tab.adding_new_snippet = false;
        self.snippets_tab.recording_snippet_keybinding = false;

        self.actions_tab.editing_action_index = None;
        self.actions_tab.adding_new_action = false;
        self.actions_tab.recording_action_keybinding = false;
        self.actions_tab.agent_launch_editing = None;
        self.actions_tab.agent_launch_adding = false;
        self.actions_tab.agent_launch_pending_delete = None;

        self.automation_tab.editing_trigger_index = None;
        self.automation_tab.adding_new_trigger = false;
        self.automation_tab.editing_coprocess_index = None;
        self.automation_tab.adding_new_coprocess = false;

        self.scripts_tab.editing_script_index = None;
        self.scripts_tab.adding_new_script = false;

        self.profiles_tab.dynamic_source_editing = None;
        self.profiles_tab.dynamic_source_edit_buffer = None;

        self.pending_list_delete = None;
    }

    /// Ask to close the window. Returns `true` when it may close now; with
    /// unsaved changes it shows the prompt instead and returns `false`.
    pub fn request_close(&mut self) -> bool {
        if self.close_pending {
            return false;
        }
        if self.has_unsaved_changes() {
            self.show_close_prompt = true;
            false
        } else {
            true
        }
    }

    /// Whether the close prompt is showing.
    pub fn is_close_prompt_visible(&self) -> bool {
        self.show_close_prompt
    }

    /// Hide the close prompt without closing (Escape, or its Cancel button).
    pub fn cancel_close_prompt(&mut self) {
        self.show_close_prompt = false;
    }

    /// Apply a close-prompt choice. For [`ClosePromptChoice::Save`] the
    /// returned config must be written by the caller; profile changes are
    /// queued through the existing profile-save request.
    pub fn resolve_close_prompt(&mut self, choice: ClosePromptChoice) -> Option<Config> {
        self.show_close_prompt = false;
        match choice {
            ClosePromptChoice::Cancel => None,
            ClosePromptChoice::Revert => {
                self.revert_to_baseline();
                self.close_pending = true;
                None
            }
            ClosePromptChoice::Save => {
                if self.profile_modal_ui.has_unsaved_changes() {
                    self.profile_modal_ui.finish_open_edit();
                    // Emptying a non-empty profile list needs its own
                    // confirmation; stay open on the Profiles tab to ask.
                    if self.profile_modal_ui.request_list_save()
                        != crate::profile_modal_ui::ProfileModalAction::Save
                    {
                        self.selected_tab = crate::sidebar::SettingsTab::Profiles;
                        return None;
                    }
                    self.profile_save_requested = true;
                }
                self.close_pending = true;
                Some(self.commit_save())
            }
        }
    }

    /// True once a resolved close prompt has no save left to hand over.
    /// Consumes the pending close.
    pub fn take_close_ready(&mut self) -> bool {
        if self.close_pending && !self.profile_save_requested {
            self.close_pending = false;
            true
        } else {
            false
        }
    }

    /// Render the close prompt when it is showing and return the choice made.
    pub(super) fn show_close_prompt_window(&mut self, ctx: &egui::Context) -> Option<Config> {
        if !self.show_close_prompt {
            return None;
        }

        let mut choice = None;
        egui::Window::new("Unsaved Changes")
            .collapsible(false)
            .resizable(false)
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(10.0);
                    ui.label("You have unsaved settings changes.");
                    ui.add_space(5.0);
                    ui.label(
                        egui::RichText::new(
                            "Save writes them to config.yaml. Revert restores every window \
                             to how it was when Settings opened or last saved.",
                        )
                        .color(egui::Color32::GRAY),
                    );
                    ui.add_space(15.0);
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() {
                            choice = Some(ClosePromptChoice::Save);
                        }
                        if ui.button("Revert").clicked() {
                            choice = Some(ClosePromptChoice::Revert);
                        }
                        if ui.button("Cancel").clicked() {
                            choice = Some(ClosePromptChoice::Cancel);
                        }
                    });
                    ui.add_space(10.0);
                });
            });

        choice.and_then(|choice| self.resolve_close_prompt(choice))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use par_term_config::Profile;

    fn opaque_config() -> Config {
        let mut config = Config::default();
        config.window.window_opacity = 1.0;
        config
    }

    fn live_config_after_frame(settings: &mut SettingsUI) -> Config {
        let ctx = egui::Context::default();
        let mut live = None;
        let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let (_, config_for_live, _, _) = settings.show_as_panel(ui);
                live = config_for_live;
            });
        });
        output.textures_delta.clear();
        live.expect("the panel forwards the working config every frame")
    }

    #[test]
    fn fresh_settings_have_no_unsaved_changes() {
        let settings = SettingsUI::new_for_tests(opaque_config());
        assert!(!settings.has_unsaved_changes());
    }

    /// Closing with no edits must not prompt, so rendering a tab must not
    /// change the working config on its own.
    #[test]
    fn rendering_any_tab_leaves_no_unsaved_changes() {
        for tab in crate::sidebar::SettingsTab::all() {
            let mut settings = SettingsUI::new_for_tests(Config::default());
            settings.selected_tab = *tab;
            live_config_after_frame(&mut settings);
            assert!(
                !settings.has_unsaved_changes(),
                "rendering {tab:?} marked the settings as changed"
            );
        }
    }

    #[test]
    fn revert_restores_changed_opacity_to_the_live_config() {
        let mut settings = SettingsUI::new_for_tests(opaque_config());
        settings.config.window.window_opacity = 0.4;
        settings.has_changes = true;
        assert!(settings.has_unsaved_changes());
        assert_eq!(
            live_config_after_frame(&mut settings).window.window_opacity,
            0.4
        );

        settings.revert_to_baseline();

        assert!(!settings.has_unsaved_changes());
        assert!(!settings.has_changes);
        // The live config is what the host applies to every terminal window.
        assert_eq!(
            live_config_after_frame(&mut settings).window.window_opacity,
            1.0
        );
    }

    #[test]
    fn edit_missed_by_has_changes_still_counts_as_unsaved() {
        let mut settings = SettingsUI::new_for_tests(opaque_config());
        settings.config.window.window_opacity = 0.7;
        assert!(!settings.has_changes);
        assert!(settings.has_unsaved_changes());
    }

    #[test]
    fn close_with_unsaved_changes_prompts_instead_of_closing() {
        let mut settings = SettingsUI::new_for_tests(opaque_config());
        assert!(settings.request_close(), "clean settings close at once");

        settings.config.window.window_opacity = 0.5;
        assert!(!settings.request_close());
        assert!(settings.is_close_prompt_visible());

        settings.cancel_close_prompt();
        assert!(!settings.is_close_prompt_visible());
        assert_eq!(settings.config.window.window_opacity, 0.5);
        assert!(!settings.take_close_ready());
    }

    #[test]
    fn close_prompt_revert_restores_baseline_then_closes() {
        let mut settings = SettingsUI::new_for_tests(opaque_config());
        settings.config.window.window_opacity = 0.5;
        assert!(!settings.request_close());

        let to_save = settings.resolve_close_prompt(ClosePromptChoice::Revert);

        assert!(to_save.is_none(), "Revert writes nothing");
        assert_eq!(settings.config.window.window_opacity, 1.0);
        assert!(settings.take_close_ready());
        assert!(!settings.take_close_ready(), "close is consumed once");
    }

    #[test]
    fn close_prompt_save_hands_over_config_and_moves_baseline() {
        let mut settings = SettingsUI::new_for_tests(opaque_config());
        settings.config.window.window_opacity = 0.5;
        assert!(!settings.request_close());

        let saved = settings
            .resolve_close_prompt(ClosePromptChoice::Save)
            .expect("Save hands the config to the host");

        assert_eq!(saved.window.window_opacity, 0.5);
        assert_eq!(settings.baseline_config().window.window_opacity, 0.5);
        assert!(!settings.has_unsaved_changes());
        assert!(settings.take_close_ready());
    }

    #[test]
    fn close_prompt_save_waits_for_the_profile_save_to_drain() {
        let mut settings = SettingsUI::new_for_tests(opaque_config());
        settings.sync_profiles(vec![Profile::new("one")]);
        settings.profile_modal_ui.has_changes = true;
        assert!(!settings.request_close());

        settings.resolve_close_prompt(ClosePromptChoice::Save);
        assert!(
            !settings.take_close_ready(),
            "close must wait until the profile save is handed over"
        );
        let profiles = settings
            .take_profile_save_request()
            .expect("profiles queued");
        assert_eq!(profiles.len(), 1);
        assert!(settings.take_close_ready());
    }

    #[test]
    fn close_prompt_save_asks_before_emptying_the_profile_list() {
        let mut settings = SettingsUI::new_for_tests(opaque_config());
        settings.sync_profiles(vec![Profile::new("keep")]);
        settings.profile_modal_ui.clear_working_profiles_for_test();
        assert!(!settings.request_close());

        let saved = settings.resolve_close_prompt(ClosePromptChoice::Save);

        assert!(saved.is_none(), "nothing is written while the ask is open");
        assert!(settings.profile_modal_ui.is_confirming_empty_save());
        assert!(!settings.take_close_ready());
        assert!(settings.take_profile_save_request().is_none());
    }

    #[test]
    fn reset_to_defaults_is_a_preview_until_save() {
        let mut config = opaque_config();
        config.font_size = 23.0;
        let mut settings = SettingsUI::new_for_tests(config);

        settings.reset_all_to_defaults();
        assert_eq!(settings.config.font_size, Config::default().font_size);
        assert_eq!(settings.baseline_config().font_size, 23.0);
        assert!(settings.has_unsaved_changes());

        settings.revert_to_baseline();
        assert_eq!(settings.config.font_size, 23.0);
    }

    #[test]
    fn revert_closes_editors_whose_index_points_into_replaced_lists() {
        let mut settings = SettingsUI::new_for_tests(opaque_config());
        settings.snippets_tab.editing_snippet_index = Some(3);
        settings.actions_tab.editing_action_index = Some(1);
        settings.automation_tab.editing_trigger_index = Some(0);
        settings.scripts_tab.editing_script_index = Some(2);

        settings.revert_to_baseline();

        assert_eq!(settings.snippets_tab.editing_snippet_index, None);
        assert_eq!(settings.actions_tab.editing_action_index, None);
        assert_eq!(settings.automation_tab.editing_trigger_index, None);
        assert_eq!(settings.scripts_tab.editing_script_index, None);
    }

    #[test]
    fn revert_restores_profiles_to_the_snapshot() {
        let mut settings = SettingsUI::new_for_tests(opaque_config());
        settings.sync_profiles(vec![Profile::new("a"), Profile::new("b")]);
        settings.profile_modal_ui.clear_working_profiles_for_test();
        assert!(settings.has_unsaved_changes());

        settings.revert_to_baseline();

        assert_eq!(settings.profile_modal_ui.get_working_profiles().len(), 2);
        assert!(!settings.has_unsaved_changes());
    }

    #[test]
    fn collapsed_sections_persist_only_when_the_set_changes() {
        let persisted = vec!["b".to_string(), "a".to_string()];
        let same: HashSet<String> = ["a", "b"].iter().map(|s| s.to_string()).collect();
        assert_eq!(collapsed_sections_to_persist(&persisted, &same), None);

        let changed: HashSet<String> = ["a"].iter().map(|s| s.to_string()).collect();
        assert_eq!(
            collapsed_sections_to_persist(&persisted, &changed),
            Some(vec!["a".to_string()])
        );
    }
}
