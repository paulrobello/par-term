//! Keybinding action dispatch for WindowState.
//!
//! - `execute_keybinding_action`: dispatches named actions (toggle shaders,
//!   new tab, copy, paste, etc.)
//!
//! Visual notification helpers (`show_toast`, `show_pane_indices`) and shader
//! toggle helpers (`toggle_background_shader`, `toggle_cursor_shader`) live in
//! `keybinding_helpers`.
//!
//! Display/navigation actions (font size, cursor style, tab index switching,
//! throughput mode, etc.) live in `keybinding_display_actions`.
//!
//! Snippet and custom action execution live in `snippet_actions`.

use crate::app::window_state::WindowState;

impl WindowState {
    /// Execute a keybinding action by name.
    ///
    /// Returns true if the action was handled, false if unknown.
    pub(crate) fn execute_keybinding_action(&mut self, action: &str) -> bool {
        match action {
            "toggle_background_shader" => {
                self.toggle_background_shader();
                true
            }
            "toggle_cursor_shader" => {
                self.toggle_cursor_shader();
                true
            }
            "toggle_prettifier" => {
                if let Some(tab) = self.tab_manager.active_tab_mut()
                    && let Some(ref mut pipeline) = tab.prettifier
                {
                    pipeline.toggle_global();
                    log::info!(
                        "Prettifier toggled: {}",
                        if pipeline.is_enabled() {
                            "enabled"
                        } else {
                            "disabled"
                        }
                    );
                }
                self.focus_state.needs_redraw = true;
                true
            }
            "reload_config" => {
                self.reload_config();
                true
            }
            "open_settings" => {
                self.overlay_state.open_settings_window_requested = true;
                self.request_redraw();
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
                self.overlay_ui.help_ui.toggle();
                self.request_redraw();
                log::info!(
                    "Help UI toggled via keybinding: {}",
                    if self.overlay_ui.help_ui.visible {
                        "visible"
                    } else {
                        "hidden"
                    }
                );
                true
            }
            "toggle_fps_overlay" => {
                self.debug.show_fps_overlay = !self.debug.show_fps_overlay;
                self.request_redraw();
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
                self.overlay_ui.search_ui.toggle();
                if self.overlay_ui.search_ui.visible {
                    self.overlay_ui.search_ui.init_from_config(
                        self.config.search.search_case_sensitive,
                        self.config.search.search_regex,
                    );
                }
                self.focus_state.needs_redraw = true;
                self.request_redraw();
                log::info!(
                    "Search UI toggled via keybinding: {}",
                    if self.overlay_ui.search_ui.visible {
                        "visible"
                    } else {
                        "hidden"
                    }
                );
                true
            }
            "toggle_ai_inspector" => {
                if self.config.ai_inspector.ai_inspector_enabled {
                    let just_opened = self.overlay_ui.ai_inspector.toggle();
                    self.sync_ai_inspector_width();
                    if just_opened {
                        self.try_auto_connect_agent();
                    }
                    self.request_redraw();
                }
                true
            }
            "new_tab" => {
                self.new_tab_or_show_profiles();
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
                    self.overlay_ui.paste_special_ui.open(text);
                    self.focus_state.needs_redraw = true;
                    self.request_redraw();
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
                self.overlay_ui.tmux_session_picker_ui.toggle();
                self.request_redraw();
                log::info!(
                    "tmux session picker toggled via keybinding: {}",
                    if self.overlay_ui.tmux_session_picker_ui.visible {
                        "visible"
                    } else {
                        "hidden"
                    }
                );
                true
            }
            "toggle_copy_mode" | "enter_copy_mode" => {
                if self.is_copy_mode_active() {
                    self.exit_copy_mode();
                } else {
                    self.enter_copy_mode();
                }
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
                    if self.overlay_ui.profile_drawer_ui.expanded {
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
                    if self.overlay_ui.clipboard_history_ui.visible {
                        "visible"
                    } else {
                        "hidden"
                    }
                );
                true
            }
            "toggle_command_history" => {
                self.toggle_command_history();
                log::info!(
                    "Command history toggled via keybinding: {}",
                    if self.overlay_ui.command_history_ui.visible {
                        "visible"
                    } else {
                        "hidden"
                    }
                );
                true
            }
            "clear_scrollback" => {
                let cleared = if let Some(tab) = self.tab_manager.active_tab_mut() {
                    // try_lock: intentional — keybinding action in sync event loop.
                    // On miss: scrollback not cleared this invocation. User can retry.
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
                    log::info!("Cleared scrollback buffer via keybinding");
                }
                true
            }
            _ => {
                // Delegate display/navigation actions to the companion handler
                if let Some(result) = self.execute_display_keybinding_action(action) {
                    return result;
                }
                // Check for snippet or action keybindings
                if let Some(snippet_id) = action.strip_prefix("snippet:") {
                    self.execute_snippet(snippet_id)
                } else if let Some(action_id) = action.strip_prefix("action:") {
                    self.execute_custom_action(action_id)
                } else if let Some(arrangement_name) = action.strip_prefix("restore_arrangement:") {
                    // Restore arrangement by name - handled by WindowManager
                    self.overlay_state.pending_arrangement_restore =
                        Some(arrangement_name.to_string());
                    self.request_redraw();
                    log::info!(
                        "Arrangement restore requested via keybinding: {}",
                        arrangement_name
                    );
                    true
                } else {
                    log::warn!("Unknown keybinding action: {}", action);
                    false
                }
            }
        }
    }
}
