//! Scroll operations for WindowState.
//!
//! This module contains methods for scrolling the terminal viewport.

use super::window_state::WindowState;

impl WindowState {
    pub(crate) fn scroll_up_page(&mut self) {
        // Calculate page size based on visible lines
        let (target_offset, scrollback_len) = {
            let tab = if let Some(t) = self.tab_manager.active_tab() {
                t
            } else {
                return;
            };
            (tab.scroll_state.target_offset, tab.cache.scrollback_len)
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
        let scrollback_len = {
            if let Some(tab) = self.tab_manager.active_tab() {
                tab.cache.scrollback_len
            } else {
                return;
            }
        };
        self.set_scroll_target(scrollback_len);
    }

    pub(crate) fn scroll_to_bottom(&mut self) {
        self.set_scroll_target(0);
    }
}
