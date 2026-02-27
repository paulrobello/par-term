//! Per-frame update logic for WindowState (`about_to_wait`).
//!
//! Contains:
//! - `about_to_wait`: per-frame polling for notifications, tmux, config reload,
//!   cursor blink, smooth scrolling, power saving, flicker reduction, throughput mode,
//!   resize/toast overlay timers, shader animation, file transfers, anti-idle keep-alive.

use crate::app::window_state::WindowState;
use winit::event_loop::{ActiveEventLoop, ControlFlow};

impl WindowState {
    /// Process per-window updates in about_to_wait
    pub(crate) fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Skip all processing if shutting down
        if self.is_shutting_down {
            return;
        }

        // Emit a periodic telemetry summary when try_lock() failures have occurred.
        // The call is cheap (two atomic loads) when no new failures happened.
        crate::debug::maybe_log_try_lock_telemetry();

        // Check for and deliver notifications (OSC 9/777)
        self.check_notifications();

        // Check for file transfer events (downloads, uploads, progress)
        self.check_file_transfers();

        // Check for bell events and play audio/visual feedback
        self.check_bell();

        // Check for trigger action results and dispatch them
        self.check_trigger_actions();

        // Check for activity/idle notifications
        self.check_activity_idle_notifications();

        // Check for session exit notifications
        self.check_session_exit_notifications();

        // Check for shader hot reload events
        if self.check_shader_reload() {
            log::debug!("Shader hot reload triggered redraw");
        }

        // Check for config file changes (e.g., from ACP agent)
        self.check_config_reload();

        // Check for MCP server config updates (.config-update.json)
        self.check_config_update_file();

        // Check for MCP screenshot requests (.screenshot-request.json)
        self.check_screenshot_request_file();

        // Check for tmux control mode notifications
        if self.check_tmux_notifications() {
            self.needs_redraw = true;
        }

        // Update window title with shell integration info (CWD, exit code)
        self.update_window_title_with_shell_integration();

        // Sync shell integration data to badge variables
        self.sync_badge_shell_integration();

        // Check for automatic profile switching based on hostname detection (OSC 7)
        if self.check_auto_profile_switch() {
            self.needs_redraw = true;
        }

        // --- POWER SAVING & SMART REDRAW LOGIC ---
        // We use ControlFlow::WaitUntil to sleep until the next expected event.
        // This drastically reduces CPU/GPU usage compared to continuous polling (ControlFlow::Poll).
        // The loop calculates the earliest time any component needs to update.

        let now = std::time::Instant::now();
        let mut next_wake = now + std::time::Duration::from_secs(1); // Default sleep for 1s of inactivity

        // Calculate frame interval based on focus state for power saving
        // When pause_refresh_on_blur is enabled and window is unfocused, use slower refresh rate
        let frame_interval_ms = if self.config.pause_refresh_on_blur && !self.is_focused {
            // Use unfocused FPS (e.g., 10 FPS = 100ms interval)
            1000 / self.config.unfocused_fps.max(1)
        } else {
            // Use normal animation rate based on max_fps
            1000 / self.config.max_fps.max(1)
        };
        let frame_interval = std::time::Duration::from_millis(frame_interval_ms as u64);

        // Check if enough time has passed since last render for FPS throttling
        let time_since_last_render = self
            .last_render_time
            .map(|t| now.duration_since(t))
            .unwrap_or(frame_interval); // If no last render, allow immediate render
        let can_render = time_since_last_render >= frame_interval;

        // --- FLICKER REDUCTION LOGIC ---
        // When reduce_flicker is enabled and cursor is hidden, delay rendering
        // to batch updates and reduce visual flicker during bulk terminal operations.
        let should_delay_for_flicker = if self.config.reduce_flicker {
            let cursor_hidden = if let Some(tab) = self.tab_manager.active_tab() {
                // try_lock: intentional — flicker check runs in about_to_wait (sync event loop).
                // On miss: assume cursor is visible (false) so rendering is not delayed.
                // Slightly conservative but never causes stale frames.
                if let Ok(term) = tab.terminal.try_lock() {
                    !term.is_cursor_visible() && !self.config.lock_cursor_visibility
                } else {
                    false
                }
            } else {
                false
            };

            if cursor_hidden {
                // Track when cursor was first hidden
                if self.cursor_hidden_since.is_none() {
                    self.cursor_hidden_since = Some(now);
                }

                // Check bypass conditions
                let delay_expired = self
                    .cursor_hidden_since
                    .map(|t| {
                        now.duration_since(t)
                            >= std::time::Duration::from_millis(
                                self.config.reduce_flicker_delay_ms as u64,
                            )
                    })
                    .unwrap_or(false);

                // Bypass for UI interactions (modals + resize overlay)
                let any_ui_visible = self.any_modal_ui_visible() || self.resize_overlay_visible;

                // Delay unless bypass conditions met
                !delay_expired && !any_ui_visible
            } else {
                // Cursor visible - clear tracking and allow render
                if self.cursor_hidden_since.is_some() {
                    self.cursor_hidden_since = None;
                    self.flicker_pending_render = false;
                    self.needs_redraw = true; // Render accumulated updates
                }
                false
            }
        } else {
            false
        };

        // Schedule wake at delay expiry if delaying
        if should_delay_for_flicker {
            self.flicker_pending_render = true;
            if let Some(hidden_since) = self.cursor_hidden_since {
                let delay =
                    std::time::Duration::from_millis(self.config.reduce_flicker_delay_ms as u64);
                let render_time = hidden_since + delay;
                if render_time < next_wake {
                    next_wake = render_time;
                }
            }
        } else if self.flicker_pending_render {
            // Delay ended - trigger accumulated render
            self.flicker_pending_render = false;
            if can_render {
                self.needs_redraw = true;
            }
        }

        // --- THROUGHPUT MODE LOGIC ---
        // When maximize_throughput is enabled, always batch renders regardless of cursor state.
        // Uses a longer interval than flicker reduction for better throughput during bulk output.
        let should_delay_for_throughput = if self.config.maximize_throughput {
            // Initialize batch start time if not set
            if self.throughput_batch_start.is_none() {
                self.throughput_batch_start = Some(now);
            }

            let interval =
                std::time::Duration::from_millis(self.config.throughput_render_interval_ms as u64);
            let batch_start = self
                .throughput_batch_start
                .expect("throughput_batch_start is Some: set to Some on the line above when None");

            // Check if interval has elapsed
            if now.duration_since(batch_start) >= interval {
                self.throughput_batch_start = Some(now); // Reset for next batch
                false // Allow render
            } else {
                true // Delay render
            }
        } else {
            // Clear tracking when disabled
            if self.throughput_batch_start.is_some() {
                self.throughput_batch_start = None;
            }
            false
        };

        // Schedule wake for throughput mode
        if should_delay_for_throughput && let Some(batch_start) = self.throughput_batch_start {
            let interval =
                std::time::Duration::from_millis(self.config.throughput_render_interval_ms as u64);
            let render_time = batch_start + interval;
            if render_time < next_wake {
                next_wake = render_time;
            }
        }

        // Combine delays: throughput mode OR flicker delay
        let should_delay_render = should_delay_for_throughput || should_delay_for_flicker;

        // 1. Cursor Blinking
        // Wake up exactly when the cursor needs to toggle visibility or fade.
        // Skip cursor blinking when unfocused with pause_refresh_on_blur to save power.
        if self.config.cursor_blink && (self.is_focused || !self.config.pause_refresh_on_blur) {
            if self.cursor_anim.cursor_blink_timer.is_none() {
                let blink_interval =
                    std::time::Duration::from_millis(self.config.cursor_blink_interval);
                self.cursor_anim.cursor_blink_timer = Some(now + blink_interval);
            }

            if let Some(next_blink) = self.cursor_anim.cursor_blink_timer {
                if now >= next_blink {
                    // Time to toggle: trigger redraw (if throttle allows) and schedule next phase
                    if can_render {
                        self.needs_redraw = true;
                    }
                    let blink_interval =
                        std::time::Duration::from_millis(self.config.cursor_blink_interval);
                    self.cursor_anim.cursor_blink_timer = Some(now + blink_interval);
                } else if next_blink < next_wake {
                    // Schedule wake-up for the next toggle
                    next_wake = next_blink;
                }
            }
        }

        // 2. Smooth Scrolling & Animations
        // If a scroll interpolation or terminal animation is active, use calculated frame interval.
        if let Some(tab) = self.tab_manager.active_tab() {
            if tab.scroll_state.animation_start.is_some() {
                if can_render {
                    self.needs_redraw = true;
                }
                let next_frame = now + frame_interval;
                if next_frame < next_wake {
                    next_wake = next_frame;
                }
            }

            // 3. Visual Bell Feedback
            // Maintain frame rate during the visual flash fade-out.
            if tab.bell.visual_flash.is_some() {
                if can_render {
                    self.needs_redraw = true;
                }
                let next_frame = now + frame_interval;
                if next_frame < next_wake {
                    next_wake = next_frame;
                }
            }

            // 4. Interactive UI Elements
            // Ensure high responsiveness during mouse dragging (text selection or scrollbar).
            // Always allow these for responsiveness, even if throttled.
            if (tab.mouse.is_selecting
                || tab.mouse.selection.is_some()
                || tab.scroll_state.dragging)
                && tab.mouse.button_pressed
            {
                self.needs_redraw = true;
            }
        }

        // 5. Resize Overlay
        // Check if the resize overlay should be hidden (timer expired).
        if self.resize_overlay_visible
            && let Some(hide_time) = self.resize_overlay_hide_time
        {
            if now >= hide_time {
                // Hide the overlay
                self.resize_overlay_visible = false;
                self.resize_overlay_hide_time = None;
                self.needs_redraw = true;
            } else {
                // Overlay still visible - request redraw and schedule wake
                if can_render {
                    self.needs_redraw = true;
                }
                if hide_time < next_wake {
                    next_wake = hide_time;
                }
            }
        }

        // 5b. Toast Notification
        // Check if the toast notification should be hidden (timer expired).
        if self.toast_message.is_some()
            && let Some(hide_time) = self.toast_hide_time
        {
            if now >= hide_time {
                // Hide the toast
                self.toast_message = None;
                self.toast_hide_time = None;
                self.needs_redraw = true;
            } else {
                // Toast still visible - request redraw and schedule wake
                if can_render {
                    self.needs_redraw = true;
                }
                if hide_time < next_wake {
                    next_wake = hide_time;
                }
            }
        }

        // 5c. Pane Identification Overlay
        // Check if the pane index overlay should be hidden (timer expired).
        if let Some(hide_time) = self.pane_identify_hide_time {
            if now >= hide_time {
                self.pane_identify_hide_time = None;
                self.needs_redraw = true;
            } else {
                if can_render {
                    self.needs_redraw = true;
                }
                if hide_time < next_wake {
                    next_wake = hide_time;
                }
            }
        }

        // 5b. Session undo expiry: prune closed tab metadata that has timed out
        if !self.closed_tabs.is_empty() && self.config.session_undo_timeout_secs > 0 {
            let timeout =
                std::time::Duration::from_secs(self.config.session_undo_timeout_secs as u64);
            self.closed_tabs
                .retain(|info| now.duration_since(info.closed_at) < timeout);
        }

        // 6. Custom Background Shaders
        // If a custom shader is animated, render at the calculated frame interval.
        // When unfocused with pause_refresh_on_blur, this uses the slower unfocused_fps rate.
        if let Some(renderer) = &self.renderer
            && renderer.needs_continuous_render()
        {
            if can_render {
                self.needs_redraw = true;
            }
            // Schedule next frame at the appropriate interval
            let next_frame = self
                .last_render_time
                .map(|t| t + frame_interval)
                .unwrap_or(now);
            // Ensure we don't schedule in the past
            let next_frame = if next_frame <= now {
                now + frame_interval
            } else {
                next_frame
            };
            if next_frame < next_wake {
                next_wake = next_frame;
            }
        }

        // 7. Shader Install Dialog
        // Force continuous redraws when shader install dialog is visible (for spinner animation)
        // and when installation is in progress (to check for completion)
        if self.overlay_ui.shader_install_ui.visible {
            self.needs_redraw = true;
            // Schedule frequent redraws for smooth spinner animation
            let next_frame = now + std::time::Duration::from_millis(16); // ~60fps
            if next_frame < next_wake {
                next_wake = next_frame;
            }
        }

        // 8. File Transfer Progress
        // Ensure rendering during active file transfers so the progress overlay
        // updates. Uses 1-second interval since progress doesn't need smooth animation.
        // Bypasses render delays (flicker/throughput) for responsive UI feedback.
        let has_active_file_transfers = !self.file_transfer_state.active_uploads.is_empty()
            || !self.file_transfer_state.recent_transfers.is_empty();
        if has_active_file_transfers {
            self.needs_redraw = true;
            // Schedule 1 FPS rendering for progress bar updates
            let next_frame = now + std::time::Duration::from_secs(1);
            if next_frame < next_wake {
                next_wake = next_frame;
            }
        }

        // 9. Anti-idle Keep-alive
        // Periodically send keep-alive codes to prevent SSH/connection timeouts.
        if let Some(next_anti_idle) = self.handle_anti_idle(now)
            && next_anti_idle < next_wake
        {
            next_wake = next_anti_idle;
        }

        // --- TRIGGER REDRAW ---
        // Request a redraw if any of the logic above determined an update is due.
        // Respect combined delay (throughput mode OR flicker reduction),
        // but bypass delays for active file transfers that need UI feedback.
        let mut redraw_requested = false;
        if self.needs_redraw
            && (!should_delay_render || has_active_file_transfers)
            && let Some(window) = &self.window
        {
            window.request_redraw();
            self.needs_redraw = false;
            redraw_requested = true;
        }

        // Set the calculated sleep interval.
        // Use Poll mode during active file transfers — WaitUntil prevents
        // RedrawRequested events from being delivered on macOS when PTY data
        // events keep the event loop busy.
        if has_active_file_transfers {
            event_loop.set_control_flow(ControlFlow::Poll);
        } else {
            // On macOS, ControlFlow::WaitUntil doesn't always prevent the event loop
            // from spinning (CVDisplayLink and NSRunLoop interactions). Add an explicit
            // sleep when no render is needed to guarantee low CPU usage when idle.
            //
            // Important: keep this independent from max_fps. Using frame interval here
            // causes idle focused windows to wake at render cadence (e.g., 60Hz), which
            // burns CPU even when nothing is changing.
            if !self.needs_redraw && !redraw_requested {
                const FOCUSED_IDLE_SPIN_SLEEP_MS: u64 = 50;
                const UNFOCUSED_IDLE_SPIN_SLEEP_MS: u64 = 100;
                let max_idle_spin_sleep = if self.is_focused {
                    std::time::Duration::from_millis(FOCUSED_IDLE_SPIN_SLEEP_MS)
                } else {
                    std::time::Duration::from_millis(UNFOCUSED_IDLE_SPIN_SLEEP_MS)
                };
                let sleep_until = next_wake.min(now + max_idle_spin_sleep);
                let sleep_dur = sleep_until.saturating_duration_since(now);
                if sleep_dur > std::time::Duration::from_millis(1) {
                    std::thread::sleep(sleep_dur);
                }
            }
            event_loop.set_control_flow(ControlFlow::WaitUntil(next_wake));
        }
    }
}
