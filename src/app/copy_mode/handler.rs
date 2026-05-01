//! Copy mode integration for WindowState.
//!
//! Handles entering/exiting copy mode and the main key dispatch loop.
//! Search helpers, motion post-processing, and clipboard operations live in
//! `copy_mode_search`.

use crate::app::window_state::WindowState;
use crate::copy_mode::{SearchDirection, VisualMode};
use winit::event::KeyEvent;
use winit::keyboard::{Key, NamedKey};

impl WindowState {
    /// Check if copy mode is currently active
    pub(crate) fn is_copy_mode_active(&self) -> bool {
        self.copy_mode.active
    }

    /// Enter copy mode, anchoring the cursor at the current terminal cursor position
    pub(crate) fn enter_copy_mode(&mut self) {
        if !self.config.load().copy_mode.copy_mode_enabled {
            return;
        }

        // try_lock: intentional — copy mode initialization in sync event loop.
        // On miss: copy mode is not entered this keypress. User can try again.
        let Some((cursor_col, cursor_row, cols, rows, scrollback_len)) =
            self.tab_manager.active_tab().and_then(|tab| {
                tab.try_with_terminal_mut(|term| {
                    let (col, row) = term.cursor_position();
                    let (cols, rows) = term.dimensions();
                    let sb_len = term.scrollback_len();
                    (col, row, cols, rows, sb_len)
                })
            })
        else {
            return;
        };

        self.copy_mode
            .enter(cursor_col, cursor_row, cols, rows, scrollback_len);
        self.sync_copy_mode_selection();
        self.focus_state.needs_redraw = true;
        self.request_redraw();
        crate::debug_info!(
            "COPY_MODE",
            "Entered copy mode at ({}, {})",
            cursor_col,
            cursor_row
        );
    }

    /// Exit copy mode, clearing selection and restoring scroll
    pub(crate) fn exit_copy_mode(&mut self) {
        self.copy_mode.exit();
        // Clear selection (per-pane aware)
        self.with_active_tab_mut(|tab| {
            tab.selection_mouse_mut().selection = None;
            tab.active_cache_mut().cells = None; // Invalidate cache
        });
        // Scroll back to bottom
        self.set_scroll_target(0);
        self.focus_state.needs_redraw = true;
        self.request_redraw();
        crate::debug_info!("COPY_MODE", "Exited copy mode");
    }

    /// Handle a key event in copy mode. Returns true if the key was consumed.
    pub(crate) fn handle_copy_mode_key(&mut self, event: &KeyEvent) {
        // Handle search input mode
        if self.copy_mode.is_searching {
            self.handle_copy_mode_search_key(event);
            return;
        }

        // Handle pending mark set (waiting for mark name after 'm')
        if self.copy_mode.pending_mark_set {
            self.copy_mode.pending_mark_set = false;
            if let Key::Character(ref ch) = event.logical_key
                && let Some(c) = ch.chars().next()
                && c.is_ascii_lowercase()
            {
                self.copy_mode.set_mark(c);
                crate::debug_info!("COPY_MODE", "Set mark '{}'", c);
                self.request_redraw();
                return;
            }
            return;
        }

        // Handle pending mark goto (waiting for mark name after "'")
        if self.copy_mode.pending_mark_goto {
            self.copy_mode.pending_mark_goto = false;
            if let Key::Character(ref ch) = event.logical_key
                && let Some(c) = ch.chars().next()
                && c.is_ascii_lowercase()
                && self.copy_mode.goto_mark(c)
            {
                crate::debug_info!("COPY_MODE", "Jumped to mark '{}'", c);
                self.after_copy_mode_motion();
                return;
            }
            return;
        }

        // Handle pending 'g' (waiting for second 'g' in 'gg')
        if self.copy_mode.pending_g {
            self.copy_mode.pending_g = false;
            if let Key::Character(ref ch) = event.logical_key
                && ch.as_str() == "g"
            {
                self.copy_mode.goto_top();
                self.after_copy_mode_motion();
                return;
            }
            // Not 'g', ignore the pending state
            return;
        }

        // Check modifiers for Ctrl key combinations
        let modifiers = &self.input_handler.modifiers;
        let ctrl = modifiers.state().control_key();

        match &event.logical_key {
            // === Directional motions ===
            Key::Character(ch) if !ctrl => match ch.as_str() {
                "h" => {
                    self.copy_mode.move_left();
                    self.after_copy_mode_motion();
                }
                "j" => {
                    self.copy_mode.move_down();
                    self.after_copy_mode_motion();
                }
                "k" => {
                    self.copy_mode.move_up();
                    self.after_copy_mode_motion();
                }
                "l" => {
                    self.copy_mode.move_right();
                    self.after_copy_mode_motion();
                }

                // === Line motions ===
                "0" => {
                    // '0' can be part of a count or line start
                    if self.copy_mode.count.is_some() {
                        self.copy_mode.push_count_digit(0);
                    } else {
                        self.copy_mode.move_to_line_start();
                        self.after_copy_mode_motion();
                    }
                }
                "$" => {
                    self.copy_mode.move_to_line_end();
                    self.after_copy_mode_motion();
                }
                "^" => {
                    if let Some(text) = self.get_copy_mode_line_text() {
                        self.copy_mode.move_to_first_non_blank(&text);
                        self.after_copy_mode_motion();
                    }
                }

                // === Word motions ===
                "w" => {
                    if let Some(text) = self.get_copy_mode_line_text() {
                        let word_chars = self.config.load().word_characters.clone();
                        self.copy_mode.move_word_forward(&text, &word_chars);
                        self.after_copy_mode_motion();
                    }
                }
                "b" => {
                    if let Some(text) = self.get_copy_mode_line_text() {
                        let word_chars = self.config.load().word_characters.clone();
                        self.copy_mode.move_word_backward(&text, &word_chars);
                        self.after_copy_mode_motion();
                    }
                }
                "e" => {
                    if let Some(text) = self.get_copy_mode_line_text() {
                        let word_chars = self.config.load().word_characters.clone();
                        self.copy_mode.move_word_end(&text, &word_chars);
                        self.after_copy_mode_motion();
                    }
                }
                "W" => {
                    if let Some(text) = self.get_copy_mode_line_text() {
                        self.copy_mode.move_big_word_forward(&text);
                        self.after_copy_mode_motion();
                    }
                }
                "B" => {
                    if let Some(text) = self.get_copy_mode_line_text() {
                        self.copy_mode.move_big_word_backward(&text);
                        self.after_copy_mode_motion();
                    }
                }
                "E" => {
                    if let Some(text) = self.get_copy_mode_line_text() {
                        self.copy_mode.move_big_word_end(&text);
                        self.after_copy_mode_motion();
                    }
                }

                // === Page/buffer motions ===
                "G" => {
                    if let Some(count) = self.copy_mode.count.take() {
                        // {count}G goes to absolute line
                        self.copy_mode.goto_line(count.saturating_sub(1));
                    } else {
                        self.copy_mode.goto_bottom();
                    }
                    self.after_copy_mode_motion();
                }
                "g" => {
                    self.copy_mode.pending_g = true;
                }

                // === Visual modes ===
                "v" => {
                    if self.copy_mode.pending_operator.is_some() {
                        // yv = yank to current pos (do nothing special)
                        self.copy_mode.pending_operator = None;
                    } else {
                        self.copy_mode.toggle_visual_char();
                        self.after_copy_mode_motion();
                    }
                }
                "V" => {
                    self.copy_mode.toggle_visual_line();
                    self.after_copy_mode_motion();
                }

                // === Yank ===
                "y" => {
                    if self.copy_mode.visual_mode != VisualMode::None {
                        // In visual mode, yank the selection
                        self.yank_copy_mode_selection();
                    } else {
                        // Set pending yank operator (yy = yank line, yw = yank word, etc.)
                        // For simplicity, just yank current line on 'y' in normal mode
                        self.copy_mode.pending_operator =
                            Some(crate::copy_mode::PendingOperator::Yank);
                    }
                }

                // === Search ===
                "/" => {
                    self.copy_mode.start_search(SearchDirection::Forward);
                    self.focus_state.needs_redraw = true;
                    self.request_redraw();
                }
                "?" => {
                    self.copy_mode.start_search(SearchDirection::Backward);
                    self.focus_state.needs_redraw = true;
                    self.request_redraw();
                }
                "n" => {
                    self.execute_copy_mode_search(false);
                }
                "N" => {
                    self.execute_copy_mode_search(true);
                }

                // === Marks ===
                "m" => {
                    self.copy_mode.pending_mark_set = true;
                }
                "'" => {
                    self.copy_mode.pending_mark_goto = true;
                }

                // === Count prefix ===
                "1" => self.copy_mode.push_count_digit(1),
                "2" => self.copy_mode.push_count_digit(2),
                "3" => self.copy_mode.push_count_digit(3),
                "4" => self.copy_mode.push_count_digit(4),
                "5" => self.copy_mode.push_count_digit(5),
                "6" => self.copy_mode.push_count_digit(6),
                "7" => self.copy_mode.push_count_digit(7),
                "8" => self.copy_mode.push_count_digit(8),
                "9" => self.copy_mode.push_count_digit(9),

                // === Exit ===
                "q" => {
                    self.exit_copy_mode();
                }

                _ => {}
            },

            // === Ctrl key combinations ===
            Key::Character(ch) if ctrl => match ch.as_str() {
                "u" => {
                    self.copy_mode.half_page_up();
                    self.after_copy_mode_motion();
                }
                "d" => {
                    self.copy_mode.half_page_down();
                    self.after_copy_mode_motion();
                }
                "b" => {
                    self.copy_mode.page_up();
                    self.after_copy_mode_motion();
                }
                "f" => {
                    self.copy_mode.page_down();
                    self.after_copy_mode_motion();
                }
                "v" => {
                    self.copy_mode.toggle_visual_block();
                    self.after_copy_mode_motion();
                }
                _ => {}
            },

            // === Named keys ===
            Key::Named(NamedKey::Escape) => {
                if self.copy_mode.visual_mode != VisualMode::None {
                    // Exit visual mode first
                    self.copy_mode.visual_mode = VisualMode::None;
                    self.copy_mode.selection_anchor = None;
                    self.with_active_tab_mut(|tab| {
                        tab.selection_mouse_mut().selection = None;
                        tab.active_cache_mut().cells = None;
                    });
                    self.focus_state.needs_redraw = true;
                    self.request_redraw();
                } else {
                    self.exit_copy_mode();
                }
            }
            Key::Named(NamedKey::ArrowLeft) => {
                self.copy_mode.move_left();
                self.after_copy_mode_motion();
            }
            Key::Named(NamedKey::ArrowRight) => {
                self.copy_mode.move_right();
                self.after_copy_mode_motion();
            }
            Key::Named(NamedKey::ArrowUp) => {
                self.copy_mode.move_up();
                self.after_copy_mode_motion();
            }
            Key::Named(NamedKey::ArrowDown) => {
                self.copy_mode.move_down();
                self.after_copy_mode_motion();
            }
            Key::Named(NamedKey::Home) => {
                self.copy_mode.move_to_line_start();
                self.after_copy_mode_motion();
            }
            Key::Named(NamedKey::End) => {
                self.copy_mode.move_to_line_end();
                self.after_copy_mode_motion();
            }
            Key::Named(NamedKey::PageUp) => {
                self.copy_mode.page_up();
                self.after_copy_mode_motion();
            }
            Key::Named(NamedKey::PageDown) => {
                self.copy_mode.page_down();
                self.after_copy_mode_motion();
            }

            _ => {}
        }
    }
}
