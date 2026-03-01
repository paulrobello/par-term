//! Shell integration title and badge synchronization for WindowState.
//!
//! Contains:
//! - `update_window_title_with_shell_integration`: syncs window title from shell CWD/exit code
//! - `sync_badge_shell_integration`: syncs badge variables from shell integration data

use crate::app::window_state::WindowState;

impl WindowState {
    /// Update window title with shell integration info (cwd and exit code)
    /// Only updates if not scrolled and not hovering over URL
    pub(crate) fn update_window_title_with_shell_integration(&self) {
        // Get active tab state
        let tab = if let Some(t) = self.tab_manager.active_tab() {
            t
        } else {
            return;
        };

        // Skip if scrolled (scrollback indicator takes priority)
        if tab.active_scroll_state().offset != 0 {
            return;
        }

        // Skip if hovering over URL (URL tooltip takes priority)
        if tab.active_mouse().hovered_url.is_some() {
            return;
        }

        // Skip if window not available
        let window = if let Some(w) = &self.window {
            w
        } else {
            return;
        };

        // Try to get shell integration info
        // try_lock: intentional — called every frame from the render path; blocking would
        // stall rendering. On miss: window title is not updated this frame. No data loss.
        if let Ok(term) = tab.terminal.try_write() {
            let mut title_parts = vec![self.config.window_title.clone()];

            // Add window number if configured
            if self.config.show_window_number {
                title_parts.push(format!("[{}]", self.window_index));
            }

            // Add current working directory if available
            if let Some(cwd) = term.shell_integration_cwd() {
                // Abbreviate home directory to ~
                let abbreviated_cwd = if let Some(home) = dirs::home_dir() {
                    cwd.replace(&home.to_string_lossy().to_string(), "~")
                } else {
                    cwd
                };
                title_parts.push(format!("({})", abbreviated_cwd));
            }

            // Add running command indicator if a command is executing
            if let Some(cmd_name) = term.get_running_command_name() {
                title_parts.push(format!("[{}]", cmd_name));
            }

            // Add exit code indicator if last command failed
            if let Some(exit_code) = term.shell_integration_exit_code()
                && exit_code != 0
            {
                title_parts.push(format!("[Exit: {}]", exit_code));
            }

            // Add recording indicator
            if self.is_recording {
                title_parts.push("[RECORDING]".to_string());
            }

            // Build and set title
            let title = title_parts.join(" ");
            window.set_title(&title);
        }
    }

    /// Sync shell integration data (exit code, command, cwd, hostname, username) to badge variables
    pub(crate) fn sync_badge_shell_integration(&mut self) {
        let tab = if let Some(t) = self.tab_manager.active_tab() {
            t
        } else {
            return;
        };

        // try_lock: intentional — sync_badge_shell_integration is called from the render
        // path. On miss: badge variables are not updated this frame; they will be on the next.
        if let Ok(term) = tab.terminal.try_write() {
            let exit_code = term.shell_integration_exit_code();
            let current_command = term.get_running_command_name();
            let cwd = term.shell_integration_cwd();
            let hostname = term.shell_integration_hostname();
            let username = term.shell_integration_username();

            let mut vars = self.badge_state.variables_mut();
            let mut badge_changed = false;

            if vars.exit_code != exit_code {
                vars.set_exit_code(exit_code);
                badge_changed = true;
            }
            if vars.current_command != current_command {
                vars.set_current_command(current_command);
                badge_changed = true;
            }
            if let Some(cwd) = cwd
                && vars.path != cwd
            {
                vars.set_path(cwd);
                badge_changed = true;
            }
            if let Some(ref host) = hostname
                && vars.hostname != *host
            {
                vars.hostname = host.clone();
                badge_changed = true;
            } else if hostname.is_none() && !vars.hostname.is_empty() {
                // Returned to localhost — keep the initial hostname from new()
            }
            if let Some(ref user) = username
                && vars.username != *user
            {
                vars.username = user.clone();
                badge_changed = true;
            }
            drop(vars);
            if badge_changed {
                self.badge_state.mark_dirty();
            }
        }
    }
}
