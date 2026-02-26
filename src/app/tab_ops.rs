//! Tab management operations for WindowState.
//!
//! This module contains methods for creating, closing, and switching between tabs.

use std::sync::Arc;

use crate::profile::{ProfileId, ProfileManager, storage as profile_storage};

use super::window_state::WindowState;

/// Metadata captured when a tab is closed, used for session undo (reopen closed tab).
pub(crate) struct ClosedTabInfo {
    pub cwd: Option<String>,
    pub title: String,
    pub has_default_title: bool,
    pub index: usize,
    pub closed_at: std::time::Instant,
    pub pane_layout: Option<crate::session::SessionPaneNode>,
    pub custom_color: Option<[u8; 3]>,
    /// When `session_undo_preserve_shell` is enabled, the live Tab is kept here
    /// instead of being dropped. Dropping this ClosedTabInfo will drop the Tab,
    /// which kills the PTY.
    pub hidden_tab: Option<crate::tab::Tab>,
}

impl WindowState {
    /// Create a new tab, or show profile picker if configured and profiles exist
    pub fn new_tab_or_show_profiles(&mut self) {
        if self.config.new_tab_shortcut_shows_profiles && !self.profile_manager.is_empty() {
            self.tab_bar_ui.show_new_tab_profile_menu = !self.tab_bar_ui.show_new_tab_profile_menu;
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            log::info!("Toggled new-tab profile menu via shortcut");
        } else {
            self.new_tab();
            log::info!("New tab created");
        }
    }

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
                let old_tab_bar_width = self.tab_bar_ui.get_width(old_tab_count, &self.config);
                let new_tab_bar_width = self.tab_bar_ui.get_width(new_tab_count, &self.config);

                // If tab bar dimensions changed, update content offsets and resize ALL existing tabs
                if ((new_tab_bar_height - old_tab_bar_height).abs() > 0.1
                    || (new_tab_bar_width - old_tab_bar_width).abs() > 0.1)
                    && let Some(renderer) = &mut self.renderer
                    && let Some((new_cols, new_rows)) = Self::apply_tab_bar_offsets_for_position(
                        self.config.tab_bar_position,
                        renderer,
                        new_tab_bar_height,
                        new_tab_bar_width,
                    )
                {
                    let cell_width = renderer.cell_width();
                    let cell_height = renderer.cell_height();
                    let width_px = (new_cols as f32 * cell_width) as usize;
                    let height_px = (new_rows as f32 * cell_height) as usize;

                    // Resize all EXISTING tabs (not including the new one yet)
                    for tab in self.tab_manager.tabs_mut() {
                        if tab.id != tab_id {
                            // try_lock: intentional — resize during new-tab creation in sync
                            // event loop. On miss: this tab keeps old dimensions; corrected
                            // on the next Resized event.
                            if let Ok(mut term) = tab.terminal.try_lock() {
                                term.set_cell_dimensions(cell_width as u32, cell_height as u32);
                                let _ = term
                                    .resize_with_pixels(new_cols, new_rows, width_px, height_px);
                            }
                            tab.cache.cells = None;
                        }
                    }
                    log::info!(
                        "Tab bar appeared (position={:?}), resized existing tabs to {}x{}",
                        self.config.tab_bar_position,
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
                        self.config.inactive_tab_fps,
                    );

                    // Resize terminal to match current renderer dimensions
                    // (which now has the correct content offset)
                    // try_lock: intentional — new-tab initialization in sync event loop.
                    // On miss: the new tab starts with default PTY dimensions; corrected
                    // on the next Resized event.
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

                // Play new tab alert sound if configured
                self.play_alert_sound(crate::config::AlertEvent::NewTab);

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
            let old_tab_bar_width = self.tab_bar_ui.get_width(old_tab_count, &self.config);

            let is_last_tab = self.tab_manager.tab_count() <= 1;
            let preserve_shell = self.config.session_undo_preserve_shell
                && self.config.session_undo_timeout_secs > 0;

            // Capture closed tab metadata for session undo (before destroying the tab)
            let is_last = if preserve_shell {
                // Preserve mode: extract the live Tab and store it in ClosedTabInfo
                if let Some(tab) = self.tab_manager.get_tab(tab_id) {
                    let cwd = tab.get_cwd();
                    let title = tab.title.clone();
                    let has_default_title = tab.has_default_title;
                    let custom_color = tab.custom_color;
                    let index = self.tab_manager.active_tab_index().unwrap_or(0);

                    if let Some((mut hidden_tab, is_empty)) = self.tab_manager.remove_tab(tab_id) {
                        // Stop refresh task to prevent invisible redraws
                        hidden_tab.stop_refresh_task();

                        let info = ClosedTabInfo {
                            cwd,
                            title,
                            has_default_title,
                            index,
                            closed_at: std::time::Instant::now(),
                            pane_layout: None, // Preserved inside the hidden Tab itself
                            custom_color,
                            hidden_tab: Some(hidden_tab),
                        };
                        self.closed_tabs.push_front(info);
                        while self.closed_tabs.len() > self.config.session_undo_max_entries {
                            self.closed_tabs.pop_back();
                        }
                        is_empty
                    } else {
                        // Fallback: tab disappeared between get and remove
                        self.tab_manager.close_tab(tab_id)
                    }
                } else {
                    self.tab_manager.close_tab(tab_id)
                }
            } else {
                // Standard mode: capture metadata, then close (drops the Tab)
                if self.config.session_undo_timeout_secs > 0
                    && let Some(tab) = self.tab_manager.get_tab(tab_id)
                {
                    let info = ClosedTabInfo {
                        cwd: tab.get_cwd(),
                        title: tab.title.clone(),
                        has_default_title: tab.has_default_title,
                        index: self.tab_manager.active_tab_index().unwrap_or(0),
                        closed_at: std::time::Instant::now(),
                        pane_layout: tab
                            .pane_manager
                            .as_ref()
                            .and_then(|pm| pm.root())
                            .map(crate::session::capture::capture_pane_node),
                        custom_color: tab.custom_color,
                        hidden_tab: None,
                    };
                    self.closed_tabs.push_front(info);
                    while self.closed_tabs.len() > self.config.session_undo_max_entries {
                        self.closed_tabs.pop_back();
                    }
                }

                self.tab_manager.close_tab(tab_id)
            };

            // Play tab close alert sound if configured
            self.play_alert_sound(crate::config::AlertEvent::TabClose);

            // Show undo toast (only if not the last tab — window is closing)
            if !is_last_tab {
                let key_hint = self
                    .config
                    .keybindings
                    .iter()
                    .find(|kb| kb.action == "reopen_closed_tab")
                    .map(|kb| kb.key.clone())
                    .unwrap_or_else(|| "keybinding".to_string());
                let timeout = self.config.session_undo_timeout_secs;
                if timeout > 0 {
                    self.show_toast(format!(
                        "Tab closed. Press {} to undo ({timeout}s)",
                        key_hint
                    ));
                }
            }

            // Check if tab bar visibility changed (e.g., from 2 to 1 tabs with WhenMultiple mode)
            if !is_last {
                let new_tab_count = self.tab_manager.tab_count();
                let new_tab_bar_height = self.tab_bar_ui.get_height(new_tab_count, &self.config);
                let new_tab_bar_width = self.tab_bar_ui.get_width(new_tab_count, &self.config);

                if ((new_tab_bar_height - old_tab_bar_height).abs() > 0.1
                    || (new_tab_bar_width - old_tab_bar_width).abs() > 0.1)
                    && let Some(renderer) = &mut self.renderer
                    && let Some((new_cols, new_rows)) = Self::apply_tab_bar_offsets_for_position(
                        self.config.tab_bar_position,
                        renderer,
                        new_tab_bar_height,
                        new_tab_bar_width,
                    )
                {
                    let cell_width = renderer.cell_width();
                    let cell_height = renderer.cell_height();
                    let width_px = (new_cols as f32 * cell_width) as usize;
                    let height_px = (new_rows as f32 * cell_height) as usize;

                    // Resize all remaining tabs
                    for tab in self.tab_manager.tabs_mut() {
                        // try_lock: intentional — tab close resize in sync event loop.
                        // On miss: tab keeps old dimensions; fixed on the next Resized event.
                        if let Ok(mut term) = tab.terminal.try_lock() {
                            term.set_cell_dimensions(cell_width as u32, cell_height as u32);
                            let _ =
                                term.resize_with_pixels(new_cols, new_rows, width_px, height_px);
                        }
                        tab.cache.cells = None;
                    }
                    log::info!(
                        "Tab bar visibility changed (position={:?}), resized remaining tabs to {}x{}",
                        self.config.tab_bar_position,
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

    /// Reopen the most recently closed tab at its original position
    pub fn reopen_closed_tab(&mut self) {
        // Prune expired entries
        if self.config.session_undo_timeout_secs > 0 {
            let timeout =
                std::time::Duration::from_secs(self.config.session_undo_timeout_secs as u64);
            let now = std::time::Instant::now();
            self.closed_tabs
                .retain(|info| now.duration_since(info.closed_at) < timeout);
        }

        let info = match self.closed_tabs.pop_front() {
            Some(info) => info,
            None => {
                self.show_toast("No recently closed tabs");
                return;
            }
        };

        // Check max tabs limit
        if self.config.max_tabs > 0 && self.tab_manager.tab_count() >= self.config.max_tabs {
            log::warn!(
                "Cannot reopen tab: max_tabs limit ({}) reached",
                self.config.max_tabs
            );
            self.show_toast("Cannot reopen tab: max tabs limit reached");
            // Put the info back so the user can try again after closing another tab
            self.closed_tabs.push_front(info);
            return;
        }

        // Remember tab count before restoring to detect tab bar visibility change
        let old_tab_count = self.tab_manager.tab_count();

        if let Some(hidden_tab) = info.hidden_tab {
            // Preserved shell: re-insert the live Tab
            let tab_id = hidden_tab.id;
            self.tab_manager.insert_tab_at(hidden_tab, info.index);

            // Handle tab bar visibility change
            self.handle_tab_bar_resize_after_add(old_tab_count, tab_id);

            // Restart refresh task and resize terminal to match current window
            if let Some(window) = &self.window
                && let Some(tab) = self.tab_manager.get_tab_mut(tab_id)
            {
                tab.start_refresh_task(
                    Arc::clone(&self.runtime),
                    Arc::clone(window),
                    self.config.max_fps,
                    self.config.inactive_tab_fps,
                );

                // Invalidate cell cache so content is re-rendered
                tab.cache.cells = None;

                // try_lock: intentional — tab switch resize in sync event loop.
                // On miss: the newly active tab uses previous dimensions until next Resized.
                if let Some(renderer) = &self.renderer
                    && let Ok(mut term) = tab.terminal.try_lock()
                {
                    let (cols, rows) = renderer.grid_size();
                    let cell_width = renderer.cell_width();
                    let cell_height = renderer.cell_height();
                    let width_px = (cols as f32 * cell_width) as usize;
                    let height_px = (rows as f32 * cell_height) as usize;
                    term.set_cell_dimensions(cell_width as u32, cell_height as u32);
                    let _ = term.resize_with_pixels(cols, rows, width_px, height_px);
                }
            }

            self.play_alert_sound(crate::config::AlertEvent::NewTab);
            self.show_toast("Tab restored (session preserved)");
            self.needs_redraw = true;
            self.request_redraw();
        } else {
            // Metadata-only: create a new tab from CWD (existing behavior)
            let grid_size = self.renderer.as_ref().map(|r| r.grid_size());

            match self.tab_manager.new_tab_with_cwd(
                &self.config,
                Arc::clone(&self.runtime),
                info.cwd,
                grid_size,
            ) {
                Ok(tab_id) => {
                    // Handle tab bar visibility change
                    self.handle_tab_bar_resize_after_add(old_tab_count, tab_id);

                    // Restore title and custom color
                    if let Some(tab) = self.tab_manager.get_tab_mut(tab_id) {
                        if !info.has_default_title {
                            tab.title = info.title;
                            tab.has_default_title = false;
                        }
                        tab.custom_color = info.custom_color;
                    }

                    // Move tab to its original position
                    self.tab_manager.move_tab_to_index(tab_id, info.index);

                    // Start refresh task and resize terminal
                    if let Some(window) = &self.window
                        && let Some(tab) = self.tab_manager.get_tab_mut(tab_id)
                    {
                        tab.start_refresh_task(
                            Arc::clone(&self.runtime),
                            Arc::clone(window),
                            self.config.max_fps,
                            self.config.inactive_tab_fps,
                        );

                        // try_lock: intentional — new pane initialization in sync event loop.
                        // On miss: pane terminal keeps default dimensions; fixed on next Resized.
                        if let Some(renderer) = &self.renderer
                            && let Ok(mut term) = tab.terminal.try_lock()
                        {
                            let (cols, rows) = renderer.grid_size();
                            let cell_width = renderer.cell_width();
                            let cell_height = renderer.cell_height();
                            let width_px = (cols as f32 * cell_width) as usize;
                            let height_px = (rows as f32 * cell_height) as usize;
                            term.set_cell_dimensions(cell_width as u32, cell_height as u32);
                            let _ = term.resize_with_pixels(cols, rows, width_px, height_px);
                        }
                    }

                    // Restore pane layout if present
                    if let Some(pane_layout) = &info.pane_layout
                        && let Some(tab) = self.tab_manager.get_tab_mut(tab_id)
                    {
                        tab.restore_pane_layout(
                            pane_layout,
                            &self.config,
                            Arc::clone(&self.runtime),
                        );
                    }

                    self.play_alert_sound(crate::config::AlertEvent::NewTab);
                    self.show_toast("Tab restored");
                    self.needs_redraw = true;
                    self.request_redraw();
                }
                Err(e) => {
                    log::error!("Failed to reopen closed tab: {}", e);
                    self.show_toast("Failed to reopen tab");
                }
            }
        }
    }

    /// Handle tab bar visibility change after adding a tab.
    /// Resizes existing tabs if the tab bar appearance changed (e.g., from 1 to 2 tabs).
    fn handle_tab_bar_resize_after_add(
        &mut self,
        old_tab_count: usize,
        new_tab_id: crate::tab::TabId,
    ) {
        let new_tab_count = self.tab_manager.tab_count();
        let old_tab_bar_height = self.tab_bar_ui.get_height(old_tab_count, &self.config);
        let new_tab_bar_height = self.tab_bar_ui.get_height(new_tab_count, &self.config);
        let old_tab_bar_width = self.tab_bar_ui.get_width(old_tab_count, &self.config);
        let new_tab_bar_width = self.tab_bar_ui.get_width(new_tab_count, &self.config);

        if ((new_tab_bar_height - old_tab_bar_height).abs() > 0.1
            || (new_tab_bar_width - old_tab_bar_width).abs() > 0.1)
            && let Some(renderer) = &mut self.renderer
            && let Some((new_cols, new_rows)) = Self::apply_tab_bar_offsets_for_position(
                self.config.tab_bar_position,
                renderer,
                new_tab_bar_height,
                new_tab_bar_width,
            )
        {
            let cell_width = renderer.cell_width();
            let cell_height = renderer.cell_height();
            let width_px = (new_cols as f32 * cell_width) as usize;
            let height_px = (new_rows as f32 * cell_height) as usize;

            for tab in self.tab_manager.tabs_mut() {
                if tab.id != new_tab_id {
                    // try_lock: intentional — tab bar resize loop in sync event loop.
                    // On miss: this tab is not resized; corrected on the next Resized event.
                    if let Ok(mut term) = tab.terminal.try_lock() {
                        term.set_cell_dimensions(cell_width as u32, cell_height as u32);
                        let _ = term.resize_with_pixels(new_cols, new_rows, width_px, height_px);
                    }
                    tab.cache.cells = None;
                }
            }
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
                        self.config.inactive_tab_fps,
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

    /// Duplicate a specific tab by ID
    pub fn duplicate_tab_by_id(&mut self, source_tab_id: crate::tab::TabId) {
        let grid_size = self.renderer.as_ref().map(|r| r.grid_size());

        match self.tab_manager.duplicate_tab_by_id(
            source_tab_id,
            &self.config,
            Arc::clone(&self.runtime),
            grid_size,
        ) {
            Ok(Some(tab_id)) => {
                if let Some(window) = &self.window
                    && let Some(tab) = self.tab_manager.get_tab_mut(tab_id)
                {
                    tab.start_refresh_task(
                        Arc::clone(&self.runtime),
                        Arc::clone(window),
                        self.config.max_fps,
                        self.config.inactive_tab_fps,
                    );
                }
                self.needs_redraw = true;
                self.request_redraw();
            }
            Ok(None) => {
                log::debug!("Tab {} not found for duplication", source_tab_id);
            }
            Err(e) => {
                log::error!("Failed to duplicate tab {}: {}", source_tab_id, e);
            }
        }
    }

    /// Check if there are multiple tabs
    pub fn has_multiple_tabs(&self) -> bool {
        self.tab_manager.has_multiple_tabs()
    }

    /// Get the active tab's terminal
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
        let custom_status_bar_height = self.status_bar_ui.height(&self.config, self.is_fullscreen);

        // Get bounds info from renderer for proper pane sizing
        let bounds_info = self.renderer.as_ref().map(|r| {
            let size = r.size();
            let padding = r.window_padding();
            let content_offset_y = r.content_offset_y();
            let cell_width = r.cell_width();
            let cell_height = r.cell_height();
            let scale = r.scale_factor();
            (
                size,
                padding,
                content_offset_y,
                cell_width,
                cell_height,
                scale,
            )
        });

        let dpi_scale = bounds_info.map(|b| b.5).unwrap_or(1.0);

        if let Some(tab) = self.tab_manager.active_tab_mut() {
            // Set pane bounds before split if we have renderer info
            if let Some((size, padding, content_offset_y, cell_width, cell_height, scale)) =
                bounds_info
            {
                // After split there will be multiple panes, so use 0 padding if configured
                let effective_padding = if self.config.hide_window_padding_on_split {
                    0.0
                } else {
                    padding
                };
                // Scale status_bar_height from logical to physical pixels
                let physical_status_bar_height =
                    (status_bar_height + custom_status_bar_height) * scale;
                let content_width = size.width as f32 - effective_padding * 2.0;
                let content_height = size.height as f32
                    - content_offset_y
                    - effective_padding
                    - physical_status_bar_height;
                let bounds = crate::pane::PaneBounds::new(
                    effective_padding,
                    content_offset_y,
                    content_width,
                    content_height,
                );
                tab.set_pane_bounds(bounds, cell_width, cell_height);
            }

            match tab.split_horizontal(&self.config, Arc::clone(&self.runtime), dpi_scale) {
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
        let custom_status_bar_height = self.status_bar_ui.height(&self.config, self.is_fullscreen);

        // Get bounds info from renderer for proper pane sizing
        let bounds_info = self.renderer.as_ref().map(|r| {
            let size = r.size();
            let padding = r.window_padding();
            let content_offset_y = r.content_offset_y();
            let cell_width = r.cell_width();
            let cell_height = r.cell_height();
            let scale = r.scale_factor();
            (
                size,
                padding,
                content_offset_y,
                cell_width,
                cell_height,
                scale,
            )
        });

        let dpi_scale = bounds_info.map(|b| b.5).unwrap_or(1.0);

        if let Some(tab) = self.tab_manager.active_tab_mut() {
            // Set pane bounds before split if we have renderer info
            if let Some((size, padding, content_offset_y, cell_width, cell_height, scale)) =
                bounds_info
            {
                // After split there will be multiple panes, so use 0 padding if configured
                let effective_padding = if self.config.hide_window_padding_on_split {
                    0.0
                } else {
                    padding
                };
                // Scale status_bar_height from logical to physical pixels
                let physical_status_bar_height =
                    (status_bar_height + custom_status_bar_height) * scale;
                let content_width = size.width as f32 - effective_padding * 2.0;
                let content_height = size.height as f32
                    - content_offset_y
                    - effective_padding
                    - physical_status_bar_height;
                let bounds = crate::pane::PaneBounds::new(
                    effective_padding,
                    content_offset_y,
                    content_width,
                    content_height,
                );
                tab.set_pane_bounds(bounds, cell_width, cell_height);
            }

            match tab.split_vertical(&self.config, Arc::clone(&self.runtime), dpi_scale) {
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
        // try_lock: intentional — called from sync event loop before showing close dialog.
        // On miss (.ok() returns None): no job confirmation is shown, so tab closes without
        // prompting. This is safe: users are extremely unlikely to close exactly when the
        // lock is held by the PTY reader.
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
            // try_lock: intentional — same rationale as check_current_tab_running_job.
            // On miss: pane closes without confirmation. Safe in practice.
            let term = pane.terminal.try_lock().ok()?;
            return term.should_confirm_close(&self.config.jobs_to_ignore);
        }

        // Single pane - use the tab's terminal
        // try_lock: intentional — same rationale as above.
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

    /// Check for automatic profile switching based on hostname, SSH command, and directory detection
    ///
    /// This checks the active tab for hostname and CWD changes (detected via OSC 7),
    /// SSH command detection, and applies matching profiles automatically.
    /// Priority: explicit user selection > hostname match > SSH command match > directory match > default
    ///
    /// Returns true if a profile was auto-applied, triggering a redraw.
    pub fn check_auto_profile_switch(&mut self) -> bool {
        if self.profile_manager.is_empty() {
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
            && let Some(profile) = self.profile_manager.find_by_hostname(&new_hostname)
            && profile.id == existing_profile_id
        {
            return false;
        }

        if let Some(profile) = self.profile_manager.find_by_hostname(&new_hostname) {
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
                        let term = terminal_clone.lock().await;
                        if let Err(e) = term.write(full_cmd.as_bytes()) {
                            log::error!("Failed to execute profile command: {}", e);
                        }
                    });
                }
            }

            // Apply profile badge settings (color, font, margins, etc.)
            self.apply_profile_badge(
                &self
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
            let cmd = if let Ok(term) = tab.terminal.try_lock() {
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
            && let Some(profile) = self.profile_manager.find_by_directory(&new_cwd)
            && profile.id == existing_profile_id
        {
            return false;
        }

        if let Some(profile) = self.profile_manager.find_by_directory(&new_cwd) {
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
                        let term = terminal_clone.lock().await;
                        if let Err(e) = term.write(full_cmd.as_bytes()) {
                            log::error!("Failed to execute profile command: {}", e);
                        }
                    });
                }
            }

            // Apply profile badge settings (color, font, margins, etc.)
            self.apply_profile_badge(
                &self
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::time::{Duration, Instant};

    fn make_info(title: &str, index: usize) -> ClosedTabInfo {
        ClosedTabInfo {
            cwd: Some("/tmp".to_string()),
            title: title.to_string(),
            has_default_title: true,
            index,
            closed_at: Instant::now(),
            pane_layout: None,
            custom_color: None,
            hidden_tab: None,
        }
    }

    #[test]
    fn closed_tab_queue_overflow() {
        let max = 3;
        let mut queue: VecDeque<ClosedTabInfo> = VecDeque::new();
        for i in 0..5 {
            queue.push_front(make_info(&format!("tab{i}"), i));
            while queue.len() > max {
                queue.pop_back();
            }
        }
        assert_eq!(queue.len(), max);
        // Most recent should be first
        assert_eq!(queue.front().unwrap().title, "tab4");
        // Oldest kept should be last
        assert_eq!(queue.back().unwrap().title, "tab2");
    }

    #[test]
    fn closed_tab_expiry() {
        let timeout = Duration::from_millis(50);
        let mut queue: VecDeque<ClosedTabInfo> = VecDeque::new();

        // Add an already-expired entry
        let mut old = make_info("old", 0);
        old.closed_at = Instant::now() - Duration::from_millis(100);
        queue.push_front(old);

        // Add a fresh entry
        queue.push_front(make_info("fresh", 1));

        let now = Instant::now();
        queue.retain(|info| now.duration_since(info.closed_at) < timeout);

        assert_eq!(queue.len(), 1);
        assert_eq!(queue.front().unwrap().title, "fresh");
    }
}
