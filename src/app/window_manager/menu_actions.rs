//! Menu action handling for the window manager.
//!
//! This module processes native menu events (Copy, Paste, New Window, etc.)
//! and dispatches them to the appropriate window or terminal.

use std::sync::Arc;

use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use crate::menu::MenuAction;

use super::WindowManager;

impl WindowManager {
    /// Handle a menu action
    pub fn handle_menu_action(
        &mut self,
        action: MenuAction,
        event_loop: &ActiveEventLoop,
        focused_window: Option<WindowId>,
    ) {
        match action {
            MenuAction::NewWindow => {
                self.create_window(event_loop);
            }
            MenuAction::CloseWindow => {
                // Smart close: close tab if multiple tabs, close window if single tab
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                    && window_state.close_current_tab()
                {
                    // Last tab closed, close the window
                    self.close_window(window_id);
                }
            }
            MenuAction::NewTab => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.new_tab();
                }
            }
            MenuAction::CloseTab => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                    && window_state.close_current_tab()
                {
                    // Last tab closed, close the window
                    self.close_window(window_id);
                }
            }
            MenuAction::NextTab => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.next_tab();
                }
            }
            MenuAction::PreviousTab => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.prev_tab();
                }
            }
            MenuAction::SwitchToTab(index) => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.switch_to_tab_index(index);
                }
            }
            MenuAction::MoveTabLeft => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.move_tab_left();
                }
            }
            MenuAction::MoveTabRight => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.move_tab_right();
                }
            }
            MenuAction::DuplicateTab => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.duplicate_tab();
                }
            }
            MenuAction::Quit => {
                // Close all windows
                let window_ids: Vec<_> = self.windows.keys().copied().collect();
                for window_id in window_ids {
                    self.close_window(window_id);
                }
            }
            MenuAction::Copy => {
                // If settings window is focused, inject copy event into egui
                if let Some(sw) = &self.settings_window
                    && sw.is_focused()
                {
                    if let Some(sw) = &mut self.settings_window {
                        sw.inject_event(egui::Event::Copy);
                    }
                    return;
                }
                // If an egui overlay (profile modal, search, etc.) is active, inject into main egui
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                    && window_state.has_egui_text_overlay_visible()
                {
                    window_state.pending_egui_events.push(egui::Event::Copy);
                    return;
                }
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                    && let Some(text) = window_state.get_selected_text_for_copy()
                {
                    if let Err(e) = window_state.input_handler.copy_to_clipboard(&text) {
                        log::error!("Failed to copy to clipboard: {}", e);
                    } else {
                        // Sync to tmux paste buffer if connected
                        window_state.sync_clipboard_to_tmux(&text);
                    }
                }
            }
            MenuAction::Paste => {
                // If settings window is focused, inject paste into its egui context
                // (macOS menu accelerator intercepts Cmd+V before egui sees it)
                if let Some(sw) = &self.settings_window
                    && sw.is_focused()
                {
                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                        if let Ok(text) = clipboard.get_text() {
                            if let Some(sw) = &mut self.settings_window {
                                sw.inject_paste(text);
                            }
                            return;
                        }
                        // Clipboard has no text — check for image below.
                        if clipboard.get_image().is_err() {
                            // Neither text nor image — nothing to paste
                            return;
                        }
                    } else {
                        return;
                    }
                }
                // If an egui overlay (profile modal, search, etc.) is active, inject into main egui
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                    && window_state.has_egui_text_overlay_visible()
                {
                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                        if let Ok(text) = clipboard.get_text() {
                            window_state
                                .pending_egui_events
                                .push(egui::Event::Paste(text));
                            return;
                        }
                        // Clipboard has no text — fall through to check for image
                        if clipboard.get_image().is_err() {
                            return;
                        }
                    } else {
                        return;
                    }
                }
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    if let Some(text) = window_state.input_handler.paste_from_clipboard() {
                        window_state.paste_text(&text);
                    } else if window_state.input_handler.clipboard_has_image() {
                        // Clipboard has an image but no text — forward as Ctrl+V (0x16) so
                        // image-aware child processes (e.g., Claude Code) can handle image paste
                        if let Some(tab) = window_state.tab_manager.active_tab() {
                            let terminal_clone = Arc::clone(&tab.terminal);
                            window_state.runtime.spawn(async move {
                                let term = terminal_clone.lock().await;
                                let _ = term.write(b"\x16");
                            });
                        }
                    }
                }
            }
            MenuAction::SelectAll => {
                // If settings window is focused, inject select-all into egui
                if let Some(sw) = &self.settings_window
                    && sw.is_focused()
                {
                    if let Some(sw) = &mut self.settings_window {
                        // egui has no dedicated SelectAll event; use Cmd+A key event
                        sw.inject_event(egui::Event::Key {
                            key: egui::Key::A,
                            physical_key: None,
                            pressed: true,
                            repeat: false,
                            modifiers: egui::Modifiers::COMMAND,
                        });
                    }
                    return;
                }
                // If an egui overlay is active, inject select-all into main egui
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                    && window_state.has_egui_text_overlay_visible()
                {
                    window_state.pending_egui_events.push(egui::Event::Key {
                        key: egui::Key::A,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::COMMAND,
                    });
                    return;
                }
                // Not implemented for terminal - would select all visible text
                log::debug!("SelectAll menu action (not implemented for terminal)");
            }
            MenuAction::ClearScrollback => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    // Clear scrollback in active tab
                    let cleared = if let Some(tab) = window_state.tab_manager.active_tab_mut() {
                        if let Ok(mut term) = tab.terminal.try_lock() {
                            term.clear_scrollback();
                            term.clear_scrollback_metadata();
                            tab.cache.scrollback_len = 0;
                            tab.trigger_marks.clear();
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    };

                    if cleared {
                        window_state.set_scroll_target(0);
                        log::info!("Cleared scrollback buffer");
                    }
                }
            }
            MenuAction::ClipboardHistory => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.overlay_ui.clipboard_history_ui.toggle();
                    window_state.needs_redraw = true;
                }
            }
            MenuAction::ToggleFullscreen => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                    && let Some(window) = &window_state.window
                {
                    window_state.is_fullscreen = !window_state.is_fullscreen;
                    if window_state.is_fullscreen {
                        window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
                    } else {
                        window.set_fullscreen(None);
                    }
                }
            }
            MenuAction::MaximizeVertically => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                    && let Some(window) = &window_state.window
                {
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
                        log::info!(
                            "Window maximized vertically to {} pixels",
                            monitor_size.height
                        );
                    }
                }
            }
            MenuAction::IncreaseFontSize => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.config.font_size = (window_state.config.font_size + 1.0).min(72.0);
                    window_state.pending_font_rebuild = true;
                    if let Some(window) = &window_state.window {
                        window.request_redraw();
                    }
                }
            }
            MenuAction::DecreaseFontSize => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.config.font_size = (window_state.config.font_size - 1.0).max(6.0);
                    window_state.pending_font_rebuild = true;
                    if let Some(window) = &window_state.window {
                        window.request_redraw();
                    }
                }
            }
            MenuAction::ResetFontSize => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.config.font_size = 14.0;
                    window_state.pending_font_rebuild = true;
                    if let Some(window) = &window_state.window {
                        window.request_redraw();
                    }
                }
            }
            MenuAction::ToggleFpsOverlay => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.debug.show_fps_overlay = !window_state.debug.show_fps_overlay;
                    if let Some(window) = &window_state.window {
                        window.request_redraw();
                    }
                }
            }
            MenuAction::OpenSettings => {
                self.open_settings_window(event_loop);
            }
            MenuAction::Minimize => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get(&window_id)
                    && let Some(window) = &window_state.window
                {
                    window.set_minimized(true);
                }
            }
            MenuAction::Zoom => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get(&window_id)
                    && let Some(window) = &window_state.window
                {
                    window.set_maximized(!window.is_maximized());
                }
            }
            MenuAction::ShowHelp => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.overlay_ui.help_ui.toggle();
                    if let Some(window) = &window_state.window {
                        window.request_redraw();
                    }
                }
            }
            MenuAction::About => {
                log::info!("About par-term v{}", env!("CARGO_PKG_VERSION"));
                // Could show an about dialog here
            }
            MenuAction::ToggleBackgroundShader => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.toggle_background_shader();
                }
            }
            MenuAction::ToggleCursorShader => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.toggle_cursor_shader();
                }
            }
            MenuAction::ReloadConfig => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.reload_config();
                }
            }
            MenuAction::ManageProfiles => {
                self.open_settings_window(event_loop);
                if let Some(sw) = &mut self.settings_window {
                    sw.settings_ui
                        .set_selected_tab(crate::settings_ui::sidebar::SettingsTab::Profiles);
                }
            }
            MenuAction::ToggleProfileDrawer => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.toggle_profile_drawer();
                }
            }
            MenuAction::OpenProfile(profile_id) => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state.open_profile(profile_id);
                }
            }
            MenuAction::SaveArrangement => {
                // Open settings window to the Arrangements tab
                self.open_settings_window(event_loop);
                if let Some(sw) = &mut self.settings_window {
                    sw.settings_ui
                        .set_selected_tab(crate::settings_ui::sidebar::SettingsTab::Arrangements);
                }
            }
            MenuAction::InstallShellIntegrationRemote => {
                if let Some(window_id) = focused_window
                    && let Some(window_state) = self.windows.get_mut(&window_id)
                {
                    window_state
                        .overlay_ui
                        .remote_shell_install_ui
                        .show_dialog();
                    window_state.needs_redraw = true;
                }
            }
        }
    }

    /// Process any pending menu events
    pub fn process_menu_events(
        &mut self,
        event_loop: &ActiveEventLoop,
        focused_window: Option<WindowId>,
    ) {
        if let Some(menu) = &self.menu {
            // Collect actions to avoid borrow conflicts
            let actions: Vec<_> = menu.poll_events().collect();
            for action in actions {
                self.handle_menu_action(action, event_loop, focused_window);
            }
        }
    }
}
