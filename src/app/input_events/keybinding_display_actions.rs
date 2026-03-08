//! Display-related keybinding action helpers for WindowState.
//!
//! Extracted from `keybinding_actions` to keep the main dispatch function
//! under the 500-line target.
//!
//! Contains `execute_display_keybinding_action`, which handles:
//! - Font size changes (increase / decrease / reset)
//! - Cursor style cycling
//! - Tab index switching (switch_to_tab_1 … switch_to_tab_9)
//! - Tab reordering (move_tab_left / move_tab_right)
//! - Throughput mode toggle
//! - Reopen closed tab
//! - Arrangement save / SSH quick-connect / dynamic profile reload

use crate::app::window_state::WindowState;

impl WindowState {
    /// Handle display- and navigation-related keybinding actions.
    ///
    /// Returns `Some(true)` when the action was handled, `Some(false)` when the
    /// action name was recognised but no-op'd, and `None` when the name is not
    /// in this handler (caller should try the next handler).
    pub(crate) fn execute_display_keybinding_action(&mut self, action: &str) -> Option<bool> {
        match action {
            "increase_font_size" => {
                self.config.font_size = (self.config.font_size + 1.0).min(72.0);
                self.render_loop.pending_font_rebuild = true;
                log::info!(
                    "Font size increased to {} via keybinding",
                    self.config.font_size
                );
                self.request_redraw();
                Some(true)
            }
            "decrease_font_size" => {
                self.config.font_size = (self.config.font_size - 1.0).max(6.0);
                self.render_loop.pending_font_rebuild = true;
                log::info!(
                    "Font size decreased to {} via keybinding",
                    self.config.font_size
                );
                self.request_redraw();
                Some(true)
            }
            "reset_font_size" => {
                self.config.font_size = 14.0;
                self.render_loop.pending_font_rebuild = true;
                log::info!("Font size reset to default (14.0) via keybinding");
                self.request_redraw();
                Some(true)
            }
            "cycle_cursor_style" => {
                use crate::config::CursorStyle;
                use par_term_emu_core_rust::cursor::CursorStyle as TermCursorStyle;

                self.config.cursor_style = match self.config.cursor_style {
                    CursorStyle::Block => CursorStyle::Beam,
                    CursorStyle::Beam => CursorStyle::Underline,
                    CursorStyle::Underline => CursorStyle::Block,
                };

                self.invalidate_tab_cache();
                self.focus_state.needs_redraw = true;

                log::info!(
                    "Cycled cursor style to {:?} via keybinding",
                    self.config.cursor_style
                );

                let term_style = if self.config.cursor_blink {
                    match self.config.cursor_style {
                        CursorStyle::Block => TermCursorStyle::BlinkingBlock,
                        CursorStyle::Beam => TermCursorStyle::BlinkingBar,
                        CursorStyle::Underline => TermCursorStyle::BlinkingUnderline,
                    }
                } else {
                    match self.config.cursor_style {
                        CursorStyle::Block => TermCursorStyle::SteadyBlock,
                        CursorStyle::Beam => TermCursorStyle::SteadyBar,
                        CursorStyle::Underline => TermCursorStyle::SteadyUnderline,
                    }
                };

                // try_lock: intentional — cursor blink toggle via keybinding in sync loop.
                // On miss: cursor style not updated this invocation. Cosmetic only.
                if let Some(tab) = self.tab_manager.active_tab()
                    && let Ok(mut term) = tab.terminal.try_write()
                {
                    term.set_cursor_style(term_style);
                }
                Some(true)
            }
            "move_tab_left" => {
                self.move_tab_left();
                log::debug!("Moved tab left via keybinding");
                Some(true)
            }
            "move_tab_right" => {
                self.move_tab_right();
                log::debug!("Moved tab right via keybinding");
                Some(true)
            }
            "switch_to_tab_1" => {
                self.switch_to_tab_index(1);
                Some(true)
            }
            "switch_to_tab_2" => {
                self.switch_to_tab_index(2);
                Some(true)
            }
            "switch_to_tab_3" => {
                self.switch_to_tab_index(3);
                Some(true)
            }
            "switch_to_tab_4" => {
                self.switch_to_tab_index(4);
                Some(true)
            }
            "switch_to_tab_5" => {
                self.switch_to_tab_index(5);
                Some(true)
            }
            "switch_to_tab_6" => {
                self.switch_to_tab_index(6);
                Some(true)
            }
            "switch_to_tab_7" => {
                self.switch_to_tab_index(7);
                Some(true)
            }
            "switch_to_tab_8" => {
                self.switch_to_tab_index(8);
                Some(true)
            }
            "switch_to_tab_9" => {
                self.switch_to_tab_index(9);
                Some(true)
            }
            "toggle_throughput_mode" => {
                self.config.maximize_throughput = !self.config.maximize_throughput;
                let message = if self.config.maximize_throughput {
                    "Throughput Mode: ON"
                } else {
                    "Throughput Mode: OFF"
                };
                self.show_toast(message);
                log::info!(
                    "Throughput mode {}",
                    if self.config.maximize_throughput {
                        "enabled"
                    } else {
                        "disabled"
                    }
                );
                Some(true)
            }
            "reopen_closed_tab" => {
                self.reopen_closed_tab();
                Some(true)
            }
            "save_arrangement" => {
                // Open settings to Arrangements tab
                self.overlay_state.open_settings_window_requested = true;
                self.request_redraw();
                log::info!("Save arrangement requested via keybinding");
                Some(true)
            }
            "ssh_quick_connect" => {
                self.overlay_ui.ssh_connect_ui.open(
                    self.config.ssh.enable_mdns_discovery,
                    self.config.ssh.mdns_scan_timeout_secs,
                );
                self.request_redraw();
                log::info!("SSH Quick Connect opened via keybinding");
                Some(true)
            }
            "reload_dynamic_profiles" => {
                self.overlay_state.reload_dynamic_profiles_requested = true;
                self.request_redraw();
                log::info!("Dynamic profiles reload requested via keybinding");
                Some(true)
            }
            _ => None, // Not handled here — caller continues to next handler
        }
    }
}
