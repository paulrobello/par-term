//! Notification and alert handling for the terminal.
//!
//! This module handles:
//! - Desktop notifications (OSC 9/777)
//! - Bell events (audio, visual, desktop)
//! - Screenshot capture with notifications

use std::sync::Arc;

use super::window_state::WindowState;

impl WindowState {
    /// Check for OSC 9/777 notifications from the terminal.
    pub(crate) fn check_notifications(&mut self) {
        let tab = if let Some(t) = self.tab_manager.active_tab() {
            t
        } else {
            return;
        };

        if let Ok(term) = tab.terminal.try_lock() {
            // Check for OSC 9/777 notifications
            if term.has_notifications() {
                let notifications = term.take_notifications();
                for notif in notifications {
                    self.deliver_notification(&notif.title, &notif.message);
                }
            }
        }
    }

    /// Check for bell events and trigger appropriate feedback.
    pub(crate) fn check_bell(&mut self) {
        // Skip if all bell notifications are disabled
        if self.config.notification_bell_sound == 0
            && !self.config.notification_bell_visual
            && !self.config.notification_bell_desktop
        {
            return;
        }

        // Get current bell count from active tab's terminal
        let (current_bell_count, last_count) = {
            let tab = if let Some(t) = self.tab_manager.active_tab() {
                t
            } else {
                return;
            };

            if let Ok(term) = tab.terminal.try_lock() {
                (term.bell_count(), tab.bell.last_count)
            } else {
                return;
            }
        };

        if current_bell_count > last_count {
            // Bell event(s) occurred
            let bell_events = current_bell_count - last_count;
            log::info!("Bell event detected ({} bell(s))", bell_events);
            log::info!(
                "  Config: sound={}, visual={}, desktop={}",
                self.config.notification_bell_sound,
                self.config.notification_bell_visual,
                self.config.notification_bell_desktop
            );

            // Play audio bell if enabled (volume > 0)
            if self.config.notification_bell_sound > 0 {
                if let Some(tab) = self.tab_manager.active_tab() {
                    if let Some(ref audio_bell) = tab.bell.audio {
                        log::info!(
                            "  Playing audio bell at {}% volume",
                            self.config.notification_bell_sound
                        );
                        audio_bell.play(self.config.notification_bell_sound);
                    } else {
                        log::warn!("  Audio bell requested but not initialized");
                    }
                }
            } else {
                log::debug!("  Audio bell disabled (volume=0)");
            }

            // Trigger visual bell flash if enabled
            if self.config.notification_bell_visual {
                log::info!("  Triggering visual bell flash");
                if let Some(tab) = self.tab_manager.active_tab_mut() {
                    tab.bell.visual_flash = Some(std::time::Instant::now());
                }
                // Request immediate redraw to show flash
                if let Some(window) = &self.window {
                    window.request_redraw();
                }
            } else {
                log::debug!("  Visual bell disabled");
            }

            // Send desktop notification if enabled
            if self.config.notification_bell_desktop {
                log::info!("  Sending desktop notification");
                let message = if bell_events == 1 {
                    "Terminal bell".to_string()
                } else {
                    format!("Terminal bell ({} events)", bell_events)
                };
                self.deliver_notification("Terminal", &message);
            } else {
                log::debug!("  Desktop notification disabled");
            }

            // Update last count
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                tab.bell.last_count = current_bell_count;
            }
        }
    }

    /// Take a screenshot of the terminal and save to file.
    #[allow(dead_code)]
    pub(crate) fn take_screenshot(&self) {
        log::info!("Taking screenshot...");

        let terminal = if let Some(tab) = self.tab_manager.active_tab() {
            Arc::clone(&tab.terminal)
        } else {
            log::warn!("No terminal available for screenshot");
            self.deliver_notification("Screenshot Error", "No terminal available");
            return;
        };

        // Generate timestamp-based filename
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let format = &self.config.screenshot_format;
        let filename = format!("par-term_screenshot_{}.{}", timestamp, format);

        // Create screenshots directory in user's home dir
        if let Some(home_dir) = dirs::home_dir() {
            let screenshot_dir = home_dir.join("par-term-screenshots");
            if !screenshot_dir.exists()
                && let Err(e) = std::fs::create_dir_all(&screenshot_dir)
            {
                log::error!("Failed to create screenshot directory: {}", e);
                self.deliver_notification(
                    "Screenshot Error",
                    &format!("Failed to create directory: {}", e),
                );
                return;
            }

            let path = screenshot_dir.join(&filename);
            let path_str = path.to_string_lossy().to_string();

            // Take screenshot (include scrollback for better context)
            let terminal_clone = terminal;
            let format_clone = format.clone();

            // Use async to avoid blocking the UI
            let result = std::thread::spawn(move || {
                if let Ok(term) = terminal_clone.try_lock() {
                    // Include 0 scrollback lines (just visible content)
                    term.screenshot_to_file(&path, &format_clone, 0)
                } else {
                    Err(anyhow::anyhow!("Failed to lock terminal"))
                }
            })
            .join();

            match result {
                Ok(Ok(())) => {
                    log::info!("Screenshot saved to: {}", path_str);
                    self.deliver_notification(
                        "Screenshot Saved",
                        &format!("Saved to: {}", path_str),
                    );
                }
                Ok(Err(e)) => {
                    log::error!("Failed to save screenshot: {}", e);
                    self.deliver_notification(
                        "Screenshot Error",
                        &format!("Failed to save: {}", e),
                    );
                }
                Err(e) => {
                    log::error!("Screenshot thread panicked: {:?}", e);
                    self.deliver_notification("Screenshot Error", "Screenshot thread failed");
                }
            }
        } else {
            log::error!("Failed to get home directory");
            self.deliver_notification("Screenshot Error", "Failed to get home directory");
        }
    }

    /// Toggle recording (placeholder - not yet implemented in core library).
    #[allow(dead_code)]
    pub(crate) fn toggle_recording(&mut self) {
        self.deliver_notification(
            "Recording Not Available",
            "Recording APIs are not yet implemented in the core library",
        );
    }

    /// Deliver a notification via desktop notification system and logs.
    pub(crate) fn deliver_notification(&self, title: &str, message: &str) {
        // Always log notifications
        if !title.is_empty() {
            log::info!("=== Notification: {} ===", title);
            log::info!("{}", message);
            log::info!("===========================");
        } else {
            log::info!("=== Notification ===");
            log::info!("{}", message);
            log::info!("===================");
        }

        // Send desktop notification if enabled
        #[cfg(not(target_os = "macos"))]
        {
            use notify_rust::Notification;
            let notification_title = if !title.is_empty() {
                title
            } else {
                "Terminal Notification"
            };

            if let Err(e) = Notification::new()
                .summary(notification_title)
                .body(message)
                .timeout(notify_rust::Timeout::Milliseconds(3000))
                .show()
            {
                log::warn!("Failed to send desktop notification: {}", e);
            }
        }

        #[cfg(target_os = "macos")]
        {
            // macOS notifications via osascript
            let notification_title = if !title.is_empty() {
                title
            } else {
                "Terminal Notification"
            };

            // Escape quotes in title and message for AppleScript
            let escaped_title = notification_title.replace('"', "\\\"");
            let escaped_message = message.replace('"', "\\\"");

            // Use osascript to display notification
            let script = format!(
                r#"display notification "{}" with title "{}""#,
                escaped_message, escaped_title
            );

            if let Err(e) = std::process::Command::new("osascript")
                .arg("-e")
                .arg(&script)
                .output()
            {
                log::warn!("Failed to send macOS desktop notification: {}", e);
            }
        }
    }
}
