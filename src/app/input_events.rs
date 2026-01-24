use crate::app::AppState;
use crate::config::Config;
use crate::terminal::ClipboardSlot;
use std::sync::Arc;
use winit::event::ElementState;
use winit::event::KeyEvent;
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, NamedKey};

impl AppState {
    pub(crate) fn handle_key_event(&mut self, event: KeyEvent, event_loop: &ActiveEventLoop) {
        // Check if any UI panel is visible
        let any_ui_visible =
            self.settings_ui.visible || self.help_ui.visible || self.clipboard_history_ui.visible;

        // When UI panels are visible, block ALL keys from going to terminal
        // except for UI control keys (Escape handled by egui, F1/F2/F3 for toggles)
        if any_ui_visible {
            let is_ui_control_key = matches!(
                event.logical_key,
                Key::Named(NamedKey::F1)
                    | Key::Named(NamedKey::F2)
                    | Key::Named(NamedKey::F3)
                    | Key::Named(NamedKey::Escape)
            );

            if !is_ui_control_key {
                log::debug!("Blocking key while UI visible: {:?}", event.logical_key);
                return;
            }
        }

        // Check if egui UI wants keyboard input (e.g., text fields, ComboBoxes)
        if self.is_egui_using_keyboard() {
            log::debug!("Blocking key event: egui wants keyboard input");
            return;
        }

        // Check if shell has exited
        let is_running = if let Some(terminal) = &self.terminal {
            if let Ok(term) = terminal.try_lock() {
                term.is_running()
            } else {
                true
            }
        } else {
            true
        };

        // If shell exited and user presses any key, exit the application
        // (fallback behavior if close_on_shell_exit is false)
        if !is_running && event.state == ElementState::Pressed {
            log::info!("Shell has exited, closing terminal on keypress");
            // Abort the refresh task to prevent lockup on shutdown
            if let Some(task) = self.refresh_task.take() {
                task.abort();
                log::info!("Refresh task aborted");
            }
            event_loop.exit();
            return;
        }

        // Update last key press time for cursor blink reset
        if event.state == ElementState::Pressed {
            self.last_key_press = Some(std::time::Instant::now());
        }

        // Check if this is a scroll navigation key
        if self.handle_scroll_keys(&event) {
            return; // Key was handled for scrolling, don't send to terminal
        }

        // Check if this is a config reload key (F5)
        if self.handle_config_reload(&event) {
            return; // Key was handled for config reload, don't send to terminal
        }

        // Check if this is a clipboard history key (Ctrl+Shift+H)
        if self.handle_clipboard_history_keys(&event) {
            return; // Key was handled for clipboard history, don't send to terminal
        }

        // Check for fullscreen toggle (F11)
        if self.handle_fullscreen_toggle(&event) {
            return; // Key was handled for fullscreen toggle
        }

        // Check for help toggle (F1)
        if self.handle_help_toggle(&event) {
            return; // Key was handled for help toggle
        }

        // Check for settings toggle (F12)
        if self.handle_settings_toggle(&event) {
            return; // Key was handled for settings toggle
        }

        // Check for shader editor toggle (F11)
        if self.handle_shader_editor_toggle(&event) {
            return; // Key was handled for shader editor toggle
        }

        // Check for FPS overlay toggle (F3)
        if self.handle_fps_overlay_toggle(&event) {
            return; // Key was handled for FPS overlay toggle
        }

        // Check for utility shortcuts (clear scrollback, font size, etc.)
        if self.handle_utility_shortcuts(&event, event_loop) {
            return; // Key was handled by utility shortcut
        }

        // Clear selection on keyboard input (except for special keys handled above)
        if event.state == ElementState::Pressed && self.mouse.selection.is_some() {
            self.mouse.selection = None;
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }

        // Debug: Log Tab and Space key before processing
        let is_tab = matches!(event.logical_key, Key::Named(NamedKey::Tab));
        let is_space = matches!(event.logical_key, Key::Named(NamedKey::Space));
        if is_tab {
            log::debug!("Tab key event received, state={:?}", event.state);
        }
        if is_space {
            log::debug!("Space key event received, state={:?}", event.state);
        }

        // Normal key handling - send to terminal
        if let Some(bytes) = self.input_handler.handle_key_event(event)
            && let Some(terminal) = &self.terminal
        {
            if is_tab {
                log::debug!("Sending Tab key to terminal ({} bytes)", bytes.len());
            }
            if is_space {
                log::debug!("Sending Space key to terminal ({} bytes)", bytes.len());
            }
            let terminal_clone = Arc::clone(terminal);

            self.runtime.spawn(async move {
                let term = terminal_clone.lock().await;
                let _ = term.write(&bytes);
            });
        }
    }

    fn handle_scroll_keys(&mut self, event: &KeyEvent) -> bool {
        if event.state != ElementState::Pressed {
            return false;
        }

        let shift = self.input_handler.modifiers.state().shift_key();

        let handled = match &event.logical_key {
            Key::Named(NamedKey::PageUp) => {
                // Scroll up one page
                self.scroll_up_page();
                true
            }
            Key::Named(NamedKey::PageDown) => {
                // Scroll down one page
                self.scroll_down_page();
                true
            }
            Key::Named(NamedKey::Home) if shift => {
                // Shift+Home: Scroll to top
                self.scroll_to_top();
                true
            }
            Key::Named(NamedKey::End) if shift => {
                // Shift+End: Scroll to bottom
                self.scroll_to_bottom();
                true
            }
            _ => false,
        };

        if handled && let Some(window) = &self.window {
            window.request_redraw();
        }

        handled
    }

    fn handle_config_reload(&mut self, event: &KeyEvent) -> bool {
        if event.state != ElementState::Pressed {
            return false;
        }

        // F5 to reload config
        if matches!(event.logical_key, Key::Named(NamedKey::F5)) {
            log::info!("Reloading configuration (F5 pressed)");
            self.reload_config();
            return true;
        }

        false
    }

    fn reload_config(&mut self) {
        match Config::load() {
            Ok(new_config) => {
                log::info!("Configuration reloaded successfully");

                // Apply settings that can be changed at runtime

                // Update auto_copy_selection
                self.config.auto_copy_selection = new_config.auto_copy_selection;

                // Update middle_click_paste
                self.config.middle_click_paste = new_config.middle_click_paste;

                // Update window title
                if self.config.window_title != new_config.window_title {
                    self.config.window_title = new_config.window_title.clone();
                    if let Some(window) = &self.window {
                        window.set_title(&new_config.window_title);
                    }
                }

                // Update theme
                if self.config.theme != new_config.theme {
                    self.config.theme = new_config.theme.clone();
                    if let Some(terminal) = &self.terminal
                        && let Ok(mut term) = terminal.try_lock()
                    {
                        term.set_theme(new_config.load_theme());
                        log::info!("Applied new theme: {}", new_config.theme);
                    }
                }

                // Note: Clipboard history and notification settings not yet available in core library
                // Config reloading for these features will be enabled when APIs become available

                // Note: Terminal dimensions and scrollback size still require restart
                if new_config.font_size != self.config.font_size {
                    log::info!(
                        "Font size changed from {} -> {} (applied live)",
                        self.config.font_size,
                        new_config.font_size
                    );
                }

                if new_config.cols != self.config.cols || new_config.rows != self.config.rows {
                    log::warn!("Terminal dimensions change requires restart");
                }

                // Request redraw to apply theme changes
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            }
            Err(e) => {
                log::error!("Failed to reload configuration: {}", e);
            }
        }
    }

    fn handle_clipboard_history_keys(&mut self, event: &KeyEvent) -> bool {
        // Handle Escape to close clipboard history UI
        if self.clipboard_history_ui.visible {
            if event.state == ElementState::Pressed {
                match &event.logical_key {
                    Key::Named(winit::keyboard::NamedKey::Escape) => {
                        self.clipboard_history_ui.visible = false;
                        self.needs_redraw = true;
                        return true;
                    }
                    Key::Named(winit::keyboard::NamedKey::ArrowUp) => {
                        self.clipboard_history_ui.select_previous();
                        self.needs_redraw = true;
                        return true;
                    }
                    Key::Named(winit::keyboard::NamedKey::ArrowDown) => {
                        self.clipboard_history_ui.select_next();
                        self.needs_redraw = true;
                        return true;
                    }
                    Key::Named(winit::keyboard::NamedKey::Enter) => {
                        // Paste the selected entry
                        if let Some(entry) = self.clipboard_history_ui.selected_entry() {
                            let content = entry.content.clone();
                            self.clipboard_history_ui.visible = false;
                            self.paste_text(&content);
                            self.needs_redraw = true;
                        }
                        return true;
                    }
                    _ => {}
                }
            }
            // While clipboard history is visible, consume all key events
            return true;
        }

        // Ctrl+Shift+H: Toggle clipboard history UI
        if event.state == ElementState::Pressed {
            let ctrl = self.input_handler.modifiers.state().control_key();
            let shift = self.input_handler.modifiers.state().shift_key();

            if ctrl
                && shift
                && matches!(event.logical_key, Key::Character(ref c) if c.as_str() == "h" || c.as_str() == "H")
            {
                self.toggle_clipboard_history();
                return true;
            }
        }

        false
    }

    fn toggle_clipboard_history(&mut self) {
        // Refresh clipboard history entries from terminal before showing
        if let Some(terminal) = &self.terminal
            && let Ok(term) = terminal.try_lock()
        {
            // Get history for all slots and merge
            let mut all_entries = Vec::new();
            all_entries.extend(term.get_clipboard_history(ClipboardSlot::Primary));
            all_entries.extend(term.get_clipboard_history(ClipboardSlot::Clipboard));
            all_entries.extend(term.get_clipboard_history(ClipboardSlot::Selection));

            // Sort by timestamp (newest first)
            all_entries.sort_by_key(|e| std::cmp::Reverse(e.timestamp));

            self.clipboard_history_ui.update_entries(all_entries);
        }

        self.clipboard_history_ui.toggle();
        self.needs_redraw = true;
        log::debug!(
            "Clipboard history UI toggled: {}",
            self.clipboard_history_ui.visible
        );
    }

    pub(crate) fn paste_text(&mut self, text: &str) {
        if let Some(terminal) = &self.terminal {
            let terminal_clone = Arc::clone(terminal);
            // Convert newlines to carriage returns for terminal
            let text = text.replace('\n', "\r");
            self.runtime.spawn(async move {
                let term = terminal_clone.lock().await;
                let _ = term.write(text.as_bytes());
                log::debug!("Pasted text from clipboard history ({} bytes)", text.len());
            });
        }
    }

    fn handle_utility_shortcuts(
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
            let cleared = if let Some(terminal) = &self.terminal
                && let Ok(term) = terminal.try_lock()
            {
                term.clear_scrollback();
                true
            } else {
                false
            };

            if cleared {
                self.cache.scrollback_len = 0;
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
            if let Some(terminal) = &self.terminal {
                let terminal_clone = Arc::clone(terminal);
                // Send the "clear" command sequence (Ctrl+L)
                let clear_sequence = vec![0x0C]; // Ctrl+L character
                self.runtime.spawn(async move {
                    if let Ok(term) = terminal_clone.try_lock() {
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
            self.cache.cells = None;
            self.needs_redraw = true;

            log::info!("Cycled cursor style to {:?}", self.config.cursor_style);
            
            // Map our config cursor style to terminal cursor style
            // This ensures consistent behavior between configured style and runtime changes
            let term_style = match self.config.cursor_style {
                CursorStyle::Block => TermCursorStyle::BlinkingBlock, // Default to blinking
                CursorStyle::Beam => TermCursorStyle::BlinkingBar,
                CursorStyle::Underline => TermCursorStyle::BlinkingUnderline,
            };
            
            if let Some(terminal) = &self.terminal 
                && let Ok(mut term) = terminal.try_lock() {
                term.set_cursor_style(term_style);
            }
            
            return true;
        }

        false
    }
}
