//! Utility keyboard shortcuts: clear scrollback, font size, cursor style.

use crate::app::window_state::WindowState;
use std::sync::Arc;
use winit::event::{ElementState, KeyEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::Key;

impl WindowState {
    pub(crate) fn handle_utility_shortcuts(
        &mut self,
        event: &KeyEvent,
        _event_loop: &ActiveEventLoop,
    ) -> bool {
        if event.state != ElementState::Pressed {
            return false;
        }

        let ctrl = self.input_handler.modifiers.state().control_key();
        let shift = self.input_handler.modifiers.state().shift_key();

        // Ctrl+Shift+K: Clear scrollback
        if ctrl
            && shift
            && matches!(event.logical_key, Key::Character(ref c) if c.as_str() == "k" || c.as_str() == "K")
        {
            // Clear scrollback if terminal is available
            let cleared = if let Some(tab) = self.tab_manager.active_tab_mut() {
                // try_lock: intentional — keyboard shortcut handler in sync event loop.
                // On miss: scrollback is not cleared this keypress. User can press again.
                let did_clear = if let Ok(mut term) = tab.terminal.try_write() {
                    term.clear_scrollback();
                    term.clear_scrollback_metadata();
                    true
                } else {
                    false
                };
                if did_clear {
                    tab.active_cache_mut().scrollback_len = 0;
                    tab.scripting.trigger_marks.clear();
                }
                did_clear
            } else {
                false
            };

            if cleared {
                self.set_scroll_target(0);
                log::info!("Cleared scrollback buffer");
            }
            return true;
        }

        // Ctrl+L: Clear screen (send clear sequence to shell)
        if ctrl
            && !shift
            && matches!(event.logical_key, Key::Character(ref c) if c.as_str() == "l" || c.as_str() == "L")
        {
            if let Some(tab) = self.tab_manager.active_tab() {
                let terminal_clone = Arc::clone(&tab.terminal);
                // Send the "clear" command sequence (Ctrl+L)
                let clear_sequence = vec![0x0C]; // Ctrl+L character
                self.runtime.spawn(async move {
                    // try_lock: intentional — spawned async task uses try_lock to avoid
                    // blocking the tokio worker. On miss: the Ctrl+L clear is silently dropped.
                    // User can press the shortcut again.
                    if let Ok(term) = terminal_clone.try_write() {
                        let _ = term.write(&clear_sequence);
                        log::debug!("Sent clear screen sequence (Ctrl+L)");
                    }
                });
            }
            return true;
        }

        // Ctrl+Plus/Equals: Increase font size (applies live)
        if ctrl
            && !shift
            && (matches!(event.logical_key, Key::Character(ref c) if c.as_str() == "+" || c.as_str() == "="))
        {
            self.config.font_size = (self.config.font_size + 1.0).min(72.0);
            self.pending_font_rebuild = true;
            log::info!(
                "Font size increased to {} (applying live)",
                self.config.font_size
            );
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            return true;
        }

        // Ctrl+Minus: Decrease font size (applies live)
        if ctrl
            && !shift
            && matches!(event.logical_key, Key::Character(ref c) if c.as_str() == "-" || c.as_str() == "_")
        {
            self.config.font_size = (self.config.font_size - 1.0).max(6.0);
            self.pending_font_rebuild = true;
            log::info!(
                "Font size decreased to {} (applying live)",
                self.config.font_size
            );
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            return true;
        }

        // Ctrl+0: Reset font size to default (applies live)
        if ctrl && !shift && matches!(event.logical_key, Key::Character(ref c) if c.as_str() == "0")
        {
            self.config.font_size = 14.0; // Default font size
            self.pending_font_rebuild = true;
            log::info!("Font size reset to default (14.0, applying live)");
            if let Some(window) = &self.window {
                window.request_redraw();
            }
            return true;
        }

        // Ctrl+, (Cmd+, on macOS): Cycle cursor style (Block -> Beam -> Underline -> Block)
        let super_key = self.input_handler.modifiers.state().super_key();
        let ctrl_or_cmd = ctrl || super_key;

        if ctrl_or_cmd
            && !shift
            && matches!(event.logical_key, Key::Character(ref c) if c.as_str() == ",")
        {
            use crate::config::CursorStyle;
            use par_term_emu_core_rust::cursor::CursorStyle as TermCursorStyle;

            // Cycle to next cursor style
            self.config.cursor_style = match self.config.cursor_style {
                CursorStyle::Block => CursorStyle::Beam,
                CursorStyle::Beam => CursorStyle::Underline,
                CursorStyle::Underline => CursorStyle::Block,
            };

            // Force cell regen to reflect cursor style change
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                tab.active_cache_mut().cells = None;
            }
            self.focus_state.needs_redraw = true;

            log::info!("Cycled cursor style to {:?}", self.config.cursor_style);

            // Map our config cursor style to terminal cursor style
            // Respect the cursor_blink setting when cycling styles
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

            // try_lock: intentional — cursor style update from keybinding handler in sync
            // event loop. On miss: the cursor style is not updated this frame; it will
            // apply on the next keybinding invocation. Cosmetic only.
            if let Some(tab) = self.tab_manager.active_tab()
                && let Ok(mut term) = tab.terminal.try_write()
            {
                term.set_cursor_style(term_style);
            }

            return true;
        }

        false
    }
}
