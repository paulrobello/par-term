//! Config reload key handling (F5) and the `reload_config` implementation.

use crate::app::window_state::WindowState;
use crate::config::Config;
use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{Key, NamedKey};

impl WindowState {
    pub(crate) fn handle_config_reload(&mut self, event: &KeyEvent) -> bool {
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

                // Update modifier remapping and physical keys preference
                self.config.modifier_remapping = new_config.modifier_remapping;
                self.config.use_physical_keys = new_config.use_physical_keys;

                // Update auto_copy_selection
                self.config.auto_copy_selection = new_config.auto_copy_selection;

                // Update middle_click_paste
                self.config.middle_click_paste = new_config.middle_click_paste;

                // Update paste_delay_ms
                self.config.paste_delay_ms = new_config.paste_delay_ms;

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
                        // try_lock: intentional — config reload (F5) runs in sync event loop.
                        // On miss: the tab's theme is not updated immediately. It will be
                        // applied on the next config reload or theme change event.
                        if let Ok(mut term) = tab.terminal.try_write() {
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
}
