//! "Open Profile in New Window" (UX.md PR3 `open_profile_window:<id>`, PR1
//! Shift+Enter in the launcher).
//!
//! A window needs the event loop, which a `WindowState` action cannot reach,
//! so the action queues the profile on `OverlayState::pending_profile_windows`
//! and the manager drains it after the per-window pass.

use winit::event_loop::ActiveEventLoop;

use super::WindowManager;

impl WindowManager {
    /// Open one window per queued profile.
    pub(crate) fn open_pending_profile_windows(&mut self, event_loop: &ActiveEventLoop) {
        let queued: Vec<crate::profile::ProfileId> = self
            .windows
            .values_mut()
            .flat_map(|ws| std::mem::take(&mut ws.overlay_state.pending_profile_windows))
            .collect();
        for profile_id in queued {
            self.open_profile_in_new_window(event_loop, profile_id);
        }
    }

    /// Create a window positioned like New Window, whose only tab runs
    /// `profile_id`.
    pub(crate) fn open_profile_in_new_window(
        &mut self,
        event_loop: &ActiveEventLoop,
        profile_id: crate::profile::ProfileId,
    ) {
        let before: std::collections::HashSet<_> = self.windows.keys().copied().collect();
        self.create_window(event_loop);
        let Some(created) = self.windows.keys().copied().find(|id| !before.contains(id)) else {
            log::warn!("Open profile in new window: the window was not created");
            return;
        };
        self.replace_first_tab_with_profile(created, profile_id);
    }
}
