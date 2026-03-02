//! CLI timer handling for WindowManager.
//!
//! Handles timing-based CLI options: delayed command sending, timed screenshots,
//! and auto-quit via `--exit-after`. These methods are called once per render tick
//! from the main application event loop.

use std::path::PathBuf;

use super::WindowManager;

impl WindowManager {
    /// Check and handle timing-based CLI options (exit-after, screenshot, command)
    pub fn check_cli_timers(&mut self) {
        let Some(start_time) = self.start_time else {
            return;
        };

        let elapsed = start_time.elapsed().as_secs_f64();

        // Send command after 1 second delay
        if !self.command_sent
            && elapsed >= 1.0
            && let Some(cmd) = self.runtime_options.command_to_send.clone()
        {
            self.send_command_to_shell(&cmd);
            self.command_sent = true;
        }

        // Take screenshot if requested (after exit_after - 1 second, or after 2 seconds if no exit_after)
        if !self.screenshot_taken && self.runtime_options.screenshot.is_some() {
            let screenshot_time = self
                .runtime_options
                .exit_after
                .map(|t| t - 1.0)
                .unwrap_or(2.0);
            if elapsed >= screenshot_time {
                self.take_screenshot();
                self.screenshot_taken = true;
            }
        }

        // Exit after specified time
        if let Some(exit_after) = self.runtime_options.exit_after
            && elapsed >= exit_after
        {
            log::info!("Exit-after timer expired ({:.1}s), exiting", exit_after);
            self.should_exit = true;
        }
    }

    /// Send a command to the shell
    pub(super) fn send_command_to_shell(&mut self, cmd: &str) {
        // Send to the first window's active tab
        if let Some(window_state) = self.windows.values_mut().next()
            && let Some(tab) = window_state.tab_manager.active_tab_mut()
        {
            // Send the command followed by Enter
            let cmd_with_enter = format!("{}\n", cmd);
            if let Ok(term) = tab.terminal.try_write() {
                if let Err(e) = term.write(cmd_with_enter.as_bytes()) {
                    log::error!("Failed to send command to shell: {}", e);
                } else {
                    log::info!("Sent command to shell: {}", cmd);
                }
            }
        }
    }

    /// Take a screenshot
    pub(super) fn take_screenshot(&mut self) {
        log::info!("Taking screenshot...");

        // Determine output path
        let output_path = match &self.runtime_options.screenshot {
            Some(path) if !path.as_os_str().is_empty() => {
                log::info!("Screenshot path specified: {:?}", path);
                path.clone()
            }
            _ => {
                // Generate timestamped filename
                let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
                let path = PathBuf::from(format!("par-term-{}.png", timestamp));
                log::info!("Using auto-generated screenshot path: {:?}", path);
                path
            }
        };

        // Get the first window and take screenshot
        if let Some(window_state) = self.windows.values_mut().next() {
            if let Some(renderer) = &mut window_state.renderer {
                log::info!("Capturing screenshot from renderer...");
                match renderer.take_screenshot() {
                    Ok(image_data) => {
                        log::info!(
                            "Screenshot captured: {}x{} pixels",
                            image_data.width(),
                            image_data.height()
                        );
                        // Save the image
                        if let Err(e) = image_data.save(&output_path) {
                            log::error!("Failed to save screenshot to {:?}: {}", output_path, e);
                        } else {
                            log::info!("Screenshot saved to {:?}", output_path);
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to take screenshot: {}", e);
                    }
                }
            } else {
                log::warn!("No renderer available for screenshot");
            }
        } else {
            log::warn!("No window available for screenshot");
        }
    }
}
