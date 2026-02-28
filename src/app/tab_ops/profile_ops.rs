//! Profile management operations: open, apply, auto-switch profiles.

use std::sync::Arc;

use crate::profile::{ProfileId, ProfileManager, storage as profile_storage};

use super::super::window_state::WindowState;

impl WindowState {
    /// Open a new tab from a profile
    pub fn open_profile(&mut self, profile_id: ProfileId) {
        log::debug!("open_profile called with id: {:?}", profile_id);

        // Check max tabs limit
        if self.config.max_tabs > 0 && self.tab_manager.tab_count() >= self.config.max_tabs {
            log::warn!(
                "Cannot open profile: max_tabs limit ({}) reached",
                self.config.max_tabs
            );
            self.deliver_notification(
                "Tab Limit Reached",
                &format!(
                    "Cannot open profile: maximum of {} tabs already open",
                    self.config.max_tabs
                ),
            );
            return;
        }

        let profile = match self.overlay_ui.profile_manager.get(&profile_id) {
            Some(p) => p.clone(),
            None => {
                log::error!("Profile not found: {:?}", profile_id);
                return;
            }
        };
        log::debug!("Found profile: {}", profile.name);

        // Get current grid size from renderer
        let grid_size = self.renderer.as_ref().map(|r| r.grid_size());

        match self.tab_manager.new_tab_from_profile(
            &self.config,
            Arc::clone(&self.runtime),
            &profile,
            grid_size,
        ) {
            Ok(tab_id) => {
                // Set profile icon on the new tab
                if let Some(tab) = self.tab_manager.get_tab_mut(tab_id) {
                    tab.profile_icon = profile.icon.clone();
                }

                // Start refresh task for the new tab and resize to match window
                if let Some(window) = &self.window
                    && let Some(tab) = self.tab_manager.get_tab_mut(tab_id)
                {
                    tab.start_refresh_task(
                        Arc::clone(&self.runtime),
                        Arc::clone(window),
                        self.config.max_fps,
                        self.config.inactive_tab_fps,
                    );

                    // Resize terminal to match current renderer dimensions
                    // try_lock: intentional — duplicate tab initialization in sync event loop.
                    // On miss: duplicate tab starts with default dimensions; corrected on next
                    // Resized event.
                    if let Some(renderer) = &self.renderer
                        && let Ok(mut term) = tab.terminal.try_write()
                    {
                        let (cols, rows) = renderer.grid_size();
                        let size = renderer.size();
                        let width_px = size.width as usize;
                        let height_px = size.height as usize;

                        term.set_cell_dimensions(
                            renderer.cell_width() as u32,
                            renderer.cell_height() as u32,
                        );
                        let _ = term.resize_with_pixels(cols, rows, width_px, height_px);
                        log::info!(
                            "Opened profile '{}' in tab {} ({}x{} at {}x{} px)",
                            profile.name,
                            tab_id,
                            cols,
                            rows,
                            width_px,
                            height_px
                        );
                    }
                }

                // Update badge with profile information
                self.apply_profile_badge(&profile);

                self.focus_state.needs_redraw = true;
                self.request_redraw();
            }
            Err(e) => {
                log::error!("Failed to open profile '{}': {}", profile.name, e);

                // Show user-friendly error notification
                let error_msg = e.to_string();
                let (title, message) = if error_msg.contains("Unable to spawn")
                    || error_msg.contains("No viable candidates")
                {
                    // Extract the command name from the error if possible
                    let cmd = profile
                        .command
                        .as_deref()
                        .unwrap_or("the configured command");
                    (
                        format!("Profile '{}' Failed", profile.name),
                        format!(
                            "Command '{}' not found. Check that it's installed and in your PATH.",
                            cmd
                        ),
                    )
                } else if error_msg.contains("No such file or directory") {
                    (
                        format!("Profile '{}' Failed", profile.name),
                        format!(
                            "Working directory not found: {}",
                            profile.working_directory.as_deref().unwrap_or("(unknown)")
                        ),
                    )
                } else {
                    (
                        format!("Profile '{}' Failed", profile.name),
                        format!("Failed to start: {}", error_msg),
                    )
                };
                self.deliver_notification(&title, &message);
            }
        }
    }

    /// Apply profile badge settings
    ///
    /// Updates the badge session variables and applies any profile-specific
    /// badge configuration (format, color, font, margins, etc.).
    pub(crate) fn apply_profile_badge(&mut self, profile: &crate::profile::Profile) {
        // Update session.profile_name variable
        {
            let mut vars = self.badge_state.variables_mut();
            vars.profile_name = profile.name.clone();
        }

        // Apply all profile badge settings (format, color, font, margins, etc.)
        self.badge_state.apply_profile_settings(profile);

        if profile.badge_text.is_some() {
            crate::debug_info!(
                "PROFILE",
                "Applied profile badge settings: format='{}', color={:?}, alpha={}",
                profile.badge_text.as_deref().unwrap_or(""),
                profile.badge_color,
                profile.badge_color_alpha.unwrap_or(0.0)
            );
        }

        // Mark badge as dirty to trigger re-render
        self.badge_state.mark_dirty();
    }

    /// Toggle the profile drawer visibility
    pub fn toggle_profile_drawer(&mut self) {
        self.overlay_ui.profile_drawer_ui.toggle();
        self.focus_state.needs_redraw = true;
        self.request_redraw();
    }

    /// Save profiles to disk
    pub fn save_profiles(&self) {
        if let Err(e) = profile_storage::save_profiles(&self.overlay_ui.profile_manager) {
            log::error!("Failed to save profiles: {}", e);
        }
    }

    /// Update profile manager from modal working copy
    pub fn apply_profile_changes(&mut self, profiles: Vec<crate::profile::Profile>) {
        self.overlay_ui.profile_manager = ProfileManager::from_profiles(profiles);
        self.save_profiles();
        // Signal that the profiles menu needs to be updated
        self.overlay_state.profiles_menu_needs_update = true;
    }

    /// Check for automatic profile switching based on hostname, SSH command, and directory detection
    ///
    /// This checks the active tab for hostname and CWD changes (detected via OSC 7),
    /// SSH command detection, and applies matching profiles automatically.
    /// Priority: explicit user selection > hostname match > SSH command match > directory match > default
    ///
    /// Returns true if a profile was auto-applied, triggering a redraw.
    pub fn check_auto_profile_switch(&mut self) -> bool {
        if self.overlay_ui.profile_manager.is_empty() {
            return false;
        }

        let mut changed = false;

        // --- Hostname-based switching (highest priority) ---
        changed |= self.check_auto_hostname_switch();

        // --- SSH command-based switching (medium priority, only if no hostname profile active) ---
        if !changed {
            changed |= self.check_ssh_command_switch();
        }

        // --- Directory-based switching (lower priority, only if no hostname profile) ---
        changed |= self.check_auto_directory_switch();

        changed
    }

    /// Check for hostname-based automatic profile switching
    fn check_auto_hostname_switch(&mut self) -> bool {
        let tab = match self.tab_manager.active_tab_mut() {
            Some(t) => t,
            None => return false,
        };

        let new_hostname = match tab.check_hostname_change() {
            Some(h) => h,
            None => {
                if tab.detected_hostname.is_none() && tab.auto_applied_profile_id.is_some() {
                    crate::debug_info!(
                        "PROFILE",
                        "Clearing auto-applied hostname profile (returned to localhost)"
                    );
                    tab.auto_applied_profile_id = None;
                    tab.profile_icon = None;
                    tab.badge_override = None;
                    // Restore original tab title
                    if let Some(original) = tab.pre_profile_title.take() {
                        tab.title = original;
                    }

                    // Revert SSH auto-switch if active
                    if tab.ssh_auto_switched {
                        crate::debug_info!(
                            "PROFILE",
                            "Reverting SSH auto-switch (disconnected from remote host)"
                        );
                        tab.ssh_auto_switched = false;
                        tab.pre_ssh_switch_profile = None;
                    }
                }
                return false;
            }
        };

        // Don't re-apply the same profile
        if let Some(existing_profile_id) = tab.auto_applied_profile_id
            && let Some(profile) = self
                .overlay_ui
                .profile_manager
                .find_by_hostname(&new_hostname)
            && profile.id == existing_profile_id
        {
            return false;
        }

        if let Some(profile) = self
            .overlay_ui
            .profile_manager
            .find_by_hostname(&new_hostname)
        {
            let profile_name = profile.name.clone();
            let profile_id = profile.id;
            let profile_tab_name = profile.tab_name.clone();
            let profile_icon = profile.icon.clone();
            let profile_badge_text = profile.badge_text.clone();
            let profile_command = profile.command.clone();
            let profile_command_args = profile.command_args.clone();

            crate::debug_info!(
                "PROFILE",
                "Auto-switching to profile '{}' for hostname '{}'",
                profile_name,
                new_hostname
            );

            // Apply profile visual settings to the tab
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                // Track SSH auto-switch state for revert on disconnect
                if !tab.ssh_auto_switched {
                    tab.pre_ssh_switch_profile = tab.auto_applied_profile_id;
                    tab.ssh_auto_switched = true;
                }

                tab.auto_applied_profile_id = Some(profile_id);
                tab.profile_icon = profile_icon;

                // Save original title before overriding (only if not already saved)
                if tab.pre_profile_title.is_none() {
                    tab.pre_profile_title = Some(tab.title.clone());
                }
                // Apply profile tab name (fall back to profile name)
                tab.title = profile_tab_name.unwrap_or_else(|| profile_name.clone());

                // Apply badge text override if configured
                if let Some(badge_text) = profile_badge_text {
                    tab.badge_override = Some(badge_text);
                }

                // Execute profile command in the running shell if configured
                if let Some(cmd) = profile_command {
                    let mut full_cmd = cmd;
                    if let Some(args) = profile_command_args {
                        for arg in args {
                            full_cmd.push(' ');
                            full_cmd.push_str(&arg);
                        }
                    }
                    full_cmd.push('\n');

                    let terminal_clone = Arc::clone(&tab.terminal);
                    self.runtime.spawn(async move {
                        let term = terminal_clone.write().await;
                        if let Err(e) = term.write(full_cmd.as_bytes()) {
                            log::error!("Failed to execute profile command: {}", e);
                        }
                    });
                }
            }

            // Apply profile badge settings (color, font, margins, etc.)
            self.apply_profile_badge(
                &self
                    .overlay_ui
                    .profile_manager
                    .get(&profile_id)
                    .expect("profile_id obtained from profile_manager.find_by_name above")
                    .clone(),
            );

            log::info!(
                "Auto-applied profile '{}' for hostname '{}'",
                profile_name,
                new_hostname
            );
            true
        } else {
            crate::debug_info!(
                "PROFILE",
                "No profile matches hostname '{}' - consider creating one",
                new_hostname
            );
            false
        }
    }

    /// Check for SSH command-based automatic profile switching
    ///
    /// When the running command is "ssh", parse the target host from the command
    /// and try to match a profile by hostname pattern. When SSH disconnects
    /// (command changes from "ssh" to something else), revert to the previous profile.
    fn check_ssh_command_switch(&mut self) -> bool {
        // Extract command info and current SSH state from the active tab
        let (current_command, already_switched, has_hostname_profile) = {
            let tab = match self.tab_manager.active_tab() {
                Some(t) => t,
                None => return false,
            };

            // try_lock: intentional — SSH command check in about_to_wait (sync event loop).
            // On miss: returns None (no command seen), skipping SSH profile switch this frame.
            // Will be evaluated again next frame.
            let cmd = if let Ok(term) = tab.terminal.try_write() {
                term.get_running_command_name()
            } else {
                None
            };

            (
                cmd,
                tab.ssh_auto_switched,
                tab.auto_applied_profile_id.is_some(),
            )
        };

        let is_ssh = current_command
            .as_ref()
            .is_some_and(|cmd| cmd == "ssh" || cmd.ends_with("/ssh"));

        if is_ssh && !already_switched && !has_hostname_profile {
            // SSH just started - try to extract the target host from the command
            // Shell integration may report just "ssh" as the command name;
            // the actual hostname will come via OSC 7 hostname detection.
            // For now, mark that SSH is active so we can revert when it ends.
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                crate::debug_info!(
                    "PROFILE",
                    "SSH command detected - waiting for hostname via OSC 7"
                );
                // Mark SSH as active for revert tracking (the actual profile
                // switch will happen via check_auto_hostname_switch when OSC 7 arrives)
                tab.ssh_auto_switched = true;
            }
            false
        } else if !is_ssh && already_switched && !has_hostname_profile {
            // SSH disconnected and no hostname-based profile is active - revert
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                crate::debug_info!("PROFILE", "SSH command ended - reverting auto-switch state");
                tab.ssh_auto_switched = false;
                let _prev_profile = tab.pre_ssh_switch_profile.take();
                // Clear any SSH-related visual overrides
                tab.profile_icon = None;
                tab.badge_override = None;
                if let Some(original) = tab.pre_profile_title.take() {
                    tab.title = original;
                }
            }
            true // Trigger redraw to reflect reverted state
        } else {
            false
        }
    }

    /// Check for directory-based automatic profile switching
    fn check_auto_directory_switch(&mut self) -> bool {
        let tab = match self.tab_manager.active_tab_mut() {
            Some(t) => t,
            None => return false,
        };

        // Don't override hostname-based profile (higher priority)
        if tab.auto_applied_profile_id.is_some() {
            return false;
        }

        let new_cwd = match tab.check_cwd_change() {
            Some(c) => c,
            None => return false,
        };

        // Don't re-apply the same profile
        if let Some(existing_profile_id) = tab.auto_applied_dir_profile_id
            && let Some(profile) = self.overlay_ui.profile_manager.find_by_directory(&new_cwd)
            && profile.id == existing_profile_id
        {
            return false;
        }

        if let Some(profile) = self.overlay_ui.profile_manager.find_by_directory(&new_cwd) {
            let profile_name = profile.name.clone();
            let profile_id = profile.id;
            let profile_tab_name = profile.tab_name.clone();
            let profile_icon = profile.icon.clone();
            let profile_badge_text = profile.badge_text.clone();
            let profile_command = profile.command.clone();
            let profile_command_args = profile.command_args.clone();

            crate::debug_info!(
                "PROFILE",
                "Auto-switching to profile '{}' for directory '{}'",
                profile_name,
                new_cwd
            );

            // Apply profile visual settings to the tab
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                tab.auto_applied_dir_profile_id = Some(profile_id);
                tab.profile_icon = profile_icon;

                // Save original title before overriding (only if not already saved)
                if tab.pre_profile_title.is_none() {
                    tab.pre_profile_title = Some(tab.title.clone());
                }
                // Apply profile tab name (fall back to profile name)
                tab.title = profile_tab_name.unwrap_or_else(|| profile_name.clone());

                // Apply badge text override if configured
                if let Some(badge_text) = profile_badge_text {
                    tab.badge_override = Some(badge_text);
                }

                // Execute profile command in the running shell if configured
                if let Some(cmd) = profile_command {
                    let mut full_cmd = cmd;
                    if let Some(args) = profile_command_args {
                        for arg in args {
                            full_cmd.push(' ');
                            full_cmd.push_str(&arg);
                        }
                    }
                    full_cmd.push('\n');

                    let terminal_clone = Arc::clone(&tab.terminal);
                    self.runtime.spawn(async move {
                        let term = terminal_clone.write().await;
                        if let Err(e) = term.write(full_cmd.as_bytes()) {
                            log::error!("Failed to execute profile command: {}", e);
                        }
                    });
                }
            }

            // Apply profile badge settings (color, font, margins, etc.)
            self.apply_profile_badge(
                &self
                    .overlay_ui
                    .profile_manager
                    .get(&profile_id)
                    .expect("profile_id obtained from profile_manager.find_by_name above")
                    .clone(),
            );

            log::info!(
                "Auto-applied profile '{}' for directory '{}'",
                profile_name,
                new_cwd
            );
            true
        } else {
            // Clear directory profile if CWD no longer matches any pattern
            if let Some(tab) = self.tab_manager.active_tab_mut()
                && tab.auto_applied_dir_profile_id.is_some()
            {
                crate::debug_info!(
                    "PROFILE",
                    "Clearing auto-applied directory profile (CWD '{}' no longer matches)",
                    new_cwd
                );
                tab.auto_applied_dir_profile_id = None;
                tab.profile_icon = None;
                tab.badge_override = None;
                // Restore original tab title
                if let Some(original) = tab.pre_profile_title.take() {
                    tab.title = original;
                }
            }
            false
        }
    }
}
