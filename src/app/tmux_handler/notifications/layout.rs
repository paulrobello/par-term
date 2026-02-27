//! Layout-change tmux notification handler.
//!
//! `handle_tmux_layout_change` parses the tmux layout string, reconciles it
//! with the current native pane tree (preserve / incrementally add / remove /
//! full rebuild), and triggers a content refresh for any new panes.
//! `request_pane_refresh` and `log_layout_node` are helpers used exclusively
//! by the layout handler.

use crate::app::window_state::WindowState;
use crate::tmux::{TmuxLayout, TmuxWindowId};

/// Render bounds info passed through layout helper methods.
///
/// Fields: (physical_size, scale_factor, viewport_x, viewport_y, cell_width, cell_height, line_height)
pub(super) type BoundsInfo = Option<(winit::dpi::PhysicalSize<u32>, f32, f32, f32, f32, f32, f32)>;

impl WindowState {
    /// Request content refresh for specific panes
    ///
    /// After learning about panes from a layout change, we need to trigger
    /// each pane to send its content. tmux only sends %output for NEW content,
    /// not existing screen content when attaching.
    ///
    /// We use two approaches:
    /// 1. Send Ctrl+L (C-l) to each pane, which triggers shell screen redraw
    /// 2. Use capture-pane -p to get the current pane content (comes as command response)
    pub(super) fn request_pane_refresh(&self, pane_ids: &[crate::tmux::TmuxPaneId]) {
        for pane_id in pane_ids {
            // Approach 1: Send Ctrl+L (screen redraw signal) to trigger shell to repaint
            // This works for interactive shells that respond to SIGWINCH-like events
            let cmd = format!("send-keys -t %{} C-l\n", pane_id);
            if self.write_to_gateway(&cmd) {
                crate::debug_trace!("TMUX", "Sent C-l to pane %{} for refresh", pane_id);
            }
        }

        // Request client refresh which may help with layout sync
        let refresh_cmd = "refresh-client\n";
        if self.write_to_gateway(refresh_cmd) {
            crate::debug_info!(
                "TMUX",
                "Requested client refresh for {} panes",
                pane_ids.len()
            );
        }
    }

    /// Handle layout change notification - updates pane arrangement
    pub(super) fn handle_tmux_layout_change(&mut self, window_id: TmuxWindowId, layout_str: &str) {
        crate::debug_info!(
            "TMUX",
            "Layout changed for window @{}: {}",
            window_id,
            layout_str
        );

        // Parse the layout string
        let parsed_layout = match TmuxLayout::parse(layout_str) {
            Some(layout) => layout,
            None => {
                crate::debug_error!(
                    "TMUX",
                    "Failed to parse layout string for window @{}: {}",
                    window_id,
                    layout_str
                );
                return;
            }
        };

        // Log the parsed layout structure
        let pane_ids = parsed_layout.pane_ids();
        crate::debug_info!(
            "TMUX",
            "Parsed layout for window @{}: {} panes (IDs: {:?})",
            window_id,
            pane_ids.len(),
            pane_ids
        );

        // Log the layout structure for debugging
        Self::log_layout_node(&parsed_layout.root, 0);

        // Update focused pane in session if we have one
        if !pane_ids.is_empty()
            && let Some(session) = &mut self.tmux_state.tmux_session
        {
            // Default to first pane if no focused pane set
            if session.focused_pane().is_none() {
                session.set_focused_pane(Some(pane_ids[0]));
            }
        }

        // Find the corresponding tab and create window mapping if needed
        let tab_id = if let Some(id) = self.tmux_state.tmux_sync.get_tab(window_id) {
            Some(id)
        } else {
            // No window mapping exists - try to find a tab that has one of our panes
            // This happens when we connect to an existing session and receive layout before window-add
            let mut found_tab_id = None;
            for pane_id in &pane_ids {
                // Check if any tab has this tmux_pane_id set
                for tab in self.tab_manager.tabs() {
                    if tab.tmux_pane_id == Some(*pane_id) {
                        found_tab_id = Some(tab.id);
                        crate::debug_info!(
                            "TMUX",
                            "Found existing tab {} with pane %{} for window @{}",
                            tab.id,
                            pane_id,
                            window_id
                        );
                        break;
                    }
                }
                if found_tab_id.is_some() {
                    break;
                }
            }

            // If we found a tab, create the window mapping
            if let Some(tid) = found_tab_id {
                self.tmux_state.tmux_sync.map_window(window_id, tid);
                crate::debug_info!(
                    "TMUX",
                    "Created window mapping: @{} -> tab {}",
                    window_id,
                    tid
                );
            }

            found_tab_id
        };

        // Get bounds info from renderer for proper pane sizing (needed for both paths)
        // Calculate status bar height for proper content area
        let is_tmux_connected = self.is_tmux_connected();
        let status_bar_height =
            crate::tmux_status_bar_ui::TmuxStatusBarUI::height(&self.config, is_tmux_connected);
        let custom_status_bar_height = self.status_bar_ui.height(&self.config, self.is_fullscreen);

        let bounds_info = self.renderer.as_ref().map(|r| {
            let size = r.size();
            let padding = r.window_padding();
            let content_offset_y = r.content_offset_y();
            let content_inset_right = r.content_inset_right();
            let cell_width = r.cell_width();
            let cell_height = r.cell_height();
            // Scale status_bar_height from logical to physical pixels
            let physical_status_bar_height =
                (status_bar_height + custom_status_bar_height) * r.scale_factor();
            (
                size,
                padding,
                content_offset_y,
                content_inset_right,
                cell_width,
                cell_height,
                physical_status_bar_height,
            )
        });

        if let Some(tab_id) = tab_id {
            self.apply_layout_to_existing_tab(
                tab_id,
                window_id,
                &parsed_layout,
                &pane_ids,
                bounds_info,
            );
        } else {
            // No tab mapping found - create a new tab for this tmux window
            self.create_tab_for_layout(window_id, &parsed_layout, &pane_ids, bounds_info);
        }
    }

    /// Apply a parsed tmux layout to an already-mapped tab.
    ///
    /// Handles three cases in priority order:
    /// 1. Same panes — preserve terminals, update layout structure.
    /// 2. Panes removed — incrementally close removed native panes, update layout.
    /// 3. Panes added — rebuild tree preserving existing terminals, add new ones.
    /// 4. Full recreation — completely replace the pane tree.
    fn apply_layout_to_existing_tab(
        &mut self,
        tab_id: crate::tab::TabId,
        window_id: TmuxWindowId,
        parsed_layout: &TmuxLayout,
        pane_ids: &[crate::tmux::TmuxPaneId],
        bounds_info: BoundsInfo,
    ) {
        crate::debug_info!(
            "TMUX",
            "Layout change for window @{} on tab {} - {} panes: {:?}",
            window_id,
            tab_id,
            pane_ids.len(),
            pane_ids
        );

        let Some(tab) = self.tab_manager.get_tab_mut(tab_id) else {
            return;
        };

        // Initialize pane manager if needed
        tab.init_pane_manager();

        // Set pane bounds before applying layout
        if let Some((
            size,
            padding,
            content_offset_y,
            content_inset_right,
            _cell_width,
            _cell_height,
            status_bar_height,
        )) = bounds_info
            && let Some(pm) = tab.pane_manager_mut()
        {
            // Tmux layouts always have multiple panes; hide window padding if configured
            let effective_padding = if self.config.hide_window_padding_on_split {
                0.0
            } else {
                padding
            };
            let content_width = size.width as f32 - effective_padding * 2.0 - content_inset_right;
            let content_height =
                size.height as f32 - content_offset_y - effective_padding - status_bar_height;
            let bounds = crate::pane::PaneBounds::new(
                effective_padding,
                content_offset_y,
                content_width,
                content_height,
            );
            pm.set_bounds(bounds);
            crate::debug_info!(
                "TMUX",
                "Set pane manager bounds: {}x{} at ({}, {})",
                content_width,
                content_height,
                effective_padding,
                content_offset_y
            );
        }

        // Compute set deltas between existing and new tmux pane IDs
        let existing_tmux_ids: std::collections::HashSet<_> = self
            .tmux_state
            .tmux_pane_to_native_pane
            .keys()
            .copied()
            .collect();
        let new_tmux_ids: std::collections::HashSet<_> = pane_ids.iter().copied().collect();

        if existing_tmux_ids == new_tmux_ids && !existing_tmux_ids.is_empty() {
            // Same panes - preserve terminals but update layout structure
            self.handle_same_pane_layout_update(tab_id, parsed_layout, bounds_info);
            return;
        }

        let panes_to_keep: std::collections::HashSet<_> = existing_tmux_ids
            .intersection(&new_tmux_ids)
            .copied()
            .collect();
        let panes_to_remove: Vec<_> = existing_tmux_ids
            .difference(&new_tmux_ids)
            .copied()
            .collect();
        let panes_to_add: Vec<_> = new_tmux_ids
            .difference(&existing_tmux_ids)
            .copied()
            .collect();

        if !panes_to_keep.is_empty() && !panes_to_remove.is_empty() && panes_to_add.is_empty() {
            self.handle_pane_removal(
                tab_id,
                parsed_layout,
                &panes_to_keep,
                &panes_to_remove,
                bounds_info,
            );
            return;
        }

        if !panes_to_keep.is_empty() && !panes_to_add.is_empty() && panes_to_remove.is_empty() {
            self.handle_pane_addition(
                tab_id,
                parsed_layout,
                &panes_to_keep,
                &panes_to_add,
                bounds_info,
            );
            return;
        }

        // Full layout recreation needed (complete replacement or complex changes)
        self.handle_full_layout_recreation(tab_id, window_id, parsed_layout, pane_ids, bounds_info);
    }

    /// Case 1: Same tmux pane IDs — only layout geometry changed.
    fn handle_same_pane_layout_update(
        &mut self,
        tab_id: crate::tab::TabId,
        parsed_layout: &TmuxLayout,
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

        self.needs_redraw = true;
    }

    /// Case 2: Some panes closed — remove them from native tree, update layout.
    fn handle_pane_removal(
        &mut self,
        tab_id: crate::tab::TabId,
        parsed_layout: &TmuxLayout,
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

        self.needs_redraw = true;
        self.request_redraw();
    }

    /// Case 3: New panes added (split) while existing panes remain.
    fn handle_pane_addition(
        &mut self,
        tab_id: crate::tab::TabId,
        parsed_layout: &TmuxLayout,
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

        self.needs_redraw = true;
        self.request_redraw();
    }

    /// Case 4: Full layout recreation — complete replacement or complex delta.
    fn handle_full_layout_recreation(
        &mut self,
        tab_id: crate::tab::TabId,
        window_id: TmuxWindowId,
        parsed_layout: &TmuxLayout,
        pane_ids: &[crate::tmux::TmuxPaneId],
        _bounds_info: BoundsInfo,
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

                    if !pane_ids.is_empty() && tab.tmux_pane_id.is_none() {
                        tab.tmux_pane_id = Some(pane_ids[0]);
                    }

                    self.request_pane_refresh(pane_ids);
                    self.needs_redraw = true;
                }
                Err(e) => {
                    crate::debug_error!(
                        "TMUX",
                        "Failed to apply tmux layout to tab {}: {}",
                        tab_id,
                        e
                    );
                    if !pane_ids.is_empty() && tab.tmux_pane_id.is_none() {
                        tab.tmux_pane_id = Some(pane_ids[0]);
                    }
                }
            }
        } else {
            // No pane manager - use legacy routing
            if !pane_ids.is_empty() && tab.tmux_pane_id.is_none() {
                tab.tmux_pane_id = Some(pane_ids[0]);
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

    /// Log a layout node and its children recursively for debugging
    pub(super) fn log_layout_node(node: &crate::tmux::LayoutNode, depth: usize) {
        let indent = "  ".repeat(depth);
        match node {
            crate::tmux::LayoutNode::Pane {
                id,
                width,
                height,
                x,
                y,
            } => {
                crate::debug_trace!(
                    "TMUX",
                    "{}Pane %{}: {}x{} at ({}, {})",
                    indent,
                    id,
                    width,
                    height,
                    x,
                    y
                );
            }
            crate::tmux::LayoutNode::VerticalSplit {
                width,
                height,
                x,
                y,
                children,
            } => {
                crate::debug_trace!(
                    "TMUX",
                    "{}VerticalSplit: {}x{} at ({}, {}) with {} children",
                    indent,
                    width,
                    height,
                    x,
                    y,
                    children.len()
                );
                for child in children {
                    Self::log_layout_node(child, depth + 1);
                }
            }
            crate::tmux::LayoutNode::HorizontalSplit {
                width,
                height,
                x,
                y,
                children,
            } => {
                crate::debug_trace!(
                    "TMUX",
                    "{}HorizontalSplit: {}x{} at ({}, {}) with {} children",
                    indent,
                    width,
                    height,
                    x,
                    y,
                    children.len()
                );
                for child in children {
                    Self::log_layout_node(child, depth + 1);
                }
            }
        }
    }
}
