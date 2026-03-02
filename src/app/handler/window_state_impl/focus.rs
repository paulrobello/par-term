//! Window focus change handling for WindowState.
//!
//! Contains:
//! - `handle_focus_change`: power-saving focus logic, focus-click suppression,
//!   shader animation pause/resume, PTY focus event forwarding, refresh rate adjustment

use crate::app::window_state::WindowState;
use std::sync::Arc;

impl WindowState {
    /// Handle window focus change for power saving
    pub(crate) fn handle_focus_change(&mut self, focused: bool) {
        if self.focus_state.is_focused == focused {
            return; // No change
        }

        self.focus_state.is_focused = focused;

        log::info!(
            "Window focus changed: {}",
            if focused { "focused" } else { "blurred" }
        );

        // Suppress the first mouse click after gaining focus to prevent it from
        // being forwarded to the PTY. Without this, clicking to focus sends a
        // mouse event to tmux (or other mouse-aware apps), which can trigger a
        // zero-char selection that clears the system clipboard.
        if focused {
            let suppressed_recent_unfocused_click = self
                .focus_state
                .focus_click_suppressed_while_unfocused_at
                .is_some_and(|t| t.elapsed() <= std::time::Duration::from_millis(500));

            self.focus_state.focus_click_pending = !suppressed_recent_unfocused_click;
            self.focus_state.focus_click_suppressed_while_unfocused_at = None;
        } else {
            self.focus_state.focus_click_pending = false;
            self.focus_state.focus_click_suppressed_while_unfocused_at = None;
        }

        // Update renderer focus state for unfocused cursor styling
        if let Some(renderer) = &mut self.renderer {
            renderer.set_focused(focused);
        }

        // Handle shader animation pause/resume
        if self.config.pause_shaders_on_blur
            && let Some(renderer) = &mut self.renderer
        {
            if focused {
                // Only resume if user has animation enabled in config
                renderer.resume_shader_animations(
                    self.config.shader.custom_shader_animation,
                    self.config.shader.cursor_shader_animation,
                );
            } else {
                renderer.pause_shader_animations();
            }
        }

        // Re-assert tmux client size when window gains focus
        // This ensures par-term's size is respected even after other clients resize tmux
        if focused {
            self.notify_tmux_of_resize();
        }

        // Forward focus events to all PTYs that have focus tracking enabled (DECSET 1004)
        // This is needed for applications like tmux that rely on focus events
        for tab in self.tab_manager.tabs_mut() {
            // try_lock: intentional — Focused fires in the sync event loop. On miss: the
            // focus change event is not delivered to this terminal/pane. For most TUI apps
            // this means the focus-change visual update (e.g., tmux pane highlight) is
            // delayed one or more frames.
            if let Ok(term) = tab.terminal.try_write() {
                term.report_focus_change(focused);
            } else {
                crate::debug::record_try_lock_failure("focus_event");
            }
            // Also forward to all panes if split panes are active
            if let Some(pm) = &tab.pane_manager {
                for pane in pm.all_panes() {
                    // try_lock: intentional — same rationale as tab terminal above.
                    if let Ok(term) = pane.terminal.try_write() {
                        term.report_focus_change(focused);
                    } else {
                        crate::debug::record_try_lock_failure("focus_event_pane");
                    }
                }
            }
        }

        // Handle refresh rate adjustment for all tabs
        if self.config.pause_refresh_on_blur
            && let Some(window) = &self.window
        {
            let fps = if focused {
                self.config.max_fps
            } else {
                self.config.unfocused_fps
            };
            for tab in self.tab_manager.tabs_mut() {
                tab.stop_refresh_task();
                tab.start_refresh_task(
                    Arc::clone(&self.runtime),
                    Arc::clone(window),
                    fps,
                    self.config.inactive_tab_fps,
                );
            }
            log::info!(
                "Adjusted refresh rate to {} FPS ({})",
                fps,
                if focused { "focused" } else { "unfocused" }
            );
        }

        // Request a redraw when focus changes
        self.focus_state.needs_redraw = true;
        self.request_redraw();
    }
}
