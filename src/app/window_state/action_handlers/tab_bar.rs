//! Tab bar action handlers.
//!
//! Contains [`WindowState::handle_tab_bar_action_after_render`], dispatching
//! all [`TabBarAction`] variants produced during egui rendering.

use crate::app::window_state::WindowState;
use crate::tab_bar_ui::TabBarAction;

impl WindowState {
    /// Handle tab bar actions collected during egui rendering (called after renderer borrow released).
    pub(crate) fn handle_tab_bar_action_after_render(
        &mut self,
        action: crate::tab_bar_ui::TabBarAction,
    ) {
        // Handle tab bar actions collected during egui rendering
        // (done here to avoid borrow conflicts with renderer)
        match action {
            TabBarAction::SwitchTo(id) => {
                // Check if we're in demote pick-tab mode
                if let crate::app::tab_ops::pane_transfer::PaneTransferState::DemotePickTab {
                    source_tab_id,
                } = &self.pane_transfer_state
                {
                    if id == *source_tab_id {
                        // Reject demote to self — just switch normally
                        self.tab_manager.switch_to(id);
                        self.clear_and_invalidate();
                        return;
                    }
                    let source = *source_tab_id;
                    self.pane_transfer_state =
                        crate::app::tab_ops::pane_transfer::PaneTransferState::DemotePickPane {
                            source_tab_id: source,
                            target_tab_id: id,
                        };
                    self.tab_manager.switch_to(id);
                    self.clear_and_invalidate();
                    return;
                }
                // Normal switch
                self.tab_manager.switch_to(id);
                // Clear renderer cells and invalidate cache to ensure clean switch
                self.clear_and_invalidate();
                self.after_user_tab_switch();
            }
            TabBarAction::Close(id) => {
                // Switch to the tab first so close_current_tab() operates on it.
                // This routes through the full close path: running-jobs confirmation,
                // session undo capture, and preserve-shell logic.
                let prev_active = self.tab_manager.active_tab_id();
                self.tab_manager.switch_to(id);
                let was_last = self.close_current_tab();
                if was_last {
                    self.is_shutting_down = true;
                } else if let Some(prev) = prev_active
                    && prev != id
                    && self.tab_manager.get_tab(prev).is_some()
                {
                    // UX.md B13/TW1: closing a background tab must not move
                    // the user — focus returns to the tab they were on. The
                    // id-addressed close dialogs stay valid: they target the
                    // tab, not the active tab.
                    self.tab_manager.switch_to(prev);
                }
                self.request_redraw();
            }
            TabBarAction::NewTab => {
                self.new_tab();
                self.request_redraw();
            }
            TabBarAction::SetColor(id, color) => {
                if let Some(tab) = self.tab_manager.get_tab_mut(id) {
                    tab.set_custom_color(color);
                    log::info!(
                        "Set custom color for tab {}: RGB({}, {}, {})",
                        id,
                        color[0],
                        color[1],
                        color[2]
                    );
                }
                self.request_redraw();
            }
            TabBarAction::ClearColor(id) => {
                if let Some(tab) = self.tab_manager.get_tab_mut(id) {
                    tab.clear_custom_color();
                    log::info!("Cleared custom color for tab {}", id);
                }
                self.request_redraw();
            }
            TabBarAction::Reorder(id, target_index) => {
                if self.tab_manager.move_tab_to_index(id, target_index) {
                    self.sync_mux_tab_order(id);
                    self.focus_state.needs_redraw = true;
                    self.request_redraw();
                }
            }
            TabBarAction::OpenProfiles => {
                self.open_profile_launcher();
            }
            TabBarAction::ChangeProfile(id) => {
                // Cmd+Enter in the launcher changes the ACTIVE tab.
                self.tab_manager.switch_to(id);
                self.open_profile_launcher();
            }
            TabBarAction::TogglePinProfile(id) => {
                self.tab_manager.switch_to(id);
                self.toggle_tab_profile_pin();
            }
            TabBarAction::RenameTab(id, name) => {
                // A mux tab's name lives daemon-side: forward the rename so
                // the daemon window carries it across detach/reattach and
                // to other attached clients. The local set below still runs
                // (the daemon's %window-renamed echo would set it too, but
                // the UI should not wait for the round-trip). The daemon's
                // rename-window takes the trailing text RAW (no quote
                // stripping) and rejects an empty name, so a blank revert
                // stays local — the daemon keeps its name until the next
                // non-empty rename.
                if let Some(window_id) = self.mux_window_for_tab(id)
                    && !name.is_empty()
                {
                    let sent =
                        self.tmux_state.transport.as_ref().map(|t| {
                            t.send_command(&format!("rename-window -t @{window_id} {name}"))
                        });
                    if let Some(Err(e)) = &sent {
                        log::error!("MUX: rename-window @{window_id} failed: {e}");
                        self.record_mux_error(format!("par-mux: rename-window failed — {e}"));
                    }
                }
                if let Some(tab) = self.tab_manager.get_tab_mut(id) {
                    if name.is_empty() {
                        // Blank name: revert to auto title mode
                        tab.user_named = false;
                        tab.has_default_title = true;
                        // Reset focused pane so the per-pane loop re-derives its title from scratch
                        if let Some(pane) = tab
                            .pane_manager
                            .as_mut()
                            .and_then(|pm| pm.focused_pane_mut())
                        {
                            pane.title = String::new();
                            pane.has_default_title = true;
                        }
                        // Trigger immediate title update
                        tab.update_title(
                            self.config.load().tabs.tab_title_mode,
                            self.config.load().tabs.remote_tab_title_format,
                            self.config.load().tabs.remote_tab_title_osc_priority,
                        );
                        // A pane with no program title (a par-mux mirror
                        // often has none) derives nothing: fall back to
                        // the tab's default "Tab N" rather than a blank tab.
                        if tab.title.trim().is_empty() {
                            let number = tab.default_number;
                            tab.set_default_title(number);
                        }
                        // UX.md U12: the daemon rejects an empty name, so
                        // clearing a par-mux tab's name sends the auto title
                        // it just re-derived; otherwise the daemon (and the
                        // next reattach) would keep the old user name.
                        let auto_title = tab.title.trim().to_string();
                        if let Some(window_id) = self.mux_window_for_tab(id)
                            && !auto_title.is_empty()
                            && let Some(Err(e)) = self.tmux_state.transport.as_ref().map(|t| {
                                t.send_command(&format!(
                                    "rename-window -t @{window_id} {auto_title}"
                                ))
                            })
                        {
                            log::error!("MUX: rename-window @{window_id} failed: {e}");
                            self.record_mux_error(format!("par-mux: rename-window failed — {e}"));
                        }
                    } else {
                        tab.set_title(&name);
                        tab.user_named = true;
                        // has_default_title = false is already set by set_title()
                    }
                }
                self.request_redraw();
            }
            TabBarAction::Duplicate(id) => {
                self.duplicate_tab_by_id(id);
                self.request_redraw();
            }
            TabBarAction::SetTabIcon(tab_id, icon) => {
                if let Some(tab) = self.tab_manager.get_tab_mut(tab_id) {
                    tab.custom_icon = icon;
                }
                self.request_redraw();
            }
            TabBarAction::None => {}
            TabBarAction::MoveTabToNewWindow(tab_id) => {
                // A mux tab mirrors a daemon window and carries no transport
                // of its own — moving it to another par-term window would
                // strand the mirror. Blocked, not carried.
                if self.mux_window_for_tab(tab_id).is_some() {
                    self.show_toast(
                        "par-mux: tabs attached to a par-mux session can't move between \
                         windows — detach first",
                    );
                    self.request_redraw();
                    return;
                }
                self.overlay_ui.pending_move_tab_request =
                    Some(crate::app::window_manager::MoveTabRequest {
                        tab_id,
                        destination: crate::app::window_manager::MoveDestination::NewWindow,
                    });
            }
            TabBarAction::MoveTabToExistingWindow(tab_id, dest_id) => {
                if self.mux_window_for_tab(tab_id).is_some() {
                    self.show_toast(
                        "par-mux: tabs attached to a par-mux session can't move between \
                         windows — detach first",
                    );
                    self.request_redraw();
                    return;
                }
                self.overlay_ui.pending_move_tab_request =
                    Some(crate::app::window_manager::MoveTabRequest {
                        tab_id,
                        destination: crate::app::window_manager::MoveDestination::ExistingWindow(
                            dest_id,
                        ),
                    });
            }
            TabBarAction::PromotePaneToTab(tab_id) => {
                self.tab_manager.switch_to(tab_id);
                self.promote_pane_to_tab();
                self.request_redraw();
            }
            TabBarAction::DemoteTabToPane(tab_id) => {
                self.tab_manager.switch_to(tab_id);
                self.start_demote_tab();
                self.request_redraw();
            }
        }
    }
}
