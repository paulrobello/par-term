//! Pane/tab promotion: promote pane to tab, demote tab to pane.

use std::sync::Arc;

use par_term_config::TabId;

use super::super::window_state::WindowState;
use crate::pane::{ExtractResult, PaneId, SplitDirection};

/// State machine for the multi-step demote (tab → pane) pick mode.
#[derive(Default)]
pub(crate) enum PaneTransferState {
    #[default]
    Idle,
    DemotePickTab {
        source_tab_id: TabId,
    },
    DemotePickPane {
        source_tab_id: TabId,
        target_tab_id: TabId,
    },
    DemoteChooseDirection {
        source_tab_id: TabId,
        target_tab_id: TabId,
        target_pane_id: PaneId,
    },
}

impl PaneTransferState {
    pub fn is_active(&self) -> bool {
        !matches!(self, PaneTransferState::Idle)
    }
}

impl WindowState {
    /// Promote the focused pane in the current tab to its own tab.
    pub fn promote_pane_to_tab(&mut self) {
        let source_tab_id = match self.tab_manager.active_tab_id() {
            Some(id) => id,
            None => return,
        };

        let focused_pane_id = match self
            .tab_manager
            .active_tab()
            .and_then(|t| t.focused_pane_id())
        {
            Some(id) => id,
            None => return,
        };

        // Extract the pane from the source tab's tree
        let pane = match self.tab_manager.get_tab_mut(source_tab_id) {
            Some(tab) => {
                let pm = match tab.pane_manager_mut() {
                    Some(pm) => pm,
                    None => return,
                };
                match pm.extract_pane(focused_pane_id) {
                    ExtractResult::Extracted { pane, remaining } => {
                        // Put the remaining tree back into the source tab
                        if let Some(remaining_node) = remaining
                            && let Some(tab) = self.tab_manager.get_tab_mut(source_tab_id)
                            && let Some(pm) = tab.pane_manager_mut()
                        {
                            pm.set_root(remaining_node);
                        }
                        pane
                    }
                    ExtractResult::OnlyPane(pane) => pane,
                    ExtractResult::NotFound => return,
                }
            }
            None => return,
        };

        // Check if source tab is now empty (pane_manager root is None)
        let source_is_empty = self
            .tab_manager
            .get_tab(source_tab_id)
            .is_none_or(|t| t.pane_count() == 0);

        // Create and insert the new tab after the source
        let new_tab_id = self.tab_manager.new_tab_from_pane(
            pane,
            &self.config.load(),
            Arc::clone(&self.runtime),
            if source_is_empty {
                None
            } else {
                Some(source_tab_id)
            },
        );

        // If source was empty, remove it without killing the terminal
        // (the promoted pane still holds an Arc to the same terminal)
        if source_is_empty {
            if let Some(source_tab) = self.tab_manager.get_tab_mut(source_tab_id) {
                source_tab.shutdown_fast = true;
                source_tab.stop_refresh_task();
            }
            let _ = self.tab_manager.remove_tab(source_tab_id);
        }

        // Start refresh tasks for the new tab
        if let Some(window) = &self.window
            && let Some(tab) = self.tab_manager.get_tab_mut(new_tab_id)
        {
            tab.start_refresh_task(
                Arc::clone(&self.runtime),
                Arc::clone(window),
                self.config.load().rendering.max_fps,
                self.config.load().power.inactive_tab_fps,
            );
            tab.start_pane_refresh_tasks(
                Arc::clone(&self.runtime),
                Arc::clone(window),
                self.config.load().rendering.max_fps,
                self.config.load().power.inactive_tab_fps,
            );
        }

        // Resize the new tab's terminal to match renderer dimensions
        if let Some(renderer) = &self.renderer
            && let Some(tab) = self.tab_manager.get_tab_mut(new_tab_id)
        {
            let (cols, rows) = renderer.grid_size();
            let cell_width = renderer.cell_width();
            let cell_height = renderer.cell_height();
            let width_px = (cols as f32 * cell_width) as usize;
            let height_px = (rows as f32 * cell_height) as usize;
            if let Ok(mut term) = tab.terminal.try_write() {
                term.set_cell_dimensions(cell_width as u32, cell_height as u32);
                let _ = term.resize_with_pixels(cols, rows, width_px, height_px);
            }
        }

        // Clear renderer and request redraw
        if let Some(renderer) = &mut self.renderer {
            renderer.clear_all_cells();
        }
        self.focus_state.needs_redraw = true;
        self.request_redraw();

        crate::debug_info!(
            "PANE_PROMOTE",
            "Promoted pane {} to new tab {}",
            focused_pane_id,
            new_tab_id
        );
    }

    /// Start the demote (tab → pane) pick mode.
    pub fn start_demote_tab(&mut self) {
        if self.tab_manager.tab_count() < 2 {
            log::warn!("Cannot demote tab: need at least 2 tabs");
            return;
        }
        if let Some(tab_id) = self.tab_manager.active_tab_id() {
            self.pane_transfer_state = PaneTransferState::DemotePickTab {
                source_tab_id: tab_id,
            };
            self.show_toast("Demote: Click a tab to merge into");
            self.focus_state.needs_redraw = true;
            self.request_redraw();
            crate::debug_info!("TAB_DEMOTE", "Started demote pick mode for tab {}", tab_id);
        }
    }

    /// Cancel the demote pick mode.
    pub fn cancel_pane_transfer(&mut self) {
        self.pane_transfer_state = PaneTransferState::Idle;
        self.show_toast("Demote cancelled");
        self.focus_state.needs_redraw = true;
        self.request_redraw();
    }

    /// Execute the demote: merge source tab's pane tree into target tab.
    pub(crate) fn execute_demote(
        &mut self,
        source_tab_id: TabId,
        target_tab_id: TabId,
        target_pane_id: PaneId,
        direction: SplitDirection,
    ) {
        // Check max_panes on target tab
        let config = self.config.load();
        if config.panes.max_panes > 0 {
            let target_count = self
                .tab_manager
                .get_tab(target_tab_id)
                .map(|t| t.pane_count())
                .unwrap_or(0);
            let source_count = self
                .tab_manager
                .get_tab(source_tab_id)
                .map(|t| t.pane_count())
                .unwrap_or(0);
            if target_count + source_count > config.panes.max_panes {
                log::warn!(
                    "Cannot demote: would exceed max_panes ({})",
                    config.panes.max_panes
                );
                self.cancel_pane_transfer();
                return;
            }
        }
        drop(config);

        // Captured before `take_root` nulls it, so a failed insert can put the
        // source tab back the way the user left it rather than merely intact.
        let source_focus = self
            .tab_manager
            .get_tab(source_tab_id)
            .and_then(|tab| tab.focused_pane_id());

        // Extract the source tab's entire pane tree
        let source_tree = match self.tab_manager.get_tab_mut(source_tab_id) {
            Some(tab) => match tab.pane_manager_mut() {
                Some(pm) => pm.take_root(),
                None => {
                    self.cancel_pane_transfer();
                    return;
                }
            },
            None => {
                self.cancel_pane_transfer();
                return;
            }
        };

        let source_tree = match source_tree {
            Some(tree) => tree,
            None => {
                self.cancel_pane_transfer();
                return;
            }
        };

        // Insert the source tree into the target tab.
        //
        // The remap is discarded because no id that survives a *successful*
        // insert needs translating: `target_pane_id` names a pane of the target
        // tree, which is never renumbered, and the source tab is removed below.
        // `source_focus` is a source-side id held across the call, but it is
        // read only on the failure path, where the subtree comes back carrying
        // the ids it arrived with — so it needs no translation either. A future
        // consumer that carries a subtree-side id into the success path must
        // translate it through the map instead of reusing the pre-move value.
        //
        // The tree is already detached from the source tab by this point, so on
        // failure it must be caught and put back: it is the only reference to
        // those panes, and dropping it kills every terminal in the source tab.
        let insert_result = match self.tab_manager.get_tab_mut(target_tab_id) {
            Some(tab) => match tab.pane_manager_mut() {
                Some(pm) => pm.insert_subtree_at(target_pane_id, source_tree, direction, 0.5),
                None => Err(source_tree),
            },
            None => Err(source_tree),
        };

        if let Err(source_tree) = insert_result {
            self.restore_demoted_tree(source_tab_id, target_pane_id, source_tree, source_focus);
            return;
        }

        // Close the source tab without killing terminals (panes are now in target)
        // Setting shutdown_fast prevents Tab::Drop from killing self.terminal,
        // which is shared with the primary pane that was transplanted into the target.
        if let Some(source_tab) = self.tab_manager.get_tab_mut(source_tab_id) {
            source_tab.shutdown_fast = true;
            // Also stop the tab-level refresh task so it doesn't poll the
            // now-empty PaneManager after the tab is removed from the list.
            source_tab.stop_refresh_task();
        }
        let _ = self.tab_manager.remove_tab(source_tab_id);

        // Start refresh tasks for all panes in the target tab
        // (this also updates is_active on transplanted panes)
        if let Some(window) = &self.window
            && let Some(tab) = self.tab_manager.get_tab_mut(target_tab_id)
        {
            tab.start_pane_refresh_tasks(
                Arc::clone(&self.runtime),
                Arc::clone(window),
                self.config.load().rendering.max_fps,
                self.config.load().power.inactive_tab_fps,
            );
        }

        self.pane_transfer_state = PaneTransferState::Idle;

        if let Some(renderer) = &mut self.renderer {
            renderer.clear_all_cells();
        }
        self.focus_state.needs_redraw = true;
        self.request_redraw();

        crate::debug_info!(
            "TAB_DEMOTE",
            "Demoted tab {} into tab {} at pane {}",
            source_tab_id,
            target_tab_id,
            target_pane_id
        );
    }

    /// Put a demoted pane tree back into its source tab after a failed insert.
    ///
    /// `execute_demote` detaches the source tree before it can know the insert
    /// will succeed, so `source_tree` is by then the only reference to those
    /// panes. Letting it drop closes every one of them and kills their PTYs —
    /// the user's whole tab, without a single error on screen.
    ///
    /// The target pane can genuinely be gone by now: it is picked by a click,
    /// and the direction overlay it opens takes further frames to answer,
    /// during which that pane's shell can exit, another binding can close it,
    /// or a tmux layout update can rebuild the tab's tree.
    ///
    /// Deliberately does not route through `cancel_pane_transfer`, which toasts
    /// "Demote cancelled" — the user did not cancel anything, and a message
    /// saying they did would hide the failure this exists to report.
    fn restore_demoted_tree(
        &mut self,
        source_tab_id: TabId,
        target_pane_id: PaneId,
        source_tree: crate::pane::PaneNode,
        source_focus: Option<PaneId>,
    ) {
        let pane_count = source_tree.pane_count();

        let restored = if let Some(pm) = self
            .tab_manager
            .get_tab_mut(source_tab_id)
            .and_then(|tab| tab.pane_manager_mut())
        {
            pm.set_root(source_tree);
            // `set_root` leaves focus alone, and `take_root` nulled it.
            if let Some(id) = source_focus {
                pm.focus_pane(id);
            }
            true
        } else {
            false
        };

        self.pane_transfer_state = PaneTransferState::Idle;

        if restored {
            log::warn!(
                "Demote aborted: target pane {} no longer exists; \
                 returned {} pane(s) to tab {}",
                target_pane_id,
                pane_count,
                source_tab_id
            );
            self.show_toast("Demote failed: target pane is gone — tab left unchanged");
        } else {
            // Unreachable in practice: the tree was taken out of this same tab
            // a few statements ago and nothing between can remove it. Reported
            // rather than swallowed, because reaching it means the panes really
            // were lost. Re-homing them into a fresh tab was considered and is
            // not possible here: `new_tab_from_pane` takes a single `Pane`, and
            // this is a whole `PaneNode` tree.
            log::error!(
                "Demote aborted and tab {} vanished; {} pane(s) could not be restored",
                source_tab_id,
                pane_count
            );
            self.show_toast("Demote failed: source tab is gone");
        }

        if let Some(renderer) = &mut self.renderer {
            renderer.clear_all_cells();
        }
        self.focus_state.needs_redraw = true;
        self.request_redraw();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::pane::{Pane, PaneNode};
    use crate::tab::Tab;
    use par_term_terminal::TerminalManager;
    use std::sync::atomic::AtomicBool;
    use tokio::sync::RwLock;

    fn test_runtime() -> Arc<tokio::runtime::Runtime> {
        Arc::new(
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build test runtime"),
        )
    }

    /// A pane whose terminal has no shell spawned, so it runs without a PTY on
    /// every supported platform. `working_directory` carries the marker.
    fn stub_pane(id: PaneId, marker: &str) -> Pane {
        let terminal = TerminalManager::new_with_scrollback(80, 24, 100)
            .expect("stub terminal creation without a shell");
        Pane::new_wrapping_terminal(
            id,
            Arc::new(RwLock::new(terminal)),
            Some(marker.to_string()),
            Arc::new(AtomicBool::new(false)),
        )
    }

    /// A `WindowState` holding two shell-less tabs: source tab 1 with a
    /// two-pane tree, target tab 2 with its stub pane.
    fn window_with_source_and_target() -> WindowState {
        let mut state = WindowState::new(Config::default(), test_runtime());

        let mut source = Tab::new_stub(1, 1);
        let pm = source
            .pane_manager_mut()
            .expect("a stub tab always has a pane manager");
        pm.set_root(PaneNode::split(
            SplitDirection::Vertical,
            0.5,
            PaneNode::leaf(stub_pane(1, "/source/one")),
            PaneNode::leaf(stub_pane(2, "/source/two")),
        ));
        pm.focus_pane(2);

        state.tab_manager.push_tab_for_test(source);
        state.tab_manager.push_tab_for_test(Tab::new_stub(2, 2));
        state
    }

    #[test]
    fn a_demote_onto_a_vanished_pane_does_not_destroy_the_source_tab() {
        // `execute_demote` detaches the source tab's whole tree before it can
        // know the insert will succeed. The target pane is picked by a click
        // and confirmed frames later through the direction overlay, so it can
        // be gone by the time this runs — its shell exits, a binding closes it,
        // a tmux layout update rebuilds the tree. If the tree is dropped on
        // that path, every terminal in the source tab dies silently.
        let mut state = window_with_source_and_target();

        // Held across the call, so terminal liveness is observable regardless
        // of where the tree ends up.
        let kept_terminals: Vec<Arc<RwLock<par_term_terminal::TerminalManager>>> = state
            .tab_manager
            .get_tab(1)
            .and_then(|t| t.pane_manager())
            .expect("source tab has a pane manager")
            .all_panes()
            .iter()
            .map(|p| Arc::clone(&p.terminal))
            .collect();
        let refs_before: Vec<usize> = kept_terminals.iter().map(Arc::strong_count).collect();

        const GONE: PaneId = 999;
        state.execute_demote(1, 2, GONE, SplitDirection::Vertical);

        let refs_after: Vec<usize> = kept_terminals.iter().map(Arc::strong_count).collect();
        assert_eq!(
            refs_after, refs_before,
            "every source terminal must still be held by its pane, not dropped"
        );

        let source = state
            .tab_manager
            .get_tab(1)
            .expect("the source tab must not be closed by a failed demote");
        assert_eq!(
            source.pane_count(),
            2,
            "both source panes must still be in the source tab"
        );

        let pm = source.pane_manager().expect("source pane manager");
        for (id, marker) in [(1, "/source/one"), (2, "/source/two")] {
            assert_eq!(
                pm.get_pane(id)
                    .and_then(|p| p.working_directory.clone())
                    .as_deref(),
                Some(marker),
                "pane {id} must be the one that was there before the demote"
            );
        }
        assert_eq!(
            pm.focused_pane_id(),
            Some(2),
            "the tab must come back focused where the user left it"
        );

        assert_eq!(
            state.tab_manager.get_tab(2).map(|t| t.pane_count()),
            Some(1),
            "the target tab must be left exactly as it was"
        );

        // A silent no-op is better than data loss but still wrong: the user
        // asked for a merge and did not get one.
        let toast = state
            .overlay_state
            .toast_message
            .as_deref()
            .expect("a failed demote must tell the user");
        assert!(
            toast.contains("Demote failed"),
            "the toast must report a failure, got {toast:?}"
        );
        assert!(
            !toast.contains("cancelled"),
            "the user did not cancel anything, got {toast:?}"
        );
        assert!(
            !state.pane_transfer_state.is_active(),
            "the pick-mode state machine must not be left mid-demote"
        );
    }
}
