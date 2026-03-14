//! Profile management operations: open, apply, and persist profiles.
//!
//! Automatic profile switching (hostname, SSH, directory) lives in
//! `profile_auto_switch`.

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

        let prior_active_idx = self.tab_manager.active_tab_index();

        match self.tab_manager.new_tab_from_profile(
            &self.config,
            Arc::clone(&self.runtime),
            &profile,
            grid_size,
        ) {
            Ok(tab_id) => {
                if self.config.new_tab_position == crate::config::NewTabPosition::AfterActive {
                    if let Some(idx) = prior_active_idx {
                        self.tab_manager.move_tab_to_index(tab_id, idx + 1);
                    }
                }

                // Set profile icon on the new tab
                if let Some(tab) = self.tab_manager.get_tab_mut(tab_id) {
                    tab.profile.profile_icon = profile.icon.clone();
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
                        if let Err(e) = term.resize_with_pixels(cols, rows, width_px, height_px) {
                            crate::debug_error!(
                                "TERMINAL",
                                "resize_with_pixels failed (open_profile): {e}"
                            );
                        }
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

                // Auto-connect tmux session if profile has one configured
                if let Some(ref session_name) = profile.tmux_session_name
                    && self.config.tmux_enabled
                    && !self.is_gateway_active()
                {
                    match profile.tmux_connection_mode {
                        par_term_config::TmuxConnectionMode::ControlMode => {
                            if let Err(e) = self.initiate_tmux_gateway(Some(session_name)) {
                                crate::debug_error!(
                                    "TMUX",
                                    "Profile tmux auto-connect failed: {}",
                                    e
                                );
                            }
                        }
                        par_term_config::TmuxConnectionMode::Normal => {
                            // Write plain tmux command directly to the PTY
                            let cmd = format!(
                                "{} new-session -A -s '{}'\n",
                                self.config.tmux_path,
                                session_name.replace('\'', "'\\''")
                            );
                            if let Some(tab) = self.tab_manager.active_tab_mut()
                                && let Ok(term) = tab.terminal.try_write()
                            {
                                let _ = term.write(cmd.as_bytes());
                            }
                        }
                    }
                }
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
}
