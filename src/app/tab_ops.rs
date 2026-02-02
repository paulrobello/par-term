//! Tab management operations for WindowState.
//!
//! This module contains methods for creating, closing, and switching between tabs.

use std::sync::Arc;

use crate::profile::{storage as profile_storage, ProfileId, ProfileManager};

use super::window_state::WindowState;

impl WindowState {
    /// Create a new tab
    pub fn new_tab(&mut self) {
        // Check max tabs limit
        if self.config.max_tabs > 0 && self.tab_manager.tab_count() >= self.config.max_tabs {
            log::warn!(
                "Cannot create new tab: max_tabs limit ({}) reached",
                self.config.max_tabs
            );
            return;
        }

        match self.tab_manager.new_tab(
            &self.config,
            Arc::clone(&self.runtime),
            self.config.tab_inherit_cwd,
        ) {
            Ok(tab_id) => {
                // Start refresh task for the new tab and resize to match window
                if let Some(window) = &self.window
                    && let Some(tab) = self.tab_manager.get_tab_mut(tab_id)
                {
                    tab.start_refresh_task(
                        Arc::clone(&self.runtime),
                        Arc::clone(window),
                        self.config.max_fps,
                    );

                    // Resize terminal to match current renderer dimensions
                    if let Some(renderer) = &self.renderer
                        && let Ok(mut term) = tab.terminal.try_lock()
                    {
                        let (cols, rows) = renderer.grid_size();
                        let size = renderer.size();
                        let width_px = size.width as usize;
                        let height_px = size.height as usize;

                        // Set cell dimensions
                        term.set_cell_dimensions(
                            renderer.cell_width() as u32,
                            renderer.cell_height() as u32,
                        );

                        // Resize terminal to match window size
                        let _ = term.resize_with_pixels(cols, rows, width_px, height_px);
                        log::info!(
                            "Resized new tab {} terminal to {}x{} ({}x{} px)",
                            tab_id,
                            cols,
                            rows,
                            width_px,
                            height_px
                        );
                    }
                }

                self.needs_redraw = true;
                self.request_redraw();
            }
            Err(e) => {
                log::error!("Failed to create new tab: {}", e);
            }
        }
    }

    /// Close the current tab
    /// Returns true if the window should close (last tab was closed)
    pub fn close_current_tab(&mut self) -> bool {
        if let Some(tab_id) = self.tab_manager.active_tab_id() {
            let is_last = self.tab_manager.close_tab(tab_id);
            self.needs_redraw = true;
            self.request_redraw();
            is_last
        } else {
            true // No tabs, window should close
        }
    }

    /// Switch to next tab
    pub fn next_tab(&mut self) {
        self.tab_manager.next_tab();
        self.clear_and_invalidate();
    }

    /// Switch to previous tab
    pub fn prev_tab(&mut self) {
        self.tab_manager.prev_tab();
        self.clear_and_invalidate();
    }

    /// Switch to tab by index (1-based)
    pub fn switch_to_tab_index(&mut self, index: usize) {
        self.tab_manager.switch_to_index(index);
        self.clear_and_invalidate();
    }

    /// Move current tab left
    pub fn move_tab_left(&mut self) {
        self.tab_manager.move_active_tab_left();
        self.needs_redraw = true;
        self.request_redraw();
    }

    /// Move current tab right
    pub fn move_tab_right(&mut self) {
        self.tab_manager.move_active_tab_right();
        self.needs_redraw = true;
        self.request_redraw();
    }

    /// Duplicate current tab
    pub fn duplicate_tab(&mut self) {
        match self
            .tab_manager
            .duplicate_active_tab(&self.config, Arc::clone(&self.runtime))
        {
            Ok(Some(tab_id)) => {
                // Start refresh task for the new tab
                if let Some(window) = &self.window
                    && let Some(tab) = self.tab_manager.get_tab_mut(tab_id)
                {
                    tab.start_refresh_task(
                        Arc::clone(&self.runtime),
                        Arc::clone(window),
                        self.config.max_fps,
                    );
                }
                self.needs_redraw = true;
                self.request_redraw();
            }
            Ok(None) => {
                log::debug!("No active tab to duplicate");
            }
            Err(e) => {
                log::error!("Failed to duplicate tab: {}", e);
            }
        }
    }

    /// Check if there are multiple tabs
    pub fn has_multiple_tabs(&self) -> bool {
        self.tab_manager.has_multiple_tabs()
    }

    /// Get the active tab's terminal
    #[allow(dead_code)]
    pub fn active_terminal(
        &self,
    ) -> Option<&Arc<tokio::sync::Mutex<crate::terminal::TerminalManager>>> {
        self.tab_manager.active_tab().map(|tab| &tab.terminal)
    }

    // ========================================================================
    // Profile Management
    // ========================================================================

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

        let profile = match self.profile_manager.get(&profile_id) {
            Some(p) => p.clone(),
            None => {
                log::error!("Profile not found: {:?}", profile_id);
                return;
            }
        };
        log::debug!("Found profile: {}", profile.name);

        match self.tab_manager.new_tab_from_profile(
            &self.config,
            Arc::clone(&self.runtime),
            &profile,
        ) {
            Ok(tab_id) => {
                // Start refresh task for the new tab and resize to match window
                if let Some(window) = &self.window
                    && let Some(tab) = self.tab_manager.get_tab_mut(tab_id)
                {
                    tab.start_refresh_task(
                        Arc::clone(&self.runtime),
                        Arc::clone(window),
                        self.config.max_fps,
                    );

                    // Resize terminal to match current renderer dimensions
                    if let Some(renderer) = &self.renderer
                        && let Ok(mut term) = tab.terminal.try_lock()
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

                self.needs_redraw = true;
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

    /// Toggle the profile drawer visibility
    pub fn toggle_profile_drawer(&mut self) {
        self.profile_drawer_ui.toggle();
        self.needs_redraw = true;
        self.request_redraw();
    }

    /// Save profiles to disk
    pub fn save_profiles(&self) {
        if let Err(e) = profile_storage::save_profiles(&self.profile_manager) {
            log::error!("Failed to save profiles: {}", e);
        }
    }

    /// Update profile manager from modal working copy
    pub fn apply_profile_changes(&mut self, profiles: Vec<crate::profile::Profile>) {
        self.profile_manager = ProfileManager::from_profiles(profiles);
        self.save_profiles();
        // Signal that the profiles menu needs to be updated
        self.profiles_menu_needs_update = true;
    }
}
