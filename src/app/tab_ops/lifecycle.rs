//! Tab lifecycle operations: create, close, navigate.
//!
//! Reopen (session undo) and tab-bar resize helpers live in `tab_reopen`.

use std::sync::Arc;

use super::super::window_state::WindowState;
use super::ClosedTabInfo;

impl WindowState {
    /// Create a new tab, or show profile picker if configured and profiles exist
    pub fn new_tab_or_show_profiles(&mut self) {
        if self.config.new_tab_shortcut_shows_profiles
            && !self.overlay_ui.profile_manager.is_empty()
        {
            self.tab_bar_ui.show_new_tab_profile_menu = !self.tab_bar_ui.show_new_tab_profile_menu;
            self.request_redraw();
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
                            if let Ok(mut term) = tab.terminal.try_write() {
                                term.set_cell_dimensions(cell_width as u32, cell_height as u32);
                                let _ = term
                                    .resize_with_pixels(new_cols, new_rows, width_px, height_px);
                            }
                            tab.active_cache_mut().cells = None;
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
                        && let Ok(mut term) = tab.terminal.try_write()
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

                self.focus_state.needs_redraw = true;
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
        log::info!(
            "[CLOSE_TAB] close_current_tab called, confirm_close_running_jobs={}",
            self.config.confirm_close_running_jobs
        );

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
            log::info!(
                "[CLOSE_TAB] Showing close confirmation for tab {} with running command: {}",
                tab_id,
                command_name
            );
            self.overlay_ui
                .close_confirmation_ui
                .show_for_tab(tab_id, &tab_title, &command_name);
            self.focus_state.needs_redraw = true;
            self.request_redraw();
            return false; // Don't close yet, waiting for confirmation
        }

        log::info!(
            "[CLOSE_TAB] No running job detected or confirmation disabled, closing immediately"
        );
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
                        self.overlay_state.closed_tabs.push_front(info);
                        while self.overlay_state.closed_tabs.len()
                            > self.config.session_undo_max_entries
                        {
                            self.overlay_state.closed_tabs.pop_back();
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
                    self.overlay_state.closed_tabs.push_front(info);
                    while self.overlay_state.closed_tabs.len()
                        > self.config.session_undo_max_entries
                    {
                        self.overlay_state.closed_tabs.pop_back();
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
                        if let Ok(mut term) = tab.terminal.try_write() {
                            term.set_cell_dimensions(cell_width as u32, cell_height as u32);
                            let _ =
                                term.resize_with_pixels(new_cols, new_rows, width_px, height_px);
                        }
                        tab.active_cache_mut().cells = None;
                    }
                    log::info!(
                        "Tab bar visibility changed (position={:?}), resized remaining tabs to {}x{}",
                        self.config.tab_bar_position,
                        new_cols,
                        new_rows
                    );
                }
            }

            self.focus_state.needs_redraw = true;
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
        self.focus_state.needs_redraw = true;
        self.request_redraw();
    }

    /// Move current tab right
    pub fn move_tab_right(&mut self) {
        self.tab_manager.move_active_tab_right();
        self.focus_state.needs_redraw = true;
        self.request_redraw();
    }
}
