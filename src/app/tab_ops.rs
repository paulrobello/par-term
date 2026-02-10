//! Tab management operations for WindowState.
//!
//! This module contains methods for creating, closing, and switching between tabs.

use std::sync::Arc;

use crate::profile::{ProfileId, ProfileManager, storage as profile_storage};

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

        // Remember tab count before creating new tab to detect tab bar visibility change
        let old_tab_count = self.tab_manager.tab_count();

        // Get current grid size from renderer to pass to new tab
        // This accounts for possible tab bar height changes
        let grid_size = self.renderer.as_ref().map(|r| r.grid_size());

        match self.tab_manager.new_tab(
            &self.config,
            Arc::clone(&self.runtime),
            self.config.tab_inherit_cwd,
            grid_size,
        ) {
            Ok(tab_id) => {
                // Check if tab bar visibility changed (e.g., from 1 to 2 tabs with WhenMultiple mode)
                let new_tab_count = self.tab_manager.tab_count();
                let old_tab_bar_height = self.tab_bar_ui.get_height(old_tab_count, &self.config);
                let new_tab_bar_height = self.tab_bar_ui.get_height(new_tab_count, &self.config);

                // If tab bar height changed, update content offset and resize ALL existing tabs
                if (new_tab_bar_height - old_tab_bar_height).abs() > 0.1
                    && let Some(renderer) = &mut self.renderer
                    && let Some((new_cols, new_rows)) =
                        renderer.set_content_offset_y(new_tab_bar_height)
                {
                    let cell_width = renderer.cell_width();
                    let cell_height = renderer.cell_height();
                    let width_px = (new_cols as f32 * cell_width) as usize;
                    let height_px = (new_rows as f32 * cell_height) as usize;

                    // Resize all EXISTING tabs (not including the new one yet)
                    for tab in self.tab_manager.tabs_mut() {
                        if tab.id != tab_id {
                            if let Ok(mut term) = tab.terminal.try_lock() {
                                term.set_cell_dimensions(cell_width as u32, cell_height as u32);
                                let _ = term
                                    .resize_with_pixels(new_cols, new_rows, width_px, height_px);
                            }
                            tab.cache.cells = None;
                        }
                    }
                    log::info!(
                        "Tab bar appeared (height={:.0}), resized existing tabs to {}x{}",
                        new_tab_bar_height,
                        new_cols,
                        new_rows
                    );
                }

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
                    // (which now has the correct content offset)
                    if let Some(renderer) = &self.renderer
                        && let Ok(mut term) = tab.terminal.try_lock()
                    {
                        let (cols, rows) = renderer.grid_size();
                        let cell_width = renderer.cell_width();
                        let cell_height = renderer.cell_height();
                        let width_px = (cols as f32 * cell_width) as usize;
                        let height_px = (rows as f32 * cell_height) as usize;

                        // Set cell dimensions
                        term.set_cell_dimensions(cell_width as u32, cell_height as u32);

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
        // Check if we need to show confirmation for running jobs
        if self.config.confirm_close_running_jobs
            && let Some(command_name) = self.check_current_tab_running_job()
            && let Some(tab) = self.tab_manager.active_tab()
        {
            let tab_id = tab.id;
            let tab_title = if tab.title.is_empty() {
                "Terminal".to_string()
            } else {
                tab.title.clone()
            };
            self.close_confirmation_ui
                .show_for_tab(tab_id, &tab_title, &command_name);
            self.needs_redraw = true;
            self.request_redraw();
            return false; // Don't close yet, waiting for confirmation
        }

        self.close_current_tab_immediately()
    }

    /// Close the current tab immediately without confirmation
    /// Returns true if the window should close (last tab was closed)
    pub fn close_current_tab_immediately(&mut self) -> bool {
        if let Some(tab_id) = self.tab_manager.active_tab_id() {
            // Remember tab count before closing to detect tab bar visibility change
            let old_tab_count = self.tab_manager.tab_count();
            let old_tab_bar_height = self.tab_bar_ui.get_height(old_tab_count, &self.config);

            let is_last = self.tab_manager.close_tab(tab_id);

            // Check if tab bar visibility changed (e.g., from 2 to 1 tabs with WhenMultiple mode)
            if !is_last {
                let new_tab_count = self.tab_manager.tab_count();
                let new_tab_bar_height = self.tab_bar_ui.get_height(new_tab_count, &self.config);

                if (new_tab_bar_height - old_tab_bar_height).abs() > 0.1
                    && let Some(renderer) = &mut self.renderer
                    && let Some((new_cols, new_rows)) =
                        renderer.set_content_offset_y(new_tab_bar_height)
                {
                    let cell_width = renderer.cell_width();
                    let cell_height = renderer.cell_height();
                    let width_px = (new_cols as f32 * cell_width) as usize;
                    let height_px = (new_rows as f32 * cell_height) as usize;

                    // Resize all remaining tabs
                    for tab in self.tab_manager.tabs_mut() {
                        if let Ok(mut term) = tab.terminal.try_lock() {
                            term.set_cell_dimensions(cell_width as u32, cell_height as u32);
                            let _ =
                                term.resize_with_pixels(new_cols, new_rows, width_px, height_px);
                        }
                        tab.cache.cells = None;
                    }
                    log::info!(
                        "Tab bar {} (height={:.0}), resized remaining tabs to {}x{}",
                        if new_tab_bar_height > 0.0 {
                            "appeared"
                        } else {
                            "disappeared"
                        },
                        new_tab_bar_height,
                        new_cols,
                        new_rows
                    );
                }
            }

            self.needs_redraw = true;
            self.request_redraw();
            is_last
        } else {
            true // No tabs, window should close
        }
    }

    /// Switch to next tab
    pub fn next_tab(&mut self) {
        self.copy_mode.exit();
        self.tab_manager.next_tab();
        self.clear_and_invalidate();
    }

    /// Switch to previous tab
    pub fn prev_tab(&mut self) {
        self.copy_mode.exit();
        self.tab_manager.prev_tab();
        self.clear_and_invalidate();
    }

    /// Switch to tab by index (1-based)
    pub fn switch_to_tab_index(&mut self, index: usize) {
        self.copy_mode.exit();
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
        // Get current grid size from renderer
        let grid_size = self.renderer.as_ref().map(|r| r.grid_size());

        match self.tab_manager.duplicate_active_tab(
            &self.config,
            Arc::clone(&self.runtime),
            grid_size,
        ) {
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
    // Split Pane Operations
    // ========================================================================

    /// Split the current pane horizontally (panes stacked top/bottom)
    pub fn split_pane_horizontal(&mut self) {
        // In tmux mode, send split command to tmux instead
        if self.is_tmux_connected() && self.split_pane_via_tmux(false) {
            crate::debug_info!("TMUX", "Sent horizontal split command to tmux");
            return;
        }
        // Fall through to local split if tmux command failed or not connected

        // Calculate status bar height for proper content area
        let is_tmux_connected = self.is_tmux_connected();
        let status_bar_height =
            crate::tmux_status_bar_ui::TmuxStatusBarUI::height(&self.config, is_tmux_connected);

        // Get bounds info from renderer for proper pane sizing
        let bounds_info = self.renderer.as_ref().map(|r| {
            let size = r.size();
            let padding = r.window_padding();
            let content_offset_y = r.content_offset_y();
            let cell_width = r.cell_width();
            let cell_height = r.cell_height();
            (size, padding, content_offset_y, cell_width, cell_height)
        });

        if let Some(tab) = self.tab_manager.active_tab_mut() {
            // Set pane bounds before split if we have renderer info
            if let Some((size, padding, content_offset_y, cell_width, cell_height)) = bounds_info {
                let content_width = size.width as f32 - padding * 2.0;
                let content_height =
                    size.height as f32 - content_offset_y - padding - status_bar_height;
                let bounds = crate::pane::PaneBounds::new(
                    padding,
                    content_offset_y,
                    content_width,
                    content_height,
                );
                tab.set_pane_bounds(bounds, cell_width, cell_height);
            }

            match tab.split_horizontal(&self.config, Arc::clone(&self.runtime)) {
                Ok(Some(pane_id)) => {
                    log::info!("Split pane horizontally, new pane {}", pane_id);
                    // Clear renderer cells to remove stale single-pane data
                    if let Some(renderer) = &mut self.renderer {
                        renderer.clear_all_cells();
                    }
                    // Invalidate tab cache
                    tab.cache.cells = None;
                    self.needs_redraw = true;
                    self.request_redraw();
                }
                Ok(None) => {
                    log::info!(
                        "Horizontal split not yet functional (renderer integration pending)"
                    );
                }
                Err(e) => {
                    log::error!("Failed to split pane horizontally: {}", e);
                }
            }
        }
    }

    /// Split the current pane vertically (panes side by side)
    pub fn split_pane_vertical(&mut self) {
        // In tmux mode, send split command to tmux instead
        if self.is_tmux_connected() && self.split_pane_via_tmux(true) {
            crate::debug_info!("TMUX", "Sent vertical split command to tmux");
            return;
        }
        // Fall through to local split if tmux command failed or not connected

        // Calculate status bar height for proper content area
        let is_tmux_connected = self.is_tmux_connected();
        let status_bar_height =
            crate::tmux_status_bar_ui::TmuxStatusBarUI::height(&self.config, is_tmux_connected);

        // Get bounds info from renderer for proper pane sizing
        let bounds_info = self.renderer.as_ref().map(|r| {
            let size = r.size();
            let padding = r.window_padding();
            let content_offset_y = r.content_offset_y();
            let cell_width = r.cell_width();
            let cell_height = r.cell_height();
            (size, padding, content_offset_y, cell_width, cell_height)
        });

        if let Some(tab) = self.tab_manager.active_tab_mut() {
            // Set pane bounds before split if we have renderer info
            if let Some((size, padding, content_offset_y, cell_width, cell_height)) = bounds_info {
                let content_width = size.width as f32 - padding * 2.0;
                let content_height =
                    size.height as f32 - content_offset_y - padding - status_bar_height;
                let bounds = crate::pane::PaneBounds::new(
                    padding,
                    content_offset_y,
                    content_width,
                    content_height,
                );
                tab.set_pane_bounds(bounds, cell_width, cell_height);
            }

            match tab.split_vertical(&self.config, Arc::clone(&self.runtime)) {
                Ok(Some(pane_id)) => {
                    log::info!("Split pane vertically, new pane {}", pane_id);
                    // Clear renderer cells to remove stale single-pane data
                    if let Some(renderer) = &mut self.renderer {
                        renderer.clear_all_cells();
                    }
                    // Invalidate tab cache
                    tab.cache.cells = None;
                    self.needs_redraw = true;
                    self.request_redraw();
                }
                Ok(None) => {
                    log::info!("Vertical split not yet functional (renderer integration pending)");
                }
                Err(e) => {
                    log::error!("Failed to split pane vertically: {}", e);
                }
            }
        }
    }

    /// Close the focused pane in the current tab
    ///
    /// If this is the last pane, the tab is closed.
    /// Returns true if the window should close (last tab was closed).
    pub fn close_focused_pane(&mut self) -> bool {
        // In tmux mode, send kill-pane command to tmux
        if self.is_tmux_connected() && self.close_pane_via_tmux() {
            crate::debug_info!("TMUX", "Sent kill-pane command to tmux");
            // Don't close the local pane - wait for tmux layout change
            return false;
        }
        // Fall through to local close if tmux command failed or not connected

        // Check if we need to show confirmation for running jobs
        if self.config.confirm_close_running_jobs
            && let Some(command_name) = self.check_current_pane_running_job()
            && let Some(tab) = self.tab_manager.active_tab()
            && let Some(pane_id) = tab.focused_pane_id()
        {
            let tab_id = tab.id;
            let tab_title = if tab.title.is_empty() {
                "Terminal".to_string()
            } else {
                tab.title.clone()
            };
            self.close_confirmation_ui
                .show_for_pane(tab_id, pane_id, &tab_title, &command_name);
            self.needs_redraw = true;
            self.request_redraw();
            return false; // Don't close yet, waiting for confirmation
        }

        self.close_focused_pane_immediately()
    }

    /// Close the focused pane immediately without confirmation
    /// Returns true if the window should close (last tab was closed).
    fn close_focused_pane_immediately(&mut self) -> bool {
        if let Some(tab) = self.tab_manager.active_tab_mut()
            && tab.has_multiple_panes()
        {
            let is_last_pane = tab.close_focused_pane();
            if is_last_pane {
                // Last pane closed, close the tab
                return self.close_current_tab_immediately();
            }
            self.needs_redraw = true;
            self.request_redraw();
            return false;
        }
        // Single pane or no tab, close the tab
        self.close_current_tab_immediately()
    }

    /// Check if the current tab's terminal has a running job that should trigger confirmation
    ///
    /// Returns Some(command_name) if confirmation should be shown, None otherwise.
    fn check_current_tab_running_job(&self) -> Option<String> {
        let tab = self.tab_manager.active_tab()?;
        let term = tab.terminal.try_lock().ok()?;
        term.should_confirm_close(&self.config.jobs_to_ignore)
    }

    /// Check if the current pane's terminal has a running job that should trigger confirmation
    ///
    /// Returns Some(command_name) if confirmation should be shown, None otherwise.
    fn check_current_pane_running_job(&self) -> Option<String> {
        let tab = self.tab_manager.active_tab()?;

        // If the tab has split panes, check the focused pane
        if tab.has_multiple_panes() {
            let pane_manager = tab.pane_manager()?;
            let focused_id = pane_manager.focused_pane_id()?;
            let pane = pane_manager.get_pane(focused_id)?;
            let term = pane.terminal.try_lock().ok()?;
            return term.should_confirm_close(&self.config.jobs_to_ignore);
        }

        // Single pane - use the tab's terminal
        let term = tab.terminal.try_lock().ok()?;
        term.should_confirm_close(&self.config.jobs_to_ignore)
    }

    /// Check if the current tab has multiple panes
    pub fn has_multiple_panes(&self) -> bool {
        self.tab_manager
            .active_tab()
            .is_some_and(|tab| tab.has_multiple_panes())
    }

    /// Navigate to an adjacent pane in the given direction
    pub fn navigate_pane(&mut self, direction: crate::pane::NavigationDirection) {
        if let Some(tab) = self.tab_manager.active_tab_mut()
            && tab.has_multiple_panes()
        {
            tab.navigate_pane(direction);
            self.needs_redraw = true;
            self.request_redraw();
        }
    }

    /// Resize the focused pane in the given direction
    ///
    /// Growing left/up decreases the pane's ratio, growing right/down increases it
    pub fn resize_pane(&mut self, direction: crate::pane::NavigationDirection) {
        use crate::pane::NavigationDirection;

        // Resize step: 5% per keypress
        const RESIZE_DELTA: f32 = 0.05;

        // Determine delta based on direction
        // Right/Down: grow focused pane (positive delta)
        // Left/Up: shrink focused pane (negative delta)
        let delta = match direction {
            NavigationDirection::Right | NavigationDirection::Down => RESIZE_DELTA,
            NavigationDirection::Left | NavigationDirection::Up => -RESIZE_DELTA,
        };

        if let Some(tab) = self.tab_manager.active_tab_mut()
            && let Some(pm) = tab.pane_manager_mut()
            && let Some(focused_id) = pm.focused_pane_id()
        {
            pm.resize_split(focused_id, delta);
            self.needs_redraw = true;
            self.request_redraw();
        }
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

        // Get current grid size from renderer
        let grid_size = self.renderer.as_ref().map(|r| r.grid_size());

        match self.tab_manager.new_tab_from_profile(
            &self.config,
            Arc::clone(&self.runtime),
            &profile,
            grid_size,
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

                // Update badge with profile information
                self.apply_profile_badge(&profile);

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

    /// Apply profile badge settings
    ///
    /// Updates the badge session variables and applies any profile-specific
    /// badge configuration (format, color, font, margins, etc.).
    fn apply_profile_badge(&mut self, profile: &crate::profile::Profile) {
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

    /// Check for automatic profile switching based on hostname detection
    ///
    /// This checks the active tab for hostname changes (detected via OSC 7)
    /// and applies matching profiles automatically.
    ///
    /// Returns true if a profile was auto-applied, triggering a redraw.
    pub fn check_auto_profile_switch(&mut self) -> bool {
        // Only check if auto-switching is enabled (at least one profile has hostname patterns)
        if self.profile_manager.is_empty() {
            return false;
        }

        // Get active tab
        let tab = match self.tab_manager.active_tab_mut() {
            Some(t) => t,
            None => return false,
        };

        // Check if hostname has changed
        let new_hostname = match tab.check_hostname_change() {
            Some(h) => h,
            None => {
                // Hostname unchanged or returned to local - check if we should clear auto-profile
                if tab.detected_hostname.is_none() && tab.auto_applied_profile_id.is_some() {
                    crate::debug_info!(
                        "PROFILE",
                        "Clearing auto-applied profile (returned to localhost)"
                    );
                    tab.clear_auto_profile();
                }
                return false;
            }
        };

        // Don't re-apply the same profile if already auto-applied
        if let Some(existing_profile_id) = tab.auto_applied_profile_id
            && let Some(profile) = self.profile_manager.find_by_hostname(&new_hostname)
            && profile.id == existing_profile_id
        {
            return false;
        }

        // Find matching profile for this hostname
        let matching_profile = self.profile_manager.find_by_hostname(&new_hostname);

        if let Some(profile) = matching_profile {
            let profile_name = profile.name.clone();
            let profile_id = profile.id;

            crate::debug_info!(
                "PROFILE",
                "Auto-switching to profile '{}' for hostname '{}'",
                profile_name,
                new_hostname
            );

            // Mark this as an auto-applied profile
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                tab.auto_applied_profile_id = Some(profile_id);
            }

            // For automatic switching, we don't open a new tab - we could optionally
            // apply theme/settings to the current tab in the future.
            // For now, we log the event and could show a notification.
            log::info!(
                "Auto-detected profile '{}' for hostname '{}' (tab already running)",
                profile_name,
                new_hostname
            );

            // Optional: Show a brief notification about the hostname detection
            // self.deliver_notification(
            //     "SSH Detected",
            //     &format!("Connected to {} (profile: {})", new_hostname, profile_name),
            // );

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
}
