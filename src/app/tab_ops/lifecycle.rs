//! Tab lifecycle operations: create, close, reopen, navigate, duplicate.

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
            self.overlay_ui
                .close_confirmation_ui
                .show_for_tab(tab_id, &tab_title, &command_name);
            self.focus_state.needs_redraw = true;
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
                        self.overlay_state.closed_tabs.push_front(info);
                        while self.overlay_state.closed_tabs.len() > self.config.session_undo_max_entries {
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
                    while self.overlay_state.closed_tabs.len() > self.config.session_undo_max_entries {
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

            self.focus_state.needs_redraw = true;
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
            self.overlay_state.closed_tabs
                .retain(|info| now.duration_since(info.closed_at) < timeout);
        }

        let info = match self.overlay_state.closed_tabs.pop_front() {
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
            self.overlay_state.closed_tabs.push_front(info);
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
            self.focus_state.needs_redraw = true;
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
                    self.focus_state.needs_redraw = true;
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
    pub(super) fn handle_tab_bar_resize_after_add(
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
        self.focus_state.needs_redraw = true;
        self.request_redraw();
    }

    /// Move current tab right
    pub fn move_tab_right(&mut self) {
        self.tab_manager.move_active_tab_right();
        self.focus_state.needs_redraw = true;
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
                self.focus_state.needs_redraw = true;
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
                self.focus_state.needs_redraw = true;
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

    /// Check if the current tab's terminal has a running job that should trigger confirmation
    ///
    /// Returns Some(command_name) if confirmation should be shown, None otherwise.
    pub(super) fn check_current_tab_running_job(&self) -> Option<String> {
        let tab = self.tab_manager.active_tab()?;
        // try_lock: intentional — called from sync event loop before showing close dialog.
        // On miss (.ok() returns None): no job confirmation is shown, so tab closes without
        // prompting. This is safe: users are extremely unlikely to close exactly when the
        // lock is held by the PTY reader.
        let term = tab.terminal.try_lock().ok()?;
        term.should_confirm_close(&self.config.jobs_to_ignore)
    }
}
