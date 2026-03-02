//! Notification and alert handling for the terminal.
//!
//! This module handles:
//! - Desktop notifications (OSC 9/777)
//! - Bell events (audio, visual, desktop)

use super::window_state::WindowState;

impl WindowState {
    /// Check for OSC 9/777 notifications from the terminal.
    pub(crate) fn check_notifications(&mut self) {
        let tab = if let Some(t) = self.tab_manager.active_tab() {
            t
        } else {
            return;
        };

        // try_lock: intentional — OSC notification polling in about_to_wait (sync loop).
        // On miss: notifications are deferred to the next poll frame. Low risk; OSC
        // notifications are informational and a one-frame delay is imperceptible.
        if let Ok(term) = tab.terminal.try_write() {
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

            // try_lock: intentional — bell count polling in about_to_wait (sync event loop).
            // On miss: bell detection is skipped this frame. The bell event will be seen
            // on the next poll. A one-frame delay in bell feedback is imperceptible.
            if let Ok(term) = tab.terminal.try_write() {
                (term.bell_count(), tab.active_bell().last_count)
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
            // Check alert_sounds config first, fall back to legacy bell_sound setting
            if let Some(alert_cfg) = self
                .config
                .alert_sounds
                .get(&crate::config::AlertEvent::Bell)
            {
                if alert_cfg.enabled
                    && alert_cfg.volume > 0
                    && let Some(tab) = self.tab_manager.active_tab()
                    && let Some(ref audio_bell) = tab.active_bell().audio
                {
                    log::info!(
                        "  Playing alert sound for bell at {}% volume",
                        alert_cfg.volume
                    );
                    audio_bell.play_alert(alert_cfg);
                }
            } else if self.config.notification_bell_sound > 0 {
                if let Some(tab) = self.tab_manager.active_tab()
                    && let Some(ref audio_bell) = tab.active_bell().audio
                {
                    log::info!(
                        "  Playing audio bell at {}% volume",
                        self.config.notification_bell_sound
                    );
                    audio_bell.play(self.config.notification_bell_sound);
                } else {
                    log::warn!("  Audio bell requested but not initialized");
                }
            } else {
                log::debug!("  Audio bell disabled (volume=0)");
            }

            // Trigger visual bell flash if enabled
            if self.config.notification_bell_visual {
                log::info!("  Triggering visual bell flash");
                if let Some(tab) = self.tab_manager.active_tab_mut() {
                    tab.active_bell_mut().visual_flash = Some(std::time::Instant::now());
                }
                // Request immediate redraw to show flash
                self.request_redraw();
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
                tab.active_bell_mut().last_count = current_bell_count;
            }
        }
    }

    /// Play an alert sound for the given event, if configured.
    pub(crate) fn play_alert_sound(&self, event: crate::config::AlertEvent) {
        if let Some(alert_cfg) = self.config.alert_sounds.get(&event)
            && alert_cfg.enabled
            && alert_cfg.volume > 0
            && let Some(tab) = self.tab_manager.active_tab()
            && let Some(ref audio_bell) = tab.active_bell().audio
        {
            log::info!(
                "Playing alert sound for {:?} at {}% volume",
                event,
                alert_cfg.volume
            );
            audio_bell.play_alert(alert_cfg);
        }
    }

    /// Check for session exit notifications across all tabs.
    ///
    /// Notifies the user when a shell/process exits, useful for long-running commands
    /// where the user may have switched to other applications.
    pub(crate) fn check_session_exit_notifications(&mut self) {
        if !self.config.notification_session_ended {
            return;
        }

        let mut notifications_to_send: Vec<(String, String)> = Vec::new();

        for tab in self.tab_manager.tabs_mut() {
            // Skip if already notified for this tab
            if tab.activity.exit_notified {
                continue;
            }

            // Check if the terminal has exited
            // try_lock: intentional — exit check in about_to_wait (sync event loop).
            // On miss: this tab's exit is not detected this frame; it will be on the next.
            let has_exited = if let Ok(term) = tab.terminal.try_write() {
                !term.is_running()
            } else {
                continue; // Skip if terminal is locked
            };

            if has_exited {
                tab.activity.exit_notified = true;
                let title = format!("Session Ended: {}", tab.title);
                let message = "The shell process has exited".to_string();
                log::info!("Session exit notification: {} has exited", tab.title);
                notifications_to_send.push((title, message));
            }
        }

        // Send collected notifications (after releasing mutable borrow)
        for (title, message) in notifications_to_send {
            self.deliver_notification(&title, &message);
        }
    }

    /// Check for activity/idle notifications across all tabs.
    ///
    /// This method handles two types of notifications:
    /// - **Activity notification**: Triggered when terminal output resumes after a period of
    ///   inactivity (useful for long-running commands completing).
    /// - **Silence notification**: Triggered when a terminal has been idle for longer than the
    ///   configured threshold (useful for detecting stalled processes).
    pub(crate) fn check_activity_idle_notifications(&mut self) {
        // Skip if both notification types are disabled
        if !self.config.notification_activity_enabled && !self.config.notification_silence_enabled {
            return;
        }

        let now = std::time::Instant::now();
        let activity_threshold =
            std::time::Duration::from_secs(self.config.notification_activity_threshold);
        let silence_threshold =
            std::time::Duration::from_secs(self.config.notification_silence_threshold);

        // Collect notification data for all tabs to avoid borrow conflicts
        let mut notifications_to_send: Vec<(String, String)> = Vec::new();

        for tab in self.tab_manager.tabs_mut() {
            // Get current terminal generation to detect new output
            // try_lock: intentional — activity/generation check in about_to_wait (sync loop).
            // On miss: activity tracking skipped for this tab this frame. Harmless.
            let current_generation = if let Ok(term) = tab.terminal.try_write() {
                term.update_generation()
            } else {
                continue; // Skip if terminal is locked
            };

            let time_since_activity = now.duration_since(tab.activity.last_activity_time);

            // Check if there's new terminal output
            if current_generation > tab.activity.last_seen_generation {
                // New output detected - this is "activity"
                let was_idle = time_since_activity >= activity_threshold;

                // Update tracking state
                tab.activity.last_seen_generation = current_generation;
                tab.activity.last_activity_time = now;
                tab.activity.silence_notified = false; // Reset silence notification flag

                // Activity notification: notify if we were idle long enough
                if self.config.notification_activity_enabled && was_idle {
                    let title = format!("Activity in {}", tab.title);
                    let message = format!(
                        "Terminal output resumed after {} seconds of inactivity",
                        time_since_activity.as_secs()
                    );
                    log::info!(
                        "Activity notification: {} idle for {}s, now active",
                        tab.title,
                        time_since_activity.as_secs()
                    );
                    notifications_to_send.push((title, message));
                }
            } else {
                // No new output - check for silence notification
                if self.config.notification_silence_enabled
                    && !tab.activity.silence_notified
                    && time_since_activity >= silence_threshold
                {
                    // Terminal has been silent for longer than threshold
                    tab.activity.silence_notified = true;
                    let title = format!("Silence in {}", tab.title);
                    let message =
                        format!("No output for {} seconds", time_since_activity.as_secs());
                    log::info!(
                        "Silence notification: {} silent for {}s",
                        tab.title,
                        time_since_activity.as_secs()
                    );
                    notifications_to_send.push((title, message));
                }
            }
        }

        // Send collected notifications (after releasing mutable borrow)
        for (title, message) in notifications_to_send {
            self.deliver_notification(&title, &message);
        }
    }

    /// Deliver a notification unconditionally (bypasses focus suppression).
    ///
    /// Used for trigger-generated notifications which the user explicitly configured,
    /// so they should always be delivered regardless of window focus state.
    pub(crate) fn deliver_notification_force(&self, title: &str, message: &str) {
        self.deliver_notification_inner(title, message, true);
    }

    /// Deliver a notification via desktop notification system and logs.
    ///
    /// If `suppress_notifications_when_focused` is enabled and the window is focused,
    /// only log the notification without sending a desktop notification (since the user
    /// is already looking at the terminal).
    pub(crate) fn deliver_notification(&self, title: &str, message: &str) {
        self.deliver_notification_inner(title, message, false);
    }

    /// Inner notification delivery with force option.
    ///
    /// When `force` is true, bypasses focus suppression (used for trigger notifications).
    fn deliver_notification_inner(&self, title: &str, message: &str, force: bool) {
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

        // Skip desktop notification if window is focused and suppression is enabled
        // (unless force is set, e.g. for trigger-generated notifications)
        if !force && self.config.suppress_notifications_when_focused && self.focus_state.is_focused
        {
            log::debug!(
                "Suppressing desktop notification (window is focused): {}",
                title
            );
            return;
        }

        // Send desktop notification via the platform abstraction layer
        crate::platform::deliver_desktop_notification(title, message, 3000);
    }
}
