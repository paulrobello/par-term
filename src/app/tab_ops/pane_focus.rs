//! Pane focus, zoom, and restart actions (UX.md A1, A7, A8, A9) and the
//! single user-intent focus hook that keeps a par-mux daemon in step (M7).
//!
//! Every focus change the USER makes — directional navigation, next/prev,
//! last pane, pane-hint letters, a click, hover focus — ends in
//! [`WindowState::after_user_pane_focus`]. On a mux tab that sends
//! `select-pane`, which a second client sees as `%window-pane-changed` and
//! which also unzooms daemon-side (the daemon clears a zoom when another
//! pane is selected). Daemon pushes (`handle_tmux_pane_focus_changed`)
//! never call it, so two clients cannot ping-pong focus.

use crate::app::window_state::WindowState;

impl WindowState {
    /// Toggle zoom on the focused pane (A1). A mux tab zooms daemon-side
    /// (`resize-pane -Z`) and mirrors the `%layout-change`; a local tab
    /// zooms its own tree.
    pub(crate) fn toggle_pane_zoom(&mut self) {
        #[cfg(feature = "mux")]
        if let Some(pane) = self.focused_mux_pane_from_native() {
            self.send_mux_pane_command(&format!("resize-pane -Z -t %{pane}"), "zoom");
            return;
        }
        if self.refuse_in_tmux_gateway("Zoom") {
            return;
        }
        let Some(pm) = self
            .tab_manager
            .active_tab_mut()
            .and_then(|tab| tab.pane_manager_mut())
        else {
            return;
        };
        if pm.pane_count() < 2 {
            self.show_toast("Zoom: the tab has only one pane");
            return;
        }
        pm.toggle_zoom();
        self.after_pane_layout_change();
    }

    /// Focus the next or previous pane in tree order (A7).
    pub(crate) fn focus_pane_cycle(&mut self, forward: bool) {
        let focused = self
            .tab_manager
            .active_tab_mut()
            .and_then(|tab| tab.pane_manager_mut())
            .and_then(|pm| pm.focus_cycle(forward));
        if focused.is_some() {
            self.after_user_pane_focus();
        }
    }

    /// Focus the previously focused pane (A8).
    pub(crate) fn focus_last_pane(&mut self) {
        let focused = self
            .tab_manager
            .active_tab_mut()
            .and_then(|tab| tab.pane_manager_mut())
            .and_then(|pm| pm.focus_last());
        if focused.is_some() {
            self.after_user_pane_focus();
        }
    }

    /// Restart the focused pane's process in place (A9): `respawn-pane -k`
    /// on a mux pane (the daemon reruns the pane's stored command), and on
    /// a local pane the program it was started with, in its current cwd.
    pub(crate) fn restart_focused_pane(&mut self) {
        #[cfg(feature = "mux")]
        if self.restart_focused_mux_pane() {
            return;
        }
        if self.refuse_in_tmux_gateway("Restart pane") {
            return;
        }
        let config = self.config.load_full();
        let Some(pane) = self
            .tab_manager
            .active_tab_mut()
            .and_then(|tab| tab.pane_manager_mut())
            .and_then(|pm| pm.focused_pane_mut())
        else {
            return;
        };
        let result = pane
            .terminal
            .try_write()
            .map_err(|_| anyhow::anyhow!("terminal busy"))
            .and_then(|mut term| {
                if term.is_running() {
                    term.kill()?;
                }
                Ok(())
            })
            .and_then(|()| pane.respawn_shell(&config));
        match result {
            Ok(()) => log::info!("Restarted pane {}", pane.id),
            Err(e) => {
                log::error!("Failed to restart pane {}: {e}", pane.id);
                self.show_toast(format!("Restart failed — {e}"));
            }
        }
        self.focus_state.needs_redraw = true;
        self.request_redraw();
    }

    /// Pane hover focus (PN14, `pane_focus_follows_mouse`): focus the pane
    /// under the pointer. Only a real change runs the focus tail, so moving
    /// inside one pane never re-sends `select-pane` to a daemon. Skipped
    /// while a button is held or a divider is dragged (a drag must not
    /// change panes), while zoomed, in a modal mode or dialog, and in tmux
    /// gateway tabs. Returns whether focus moved.
    pub(crate) fn hover_focus_pane(&mut self, x: f32, y: f32) -> bool {
        if !self.config.load().mouse.pane_focus_follows_mouse
            || self.is_copy_mode_active()
            || self.pane_hint_select.is_active()
            || self.pane_resize_mode.is_active()
            || self.any_modal_ui_visible()
            || self.is_tmux_connected()
        {
            return false;
        }
        let Some(tab) = self.tab_manager.active_tab_mut() else {
            return false;
        };
        let mouse = tab.active_mouse();
        if mouse.button_pressed || mouse.is_selecting || mouse.dragging_divider.is_some() {
            return false;
        }
        let Some(pm) = tab.pane_manager_mut() else {
            return false;
        };
        if !pm.has_multiple_panes() || pm.is_zoomed() {
            return false;
        }
        let Some(target) = pm.visible_pane_at(x, y).map(|p| p.id) else {
            return false;
        };
        if pm.focused_pane_id() == Some(target) {
            return false;
        }
        pm.focus_pane(target);
        self.after_user_pane_focus();
        true
    }

    /// The tail of every user-initiated pane focus change: route input to
    /// the new pane (tmux gateway focus / mux focus + roster seen mark),
    /// tell the daemon (`select-pane`, M7), and redraw.
    pub(crate) fn after_user_pane_focus(&mut self) {
        let focused = self
            .tab_manager
            .active_tab()
            .and_then(|t| t.focused_pane_id());
        if let Some(pane_id) = focused {
            self.set_tmux_focused_pane_from_native(pane_id);
            #[cfg(feature = "mux")]
            if let Some(mux_pane) = self.focused_mux_pane_from_native() {
                self.send_mux_pane_command(&format!("select-pane -t %{mux_pane}"), "focus");
            }
        }
        self.after_pane_layout_change();
    }

    /// The tail of a user-initiated tab switch (M7): on an attached tab,
    /// `select-window` makes its daemon window the session's active one, so
    /// a second client sees `%window-pane-changed` for it. The mux drain's
    /// own tab changes never call this, so two clients cannot ping-pong.
    pub(crate) fn after_user_tab_switch(&mut self) {
        #[cfg(feature = "mux")]
        if let Some(window) = self
            .tab_manager
            .active_tab_id()
            .and_then(|id| self.mux_window_for_tab(id))
        {
            self.send_mux_pane_command(&format!("select-window -t @{window}"), "tab switch");
        }
    }

    /// Local-tree pane operations must not run on a tmux gateway window:
    /// its panes mirror tmux panes, tmux owns the layout, and a local edit
    /// is overwritten (or kills a mirror). Toasts and returns true there.
    pub(crate) fn refuse_in_tmux_gateway(&mut self, what: &str) -> bool {
        if !self.is_tmux_connected() {
            return false;
        }
        self.show_toast(format!("{what} is not available in tmux gateway tabs"));
        true
    }

    /// Redraw after a local layout or focus change.
    pub(crate) fn after_pane_layout_change(&mut self) {
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.active_cache_mut().cells = None;
        }
        self.focus_state.needs_redraw = true;
        self.request_redraw();
    }

    /// Send a pane command whose success reply is an empty body; a
    /// non-empty body is an `%error` block (the same failure shape as
    /// kill-pane and swap-pane). Returns whether the daemon accepted it.
    #[cfg(feature = "mux")]
    pub(crate) fn send_mux_pane_command(&mut self, cmd: &str, what: &str) -> bool {
        let Some(transport) = &self.tmux_state.transport else {
            return false;
        };
        match transport.send_command(cmd) {
            Ok(body) if body.iter().all(|l| l.trim().is_empty()) => {
                crate::debug_info!("MUX", "sent {cmd}");
                true
            }
            Ok(body) => {
                let text = body.join("\n");
                log::error!("par-mux {cmd} rejected: {text}");
                self.record_mux_error(format!("par-mux: {what} failed — {text}"));
                false
            }
            Err(e) => {
                log::error!("par-mux {cmd} failed: {e}");
                self.record_mux_error(format!("par-mux: {what} failed — {e}"));
                false
            }
        }
    }
}
