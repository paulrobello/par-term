//! Scroll operations for WindowState.
//!
//! This module contains methods for scrolling the terminal viewport.

use super::window_state::WindowState;

impl WindowState {
    /// Return the scrollback length for the active terminal.
    ///
    /// `tab.cache.scrollback_len` is updated every render frame: for single-pane
    /// mode by the normal PTY-reader path; for split-pane mode by the pane data
    /// gather loop which caches the focused pane's value.  This means we never
    /// need to lock the terminal here and won't get a spurious 0 on lock contention.
    pub(crate) fn get_active_scrollback_len(&self) -> usize {
        self.tab_manager
            .active_tab()
            .map(|t| t.cache.scrollback_len)
            .unwrap_or(0)
    }

    pub(crate) fn scroll_up_page(&mut self) {
        // Calculate page size based on visible lines
        let scrollback_len = self.get_active_scrollback_len();
        let target_offset = {
            let tab = if let Some(t) = self.tab_manager.active_tab() {
                t
            } else {
                return;
            };
            tab.scroll_state.target_offset
        };

        if let Some(renderer) = &self.renderer {
            let char_height = self.config.font_size * 1.2;
            let page_size = (renderer.size().height as f32 / char_height) as usize;

            let new_target = target_offset.saturating_add(page_size);
            let clamped_target = new_target.min(scrollback_len);
            self.set_scroll_target(clamped_target);
        }
    }

    pub(crate) fn scroll_down_page(&mut self) {
        // Calculate page size based on visible lines
        let target_offset = {
            if let Some(tab) = self.tab_manager.active_tab() {
                tab.scroll_state.target_offset
            } else {
                return;
            }
        };

        if let Some(renderer) = &self.renderer {
            let char_height = self.config.font_size * 1.2;
            let page_size = (renderer.size().height as f32 / char_height) as usize;

            let new_target = target_offset.saturating_sub(page_size);
            self.set_scroll_target(new_target);
        }
    }

    pub(crate) fn scroll_to_top(&mut self) {
        let scrollback_len = self.get_active_scrollback_len();
        if self.tab_manager.active_tab().is_none() {
            return;
        }
        self.set_scroll_target(scrollback_len);
    }

    pub(crate) fn scroll_to_bottom(&mut self) {
        self.set_scroll_target(0);
    }

    pub(crate) fn scroll_to_previous_mark(&mut self) {
        let Some(tab) = self.tab_manager.active_tab() else {
            return;
        };
        let scrollback_len = tab.cache.scrollback_len;
        let current_top = scrollback_len.saturating_sub(tab.scroll_state.offset);

        let prev = tab
            .terminal
            .try_lock()
            .ok()
            .and_then(|term| term.scrollback_previous_mark(current_top));

        if let Some(line) = prev {
            let new_offset = scrollback_len.saturating_sub(line);
            self.set_scroll_target(new_offset);
        }
    }

    pub(crate) fn scroll_to_next_mark(&mut self) {
        let Some(tab) = self.tab_manager.active_tab() else {
            return;
        };
        let scrollback_len = tab.cache.scrollback_len;
        let current_top = scrollback_len.saturating_sub(tab.scroll_state.offset);

        let next = tab
            .terminal
            .try_lock()
            .ok()
            .and_then(|term| term.scrollback_next_mark(current_top));

        if let Some(line) = next {
            let new_offset = scrollback_len.saturating_sub(line);
            self.set_scroll_target(new_offset);
        }
    }
}
