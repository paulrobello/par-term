//! Helper methods for applying a parsed tmux layout to an existing tab.
//!
//! These four methods are called exclusively from
//! `layout::apply_layout_to_existing_tab` and handle each of the four
//! reconciliation cases:
//!
//! 1. `handle_same_pane_layout_update` — same pane set, geometry only changed
//! 2. `handle_pane_removal`            — some panes were closed
//! 3. `handle_pane_addition`           — new panes split in
//! 4. `handle_full_layout_recreation`  — complete replacement or complex delta

use crate::app::window_state::WindowState;
use crate::tmux::TmuxWindowId;

use super::layout::BoundsInfo;

impl WindowState {
    /// Case 1: Same tmux pane IDs — only layout geometry changed.
    pub(super) fn handle_same_pane_layout_update(
        &mut self,
        tab_id: crate::tab::TabId,
        parsed_layout: &crate::tmux::TmuxLayout,
        bounds_info: BoundsInfo,
    ) {
        crate::debug_info!(
            "TMUX",
            "Layout change with same panes - preserving terminals, updating layout"
        );

        let Some(tab) = self.tab_manager.get_tab_mut(tab_id) else {
            return;
        };

        if let Some(pm) = tab.pane_manager_mut() {
            pm.update_layout_from_tmux(parsed_layout, &self.tmux_state.tmux_pane_to_native_pane);
            pm.recalculate_bounds();

            if let Some((_, _, _, _, cell_width, cell_height, _)) = bounds_info {
                pm.resize_all_terminals(cell_width, cell_height);
            }
        }

        self.focus_state.needs_redraw = true;
    }

    /// Case 2: Some panes closed — remove them from native tree, update layout.
    pub(super) fn handle_pane_removal(
        &mut self,
        tab_id: crate::tab::TabId,
        parsed_layout: &crate::tmux::TmuxLayout,
        panes_to_keep: &std::collections::HashSet<crate::tmux::TmuxPaneId>,
        panes_to_remove: &[crate::tmux::TmuxPaneId],
        bounds_info: BoundsInfo,
    ) {
        crate::debug_info!(
            "TMUX",
            "Layout change: keeping {:?}, removing {:?}",
            panes_to_keep,
            panes_to_remove
        );

        let current_focused = self
            .tmux_state
            .tmux_session
            .as_ref()
            .and_then(|s| s.focused_pane());
        let focused_pane_removed = current_focused
            .map(|fp| panes_to_remove.contains(&fp))
            .unwrap_or(false);

        let Some(tab) = self.tab_manager.get_tab_mut(tab_id) else {
            return;
        };

        if let Some(pm) = tab.pane_manager_mut() {
            for tmux_pane_id in panes_to_remove {
                if let Some(native_pane_id) =
                    self.tmux_state.tmux_pane_to_native_pane.get(tmux_pane_id)
                {
                    crate::debug_info!(
                        "TMUX",
                        "Removing native pane {} for closed tmux pane %{}",
                        native_pane_id,
                        tmux_pane_id
                    );
                    pm.close_pane(*native_pane_id);
                }
            }

            let kept_mappings: std::collections::HashMap<_, _> = self
                .tmux_state
                .tmux_pane_to_native_pane
                .iter()
                .filter(|(tmux_id, _)| panes_to_keep.contains(tmux_id))
                .map(|(k, v)| (*k, *v))
                .collect();

            pm.update_layout_from_tmux(parsed_layout, &kept_mappings);
            pm.recalculate_bounds();

            if let Some((_, _, _, _, cell_width, cell_height, _)) = bounds_info {
                pm.resize_all_terminals(cell_width, cell_height);
            }
        }

        // Update mappings - remove closed panes
        for tmux_pane_id in panes_to_remove {
            if let Some(native_id) = self
                .tmux_state
                .tmux_pane_to_native_pane
                .remove(tmux_pane_id)
            {
                self.tmux_state.native_pane_to_tmux_pane.remove(&native_id);
            }
        }

        if focused_pane_removed && let Some(new_focus) = panes_to_keep.iter().next().copied() {
            crate::debug_info!(
                "TMUX",
                "Focused pane was removed, updating tmux session focus to %{}",
                new_focus
            );
            if let Some(session) = &mut self.tmux_state.tmux_session {
                session.set_focused_pane(Some(new_focus));
            }
        }

        crate::debug_info!(
            "TMUX",
            "After pane removal, mappings: {:?}",
            self.tmux_state.tmux_pane_to_native_pane
        );

        self.focus_state.needs_redraw = true;
        self.request_redraw();
    }

    /// Case 3: New panes added (split) while existing panes remain.
    pub(super) fn handle_pane_addition(
        &mut self,
        tab_id: crate::tab::TabId,
        parsed_layout: &crate::tmux::TmuxLayout,
        panes_to_keep: &std::collections::HashSet<crate::tmux::TmuxPaneId>,
        panes_to_add: &[crate::tmux::TmuxPaneId],
        bounds_info: BoundsInfo,
    ) {
        crate::debug_info!(
            "TMUX",
            "Layout change: keeping {:?}, adding {:?}",
            panes_to_keep,
            panes_to_add
        );

        let Some(tab) = self.tab_manager.get_tab_mut(tab_id) else {
            return;
        };

        if let Some(pm) = tab.pane_manager_mut() {
            let existing_mappings: std::collections::HashMap<_, _> = panes_to_keep
                .iter()
                .filter_map(|tmux_id| {
                    self.tmux_state
                        .tmux_pane_to_native_pane
                        .get(tmux_id)
                        .map(|native_id| (*tmux_id, *native_id))
                })
                .collect();

            match pm.rebuild_from_tmux_layout(
                parsed_layout,
                &existing_mappings,
                panes_to_add,
                &self.config,
                std::sync::Arc::clone(&self.runtime),
            ) {
                Ok(new_mappings) => {
                    self.tmux_state.tmux_pane_to_native_pane = new_mappings.clone();
                    self.tmux_state.native_pane_to_tmux_pane = new_mappings
                        .iter()
                        .map(|(tmux_id, native_id)| (*native_id, *tmux_id))
                        .collect();

                    crate::debug_info!(
                        "TMUX",
                        "Rebuilt layout with {} panes: {:?}",
                        new_mappings.len(),
                        new_mappings
                    );

                    if let Some((_, _, _, _, cell_width, cell_height, _)) = bounds_info {
                        pm.resize_all_terminals(cell_width, cell_height);
                    }
                }
                Err(e) => {
                    crate::debug_error!("TMUX", "Failed to rebuild layout: {}", e);
                }
            }
        }

        self.request_pane_refresh(panes_to_add);

        crate::debug_info!(
            "TMUX",
            "After pane addition, mappings: {:?}",
            self.tmux_state.tmux_pane_to_native_pane
        );

        self.focus_state.needs_redraw = true;
        self.request_redraw();
    }

    /// Case 4: Full layout recreation — complete replacement or complex delta.
    pub(super) fn handle_full_layout_recreation(
        &mut self,
        tab_id: crate::tab::TabId,
        window_id: TmuxWindowId,
        parsed_layout: &crate::tmux::TmuxLayout,
        pane_ids: &[crate::tmux::TmuxPaneId],
        bounds_info: BoundsInfo,
    ) {
        let existing_tmux_ids: std::collections::HashSet<_> = self
            .tmux_state
            .tmux_pane_to_native_pane
            .keys()
            .copied()
            .collect();
        let new_tmux_ids: std::collections::HashSet<crate::tmux::TmuxPaneId> =
            pane_ids.iter().copied().collect();

        crate::debug_info!(
            "TMUX",
            "Full layout recreation: existing={:?}, new={:?}",
            existing_tmux_ids,
            new_tmux_ids
        );

        let Some(tab) = self.tab_manager.get_tab_mut(tab_id) else {
            return;
        };

        if let Some(pm) = tab.pane_manager_mut() {
            match pm.set_from_tmux_layout(
                parsed_layout,
                &self.config,
                std::sync::Arc::clone(&self.runtime),
            ) {
                Ok(pane_mappings) => {
                    crate::debug_info!("TMUX", "Storing pane mappings: {:?}", pane_mappings);
                    self.tmux_state.tmux_pane_to_native_pane = pane_mappings.clone();
                    self.tmux_state.native_pane_to_tmux_pane = pane_mappings
                        .iter()
                        .map(|(tmux_id, native_id)| (*native_id, *tmux_id))
                        .collect();

                    crate::debug_info!(
                        "TMUX",
                        "Applied tmux layout to tab {}: {} pane mappings created",
                        tab_id,
                        pane_mappings.len()
                    );

                    // Resize pane terminals to their actual display sizes so that
                    // when tmux sends %output the content fills each pane correctly.
                    // NOTE: must happen before tab.tmux is accessed so that pm's
                    // mutable borrow of tab ends here (NLL: last use of pm).
                    if let Some((_, _, _, _, cell_width, cell_height, _)) = bounds_info {
                        pm.resize_all_terminals(cell_width, cell_height);
                    }

                    if !pane_ids.is_empty() && tab.tmux.tmux_pane_id.is_none() {
                        tab.tmux.tmux_pane_id = Some(pane_ids[0]);
                    }

                    self.request_pane_refresh(pane_ids);
                    self.focus_state.needs_redraw = true;
                }
                Err(e) => {
                    crate::debug_error!(
                        "TMUX",
                        "Failed to apply tmux layout to tab {}: {}",
                        tab_id,
                        e
                    );
                    if !pane_ids.is_empty() && tab.tmux.tmux_pane_id.is_none() {
                        tab.tmux.tmux_pane_id = Some(pane_ids[0]);
                    }
                }
            }
        } else {
            // No pane manager - use legacy routing
            if !pane_ids.is_empty() && tab.tmux.tmux_pane_id.is_none() {
                tab.tmux.tmux_pane_id = Some(pane_ids[0]);
                crate::debug_info!(
                    "TMUX",
                    "Set tab {} tmux_pane_id to %{} for output routing (no pane manager)",
                    tab_id,
                    pane_ids[0]
                );
            }
        }

        let _ = window_id; // used only in debug messages above
    }
}
