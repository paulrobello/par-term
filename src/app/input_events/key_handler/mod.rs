//! Keyboard event handler and key-shortcut sub-handlers.
//!
//! This module handles all keyboard input routing:
//! - `handle_key_event`: main key dispatch entry point (this file)
//! - `scroll`: PageUp/PageDown, Home/End, mark navigation
//! - `config_reload`: F5 config reload + `reload_config`
//! - `clipboard`: clipboard history, paste special, `paste_text`
//! - `command_history`: Cmd/Ctrl+R command history UI
//! - `search`: Cmd/Ctrl+F search UI
//! - `ui_toggles`: AI inspector (Assistant panel) toggle
//! - `utility`: font size, clear scrollback, cursor style
//! - `tabs`: new/close/navigate/move/number-switch tab shortcuts
//! - `profiles`: per-profile hotkeys and shortcut string building

mod clipboard;
mod command_history;
mod config_reload;
mod profiles;
mod scroll;
mod search;
mod tabs;
mod ui_toggles;
mod utility;

use crate::app::window_state::WindowState;
use std::sync::Arc;
use winit::event::ElementState;
use winit::event::KeyEvent;
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, NamedKey};

impl WindowState {
    pub(crate) fn handle_key_event(&mut self, event: KeyEvent, event_loop: &ActiveEventLoop) {
        // Track Alt key press/release for Option key mode detection
        self.input_handler.track_alt_key(&event);

        // Check if any modal UI panel is visible that should block keyboard input
        // Note: Settings are handled by standalone SettingsWindow, not embedded UI
        // Note: Profile drawer does NOT block input - only modal dialogs do

        // When UI panels are visible, block ALL keys from going to terminal
        // except for UI control keys (Escape handled by egui, F1/F2/F3 for toggles)
        if self.any_modal_ui_visible() {
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

        // Copy mode intercepts all keyboard input
        if self.is_copy_mode_active() {
            if event.state == ElementState::Pressed {
                self.handle_copy_mode_key(&event);
            }
            return;
        }

        // Check if active tab's shell has exited
        let is_running = if let Some(tab) = self.tab_manager.active_tab() {
            // try_lock: intentional — handle_key_event runs in the sync event loop.
            // On miss: assume shell is still running (true) to avoid spuriously exiting
            // on a keypress when the lock is briefly held by the PTY reader.
            if let Ok(term) = tab.terminal.try_write() {
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
            self.cursor_anim.last_key_press = Some(std::time::Instant::now());
            // Update shader key press time for visual effects (iTimeKeyPress uniform)
            if let Some(renderer) = &mut self.renderer {
                renderer.update_key_press_time();
            }
        }

        // Check user-defined keybindings first (before hardcoded shortcuts)
        if event.state == ElementState::Pressed
            && let Some(action) = self.keybinding_registry.lookup_with_options(
                &event,
                &self.input_handler.modifiers,
                &self.config.modifier_remapping,
                self.config.use_physical_keys,
            )
        {
            crate::debug_info!(
                "KEYBINDING",
                "Keybinding matched: action={}, key={:?}, modifiers={:?}",
                action,
                event.logical_key,
                self.input_handler.modifiers
            );
            // Clone to avoid borrow conflict
            let action = action.to_string();
            if self.execute_keybinding_action(&action) {
                return; // Key was handled by user-defined keybinding
            }
        } else if event.state == ElementState::Pressed {
            crate::debug_log!(
                "KEYBINDING",
                "No keybinding match for key={:?}, modifiers={:?}",
                event.logical_key,
                self.input_handler.modifiers
            );
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

        // Check if this is a command history key (Ctrl+R / Cmd+R)
        if self.handle_command_history_keys(&event) {
            return; // Key was handled for command history, don't send to terminal
        }

        // Check if paste special UI is handling keys
        if self.handle_paste_special_keys(&event) {
            return; // Key was handled for paste special, don't send to terminal
        }

        // Check for search keys (Cmd/Ctrl+F)
        if self.handle_search_keys(&event) {
            return; // Key was handled for search, don't send to terminal
        }

        // Check for Assistant panel toggle (Cmd+I / Ctrl+Shift+I)
        if self.handle_ai_inspector_toggle(&event) {
            return; // Key was handled for Assistant panel, don't send to terminal
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

        // Check for profile keyboard shortcuts (per-profile hotkeys)
        if self.handle_profile_shortcuts(&event) {
            return; // Key was handled for opening a profile
        }

        // Check for utility shortcuts (clear scrollback, font size, etc.)
        if self.handle_utility_shortcuts(&event, event_loop) {
            return; // Key was handled by utility shortcut
        }

        // Check for tab shortcuts
        if self.handle_tab_shortcuts(&event, event_loop) {
            return; // Key was handled by tab shortcut
        }

        // Handle paste shortcuts with bracketed paste support
        if event.state == ElementState::Pressed {
            // macOS: Cmd+V, NamedKey::Paste
            // Windows/Linux: Ctrl+Shift+V, Shift+Insert, NamedKey::Paste
            // (Ctrl+V is "literal next" in terminals, must not be intercepted)
            #[cfg(not(target_os = "macos"))]
            let is_paste = {
                let ctrl = self.input_handler.modifiers.state().control_key();
                let shift = self.input_handler.modifiers.state().shift_key();
                matches!(event.logical_key, Key::Named(NamedKey::Paste))
                    || (ctrl
                        && shift
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
                if let Some(text) = self.input_handler.paste_from_clipboard() {
                    let text = crate::paste_transform::sanitize_paste_content(&text);
                    log::debug!("Paste: got {} chars of text from clipboard", text.len());
                    if let Some(tab) = self.tab_manager.active_tab() {
                        let terminal_clone = Arc::clone(&tab.terminal);
                        self.runtime.spawn(async move {
                            let term = terminal_clone.write().await;
                            let _ = term.paste(&text);
                        });
                    }
                } else if self.input_handler.clipboard_has_image() {
                    // Clipboard has an image but no text — forward as Ctrl+V (0x16) so
                    // image-aware child processes (e.g., Claude Code) can handle image paste
                    log::debug!(
                        "Paste: clipboard has image but no text, forwarding Ctrl+V to terminal"
                    );
                    if let Some(tab) = self.tab_manager.active_tab() {
                        let terminal_clone = Arc::clone(&tab.terminal);
                        self.runtime.spawn(async move {
                            let term = terminal_clone.write().await;
                            let _ = term.write(b"\x16");
                        });
                    }
                } else {
                    log::debug!("Paste: clipboard has neither text nor image");
                }
                return;
            }

            // macOS: Cmd+C, NamedKey::Copy
            // Windows/Linux: Ctrl+Shift+C, NamedKey::Copy
            // (Ctrl+C is SIGINT in terminals, must not be intercepted)
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
                let shift = self.input_handler.modifiers.state().shift_key();
                matches!(event.logical_key, Key::Named(NamedKey::Copy))
                    || (ctrl
                        && shift
                        && matches!(event.logical_key, Key::Character(ref c) if c.eq_ignore_ascii_case("c")))
            };

            if is_copy {
                // Try prettifier-aware copy first, then fall back to normal selection copy.
                let text = self
                    .get_prettifier_copy_text()
                    .or_else(|| self.get_selected_text_for_copy());
                if let Some(selected_text) = text {
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
            && tab.selection_mouse().selection.is_some()
        {
            tab.selection_mouse_mut().selection = None;
            self.request_redraw();
        }

        // Handle tmux prefix key mode
        if self.handle_tmux_prefix_key(&event) {
            return; // Key was handled by prefix system
        }

        // Get terminal modes (if available)
        // try_lock: intentional — reading terminal mode flags from the sync event loop.
        // On miss: fall back to defaults (0, false). This means a key press encodes with
        // default cursor/modify-other-keys mode for one frame — safe for interactive typing.
        let (modify_other_keys_mode, application_cursor) =
            if let Some(tab) = self.tab_manager.active_tab() {
                if let Ok(term) = tab.terminal.try_write() {
                    (term.modify_other_keys_mode(), term.application_cursor())
                } else {
                    (0, false)
                }
            } else {
                (0, false)
            };

        // Normal key handling - send to terminal (or via tmux if connected)
        if let Some(bytes) = self.input_handler.handle_key_event_with_mode(
            event,
            modify_other_keys_mode,
            application_cursor,
        ) {
            // Try to send via tmux if connected (check before borrowing tab)
            if self.send_input_via_tmux(&bytes) {
                // Still need to reset anti-idle timer
                if let Some(tab) = self.tab_manager.active_tab_mut() {
                    tab.activity.anti_idle_last_activity = std::time::Instant::now();
                }
                return; // Input was routed through tmux
            }

            // Broadcast input to all panes or just the focused pane
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                // Reset anti-idle timer on keyboard input
                tab.activity.anti_idle_last_activity = std::time::Instant::now();

                // Check if focused pane is awaiting restart input (Enter key to restart)
                if let Some(ref mut pane_manager) = tab.pane_manager
                    && let Some(focused_pane) = pane_manager.focused_pane_mut()
                    && matches!(
                        focused_pane.restart_state,
                        Some(crate::pane::RestartState::AwaitingInput)
                    )
                {
                    // Check if this is an Enter key (bytes == "\r" or "\n")
                    if bytes == b"\r" || bytes == b"\n" || bytes == b"\r\n" {
                        log::info!(
                            "Enter pressed, restarting shell in pane {}",
                            focused_pane.id
                        );
                        if let Err(e) = focused_pane.respawn_shell(&self.config) {
                            log::error!(
                                "Failed to respawn shell in pane {}: {}",
                                focused_pane.id,
                                e
                            );
                        }
                        return;
                    }
                    // For any other key, ignore it while awaiting input
                    return;
                }

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
                            let term = terminal.write().await;
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
                    let term = terminal_clone.write().await;
                    let _ = term.write(&bytes);
                });
            }
        }
    }
}
