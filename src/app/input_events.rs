use crate::app::window_state::WindowState;
use crate::config::{Config, resolve_shader_config};
use crate::terminal::ClipboardSlot;
use std::sync::Arc;
use winit::event::ElementState;
use winit::event::KeyEvent;
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, NamedKey};

impl WindowState {
    pub(crate) fn handle_key_event(&mut self, event: KeyEvent, event_loop: &ActiveEventLoop) {
        // Track Alt key press/release for Option key mode detection
        self.input_handler.track_alt_key(&event);

        // Check if any UI panel is visible that should block keyboard input
        // Note: Settings are handled by standalone SettingsWindow, not embedded UI
        // Note: Profile drawer does NOT block input - only modal dialogs do
        let any_ui_visible = self.help_ui.visible
            || self.clipboard_history_ui.visible
            || self.search_ui.visible
            || self.profile_modal_ui.visible
            || self.tmux_session_picker_ui.visible;

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
                return;
            }
        }

        // Check if egui UI wants keyboard input (e.g., text fields, ComboBoxes)
        if self.is_egui_using_keyboard() {
            return;
        }

        // Check if active tab's shell has exited
        let is_running = if let Some(tab) = self.tab_manager.active_tab() {
            if let Ok(term) = tab.terminal.try_lock() {
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
            // Abort refresh tasks for all tabs
            for tab in self.tab_manager.tabs_mut() {
                if let Some(task) = tab.refresh_task.take() {
                    task.abort();
                }
            }
            log::info!("Refresh tasks aborted");
            event_loop.exit();
            return;
        }

        // Update last key press time for cursor blink reset and shader effects
        if event.state == ElementState::Pressed {
            self.last_key_press = Some(std::time::Instant::now());
            // Update shader key press time for visual effects (iTimeKeyPress uniform)
            if let Some(renderer) = &mut self.renderer {
                renderer.update_key_press_time();
            }
        }

        // Check user-defined keybindings first (before hardcoded shortcuts)
        if event.state == ElementState::Pressed
            && let Some(action) = self
                .keybinding_registry
                .lookup(&event, &self.input_handler.modifiers)
        {
            // Clone to avoid borrow conflict
            let action = action.to_string();
            if self.execute_keybinding_action(&action) {
                return; // Key was handled by user-defined keybinding
            }
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

        // Check if paste special UI is handling keys
        if self.handle_paste_special_keys(&event) {
            return; // Key was handled for paste special, don't send to terminal
        }

        // Check for search keys (Cmd/Ctrl+F)
        if self.handle_search_keys(&event) {
            return; // Key was handled for search, don't send to terminal
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

        // Check for profile drawer toggle (Cmd+Shift+P / Ctrl+Shift+P)
        if self.handle_profile_drawer_toggle(&event) {
            return; // Key was handled for profile drawer toggle
        }

        // Check for utility shortcuts (clear scrollback, font size, etc.)
        if self.handle_utility_shortcuts(&event, event_loop) {
            return; // Key was handled by utility shortcut
        }

        // Check for tab shortcuts (Cmd+T, Cmd+W, Cmd+Shift+[/], Cmd+1-9)
        if self.handle_tab_shortcuts(&event, event_loop) {
            return; // Key was handled by tab shortcut
        }

        // Handle paste shortcuts with bracketed paste support
        if event.state == ElementState::Pressed {
            // Ctrl/Cmd+V, Shift+Insert, or NamedKey::Paste
            #[cfg(not(target_os = "macos"))]
            let is_paste = {
                let ctrl = self.input_handler.modifiers.state().control_key();
                let shift = self.input_handler.modifiers.state().shift_key();
                matches!(event.logical_key, Key::Named(NamedKey::Paste))
                    || (ctrl
                        && matches!(event.logical_key, Key::Character(ref c) if c.eq_ignore_ascii_case("v")))
                    || (shift && matches!(event.logical_key, Key::Named(NamedKey::Insert)))
            };

            #[cfg(target_os = "macos")]
            let is_paste = {
                let cmd = self.input_handler.modifiers.state().super_key();
                matches!(event.logical_key, Key::Named(NamedKey::Paste))
                    || (cmd
                        && matches!(event.logical_key, Key::Character(ref c) if c.eq_ignore_ascii_case("v")))
            };

            if is_paste {
                if let Some(text) = self.input_handler.paste_from_clipboard()
                    && let Some(tab) = self.tab_manager.active_tab()
                {
                    let terminal_clone = Arc::clone(&tab.terminal);
                    self.runtime.spawn(async move {
                        let term = terminal_clone.lock().await;
                        let _ = term.paste(&text);
                    });
                }
                return;
            }

            // Cmd/Ctrl+C or NamedKey::Copy
            #[cfg(target_os = "macos")]
            let is_copy = {
                let cmd = self.input_handler.modifiers.state().super_key();
                matches!(event.logical_key, Key::Named(NamedKey::Copy))
                    || (cmd
                        && matches!(event.logical_key, Key::Character(ref c) if c.eq_ignore_ascii_case("c")))
            };

            #[cfg(not(target_os = "macos"))]
            let is_copy = {
                let ctrl = self.input_handler.modifiers.state().control_key();
                matches!(event.logical_key, Key::Named(NamedKey::Copy))
                    || (ctrl
                        && matches!(event.logical_key, Key::Character(ref c) if c.eq_ignore_ascii_case("c")))
            };

            if is_copy {
                if let Some(selected_text) = self.get_selected_text()
                    && !selected_text.is_empty()
                {
                    if let Err(e) = self.input_handler.copy_to_clipboard(&selected_text) {
                        log::error!("Failed to copy to clipboard: {}", e);
                    } else {
                        log::debug!("Copied {} chars via keyboard copy", selected_text.len());
                    }
                }
                return;
            }
        }

        // Clear selection on keyboard input (except for modifier-only keys and special keys handled above)
        // Don't clear selection when pressing just modifier keys (Ctrl, Alt, Shift, Cmd)
        let is_modifier_only = matches!(
            event.logical_key,
            Key::Named(
                NamedKey::Control
                    | NamedKey::Alt
                    | NamedKey::Shift
                    | NamedKey::Super
                    | NamedKey::Meta
            )
        );

        if event.state == ElementState::Pressed
            && !is_modifier_only
            && let Some(tab) = self.tab_manager.active_tab_mut()
            && tab.mouse.selection.is_some()
        {
            tab.mouse.selection = None;
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }

        // Handle tmux prefix key mode
        if self.handle_tmux_prefix_key(&event) {
            return; // Key was handled by prefix system
        }

        // Normal key handling - send to terminal (or via tmux if connected)
        if let Some(bytes) = self.input_handler.handle_key_event(event) {
            // Try to send via tmux if connected (check before borrowing tab)
            if self.send_input_via_tmux(&bytes) {
                // Still need to reset anti-idle timer
                if let Some(tab) = self.tab_manager.active_tab_mut() {
                    tab.anti_idle_last_activity = std::time::Instant::now();
                }
                return; // Input was routed through tmux
            }

            // Broadcast input to all panes or just the focused pane
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                // Reset anti-idle timer on keyboard input
                tab.anti_idle_last_activity = std::time::Instant::now();

                // Check if we should broadcast to all panes
                if self.broadcast_input
                    && let Some(ref mut pane_manager) = tab.pane_manager
                    && pane_manager.has_multiple_panes()
                {
                    // Broadcast to all panes
                    let terminals: Vec<_> = pane_manager
                        .all_panes()
                        .iter()
                        .map(|p| Arc::clone(&p.terminal))
                        .collect();

                    let bytes_clone = bytes.clone();
                    self.runtime.spawn(async move {
                        for terminal in terminals {
                            let term = terminal.lock().await;
                            let _ = term.write(&bytes_clone);
                        }
                    });
                    return;
                }

                // Get the terminal to write to:
                // - If split panes exist, use the focused pane's terminal
                // - Otherwise, use the tab's main terminal
                let terminal_clone = if let Some(ref pane_manager) = tab.pane_manager {
                    if let Some(focused_pane) = pane_manager.focused_pane() {
                        Arc::clone(&focused_pane.terminal)
                    } else {
                        Arc::clone(&tab.terminal)
                    }
                } else {
                    Arc::clone(&tab.terminal)
                };

                self.runtime.spawn(async move {
                    let term = terminal_clone.lock().await;
                    let _ = term.write(&bytes);
                });
            }
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

    /// Reload configuration from disk (called internally from F5 handler).
    pub(crate) fn reload_config(&mut self) {
        match Config::load() {
            Ok(new_config) => {
                log::info!("Configuration reloaded successfully");

                // Apply settings that can be changed at runtime

                // Update Option/Alt key modes
                self.config.left_option_key_mode = new_config.left_option_key_mode;
                self.config.right_option_key_mode = new_config.right_option_key_mode;
                self.input_handler.update_option_key_modes(
                    new_config.left_option_key_mode,
                    new_config.right_option_key_mode,
                );

                // Update auto_copy_selection
                self.config.auto_copy_selection = new_config.auto_copy_selection;

                // Update middle_click_paste
                self.config.middle_click_paste = new_config.middle_click_paste;

                // Update window title (check both title and show_window_number)
                if self.config.window_title != new_config.window_title
                    || self.config.show_window_number != new_config.show_window_number
                {
                    self.config.window_title = new_config.window_title.clone();
                    self.config.show_window_number = new_config.show_window_number;
                    if let Some(window) = &self.window {
                        window.set_title(&self.format_title(&new_config.window_title));
                    }
                }

                // Update theme
                if self.config.theme != new_config.theme {
                    self.config.theme = new_config.theme.clone();
                    // Apply theme to all tabs
                    for tab in self.tab_manager.tabs_mut() {
                        if let Ok(mut term) = tab.terminal.try_lock() {
                            term.set_theme(new_config.load_theme());
                        }
                    }
                    log::info!("Applied new theme: {}", new_config.theme);
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

                // Refresh keybinding registry if keybindings changed
                if new_config.keybindings != self.config.keybindings {
                    self.keybinding_registry = crate::keybindings::KeybindingRegistry::from_config(
                        &new_config.keybindings,
                    );
                    self.config.keybindings = new_config.keybindings;
                    log::info!("Keybindings reloaded");
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
                        // Check if Shift is held for paste special
                        let shift = self.input_handler.modifiers.state().shift_key();
                        if let Some(entry) = self.clipboard_history_ui.selected_entry() {
                            let content = entry.content.clone();
                            self.clipboard_history_ui.visible = false;

                            if shift {
                                // Shift+Enter: Open paste special UI with the selected content
                                self.paste_special_ui.open(content);
                                log::info!("Paste special UI opened from clipboard history");
                            } else {
                                // Enter: Paste directly
                                self.paste_text(&content);
                            }
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
        if let Some(tab) = self.tab_manager.active_tab()
            && let Ok(term) = tab.terminal.try_lock()
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

    fn handle_paste_special_keys(&mut self, event: &KeyEvent) -> bool {
        // Handle keys when paste special UI is visible
        if self.paste_special_ui.visible {
            if event.state == ElementState::Pressed {
                match &event.logical_key {
                    Key::Named(winit::keyboard::NamedKey::Escape) => {
                        self.paste_special_ui.close();
                        self.needs_redraw = true;
                        return true;
                    }
                    Key::Named(winit::keyboard::NamedKey::ArrowUp) => {
                        self.paste_special_ui.select_previous();
                        self.needs_redraw = true;
                        return true;
                    }
                    Key::Named(winit::keyboard::NamedKey::ArrowDown) => {
                        self.paste_special_ui.select_next();
                        self.needs_redraw = true;
                        return true;
                    }
                    Key::Named(winit::keyboard::NamedKey::Enter) => {
                        // Apply the selected transformation and paste
                        if let Some(result) = self.paste_special_ui.apply_selected() {
                            self.paste_special_ui.close();
                            self.paste_text(&result);
                            self.needs_redraw = true;
                        }
                        return true;
                    }
                    _ => {}
                }
            }
            // While paste special is visible, consume all key events
            // to prevent them from going to the terminal
            return true;
        }
        false
    }

    pub(crate) fn paste_text(&mut self, text: &str) {
        // Try to paste via tmux if connected
        if self.paste_via_tmux(text) {
            return; // Paste was routed through tmux
        }

        // Fall back to direct terminal paste
        if let Some(tab) = self.tab_manager.active_tab() {
            let terminal_clone = Arc::clone(&tab.terminal);
            let text = text.to_string();
            self.runtime.spawn(async move {
                let term = terminal_clone.lock().await;
                let _ = term.paste(&text);
                log::debug!("Pasted text ({} chars)", text.len());
            });
        }
    }

    fn handle_search_keys(&mut self, event: &KeyEvent) -> bool {
        // Handle keys when search UI is visible
        if self.search_ui.visible {
            if event.state == ElementState::Pressed
                && let Key::Named(winit::keyboard::NamedKey::Escape) = &event.logical_key
            {
                self.search_ui.close();
                self.needs_redraw = true;
                return true;
            }
            // While search is visible, let egui handle most keys
            // Return false to let the event propagate to the UI
            return false;
        }

        // Cmd+F (macOS) or Ctrl+F: Open search
        if event.state == ElementState::Pressed {
            #[cfg(target_os = "macos")]
            let cmd_or_ctrl = self.input_handler.modifiers.state().super_key();
            #[cfg(not(target_os = "macos"))]
            let cmd_or_ctrl = self.input_handler.modifiers.state().control_key();

            let shift = self.input_handler.modifiers.state().shift_key();

            if cmd_or_ctrl
                && !shift
                && matches!(event.logical_key, Key::Character(ref c) if c.eq_ignore_ascii_case("f"))
            {
                self.search_ui.open();
                // Initialize from config
                self.search_ui
                    .init_from_config(self.config.search_case_sensitive, self.config.search_regex);
                self.needs_redraw = true;
                log::debug!("Search UI opened via Cmd/Ctrl+F");
                return true;
            }
        }

        false
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
            let cleared = if let Some(tab) = self.tab_manager.active_tab_mut() {
                if let Ok(term) = tab.terminal.try_lock() {
                    term.clear_scrollback();
                    tab.cache.scrollback_len = 0;
                    true
                } else {
                    false
                }
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
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                tab.cache.cells = None;
            }
            self.needs_redraw = true;

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

            if let Some(tab) = self.tab_manager.active_tab()
                && let Ok(mut term) = tab.terminal.try_lock()
            {
                term.set_cursor_style(term_style);
            }

            return true;
        }

        false
    }

    fn handle_tab_shortcuts(&mut self, event: &KeyEvent, _event_loop: &ActiveEventLoop) -> bool {
        if event.state != ElementState::Pressed {
            return false;
        }

        let ctrl = self.input_handler.modifiers.state().control_key();
        let shift = self.input_handler.modifiers.state().shift_key();

        // Use Cmd on macOS, Ctrl on other platforms
        #[cfg(target_os = "macos")]
        let cmd_or_ctrl = self.input_handler.modifiers.state().super_key();
        #[cfg(not(target_os = "macos"))]
        let cmd_or_ctrl = ctrl;

        // Cmd+T: New tab
        if cmd_or_ctrl
            && !shift
            && matches!(event.logical_key, Key::Character(ref c) if c.as_str() == "t" || c.as_str() == "T")
        {
            self.new_tab();
            log::info!("New tab created via Cmd+T");
            return true;
        }

        // Cmd+W: Smart close (close tab if multiple, close window if single)
        // Note: This is typically handled by the menu accelerator, but we handle it here
        // as a fallback in case the menu is not available
        if cmd_or_ctrl
            && !shift
            && matches!(event.logical_key, Key::Character(ref c) if c.as_str() == "w" || c.as_str() == "W")
        {
            // Always close the current tab - if it's the last tab,
            // close_current_tab returns true and signals window should close
            let should_close_window = self.close_current_tab();
            log::info!(
                "Tab closed via Cmd+W (should_close_window: {})",
                should_close_window
            );
            // If last tab was closed, signal that window should close
            if should_close_window {
                self.is_shutting_down = true;
            }
            return true;
        }

        // Cmd+Shift+]: Next tab
        if cmd_or_ctrl
            && shift
            && matches!(event.logical_key, Key::Character(ref c) if c.as_str() == "]")
        {
            self.next_tab();
            log::debug!("Switched to next tab via Cmd+Shift+]");
            return true;
        }

        // Cmd+Shift+[: Previous tab
        if cmd_or_ctrl
            && shift
            && matches!(event.logical_key, Key::Character(ref c) if c.as_str() == "[")
        {
            self.prev_tab();
            log::debug!("Switched to previous tab via Cmd+Shift+[");
            return true;
        }

        // Ctrl+Tab: Next tab (alternative)
        if ctrl && !shift && matches!(event.logical_key, Key::Named(NamedKey::Tab)) {
            self.next_tab();
            log::debug!("Switched to next tab via Ctrl+Tab");
            return true;
        }

        // Ctrl+Shift+Tab: Previous tab (alternative)
        if ctrl && shift && matches!(event.logical_key, Key::Named(NamedKey::Tab)) {
            self.prev_tab();
            log::debug!("Switched to previous tab via Ctrl+Shift+Tab");
            return true;
        }

        // Cmd+Shift+Left: Move tab left
        if cmd_or_ctrl && shift && matches!(event.logical_key, Key::Named(NamedKey::ArrowLeft)) {
            self.move_tab_left();
            log::debug!("Moved tab left via Cmd+Shift+Left");
            return true;
        }

        // Cmd+Shift+Right: Move tab right
        if cmd_or_ctrl && shift && matches!(event.logical_key, Key::Named(NamedKey::ArrowRight)) {
            self.move_tab_right();
            log::debug!("Moved tab right via Cmd+Shift+Right");
            return true;
        }

        // Cmd+1-9: Switch to tab N
        if cmd_or_ctrl && !shift {
            let tab_num = match &event.logical_key {
                Key::Character(c) => match c.as_str() {
                    "1" => Some(1),
                    "2" => Some(2),
                    "3" => Some(3),
                    "4" => Some(4),
                    "5" => Some(5),
                    "6" => Some(6),
                    "7" => Some(7),
                    "8" => Some(8),
                    "9" => Some(9),
                    _ => None,
                },
                _ => None,
            };

            if let Some(n) = tab_num {
                self.switch_to_tab_index(n);
                log::debug!("Switched to tab {} via Cmd+{}", n, n);
                return true;
            }
        }

        false
    }

    /// Execute a keybinding action by name.
    ///
    /// Returns true if the action was handled, false if unknown.
    fn execute_keybinding_action(&mut self, action: &str) -> bool {
        match action {
            "toggle_background_shader" => {
                self.toggle_background_shader();
                true
            }
            "toggle_cursor_shader" => {
                self.toggle_cursor_shader();
                true
            }
            "reload_config" => {
                self.reload_config();
                true
            }
            "open_settings" => {
                self.open_settings_window_requested = true;
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
                log::info!("Settings window requested via keybinding");
                true
            }
            "toggle_fullscreen" => {
                if let Some(window) = &self.window {
                    self.is_fullscreen = !self.is_fullscreen;
                    if self.is_fullscreen {
                        window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
                        log::info!("Entering fullscreen mode via keybinding");
                    } else {
                        window.set_fullscreen(None);
                        log::info!("Exiting fullscreen mode via keybinding");
                    }
                }
                true
            }
            "maximize_vertically" => {
                if let Some(window) = &self.window {
                    // Get current monitor to determine screen height
                    if let Some(monitor) = window.current_monitor() {
                        let monitor_pos = monitor.position();
                        let monitor_size = monitor.size();
                        let window_pos = window.outer_position().unwrap_or_default();
                        let window_size = window.outer_size();

                        // Set window to span full height while keeping current X position and width
                        window.set_outer_position(winit::dpi::PhysicalPosition::new(
                            window_pos.x,
                            monitor_pos.y,
                        ));
                        let _ = window.request_inner_size(winit::dpi::PhysicalSize::new(
                            window_size.width,
                            monitor_size.height,
                        ));
                        log::info!("Window maximized vertically via keybinding");
                    }
                }
                true
            }
            "toggle_help" => {
                self.help_ui.toggle();
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
                log::info!(
                    "Help UI toggled via keybinding: {}",
                    if self.help_ui.visible {
                        "visible"
                    } else {
                        "hidden"
                    }
                );
                true
            }
            "toggle_fps_overlay" => {
                self.debug.show_fps_overlay = !self.debug.show_fps_overlay;
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
                log::info!(
                    "FPS overlay toggled via keybinding: {}",
                    if self.debug.show_fps_overlay {
                        "visible"
                    } else {
                        "hidden"
                    }
                );
                true
            }
            "toggle_search" => {
                self.search_ui.toggle();
                if self.search_ui.visible {
                    self.search_ui.init_from_config(
                        self.config.search_case_sensitive,
                        self.config.search_regex,
                    );
                }
                self.needs_redraw = true;
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
                log::info!(
                    "Search UI toggled via keybinding: {}",
                    if self.search_ui.visible {
                        "visible"
                    } else {
                        "hidden"
                    }
                );
                true
            }
            "new_tab" => {
                self.new_tab();
                log::info!("New tab created via keybinding");
                true
            }
            "close_tab" => {
                if self.has_multiple_tabs() {
                    self.close_current_tab();
                    log::info!("Tab closed via keybinding");
                }
                true
            }
            "next_tab" => {
                self.next_tab();
                log::debug!("Switched to next tab via keybinding");
                true
            }
            "prev_tab" => {
                self.prev_tab();
                log::debug!("Switched to previous tab via keybinding");
                true
            }
            "paste_special" => {
                // Get clipboard content and open paste special UI
                if let Some(text) = self.input_handler.paste_from_clipboard() {
                    self.paste_special_ui.open(text);
                    self.needs_redraw = true;
                    if let Some(window) = &self.window {
                        window.request_redraw();
                    }
                    log::info!("Paste special UI opened");
                } else {
                    log::debug!("Paste special: no clipboard content");
                }
                true
            }
            "toggle_session_logging" => {
                if let Some(tab) = self.tab_manager.active_tab_mut() {
                    match tab.toggle_session_logging(&self.config) {
                        Ok(is_active) => {
                            let message = if is_active {
                                "⏺ Recording Started"
                            } else {
                                "⏹ Recording Stopped"
                            };
                            log::info!(
                                "Session logging toggled: {}",
                                if is_active { "started" } else { "stopped" }
                            );
                            // Show toast after releasing tab borrow
                            self.show_toast(message);
                        }
                        Err(e) => {
                            log::error!("Failed to toggle session logging: {}", e);
                            self.show_toast(format!("Recording Error: {}", e));
                        }
                    }
                }
                true
            }
            "split_horizontal" => {
                self.split_pane_horizontal();
                true
            }
            "split_vertical" => {
                self.split_pane_vertical();
                true
            }
            "close_pane" => {
                self.close_focused_pane();
                true
            }
            "navigate_pane_left" => {
                self.navigate_pane(crate::pane::NavigationDirection::Left);
                true
            }
            "navigate_pane_right" => {
                self.navigate_pane(crate::pane::NavigationDirection::Right);
                true
            }
            "navigate_pane_up" => {
                self.navigate_pane(crate::pane::NavigationDirection::Up);
                true
            }
            "navigate_pane_down" => {
                self.navigate_pane(crate::pane::NavigationDirection::Down);
                true
            }
            "resize_pane_left" => {
                self.resize_pane(crate::pane::NavigationDirection::Left);
                true
            }
            "resize_pane_right" => {
                self.resize_pane(crate::pane::NavigationDirection::Right);
                true
            }
            "resize_pane_up" => {
                self.resize_pane(crate::pane::NavigationDirection::Up);
                true
            }
            "resize_pane_down" => {
                self.resize_pane(crate::pane::NavigationDirection::Down);
                true
            }
            "toggle_tmux_session_picker" => {
                self.tmux_session_picker_ui.toggle();
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
                log::info!(
                    "tmux session picker toggled via keybinding: {}",
                    if self.tmux_session_picker_ui.visible {
                        "visible"
                    } else {
                        "hidden"
                    }
                );
                true
            }
            "toggle_broadcast_input" => {
                self.broadcast_input = !self.broadcast_input;
                let message = if self.broadcast_input {
                    "Broadcast Input: ON"
                } else {
                    "Broadcast Input: OFF"
                };
                self.show_toast(message);
                log::info!(
                    "Broadcast input mode {}",
                    if self.broadcast_input {
                        "enabled"
                    } else {
                        "disabled"
                    }
                );
                true
            }
            "toggle_profile_drawer" => {
                self.toggle_profile_drawer();
                log::info!(
                    "Profile drawer toggled via keybinding: {}",
                    if self.profile_drawer_ui.expanded {
                        "expanded"
                    } else {
                        "collapsed"
                    }
                );
                true
            }
            "toggle_clipboard_history" => {
                self.toggle_clipboard_history();
                log::info!(
                    "Clipboard history toggled via keybinding: {}",
                    if self.clipboard_history_ui.visible {
                        "visible"
                    } else {
                        "hidden"
                    }
                );
                true
            }
            "clear_scrollback" => {
                let cleared = if let Some(tab) = self.tab_manager.active_tab_mut() {
                    if let Ok(term) = tab.terminal.try_lock() {
                        term.clear_scrollback();
                        tab.cache.scrollback_len = 0;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                };
                if cleared {
                    self.set_scroll_target(0);
                    log::info!("Cleared scrollback buffer via keybinding");
                }
                true
            }
            "increase_font_size" => {
                self.config.font_size = (self.config.font_size + 1.0).min(72.0);
                self.pending_font_rebuild = true;
                log::info!(
                    "Font size increased to {} via keybinding",
                    self.config.font_size
                );
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
                true
            }
            "decrease_font_size" => {
                self.config.font_size = (self.config.font_size - 1.0).max(6.0);
                self.pending_font_rebuild = true;
                log::info!(
                    "Font size decreased to {} via keybinding",
                    self.config.font_size
                );
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
                true
            }
            "reset_font_size" => {
                self.config.font_size = 14.0;
                self.pending_font_rebuild = true;
                log::info!("Font size reset to default (14.0) via keybinding");
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
                true
            }
            "cycle_cursor_style" => {
                use crate::config::CursorStyle;
                use par_term_emu_core_rust::cursor::CursorStyle as TermCursorStyle;

                self.config.cursor_style = match self.config.cursor_style {
                    CursorStyle::Block => CursorStyle::Beam,
                    CursorStyle::Beam => CursorStyle::Underline,
                    CursorStyle::Underline => CursorStyle::Block,
                };

                if let Some(tab) = self.tab_manager.active_tab_mut() {
                    tab.cache.cells = None;
                }
                self.needs_redraw = true;

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

                if let Some(tab) = self.tab_manager.active_tab()
                    && let Ok(mut term) = tab.terminal.try_lock()
                {
                    term.set_cursor_style(term_style);
                }
                true
            }
            "move_tab_left" => {
                self.move_tab_left();
                log::debug!("Moved tab left via keybinding");
                true
            }
            "move_tab_right" => {
                self.move_tab_right();
                log::debug!("Moved tab right via keybinding");
                true
            }
            "switch_to_tab_1" => {
                self.switch_to_tab_index(1);
                true
            }
            "switch_to_tab_2" => {
                self.switch_to_tab_index(2);
                true
            }
            "switch_to_tab_3" => {
                self.switch_to_tab_index(3);
                true
            }
            "switch_to_tab_4" => {
                self.switch_to_tab_index(4);
                true
            }
            "switch_to_tab_5" => {
                self.switch_to_tab_index(5);
                true
            }
            "switch_to_tab_6" => {
                self.switch_to_tab_index(6);
                true
            }
            "switch_to_tab_7" => {
                self.switch_to_tab_index(7);
                true
            }
            "switch_to_tab_8" => {
                self.switch_to_tab_index(8);
                true
            }
            "switch_to_tab_9" => {
                self.switch_to_tab_index(9);
                true
            }
            _ => {
                log::warn!("Unknown keybinding action: {}", action);
                false
            }
        }
    }

    /// Show a toast notification with the given message.
    ///
    /// The toast will be displayed for 2 seconds and then automatically hidden.
    pub(crate) fn show_toast(&mut self, message: impl Into<String>) {
        self.toast_message = Some(message.into());
        self.toast_hide_time = Some(std::time::Instant::now() + std::time::Duration::from_secs(2));
        self.needs_redraw = true;
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    /// Toggle the background/custom shader on/off.
    pub(crate) fn toggle_background_shader(&mut self) {
        self.config.custom_shader_enabled = !self.config.custom_shader_enabled;

        if let Some(renderer) = &mut self.renderer {
            // Get shader metadata from cache for resolution
            let metadata = self
                .config
                .custom_shader
                .as_ref()
                .and_then(|name| self.shader_metadata_cache.get(name).cloned());

            // Get per-shader overrides
            let shader_override = self
                .config
                .custom_shader
                .as_ref()
                .and_then(|name| self.config.shader_configs.get(name).cloned());

            // Resolve config with 3-tier system
            let resolved =
                resolve_shader_config(shader_override.as_ref(), metadata.as_ref(), &self.config);

            let _ = renderer.set_custom_shader_enabled(
                self.config.custom_shader_enabled,
                self.config.custom_shader.as_deref(),
                self.config.window_opacity,
                resolved.text_opacity,
                self.config.custom_shader_animation,
                resolved.animation_speed,
                resolved.full_content,
                resolved.brightness,
                &resolved.channel_paths(),
                resolved.cubemap_path().map(|p| p.as_path()),
            );
        }

        self.needs_redraw = true;
        if let Some(window) = &self.window {
            window.request_redraw();
        }

        log::info!(
            "Background shader {}",
            if self.config.custom_shader_enabled {
                "enabled"
            } else {
                "disabled"
            }
        );
    }

    /// Toggle the cursor shader on/off.
    pub(crate) fn toggle_cursor_shader(&mut self) {
        self.config.cursor_shader_enabled = !self.config.cursor_shader_enabled;

        if let Some(renderer) = &mut self.renderer {
            let _ = renderer.set_cursor_shader_enabled(
                self.config.cursor_shader_enabled,
                self.config.cursor_shader.as_deref(),
                self.config.window_opacity,
                self.config.cursor_shader_animation,
                self.config.cursor_shader_animation_speed,
            );
        }

        self.needs_redraw = true;
        if let Some(window) = &self.window {
            window.request_redraw();
        }

        log::info!(
            "Cursor shader {}",
            if self.config.cursor_shader_enabled {
                "enabled"
            } else {
                "disabled"
            }
        );
    }
}
