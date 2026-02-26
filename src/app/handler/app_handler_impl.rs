//! `ApplicationHandler` impl for `WindowManager`.
//!
//! Implements the winit `ApplicationHandler` trait: `resumed`, `window_event`,
//! and the top-level `about_to_wait` coordinator for all windows.

use crate::app::window_manager::WindowManager;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

impl ApplicationHandler for WindowManager {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Create the first window on app resume (or if all windows were closed on some platforms)
        if self.windows.is_empty() {
            if !self.auto_restore_done {
                self.auto_restore_done = true;

                // Session restore takes precedence when enabled
                if self.config.restore_session && self.restore_session(event_loop) {
                    return;
                }

                // Try auto-restore arrangement if configured
                if let Some(ref name) = self.config.auto_restore_arrangement.clone()
                    && !name.is_empty()
                    && self.arrangement_manager.find_by_name(name).is_some()
                {
                    log::info!("Auto-restoring arrangement: {}", name);
                    self.restore_arrangement_by_name(name, event_loop);
                    return;
                }
            }
            self.create_window(event_loop);
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        // Check if this event is for the settings window
        if self.is_settings_window(window_id) {
            if let Some(action) = self.handle_settings_window_event(event) {
                use crate::settings_window::SettingsWindowAction;
                match action {
                    SettingsWindowAction::Close => {
                        // Already handled in handle_settings_window_event
                    }
                    SettingsWindowAction::ApplyConfig(config) => {
                        // Apply live config changes to all terminal windows
                        log::info!("SETTINGS: ApplyConfig shader={:?}", config.custom_shader);
                        self.apply_config_to_windows(&config);
                    }
                    SettingsWindowAction::SaveConfig(config) => {
                        // Save config to disk and apply to all windows
                        if let Err(e) = config.save() {
                            log::error!("Failed to save config: {}", e);
                        } else {
                            log::info!("Configuration saved successfully");
                        }
                        self.apply_config_to_windows(&config);
                        // Update settings window with saved config
                        if let Some(settings_window) = &mut self.settings_window {
                            settings_window.update_config(config);
                        }
                    }
                    SettingsWindowAction::ApplyShader(shader_result) => {
                        let _ = self.apply_shader_from_editor(&shader_result.source);
                    }
                    SettingsWindowAction::ApplyCursorShader(cursor_shader_result) => {
                        let _ = self.apply_cursor_shader_from_editor(&cursor_shader_result.source);
                    }
                    SettingsWindowAction::TestNotification => {
                        // Send a test notification to verify permissions
                        self.send_test_notification();
                    }
                    SettingsWindowAction::SaveProfiles(profiles) => {
                        // Apply saved profiles to all terminal windows
                        for window_state in self.windows.values_mut() {
                            window_state.apply_profile_changes(profiles.clone());
                        }
                        // Update the profiles menu
                        if let Some(menu) = &mut self.menu {
                            let profile_refs: Vec<&crate::profile::Profile> =
                                profiles.iter().collect();
                            menu.update_profiles(&profile_refs);
                        }
                    }
                    SettingsWindowAction::OpenProfile(id) => {
                        // Open profile in the focused terminal window
                        if let Some(window_id) = self.get_focused_window_id()
                            && let Some(window_state) = self.windows.get_mut(&window_id)
                        {
                            window_state.open_profile(id);
                        }
                    }
                    SettingsWindowAction::StartCoprocess(index) => {
                        log::debug!("Handler: received StartCoprocess({})", index);
                        self.start_coprocess(index);
                    }
                    SettingsWindowAction::StopCoprocess(index) => {
                        log::debug!("Handler: received StopCoprocess({})", index);
                        self.stop_coprocess(index);
                    }
                    SettingsWindowAction::StartScript(index) => {
                        crate::debug_info!("SCRIPT", "Handler: received StartScript({})", index);
                        self.start_script(index);
                    }
                    SettingsWindowAction::StopScript(index) => {
                        log::debug!("Handler: received StopScript({})", index);
                        self.stop_script(index);
                    }
                    SettingsWindowAction::OpenLogFile => {
                        let log_path = crate::debug::log_path();
                        log::info!("Opening log file: {}", log_path.display());
                        if let Err(e) = open::that(&log_path) {
                            log::error!("Failed to open log file: {}", e);
                        }
                    }
                    SettingsWindowAction::SaveArrangement(name) => {
                        self.save_arrangement(name, event_loop);
                    }
                    SettingsWindowAction::RestoreArrangement(id) => {
                        self.restore_arrangement(id, event_loop);
                    }
                    SettingsWindowAction::DeleteArrangement(id) => {
                        self.delete_arrangement(id);
                    }
                    SettingsWindowAction::RenameArrangement(id, new_name) => {
                        // Special sentinel values for reorder operations
                        if new_name == "__move_up__" {
                            self.move_arrangement_up(id);
                        } else if new_name == "__move_down__" {
                            self.move_arrangement_down(id);
                        } else {
                            self.rename_arrangement(id, new_name);
                        }
                    }
                    SettingsWindowAction::ForceUpdateCheck => {
                        self.force_update_check_for_settings();
                    }
                    SettingsWindowAction::InstallUpdate(_version) => {
                        // The update is handled asynchronously inside SettingsUI.
                        // The InstallUpdate action is emitted for logging purposes.
                        log::info!("Self-update initiated from settings UI");
                    }
                    SettingsWindowAction::IdentifyPanes => {
                        // Flash pane index overlays on all terminal windows
                        for window_state in self.windows.values_mut() {
                            window_state.show_pane_indices(std::time::Duration::from_secs(3));
                        }
                    }
                    SettingsWindowAction::InstallShellIntegration => {
                        match crate::shell_integration_installer::install(None) {
                            Ok(result) => {
                                log::info!(
                                    "Shell integration installed for {:?} at {:?}",
                                    result.shell,
                                    result.script_path
                                );
                                if let Some(sw) = &mut self.settings_window {
                                    sw.request_redraw();
                                }
                            }
                            Err(e) => {
                                log::error!("Failed to install shell integration: {}", e);
                            }
                        }
                    }
                    SettingsWindowAction::UninstallShellIntegration => {
                        match crate::shell_integration_installer::uninstall() {
                            Ok(_) => {
                                log::info!("Shell integration uninstalled");
                                if let Some(sw) = &mut self.settings_window {
                                    sw.request_redraw();
                                }
                            }
                            Err(e) => {
                                log::error!("Failed to uninstall shell integration: {}", e);
                            }
                        }
                    }
                    SettingsWindowAction::None => {}
                }
            }
            return;
        }

        // Check if this is a resize event (before the event is consumed)
        let is_resize = matches!(event, WindowEvent::Resized(_));

        // Route event to the appropriate terminal window
        let (should_close, shader_states, grid_size) =
            if let Some(window_state) = self.windows.get_mut(&window_id) {
                let close = window_state.handle_window_event(event_loop, event);
                // Capture shader states to sync to settings window
                let states = (
                    window_state.config.custom_shader_enabled,
                    window_state.config.cursor_shader_enabled,
                );
                // Capture grid size if this was a resize
                let size = if is_resize {
                    window_state.renderer.as_ref().map(|r| r.grid_size())
                } else {
                    None
                };
                (close, Some(states), size)
            } else {
                (false, None, None)
            };

        // Sync shader states to settings window to prevent it from overwriting keybinding toggles
        if let (Some(settings_window), Some((custom_enabled, cursor_enabled))) =
            (&mut self.settings_window, shader_states)
        {
            settings_window.sync_shader_states(custom_enabled, cursor_enabled);
        }

        // Update settings window with new terminal dimensions after resize
        if let (Some(settings_window), Some((cols, rows))) = (&mut self.settings_window, grid_size)
        {
            settings_window.settings_ui.update_current_size(cols, rows);
        }

        // Close window if requested
        if should_close {
            self.close_window(window_id);
        }

        // Exit if no windows remain
        if self.should_exit {
            event_loop.exit();
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Check CLI timing-based options (exit-after, screenshot, command)
        self.check_cli_timers();

        // Check for updates (respects configured frequency)
        self.check_for_updates();

        // Process menu events
        // Find the actually focused window (the one with is_focused == true)
        let focused_window = self.get_focused_window_id();
        self.process_menu_events(event_loop, focused_window);

        // Check if any window requested opening the settings window
        // Also collect shader reload results for propagation to standalone settings window
        let mut open_settings = false;
        let mut open_settings_profiles_tab = false;
        let mut background_shader_result: Option<Option<String>> = None;
        let mut cursor_shader_result: Option<Option<String>> = None;
        let mut profiles_to_update: Option<Vec<crate::profile::Profile>> = None;
        let mut arrangement_restore_name: Option<String> = None;
        let mut reload_dynamic_profiles = false;
        let mut config_changed_by_agent = false;

        for window_state in self.windows.values_mut() {
            if window_state.open_settings_window_requested {
                window_state.open_settings_window_requested = false;
                open_settings = true;
            }
            if window_state.open_settings_profiles_tab {
                window_state.open_settings_profiles_tab = false;
                open_settings_profiles_tab = true;
            }

            // Check for arrangement restore request from keybinding
            if let Some(name) = window_state.pending_arrangement_restore.take() {
                arrangement_restore_name = Some(name);
            }

            // Check for dynamic profile reload request from keybinding
            if window_state.reload_dynamic_profiles_requested {
                window_state.reload_dynamic_profiles_requested = false;
                reload_dynamic_profiles = true;
            }

            // Check if profiles menu needs updating (from profile modal save)
            if window_state.profiles_menu_needs_update {
                window_state.profiles_menu_needs_update = false;
                // Get a copy of the profiles for menu update
                profiles_to_update = Some(window_state.profile_manager.to_vec());
            }

            window_state.about_to_wait(event_loop);

            // If an agent/MCP config update was applied, sync to WindowManager's
            // config so that subsequent saves (update checker, settings) don't
            // overwrite the agent's changes.
            if window_state.config_changed_by_agent {
                window_state.config_changed_by_agent = false;
                config_changed_by_agent = true;
            }

            // Collect shader reload results and clear them from window_state
            if let Some(result) = window_state.background_shader_reload_result.take() {
                background_shader_result = Some(result);
            }
            if let Some(result) = window_state.cursor_shader_reload_result.take() {
                cursor_shader_result = Some(result);
            }
        }

        // Sync agent config changes to WindowManager and settings window
        // so other saves (update checker, settings) don't overwrite the agent's changes
        if config_changed_by_agent && let Some(window_state) = self.windows.values().next() {
            log::info!("CONFIG: syncing agent config changes to WindowManager");
            self.config = window_state.config.clone();
            // Force-update the settings window's config copy so it doesn't
            // send stale values back via ApplyConfig/SaveConfig.
            // Must use force_update_config to bypass the has_changes guard.
            if let Some(settings_window) = &mut self.settings_window {
                settings_window.force_update_config(self.config.clone());
            }
        }

        // Check for dynamic profile updates
        while let Some(update) = self.dynamic_profile_manager.try_recv() {
            self.dynamic_profile_manager.update_status(&update);

            // Merge into all window profile managers
            for window_state in self.windows.values_mut() {
                crate::profile::dynamic::merge_dynamic_profiles(
                    &mut window_state.profile_manager,
                    &update.profiles,
                    &update.url,
                    &update.conflict_resolution,
                );
                window_state.profiles_menu_needs_update = true;
            }

            log::info!(
                "Dynamic profiles updated from {}: {} profiles{}",
                update.url,
                update.profiles.len(),
                update
                    .error
                    .as_ref()
                    .map_or(String::new(), |e| format!(" (error: {e})"))
            );

            // Ensure profiles_to_update is refreshed after dynamic merge
            if let Some(window_state) = self.windows.values().next() {
                profiles_to_update = Some(window_state.profile_manager.to_vec());
            }
        }

        // Trigger dynamic profile refresh if requested via keybinding
        if reload_dynamic_profiles {
            self.dynamic_profile_manager
                .refresh_all(&self.config.dynamic_profile_sources, &self.runtime);
        }

        // Update profiles menu if profiles changed
        if let Some(profiles) = profiles_to_update
            && let Some(menu) = &mut self.menu
        {
            let profile_refs: Vec<&crate::profile::Profile> = profiles.iter().collect();
            menu.update_profiles(&profile_refs);
        }

        // Open settings window if requested (F12 or Cmd+,)
        if open_settings {
            self.open_settings_window(event_loop);
        }

        // Navigate to Profiles tab if requested (from drawer "Manage" button)
        if open_settings_profiles_tab && let Some(sw) = &mut self.settings_window {
            sw.settings_ui
                .set_selected_tab(crate::settings_ui::sidebar::SettingsTab::Profiles);
        }

        // Restore arrangement if requested via keybinding
        if let Some(name) = arrangement_restore_name {
            self.restore_arrangement_by_name(&name, event_loop);
        }

        // Propagate shader reload results to standalone settings window
        if let Some(settings_window) = &mut self.settings_window {
            if let Some(result) = background_shader_result {
                match result {
                    Some(err) => settings_window.set_shader_error(Some(err)),
                    None => settings_window.clear_shader_error(),
                }
            }
            if let Some(result) = cursor_shader_result {
                match result {
                    Some(err) => settings_window.set_cursor_shader_error(Some(err)),
                    None => settings_window.clear_cursor_shader_error(),
                }
            }
        }

        // Close any windows that have is_shutting_down set
        // This handles deferred closes from quit confirmation, tab bar close, and shell exit
        let shutting_down: Vec<_> = self
            .windows
            .iter()
            .filter(|(_, ws)| ws.is_shutting_down)
            .map(|(id, _)| *id)
            .collect();

        for window_id in shutting_down {
            self.close_window(window_id);
        }

        // Sync coprocess and script running state to settings window
        if self.settings_window.is_some() {
            self.sync_coprocess_running_state();
            self.sync_script_running_state();
        }

        // Request redraw for settings window if it needs continuous updates
        self.request_settings_redraw();

        // Exit if no windows remain
        if self.should_exit {
            event_loop.exit();
        }
    }
}

