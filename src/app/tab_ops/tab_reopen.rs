//! Tab reopen (session undo) and tab-bar resize helpers.
//!
//! Extracted from `lifecycle` to keep create/close logic separate from
//! the undo-stack / preserved-shell reopen path.
//!
//! Contains:
//! - `reopen_closed_tab` — restore most-recently-closed tab from undo stack
//! - `handle_tab_bar_resize_after_add` — resize existing tabs when tab bar appears

use std::sync::Arc;

use super::super::window_state::WindowState;

impl WindowState {
    /// Reopen the most recently closed tab at its original position
    pub fn reopen_closed_tab(&mut self) {
        // Prune expired entries
        if self.config.load().session_undo_timeout_secs > 0 {
            let timeout =
                std::time::Duration::from_secs(self.config.load().session_undo_timeout_secs as u64);
            let now = std::time::Instant::now();
            self.overlay_state
                .closed_tabs
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
        if self.config.load().max_tabs > 0
            && self.tab_manager.tab_count() >= self.config.load().max_tabs
        {
            log::warn!(
                "Cannot reopen tab: max_tabs limit ({}) reached",
                self.config.load().max_tabs
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
                    self.config.load().max_fps,
                    self.config.load().inactive_tab_fps,
                );

                // Invalidate cell cache so content is re-rendered
                tab.active_cache_mut().cells = None;

                // try_lock: intentional — tab switch resize in sync event loop.
                // On miss: the newly active tab uses previous dimensions until next Resized.
                if let Some(renderer) = &self.renderer
                    && let Ok(mut term) = tab.terminal.try_write()
                {
                    let (cols, rows) = renderer.grid_size();
                    let cell_width = renderer.cell_width();
                    let cell_height = renderer.cell_height();
                    let width_px = (cols as f32 * cell_width) as usize;
                    let height_px = (rows as f32 * cell_height) as usize;
                    term.set_cell_dimensions(cell_width as u32, cell_height as u32);
                    if let Err(e) = term.resize_with_pixels(cols, rows, width_px, height_px) {
                        crate::debug_error!(
                            "TERMINAL",
                            "resize_with_pixels failed (reopen_preserved): {e}"
                        );
                    }
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
                &self.config.load(),
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
                            tab.set_title(&info.title);
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
                            self.config.load().max_fps,
                            self.config.load().inactive_tab_fps,
                        );

                        // try_lock: intentional — new pane initialization in sync event loop.
                        // On miss: pane terminal keeps default dimensions; fixed on next Resized.
                        if let Some(renderer) = &self.renderer
                            && let Ok(mut term) = tab.terminal.try_write()
                        {
                            let (cols, rows) = renderer.grid_size();
                            let cell_width = renderer.cell_width();
                            let cell_height = renderer.cell_height();
                            let width_px = (cols as f32 * cell_width) as usize;
                            let height_px = (rows as f32 * cell_height) as usize;
                            term.set_cell_dimensions(cell_width as u32, cell_height as u32);
                            if let Err(e) = term.resize_with_pixels(cols, rows, width_px, height_px)
                            {
                                crate::debug_error!(
                                    "TERMINAL",
                                    "resize_with_pixels failed (reopen_cwd): {e}"
                                );
                            }
                        }
                    }

                    // Restore pane layout if present
                    if let Some(pane_layout) = &info.pane_layout
                        && let Some(tab) = self.tab_manager.get_tab_mut(tab_id)
                    {
                        tab.restore_pane_layout(
                            pane_layout,
                            &self.config.load(),
                            Arc::clone(&self.runtime),
                        );
                        // Start refresh tasks for restored panes
                        if let Some(window) = &self.window
                            && let Some(tab) = self.tab_manager.get_tab_mut(tab_id)
                        {
                            tab.start_pane_refresh_tasks(
                                Arc::clone(&self.runtime),
                                Arc::clone(window),
                                self.config.load().max_fps,
                                self.config.load().inactive_tab_fps,
                            );
                        }
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
        let old_tab_bar_height = self
            .tab_bar_ui
            .get_height(old_tab_count, &self.config.load());
        let new_tab_bar_height = self
            .tab_bar_ui
            .get_height(new_tab_count, &self.config.load());
        let old_tab_bar_width = self
            .tab_bar_ui
            .get_width(old_tab_count, &self.config.load());
        let new_tab_bar_width = self
            .tab_bar_ui
            .get_width(new_tab_count, &self.config.load());

        if ((new_tab_bar_height - old_tab_bar_height).abs() > 0.1
            || (new_tab_bar_width - old_tab_bar_width).abs() > 0.1)
            && let Some(renderer) = &mut self.renderer
            && let Some((new_cols, new_rows)) = Self::apply_tab_bar_offsets_for_position(
                self.config.load().tab_bar_position,
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
                    if let Ok(mut term) = tab.terminal.try_write() {
                        term.set_cell_dimensions(cell_width as u32, cell_height as u32);
                        if let Err(e) =
                            term.resize_with_pixels(new_cols, new_rows, width_px, height_px)
                        {
                            crate::debug_error!(
                                "TERMINAL",
                                "resize_with_pixels failed (tab_bar_resize): {e}"
                            );
                        }
                    }
                    tab.active_cache_mut().cells = None;
                }
            }
        }
    }
}
