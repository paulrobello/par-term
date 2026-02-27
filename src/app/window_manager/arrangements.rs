//! Window arrangement save, restore, and management for the window manager.
//!
//! This module handles saving the current window layout as a named arrangement,
//! restoring arrangements by ID or name, and CRUD operations on stored arrangements.

use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use crate::arrangements::ArrangementId;

use super::WindowManager;

impl WindowManager {
    /// Save the current window layout as an arrangement
    pub fn save_arrangement(&mut self, name: String, event_loop: &ActiveEventLoop) {
        // Remove existing arrangement with the same name (case-insensitive) to allow overwrite
        if let Some(existing) = self.arrangement_manager.find_by_name(&name) {
            let existing_id = existing.id;
            self.arrangement_manager.remove(&existing_id);
            log::info!("Overwriting existing arrangement '{}'", name);
        }

        let arrangement = crate::arrangements::capture::capture_arrangement(
            name.clone(),
            &self.windows,
            event_loop,
        );
        log::info!(
            "Saved arrangement '{}' with {} windows",
            name,
            arrangement.windows.len()
        );
        self.arrangement_manager.add(arrangement);
        if let Err(e) =
            crate::arrangements::storage::save_arrangements(&self.arrangement_manager)
        {
            log::error!("Failed to save arrangements: {}", e);
        }
        self.sync_arrangements_to_settings();
    }

    /// Restore a saved arrangement by ID.
    ///
    /// Closes all existing windows and creates new ones according to the arrangement.
    pub fn restore_arrangement(&mut self, id: ArrangementId, event_loop: &ActiveEventLoop) {
        let arrangement = match self.arrangement_manager.get(&id) {
            Some(a) => a.clone(),
            None => {
                log::error!("Arrangement not found: {}", id);
                return;
            }
        };

        log::info!(
            "Restoring arrangement '{}' ({} windows)",
            arrangement.name,
            arrangement.windows.len()
        );

        // Close all existing windows
        let window_ids: Vec<WindowId> = self.windows.keys().copied().collect();
        for window_id in window_ids {
            if let Some(window_state) = self.windows.remove(&window_id) {
                drop(window_state);
            }
        }

        // Build monitor mapping
        let available_monitors: Vec<_> = event_loop.available_monitors().collect();
        let monitor_mapping = crate::arrangements::restore::build_monitor_mapping(
            &arrangement.monitor_layout,
            &available_monitors,
        );

        // Create windows from arrangement
        for (i, window_snapshot) in arrangement.windows.iter().enumerate() {
            let Some((x, y, w, h)) = crate::arrangements::restore::compute_restore_position(
                window_snapshot,
                &monitor_mapping,
                &available_monitors,
            ) else {
                log::warn!("Could not compute position for window {} in arrangement", i);
                continue;
            };

            let tab_cwds = crate::arrangements::restore::tab_cwds(&arrangement, i);
            let created_window_id = self.create_window_with_overrides(
                event_loop,
                (x, y),
                (w, h),
                &tab_cwds,
                window_snapshot.active_tab_index,
            );

            // Restore user titles, custom colors, and icons from arrangement
            if let Some(window_id) = created_window_id
                && let Some(window_state) = self.windows.get_mut(&window_id)
            {
                let tabs = window_state.tab_manager.tabs_mut();
                for (tab_idx, snapshot) in window_snapshot.tabs.iter().enumerate() {
                    if let Some(tab) = tabs.get_mut(tab_idx) {
                        if let Some(ref user_title) = snapshot.user_title {
                            tab.title = user_title.clone();
                            tab.user_named = true;
                            tab.has_default_title = false;
                        }
                        if let Some(color) = snapshot.custom_color {
                            tab.set_custom_color(color);
                        }
                        if let Some(ref icon) = snapshot.custom_icon {
                            tab.custom_icon = Some(icon.clone());
                        }
                    }
                }
            }
        }

        // If no windows were created (e.g., empty arrangement), create one default window
        if self.windows.is_empty() {
            log::warn!("Arrangement had no restorable windows, creating default window");
            self.create_window(event_loop);
        }
    }

    /// Restore an arrangement by name (for auto-restore and keybinding actions)
    pub fn restore_arrangement_by_name(
        &mut self,
        name: &str,
        event_loop: &ActiveEventLoop,
    ) -> bool {
        if let Some(arrangement) = self.arrangement_manager.find_by_name(name) {
            let id = arrangement.id;
            self.restore_arrangement(id, event_loop);
            true
        } else {
            log::warn!("Arrangement not found by name: {}", name);
            false
        }
    }

    /// Delete an arrangement by ID
    pub fn delete_arrangement(&mut self, id: ArrangementId) {
        if let Some(removed) = self.arrangement_manager.remove(&id) {
            log::info!("Deleted arrangement '{}'", removed.name);
            if let Err(e) =
                crate::arrangements::storage::save_arrangements(&self.arrangement_manager)
            {
                log::error!("Failed to save arrangements after delete: {}", e);
            }
            self.sync_arrangements_to_settings();
        }
    }

    /// Rename an arrangement by ID
    pub fn rename_arrangement(&mut self, id: ArrangementId, new_name: String) {
        if let Some(arrangement) = self.arrangement_manager.get_mut(&id) {
            log::info!(
                "Renamed arrangement '{}' -> '{}'",
                arrangement.name,
                new_name
            );
            arrangement.name = new_name;
            if let Err(e) =
                crate::arrangements::storage::save_arrangements(&self.arrangement_manager)
            {
                log::error!("Failed to save arrangements after rename: {}", e);
            }
            self.sync_arrangements_to_settings();
        }
    }

    /// Move an arrangement up in the order
    pub fn move_arrangement_up(&mut self, id: ArrangementId) {
        self.arrangement_manager.move_up(&id);
        if let Err(e) =
            crate::arrangements::storage::save_arrangements(&self.arrangement_manager)
        {
            log::error!("Failed to save arrangements after reorder: {}", e);
        }
        self.sync_arrangements_to_settings();
    }

    /// Move an arrangement down in the order
    pub fn move_arrangement_down(&mut self, id: ArrangementId) {
        self.arrangement_manager.move_down(&id);
        if let Err(e) =
            crate::arrangements::storage::save_arrangements(&self.arrangement_manager)
        {
            log::error!("Failed to save arrangements after reorder: {}", e);
        }
        self.sync_arrangements_to_settings();
    }

    /// Sync arrangement manager data to the settings window (for UI display)
    pub fn sync_arrangements_to_settings(&mut self) {
        if let Some(sw) = &mut self.settings_window {
            sw.settings_ui.arrangement_manager = self.arrangement_manager.clone();
        }
    }
}
