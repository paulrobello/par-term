//! Helper and utility methods for `WindowState`, plus the `Drop` implementation.
//!
//! Covers:
//! - DRY rendering helpers (`invalidate_tab_cache`, `request_redraw`, `clear_and_invalidate`)
//! - Window access helpers (`with_window`, AUD-033)
//! - Active-tab access helpers (`with_active_tab`, `with_active_tab_mut`, AUD-030)
//! - Debounced config save (`save_config_debounced`, `process_pending_config_save`)
//! - Anti-idle keep-alive logic
//! - egui pointer / keyboard query helpers
//! - Modal-visibility query helpers
//! - Scrollbar visibility logic
//! - Shutdown sequence (`perform_shutdown`)
//! - `Drop` implementation (fast-path window teardown)

use super::{ConfigSaveState, WindowState};
use crate::app::anti_idle::should_send_keep_alive;
use crate::tab::Tab;
use anyhow::Result;
use std::sync::Arc;

impl WindowState {
    // ========================================================================
    // DRY Helper Methods
    // ========================================================================

    /// Invalidate the active tab's cell cache, forcing regeneration on next render
    #[inline]
    pub(crate) fn invalidate_tab_cache(&mut self) {
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.active_cache_mut().cells = None;
        }
    }

    /// Request window redraw if window exists.
    ///
    /// Prefer this over the inline `if let Some(window) = &self.window { window.request_redraw() }`
    /// pattern (AUD-032).
    #[inline]
    pub(crate) fn request_redraw(&self) {
        if let Some(window) = &self.window {
            crate::debug_trace!("REDRAW", "request_redraw called");
            window.request_redraw();
        } else {
            crate::debug_trace!("REDRAW", "request_redraw called but no window");
        }
    }

    /// Run a closure with the winit `Window`, returning `None` when the window is absent.
    ///
    /// Use this instead of the inline `if let Some(window) = &self.window { ... }` pattern
    /// for one-shot operations on the window (cursor changes, title updates, etc.) (AUD-033).
    ///
    /// # Example
    /// ```ignore
    /// self.with_window(|w| w.set_cursor(cursor));
    /// ```
    #[inline]
    pub(crate) fn with_window<R>(&self, f: impl FnOnce(&winit::window::Window) -> R) -> Option<R> {
        self.window.as_deref().map(f)
    }

    /// Run a closure with the active tab (immutable), returning `None` when no tab is active.
    ///
    /// Use this instead of the inline `if let Some(tab) = self.tab_manager.active_tab() { ... }`
    /// pattern (AUD-030).
    #[inline]
    pub(crate) fn with_active_tab<R>(&self, f: impl FnOnce(&Tab) -> R) -> Option<R> {
        self.tab_manager.active_tab().map(f)
    }

    /// Run a closure with the active tab (mutable), returning `None` when no tab is active.
    ///
    /// Use this instead of the inline `if let Some(tab) = self.tab_manager.active_tab_mut() { ... }`
    /// pattern (AUD-030).
    #[inline]
    pub(crate) fn with_active_tab_mut<R>(&mut self, f: impl FnOnce(&mut Tab) -> R) -> Option<R> {
        self.tab_manager.active_tab_mut().map(f)
    }

    /// Clear renderer cells and invalidate cache (used when switching tabs)
    pub(crate) fn clear_and_invalidate(&mut self) {
        if let Some(renderer) = &mut self.renderer {
            renderer.clear_all_cells();
        }
        self.invalidate_tab_cache();
        self.focus_state.needs_redraw = true;
        self.request_redraw();
    }

    // ========================================================================
    // Debounced Config Save
    // ========================================================================

    /// Save config with debouncing to prevent rapid concurrent writes.
    ///
    /// Multiple code paths may request config saves in quick succession (e.g.,
    /// user changes a setting, an agent modifies config, update checker records
    /// timestamp). This method batches those saves together.
    ///
    /// - If called within DEBOUNCE_INTERVAL of last save, marks a pending save
    ///   and returns immediately (no error).
    /// - If a save is already pending, just updates the pending flag (idempotent).
    ///
    /// Callers should invoke `process_pending_config_save()` periodically (e.g.,
    /// once per frame) to flush any deferred saves.
    pub(crate) fn save_config_debounced(&mut self) -> Result<()> {
        let now = std::time::Instant::now();
        let debounce_interval =
            std::time::Duration::from_millis(ConfigSaveState::DEBOUNCE_INTERVAL_MS);

        // Check if we're within the debounce window
        if let Some(last_save) = self.config_save_state.last_save
            && now.duration_since(last_save) < debounce_interval
        {
            // Defer this save - mark as pending
            self.config_save_state.pending_save = true;
            log::debug!(
                "Config save debounced (within {}ms window)",
                ConfigSaveState::DEBOUNCE_INTERVAL_MS
            );
            return Ok(());
        }

        // Perform the actual save
        self.config.save()?;
        self.config_save_state.last_save = Some(now);
        self.config_save_state.pending_save = false;
        log::debug!("Config saved immediately");
        Ok(())
    }

    /// Process any pending config save that was deferred by debouncing.
    ///
    /// Should be called once per frame (e.g., in the render loop) to ensure
    /// deferred saves are eventually flushed.
    ///
    /// Returns `true` if a save was performed, `false` if nothing was pending.
    pub(crate) fn process_pending_config_save(&mut self) -> bool {
        if !self.config_save_state.pending_save {
            return false;
        }

        let now = std::time::Instant::now();
        let debounce_interval =
            std::time::Duration::from_millis(ConfigSaveState::DEBOUNCE_INTERVAL_MS);

        // Check if enough time has passed since last save
        if let Some(last_save) = self.config_save_state.last_save
            && now.duration_since(last_save) < debounce_interval
        {
            // Still within debounce window, wait longer
            return false;
        }

        // Perform the pending save
        if let Err(e) = self.config.save() {
            log::error!("Failed to save pending config: {}", e);
        } else {
            log::debug!("Pending config save flushed");
        }

        self.config_save_state.last_save = Some(now);
        self.config_save_state.pending_save = false;
        true
    }

    // ========================================================================
    // Anti-idle
    // ========================================================================

    /// Check anti-idle timers and send keep-alive codes when due.
    ///
    /// Returns the next Instant when anti-idle should run, or None if disabled.
    pub(crate) fn handle_anti_idle(
        &mut self,
        now: std::time::Instant,
    ) -> Option<std::time::Instant> {
        if !self.config.anti_idle_enabled {
            return None;
        }

        let idle_threshold = std::time::Duration::from_secs(self.config.anti_idle_seconds.max(1));
        let keep_alive_code = [self.config.anti_idle_code];
        let mut next_due: Option<std::time::Instant> = None;

        for tab in self.tab_manager.tabs_mut() {
            if let Ok(term) = tab.terminal.try_write() {
                // Treat new terminal output as activity
                let current_generation = term.update_generation();
                if current_generation > tab.activity.anti_idle_last_generation {
                    tab.activity.anti_idle_last_generation = current_generation;
                    tab.activity.anti_idle_last_activity = now;
                }

                // If idle long enough, send keep-alive code
                if should_send_keep_alive(tab.activity.anti_idle_last_activity, now, idle_threshold)
                {
                    if let Err(e) = term.write(&keep_alive_code) {
                        log::warn!(
                            "Failed to send anti-idle keep-alive for tab {}: {}",
                            tab.id,
                            e
                        );
                    } else {
                        tab.activity.anti_idle_last_activity = now;
                    }
                }

                // Compute next due time for this tab
                let elapsed = now.duration_since(tab.activity.anti_idle_last_activity);
                let remaining = if elapsed >= idle_threshold {
                    idle_threshold
                } else {
                    idle_threshold - elapsed
                };
                let candidate = now + remaining;
                next_due = Some(next_due.map_or(candidate, |prev| prev.min(candidate)));
            }
        }

        next_due
    }

    // ========================================================================
    // egui / UI state queries
    // ========================================================================

    /// Check if egui is currently using the pointer (mouse is over an egui UI element)
    pub(crate) fn is_egui_using_pointer(&self) -> bool {
        // AI Inspector resize handle uses direct pointer tracking (not egui widgets),
        // so egui doesn't know about it. Check explicitly to prevent mouse events
        // from reaching the terminal during resize drag or initial click on the handle.
        if self.overlay_ui.ai_inspector.wants_pointer() {
            return true;
        }
        // Before first render, egui state is unreliable - allow mouse events through
        if !self.egui.initialized {
            return false;
        }
        // Always check egui context - the tab bar is always rendered via egui
        // and can consume pointer events (e.g., close button clicks)
        if let Some(ctx) = &self.egui.ctx {
            ctx.is_using_pointer() || ctx.wants_pointer_input()
        } else {
            false
        }
    }

    /// Canonical check: is any modal UI overlay visible?
    ///
    /// This is the single source of truth for "should input be blocked from the terminal
    /// because a modal dialog is open?" When adding a new modal panel, add it here.
    ///
    /// Note: Side panels (ai_inspector, profile drawer) and inline edit states
    /// (tab_bar_ui.is_renaming()) are NOT modals — they are checked separately
    /// at call sites that need them. The resize overlay is also not a modal.
    pub(crate) fn any_modal_ui_visible(&self) -> bool {
        self.overlay_ui.help_ui.visible
            || self.overlay_ui.clipboard_history_ui.visible
            || self.overlay_ui.command_history_ui.visible
            || self.overlay_ui.search_ui.visible
            || self.overlay_ui.tmux_session_picker_ui.visible
            || self.overlay_ui.shader_install_ui.visible
            || self.overlay_ui.integrations_ui.visible
            || self.overlay_ui.ssh_connect_ui.is_visible()
            || self.overlay_ui.remote_shell_install_ui.is_visible()
            || self.overlay_ui.quit_confirmation_ui.is_visible()
    }

    /// Check if any egui overlay with text input is visible.
    /// Used to route clipboard operations (paste/copy/select-all) to egui
    /// instead of the terminal when a modal dialog or the AI inspector is active.
    pub(crate) fn has_egui_text_overlay_visible(&self) -> bool {
        self.any_modal_ui_visible() || self.overlay_ui.ai_inspector.open
    }

    /// Check if egui is currently using keyboard input (e.g., text input or ComboBox has focus)
    pub(crate) fn is_egui_using_keyboard(&self) -> bool {
        // If any UI panel is visible, check if egui wants keyboard input
        // Note: Settings are handled by standalone SettingsWindow, not embedded UI
        // Note: Profile drawer does NOT block input - only modal dialogs do
        // Also check ai_inspector (side panel with text input) and tab rename (inline edit)
        let any_ui_visible = self.any_modal_ui_visible()
            || self.overlay_ui.ai_inspector.open
            || self.tab_bar_ui.is_renaming();
        if !any_ui_visible {
            return false;
        }

        // Check egui context for keyboard usage
        if let Some(ctx) = &self.egui.ctx {
            ctx.wants_keyboard_input()
        } else {
            false
        }
    }

    // ========================================================================
    // Scrollbar visibility
    // ========================================================================

    /// Determine if scrollbar should be visible based on autohide setting and recent activity
    pub(crate) fn should_show_scrollbar(&self) -> bool {
        let tab = match self.tab_manager.active_tab() {
            Some(t) => t,
            None => return false,
        };

        // No scrollbar needed if no scrollback available
        if tab.active_cache().scrollback_len == 0 {
            return false;
        }

        // Always show when dragging or moving
        if tab.active_scroll_state().dragging {
            return true;
        }

        // If autohide disabled, always show
        if self.config.scrollbar_autohide_delay == 0 {
            return true;
        }

        // If scrolled away from bottom, keep visible
        if tab.active_scroll_state().offset > 0 || tab.active_scroll_state().target_offset > 0 {
            return true;
        }

        // Show when pointer is near the scrollbar edge (hover reveal)
        if let Some(window) = &self.window {
            let padding = 32.0; // px hover band
            let width = window.inner_size().width as f64;
            let near_right = self.config.scrollbar_position != "left"
                && (width - tab.active_mouse().position.0) <= padding;
            let near_left = self.config.scrollbar_position == "left"
                && tab.active_mouse().position.0 <= padding;
            if near_left || near_right {
                return true;
            }
        }

        // Otherwise, hide after delay
        tab.active_scroll_state()
            .last_activity
            .elapsed()
            .as_millis()
            < self.config.scrollbar_autohide_delay as u128
    }

    // ========================================================================
    // Shutdown
    // ========================================================================

    /// Perform the shutdown sequence (save state and set shutdown flag)
    pub(crate) fn perform_shutdown(&mut self) {
        // Save last working directory for "previous session" mode
        if self.config.startup_directory_mode == crate::config::StartupDirectoryMode::Previous
            && let Some(tab) = self.tab_manager.active_tab()
            && let Ok(term) = tab.terminal.try_write()
            && let Some(cwd) = term.shell_integration_cwd()
        {
            log::info!("Saving last working directory: {}", cwd);
            if let Err(e) = self.config.save_last_working_directory(&cwd) {
                log::warn!("Failed to save last working directory: {}", e);
            }
        }

        // Set shutdown flag to stop redraw loop
        self.is_shutting_down = true;
        // Abort refresh tasks for all tabs
        for tab in self.tab_manager.tabs_mut() {
            if let Some(task) = tab.refresh_task.take() {
                task.abort();
            }
        }
        log::info!("Refresh tasks aborted, shutdown initiated");
    }
}

// ---------------------------------------------------------------------------
impl Drop for WindowState {
    fn drop(&mut self) {
        let t0 = std::time::Instant::now();
        log::info!("Shutting down window (fast path)");

        // Signal status bar polling threads to stop immediately.
        // They check the flag every 50ms, so by the time the auto-drop
        // calls join() later, the threads will already be exiting.
        self.status_bar_ui.signal_shutdown();

        // Save command history on a background thread (serializes in-memory, writes async)
        self.overlay_ui.command_history.save_background();

        // Set shutdown flag
        self.is_shutting_down = true;

        // Hide the window immediately for instant visual feedback
        if let Some(ref window) = self.window {
            window.set_visible(false);
            log::info!(
                "Window hidden for instant visual close (+{:.1}ms)",
                t0.elapsed().as_secs_f64() * 1000.0
            );
        }

        // Clean up egui state FIRST before any other resources are dropped
        self.egui.state = None;
        self.egui.ctx = None;

        // Drain all tabs from the manager (takes ownership without dropping)
        let mut tabs = self.tab_manager.drain_tabs();
        let tab_count = tabs.len();
        log::info!(
            "Fast shutdown: draining {} tabs (+{:.1}ms)",
            tab_count,
            t0.elapsed().as_secs_f64() * 1000.0
        );

        // Collect terminal Arcs and session loggers from all tabs and panes
        // BEFORE setting shutdown_fast. Cloning the Arc keeps TerminalManager
        // alive even after Tab/Pane is dropped. Session loggers are collected
        // so they can be stopped on a background thread instead of blocking.
        let mut terminal_arcs = Vec::new();
        let mut session_loggers = Vec::new();

        for tab in &mut tabs {
            // Stop refresh tasks (fast - just aborts tokio tasks)
            tab.stop_refresh_task();

            // Collect session logger for background stop
            session_loggers.push(Arc::clone(&tab.session_logger));

            // Clone terminal Arc before we mark shutdown_fast
            terminal_arcs.push(Arc::clone(&tab.terminal));

            // Also handle panes if this tab has splits
            if let Some(ref mut pm) = tab.pane_manager {
                for pane in pm.all_panes_mut() {
                    pane.stop_refresh_task();
                    session_loggers.push(Arc::clone(&pane.session_logger));
                    terminal_arcs.push(Arc::clone(&pane.terminal));
                    pane.shutdown_fast = true;
                }
            }

            // Mark tab for fast drop (skips sleep + kill in Tab::drop)
            tab.shutdown_fast = true;
        }

        // Pre-kill all PTY processes (sends SIGKILL, fast non-blocking)
        for arc in &terminal_arcs {
            if let Ok(mut term) = arc.try_write()
                && term.is_running()
            {
                let _ = term.kill();
            }
        }
        log::info!(
            "Pre-killed {} terminal sessions (+{:.1}ms)",
            terminal_arcs.len(),
            t0.elapsed().as_secs_f64() * 1000.0
        );

        // Drop tabs on main thread (fast - Tab::drop just returns immediately)
        drop(tabs);
        log::info!(
            "Tabs dropped (+{:.1}ms)",
            t0.elapsed().as_secs_f64() * 1000.0
        );

        // Fire-and-forget: stop session loggers on a background thread.
        // Each logger.stop() flushes buffered I/O which can block.
        if !session_loggers.is_empty() {
            let _ = std::thread::Builder::new()
                .name("logger-cleanup".into())
                .spawn(move || {
                    for logger_arc in session_loggers {
                        if let Some(ref mut logger) = *logger_arc.lock() {
                            let _ = logger.stop();
                        }
                    }
                });
        }

        // Fire-and-forget: drop the cloned terminal Arcs on background threads.
        // When our clone is the last reference, TerminalManager::drop runs,
        // which triggers PtySession::drop (up to 2s reader thread wait).
        // By running these in parallel, all sessions clean up concurrently.
        // We intentionally do NOT join these threads — the process is exiting
        // and the OS will reclaim all resources.
        for (i, arc) in terminal_arcs.into_iter().enumerate() {
            let _ = std::thread::Builder::new()
                .name(format!("pty-cleanup-{}", i))
                .spawn(move || {
                    let t = std::time::Instant::now();
                    drop(arc);
                    log::info!(
                        "pty-cleanup-{} finished in {:.1}ms",
                        i,
                        t.elapsed().as_secs_f64() * 1000.0
                    );
                });
        }

        log::info!(
            "Window shutdown complete ({} tabs, main thread blocked {:.1}ms)",
            tab_count,
            t0.elapsed().as_secs_f64() * 1000.0
        );
    }
}
