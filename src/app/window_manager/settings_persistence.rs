//! Config and profile writes that must respect the Settings window's
//! baseline (UX.md SS1 follow-up, SS7, SS10).
//!
//! While Settings is open, every terminal window and the manager hold the
//! live preview, which may contain unsaved Settings edits. A write taken from
//! that copy would persist edits the user has not saved, and a change made
//! only to a window's copy would be wiped by the next live update. So changes
//! a window makes on its own (theme follow, Assistant panel width, "Skip This
//! Version", the update-check timestamp) go through
//! [`WindowManager::persist_config_change`], which folds them into Settings'
//! baseline and writes the baseline plus the change.

use crate::app::window_state::ExternalConfigChange;
use crate::config::Config;

use super::WindowManager;

impl WindowManager {
    /// Persist a config change made outside Settings.
    ///
    /// With Settings open: the change lands in Settings' working config and
    /// baseline, the working config (which carries it) is pushed to every
    /// window, and the baseline plus the change is written. With Settings
    /// closed: the change is applied to the shared config and written.
    pub(crate) fn persist_config_change(&mut self, change: impl Fn(&mut Config), what: &str) {
        let to_write = if let Some(settings_window) = &mut self.settings_window {
            let to_write = settings_window.settings_ui.apply_external_change(&change);
            let live = settings_window.settings_ui.config.clone();
            self.apply_config_to_windows(&live);
            to_write
        } else {
            // The window that made the change already holds it; other windows
            // apply their own copy of an OS theme switch through their own
            // event, which must still see it as a change to refresh its
            // renderer. So only the shared config is updated here.
            self.config.rcu(|old| {
                let mut new = (**old).clone();
                change(&mut new);
                std::sync::Arc::new(new)
            });
            (**self.config.load()).clone()
        };
        if let Err(e) = self.write_config(&to_write) {
            log::error!("Failed to save config after {what}: {e}");
        }
    }

    /// Write config.yaml and remember the exact text, so the file watcher's
    /// echo of this write is not mistaken for an outside edit (SS10).
    fn write_config(&mut self, config: &Config) -> Result<(), String> {
        let yaml = serde_yaml_ng::to_string(config).map_err(|e| format!("{e:#}"))?;
        par_term_config::atomic_save::save_string_atomic(&Config::config_path(), &yaml)
            .map_err(|e| format!("{e:#}"))?;
        self.last_written_config_yaml = Some(yaml);
        Ok(())
    }

    /// Persist the changes each window queued this tick.
    ///
    /// With Settings open they go through its baseline (see
    /// [`Self::persist_config_change`]). With Settings closed the window that
    /// made the change writes its own config, as it always has: that config
    /// can hold window-local state (a keybinding shader toggle) the shared
    /// config never received, and writing the shared one would revert it on
    /// disk. The shared config still takes the change, so a later write from
    /// it (the update check) does not drop it.
    pub(crate) fn persist_window_config_changes(&mut self) {
        let settings_open = self.settings_window.is_some();
        let mut changes: Vec<ExternalConfigChange> = Vec::new();
        let mut reloaded = false;
        for window_state in self.windows.values_mut() {
            let queued = std::mem::take(&mut window_state.render_loop.external_config_changes);
            if !settings_open && !queued.is_empty() {
                if let Err(e) = window_state.save_config_debounced() {
                    log::error!("Failed to save config after a window setting change: {e}");
                }
                for change in &queued {
                    self.config.rcu(|old| {
                        let mut new = (**old).clone();
                        change.apply(&mut new);
                        std::sync::Arc::new(new)
                    });
                }
            }
            changes.extend(queued);
            reloaded |= std::mem::take(&mut window_state.render_loop.config_reloaded_from_disk);
        }
        if settings_open {
            // Several windows report the same OS theme switch; write it once.
            changes.dedup();
            for change in changes {
                self.persist_config_change(
                    |config| change.apply(config),
                    "a window setting change",
                );
            }
        }
        if reloaded {
            self.sync_disk_reload_to_settings();
        }
    }

    /// config.yaml changed on disk (SS10). Settings' baseline follows it when
    /// there are no unsaved edits; otherwise Settings shows a banner offering
    /// to load it.
    fn sync_disk_reload_to_settings(&mut self) {
        let Some(settings_window) = &mut self.settings_window else {
            return;
        };
        // Our own Save fires the watcher too. The file's bytes are then
        // exactly what we serialized, so compare bytes rather than a
        // reloaded Config (load migrates, merges defaults, and reads the
        // state file, which would make our own write look like an edit).
        let on_disk = std::fs::read_to_string(Config::config_path()).ok();
        if is_own_write_echo(self.last_written_config_yaml.as_deref(), on_disk.as_deref()) {
            return;
        }
        match Config::load() {
            Ok(on_disk) => {
                self.config.store(std::sync::Arc::new(on_disk.clone()));
                settings_window.settings_ui.config_changed_on_disk(on_disk);
            }
            Err(e) => settings_window.settings_ui.show_error_banner(format!(
                "config.yaml changed on disk but could not be read: {e}"
            )),
        }
    }

    /// Write the config Settings handed over on Save and report the result
    /// back to Settings, which rolls its baseline back on failure (SS7).
    pub(crate) fn save_config_from_settings(&mut self, config: Config) {
        let result = self.write_config(&config);
        match &result {
            Ok(()) => log::info!("Configuration saved successfully"),
            Err(e) => log::error!("Failed to save config: {e}"),
        }
        self.apply_config_to_windows(&config);
        if let Some(settings_window) = &mut self.settings_window {
            settings_window.settings_ui.report_config_save(result);
        }
    }

    /// Write the profiles Settings handed over on Save, apply them to every
    /// window, and report the result back to Settings (SS5, SS7).
    pub(crate) fn save_profiles_from_settings(
        &mut self,
        mut profiles: Vec<crate::profile::Profile>,
    ) {
        // A shortcut typed into the profile editor becomes an
        // `open_profile:<id>` binding at once (UX.md MD3), through
        // Settings' baseline so its next Save keeps the binding.
        let mut probe = (**self.config.load()).clone();
        let before = probe.keybindings.len();
        let report = crate::profile::actions::migrate_profile_shortcuts(&mut probe, &mut profiles);
        let added: Vec<crate::config::KeyBinding> = probe.keybindings.split_off(before);
        if !added.is_empty() {
            self.persist_config_change(
                |config| {
                    for binding in &added {
                        if !config.keybindings.contains(binding) {
                            config.keybindings.push(binding.clone());
                        }
                    }
                },
                "a profile shortcut binding",
            );
        }
        for (name, chord, why) in &report.skipped {
            log::warn!("Profile '{name}' shortcut {chord:?} not bound: {why:?}");
        }
        let manager = crate::profile::ProfileManager::from_profiles(profiles.clone());
        let result = crate::profile::storage::save_profiles(&manager).map_err(|e| format!("{e:#}"));
        if let Err(e) = &result {
            log::error!("Failed to save profiles: {e}");
        }
        for window_state in self.windows.values_mut() {
            window_state.overlay_ui.profile_manager =
                crate::profile::ProfileManager::from_profiles(profiles.clone());
            window_state.overlay_state.profiles_menu_needs_update = true;
        }
        if let Some(menu) = &mut self.menu {
            let profile_refs: Vec<&crate::profile::Profile> = profiles.iter().collect();
            menu.update_profiles(&profile_refs);
        }
        if let Some(settings_window) = &mut self.settings_window {
            settings_window.settings_ui.report_profile_save(result);
        }
    }
}

/// Whether config.yaml holds exactly the text this process last wrote, so a
/// watcher event for it is the echo of our own write, not an outside edit.
fn is_own_write_echo(last_written: Option<&str>, on_disk: Option<&str>) -> bool {
    matches!((last_written, on_disk), (Some(written), Some(disk)) if written == disk)
}

#[cfg(test)]
mod tests {
    use super::is_own_write_echo;

    #[test]
    fn own_write_is_recognised_and_outside_edits_are_not() {
        assert!(is_own_write_echo(
            Some("font_size: 12\n"),
            Some("font_size: 12\n")
        ));
        assert!(!is_own_write_echo(
            Some("font_size: 12\n"),
            Some("font_size: 14\n")
        ));
        assert!(!is_own_write_echo(None, Some("font_size: 12\n")));
        assert!(!is_own_write_echo(Some("font_size: 12\n"), None));
    }
}
