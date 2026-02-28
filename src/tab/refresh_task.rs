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

                let should_redraw = if let Ok(term) = terminal_clone.try_lock() {
                    let current_gen = term.update_generation();
                    if current_gen > last_gen {
                        last_gen = current_gen;
                        true
                    } else {
                        false
                    }
                } else {
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
