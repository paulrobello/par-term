//! Tab refresh polling task management.
//!
//! Provides methods for starting and stopping the background async task
//! that polls the terminal for new output and triggers window redraws.
//! Uses adaptive polling with exponential backoff for inactive tabs.

use crate::tab::Tab;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tokio::runtime::Runtime;

impl Tab {
    /// Start the refresh polling task for this tab
    pub fn start_refresh_task(
        &mut self,
        runtime: Arc<Runtime>,
        window: Arc<winit::window::Window>,
        active_fps: u32,
        inactive_fps: u32,
    ) {
        let terminal_clone = Arc::clone(&self.terminal);
        let is_active = Arc::clone(&self.is_active);
        let active_interval_ms = (1000 / active_fps.max(1)) as u64;
        let inactive_interval_ms = (1000 / inactive_fps.max(1)) as u64;

        let handle = runtime.spawn(async move {
            let mut last_gen = 0u64;
            let mut was_running: Option<bool> = None;
            let mut idle_streak = 0u32;
            const MAX_INACTIVE_IDLE_INTERVAL_MS: u64 = 250;

            loop {
                let is_active_now = is_active.load(Ordering::Relaxed);
                // Keep the active tab responsive: only apply backoff to inactive tabs.
                let interval_ms = if is_active_now {
                    active_interval_ms
                } else if idle_streak > 0 {
                    (inactive_interval_ms << idle_streak.min(4)).min(MAX_INACTIVE_IDLE_INTERVAL_MS)
                } else {
                    inactive_interval_ms
                };
                tokio::time::sleep(tokio::time::Duration::from_millis(interval_ms)).await;

                let should_redraw = if let Ok(term) = terminal_clone.try_read() {
                    let current_gen = term.update_generation();
                    let gen_changed = current_gen > last_gen;
                    if gen_changed {
                        last_gen = current_gen;
                    }
                    // A pane's child can die without producing output: the
                    // reader's EOF path stores running=false but bumps no
                    // generation counter, so the output-driven check above
                    // never redraws and the shell-exit handling in
                    // RedrawRequested never runs — the window survives as a
                    // frozen frame. Redraw once on the running→dead edge;
                    // poll_liveness asks the OS while the flag lags the real
                    // child.
                    let running = term.poll_liveness();
                    let died = refresh_redraw_on_death(&mut was_running, running);
                    gen_changed || died
                } else {
                    // Lock miss: keep the previous liveness state so a
                    // contended read cannot fabricate a death edge.
                    false
                };

                if should_redraw {
                    idle_streak = 0;
                    window.request_redraw();
                } else if is_active_now {
                    idle_streak = 0;
                } else {
                    idle_streak = idle_streak.saturating_add(1);
                }
            }
        });

        self.refresh_task = Some(handle);
    }

    /// Stop the refresh polling task
    pub fn stop_refresh_task(&mut self) {
        if let Some(handle) = self.refresh_task.take() {
            handle.abort();
        }
    }
}

/// One liveness poll of the refresh loop: true on the running→dead edge.
///
/// `was_running` carries the previous poll's state across calls (`None` =
/// never seen, which can never be an edge). Returns true exactly once per
/// death, so a kept-open dead pane (Keep action / restart prompt) does not
/// redraw at every tick.
fn refresh_redraw_on_death(was_running: &mut Option<bool>, running: bool) -> bool {
    let died = *was_running == Some(true) && !running;
    *was_running = Some(running);
    died
}

#[cfg(test)]
mod tests {
    use super::refresh_redraw_on_death;

    #[test]
    fn death_edge_fires_once_and_not_on_first_poll() {
        // First poll of a dead pane: no previous state, no edge.
        let mut state = None;
        assert!(!refresh_redraw_on_death(&mut state, false));
        assert_eq!(state, Some(false));

        // A kept-open dead pane does not redraw again.
        assert!(!refresh_redraw_on_death(&mut state, false));
        assert!(!refresh_redraw_on_death(&mut state, false));

        // Respawn (running again) re-arms the edge for a later death.
        assert!(!refresh_redraw_on_death(&mut state, true));
        assert!(refresh_redraw_on_death(&mut state, false));
    }
}
