//! Broadcast input (UX.md V5, D8): per tab, with a per-pane opt-out, for
//! keystrokes and pastes, local and attached panes alike.
//!
//! A paste goes through each receiving pane's own paste path rather than
//! one pre-wrapped byte string: bracketed-paste mode is per terminal, and
//! an attached pane's paste is sent daemon-side.

use crate::app::window_state::WindowState;
use crate::pane::PaneId;
use std::sync::Arc;

impl WindowState {
    /// Toggle broadcast for the active tab (I18).
    pub(crate) fn toggle_broadcast_input(&mut self) {
        let Some(tab) = self.tab_manager.active_tab_mut() else {
            return;
        };
        tab.broadcast_input = !tab.broadcast_input;
        let on = tab.broadcast_input;
        log::info!(
            "Broadcast input {} for tab {}",
            if on { "on" } else { "off" },
            tab.id
        );
        self.show_toast(if on {
            "Broadcast Input: ON (this tab)"
        } else {
            "Broadcast Input: OFF"
        });
        self.after_pane_layout_change();
    }

    /// Exclude the focused pane from its tab's broadcast, or include it
    /// again (I21).
    pub(crate) fn toggle_broadcast_for_focused_pane(&mut self) {
        let Some(pane) = self
            .tab_manager
            .active_tab_mut()
            .and_then(|t| t.pane_manager_mut())
            .and_then(|pm| pm.focused_pane_mut())
        else {
            return;
        };
        pane.broadcast_excluded = !pane.broadcast_excluded;
        let excluded = pane.broadcast_excluded;
        self.show_toast(if excluded {
            "This pane no longer receives broadcast input"
        } else {
            "This pane receives broadcast input"
        });
        self.after_pane_layout_change();
    }

    /// The active tab's broadcast receivers, or `None` when broadcast is
    /// off or the tab has a single pane (nothing to fan out to).
    pub(crate) fn broadcast_targets(&self) -> Option<Vec<PaneId>> {
        // A tmux gateway window's panes are mirrors of tmux panes whose
        // input goes through the gateway (send-keys); writing to the mirror
        // terminals would reach the wrong terminal. Gateway tabs keep their
        // single-target routing.
        if self.is_tmux_connected() {
            return None;
        }
        let tab = self.tab_manager.active_tab()?;
        let pm = tab.pane_manager()?;
        if !tab.broadcast_input || !pm.has_multiple_panes() {
            return None;
        }
        Some(tab.broadcast_receivers())
    }

    /// Write `bytes` to every broadcast receiver of the active tab: daemon
    /// panes through the transport, local panes to their PTY. Returns
    /// whether broadcast consumed the key.
    pub(crate) fn broadcast_bytes(&self, bytes: &[u8]) -> bool {
        let Some(targets) = self.broadcast_targets() else {
            return false;
        };
        let Some(tab) = self.tab_manager.active_tab() else {
            return false;
        };
        let Some(pm) = tab.pane_manager() else {
            return false;
        };
        for id in targets {
            if self.route_mux_pane_write(tab.id, id, bytes) {
                continue;
            }
            if let Some(pane) = pm.get_pane(id) {
                let terminal = Arc::clone(&pane.terminal);
                let bytes = bytes.to_vec();
                self.runtime.spawn(async move {
                    let term = terminal.read().await;
                    if let Err(e) = term.write(&bytes) {
                        crate::debug_error!("INPUT", "PTY write failed (broadcast): {e}");
                    }
                });
            }
        }
        true
    }

    /// Paste `text` (already sanitized) into every broadcast receiver of
    /// the active tab. Each pane is wrapped in ITS OWN bracketed-paste
    /// sequences (the mode is per terminal): local panes through
    /// `TerminalManager::paste`, attached panes as one daemon-side burst
    /// built from their mirror terminal. `paste_delay_ms` applies to local
    /// panes; attached panes get the burst form (the delayed mux queue has
    /// one slot per window, which a fan-out would overwrite). Returns
    /// whether broadcast consumed the paste.
    pub(crate) fn broadcast_paste(&mut self, text: &str) -> bool {
        let Some(targets) = self.broadcast_targets() else {
            return false;
        };
        let Some(tab) = self.tab_manager.active_tab() else {
            return false;
        };
        let Some(pm) = tab.pane_manager() else {
            return false;
        };
        let delay_ms = self.config.load().selection.paste_delay_ms;
        for id in targets {
            let Some(pane) = pm.get_pane(id) else {
                continue;
            };
            let (start, end) = pane
                .terminal
                .try_read()
                .map(|t| t.bracketed_paste_sequences())
                .unwrap_or_default();
            let mut burst = start;
            burst.extend_from_slice(text.replace('\n', "\r").as_bytes());
            burst.extend_from_slice(&end);
            if self.route_mux_pane_write(tab.id, id, &burst) {
                continue;
            }
            let terminal = Arc::clone(&pane.terminal);
            let text = text.to_string();
            self.runtime.spawn(async move {
                let term = terminal.read().await;
                if delay_ms > 0 && text.contains('\n') {
                    let _ = term.paste_with_delay(&text, delay_ms).await;
                } else {
                    let _ = term.paste(&text);
                }
            });
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::app::window_state::WindowState;
    use crate::config::Config;
    use crate::pane::{Pane, PaneBounds, PaneNode, SplitDirection};
    use par_term_terminal::TerminalManager;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use tokio::sync::RwLock;

    fn state() -> WindowState {
        let runtime = Arc::new(
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("runtime"),
        );
        WindowState::new(Config::default(), runtime)
    }

    fn stub(id: crate::pane::PaneId) -> Pane {
        let terminal = TerminalManager::new_with_scrollback(20, 5, 0).expect("stub terminal");
        Pane::new_wrapping_terminal(
            id,
            Arc::new(RwLock::new(terminal)),
            None,
            Arc::new(AtomicBool::new(false)),
        )
    }

    /// Two local tabs; the first split into panes 1 | 2.
    fn two_tabs() -> (WindowState, crate::tab::TabId, crate::tab::TabId) {
        let mut ws = state();
        let rt = Arc::clone(&ws.runtime);
        let first = ws
            .tab_manager
            .new_tab(&Config::default(), Arc::clone(&rt), false, None)
            .expect("tab 1");
        let second = ws
            .tab_manager
            .new_tab(&Config::default(), rt, false, None)
            .expect("tab 2");
        ws.tab_manager.switch_to(first);
        let pm = ws
            .tab_manager
            .active_tab_mut()
            .and_then(|t| t.pane_manager_mut())
            .expect("pm");
        pm.set_root(PaneNode::split(
            SplitDirection::Vertical,
            0.5,
            PaneNode::leaf(stub(1)),
            PaneNode::leaf(stub(2)),
        ));
        pm.focus_pane(1);
        pm.set_bounds(PaneBounds::new(0.0, 0.0, 800.0, 400.0));
        (ws, first, second)
    }

    /// Criterion 6 (local): broadcast is per tab, outlines every receiving
    /// pane, badges the tab, and consumes pastes; an opted-out pane drops
    /// out of both the receivers and the outlines.
    #[test]
    fn broadcast_is_per_tab_outlines_panes_badges_the_tab_and_pastes() {
        let (mut ws, first, second) = two_tabs();
        ws.toggle_broadcast_input();

        let tab = ws.tab_manager.get_tab(first).unwrap();
        assert!(tab.broadcast_input);
        assert_eq!(
            tab.pane_mode_badge(),
            Some(crate::tab::pane_badges::BROADCAST_BADGE)
        );
        assert_eq!(
            tab.broadcast_outline_bounds().len(),
            2,
            "both panes outlined"
        );
        let other = ws.tab_manager.get_tab(second).unwrap();
        assert!(!other.broadcast_input, "the other tab is unaffected (D8)");
        assert_eq!(other.pane_mode_badge(), None);
        assert_eq!(ws.broadcast_targets().map(|t| t.len()), Some(2));

        ws.tab_manager.switch_to(second);
        assert_eq!(
            ws.broadcast_targets(),
            None,
            "the other tab does not broadcast"
        );
        ws.tab_manager.switch_to(first);

        assert!(ws.broadcast_paste("hello"), "broadcast consumes the paste");
        assert!(ws.broadcast_bytes(b"x"), "and keystrokes");

        ws.toggle_broadcast_for_focused_pane();
        let tab = ws.tab_manager.get_tab(first).unwrap();
        assert_eq!(tab.broadcast_receivers(), vec![2]);
        assert_eq!(tab.broadcast_outline_bounds().len(), 1);

        ws.toggle_broadcast_input();
        assert_eq!(ws.broadcast_targets(), None, "toggled off");
        assert_eq!(
            ws.tab_manager.get_tab(first).unwrap().pane_mode_badge(),
            None
        );
    }

    /// Criterion 6 (local paste delivery): two panes each running `cat`;
    /// a broadcast paste is echoed into BOTH panes' grids.
    #[test]
    #[ignore = "requires PTY spawn"]
    fn a_broadcast_paste_reaches_every_local_pane() {
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("runtime"),
        );
        let config = Config::default();
        let mut ws = WindowState::new(config.clone(), Arc::clone(&runtime));
        let tab = ws
            .tab_manager
            .new_tab(&config, Arc::clone(&runtime), false, Some((80, 24)))
            .expect("tab");
        ws.tab_manager.switch_to(tab);
        let cat = |id| {
            Pane::new_with_command(
                id,
                &config,
                Arc::clone(&runtime),
                None,
                "cat".to_string(),
                Vec::new(),
            )
            .expect("cat pane")
        };
        let pm = ws
            .tab_manager
            .active_tab_mut()
            .and_then(|t| t.pane_manager_mut())
            .expect("pm");
        pm.set_root(PaneNode::split(
            SplitDirection::Vertical,
            0.5,
            PaneNode::leaf(cat(1)),
            PaneNode::leaf(cat(2)),
        ));
        pm.focus_pane(1);
        ws.toggle_broadcast_input();

        assert!(ws.broadcast_paste("broadcast-paste-mark\n"));

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        for id in [1, 2] {
            loop {
                let text = ws
                    .tab_manager
                    .active_tab()
                    .and_then(|t| t.pane_manager())
                    .and_then(|pm| pm.get_pane(id))
                    .and_then(|p| p.terminal.try_read().ok().map(|t| t.export_text()))
                    .unwrap_or_default();
                if text.contains("broadcast-paste-mark") {
                    break;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "pane {id} never received the paste: {text:?}"
                );
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
    }
}
