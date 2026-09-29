//! Daemon-side pane moves (`break-pane`, `join-pane`, `swap-window`,
//! `move-window`, `resize-pane -Z`) and the client's daemon link: every
//! daemon pane must stay mapped to the tab that mirrors ITS window.
//!
//! Two gaps close here. A window that appears mid-session (`new-window`,
//! `break-pane`, from this client or any other) gets a tab via
//! `%window-add`, but the daemon broadcasts no layout for it, so its panes
//! stayed unmapped and input to that tab fell through to the hidden local
//! shell. And a pane that moves between windows gets a NEW native mirror
//! in its destination tab, which starts blank until re-seeded.

use crate::app::window_state::WindowState;
use par_term_tmux::{TmuxPaneId, TmuxWindowId};

/// The window a `pane-info` reply (`%N @W COLSxROWS [cmd=…]`) places the
/// pane in, with the pane's grid size.
fn parse_pane_info(line: &str) -> Option<(TmuxWindowId, (u16, u16))> {
    let mut fields = line.split_whitespace().skip(1);
    let window = fields.next()?.strip_prefix('@')?.parse().ok()?;
    let (cols, rows) = fields.next()?.split_once('x')?;
    Some((window, (cols.parse().ok()?, rows.parse().ok()?)))
}

impl WindowState {
    /// Map a window that arrived by `%window-add` while attached: refit it
    /// (the refit broadcasts its `%layout-change`, which the layout
    /// consumer maps) and queue a screen seed per pane — a pane moved in
    /// by `break-pane` already has content its new mirror lacks.
    pub(crate) fn adopt_new_mux_window(&mut self, window_id: TmuxWindowId) {
        let Some(transport) = &self.tmux_state.transport else {
            return;
        };
        let Ok(listed) = transport.send_command("list-panes") else {
            return;
        };
        let mut panes: Vec<(TmuxPaneId, (u16, u16))> = Vec::new();
        for pane in listed
            .iter()
            .filter_map(|l| l.trim().strip_prefix('%')?.parse::<TmuxPaneId>().ok())
        {
            if let Ok(info) = transport.send_command(&format!("pane-info -t %{pane}"))
                && let Some((window, size)) = info.first().and_then(|l| parse_pane_info(l))
                && window == window_id
            {
                panes.push((pane, size));
            }
        }
        let Some(&(anchor, pane_size)) = panes.first() else {
            return;
        };
        let (cols, rows) = self
            .renderer
            .as_ref()
            .map(super::mux::mux_client_grid)
            .unwrap_or(pane_size);
        let cmd = match self.renderer.as_ref().map(super::mux::mux_client_cell_px) {
            Some((w, h)) => format!("refresh-client -t %{anchor} -C {cols}x{rows} -p {w}x{h}"),
            None => format!("refresh-client -t %{anchor} -C {cols}x{rows}"),
        };
        // Synchronous, not the fire-and-forget queue: a queued pump can
        // reach the daemon after a later kill-window/close from this
        // client, and its late %layout-change for the then-unmapped window
        // makes the layout fallback resurrect the closed tab.
        if let Err(e) = transport.send_command(&cmd) {
            log::warn!("par-mux: layout pump for window @{window_id} failed: {e}");
        }
        let ids: Vec<TmuxPaneId> = panes.iter().map(|(p, _)| *p).collect();
        self.queue_mux_pane_seeds(&ids);
    }

    /// Promote Pane to Tab on a mux tab: `break-pane -s %N` moves the
    /// focused daemon pane into a new daemon window; `%window-add` then
    /// creates and maps its tab. A local promote here would strand the
    /// daemon pane's mirror in a tab no daemon window backs. Returns
    /// whether the call was consumed (a mux pane was focused).
    pub(crate) fn promote_mux_pane_to_tab(&mut self) -> bool {
        let Some(pane) = self.focused_mux_pane_from_native() else {
            return false;
        };
        if !self
            .tab_manager
            .active_tab()
            .is_some_and(|t| t.has_multiple_panes())
        {
            self.show_toast("Promote: the pane is already the only one in its tab");
            return true;
        }
        self.send_mux_move(&format!("break-pane -s %{pane}"), "promote");
        true
    }

    /// Why Demote Tab to Pane is refused for `source` — `None` when the
    /// demote is allowed. A mux tab can only be joined as a single pane
    /// into another mux tab (`join-pane` moves one pane; a whole split tree
    /// has no single-command form), and a mux pane cannot enter a local tab
    /// (or a local pane a mux tab) without breaking its daemon link.
    pub(crate) fn mux_demote_refusal(
        &self,
        source: crate::tab::TabId,
        target: Option<crate::tab::TabId>,
    ) -> Option<&'static str> {
        let is_mux = |id| self.mux_window_for_tab(id).is_some();
        let source_mux = is_mux(source);
        if source_mux
            && self
                .tab_manager
                .get_tab(source)
                .is_some_and(|t| t.has_multiple_panes())
        {
            return Some("Demote: a par-mux tab with split panes cannot merge — move one pane");
        }
        match target {
            Some(target) if is_mux(target) != source_mux => {
                Some("Demote: par-mux and local tabs cannot merge")
            }
            None if source_mux
                && !self
                    .tab_manager
                    .tabs()
                    .iter()
                    .any(|t| t.id != source && is_mux(t.id)) =>
            {
                Some("Demote: no other par-mux tab to merge into")
            }
            _ => None,
        }
    }

    /// Demote on a mux tab: `join-pane` the source tab's only daemon pane
    /// beside the picked daemon pane. The layout consumers move the mirror
    /// (the source window closes, its tab with it). Returns whether the
    /// call was consumed.
    pub(crate) fn demote_mux_tab(
        &mut self,
        source_tab: crate::tab::TabId,
        target_tab: crate::tab::TabId,
        target_pane: crate::pane::PaneId,
        direction: crate::pane::SplitDirection,
    ) -> bool {
        if self.mux_window_for_tab(source_tab).is_none()
            && self.mux_window_for_tab(target_tab).is_none()
        {
            return false;
        }
        // Also reached for a LOCAL source into a mux target: merging a
        // local PTY subtree into a daemon mirror is dropped by the next
        // layout push, so the refusal must run for that direction too.
        if let Some(reason) = self.mux_demote_refusal(source_tab, Some(target_tab)) {
            self.show_toast(reason);
            return true;
        }
        let source = self
            .tab_manager
            .get_tab(source_tab)
            .and_then(|t| t.pane_manager())
            .and_then(|pm| pm.focused_pane())
            .and_then(|p| self.tmux_state.tmux_pane_in_tab(source_tab, p.id));
        let target = self.tmux_state.tmux_pane_in_tab(target_tab, target_pane);
        let (Some(source), Some(target)) = (source, target) else {
            self.show_toast("Demote: the par-mux panes are not mapped yet");
            return true;
        };
        // par-term's Vertical is side by side (tmux -h); Horizontal stacks.
        let flag = match direction {
            crate::pane::SplitDirection::Vertical => "-h",
            crate::pane::SplitDirection::Horizontal => "-v",
        };
        self.send_mux_move(
            &format!("join-pane -s %{source} -t %{target} {flag}"),
            "demote",
        );
        true
    }

    /// Send a pane-move command; a non-empty reply that is not a new window
    /// id is an `%error` block (e.g. a daemon predating break/join-pane).
    fn send_mux_move(&mut self, cmd: &str, what: &str) {
        let Some(transport) = &self.tmux_state.transport else {
            return;
        };
        match transport.send_command(cmd) {
            Ok(body)
                if body
                    .iter()
                    .all(|l| l.trim().is_empty() || l.trim().starts_with('@')) =>
            {
                log::info!("MUX: {cmd}");
            }
            Ok(body) => {
                let text = body.join("\n");
                log::error!("par-mux {cmd} rejected: {text}");
                self.show_toast(format!("par-mux: {what} failed — {text}"));
            }
            Err(e) => {
                log::error!("par-mux {cmd} failed: {e}");
                self.show_toast(format!("par-mux: {what} failed — {e}"));
            }
        }
    }

    /// Queue a `refresh-client -t %N` screen seed per pane; delivered by
    /// the end-of-poll sweep once the pane's native mirror exists.
    pub(crate) fn queue_mux_pane_seeds(&mut self, panes: &[TmuxPaneId]) {
        let Some(transport) = &self.tmux_state.transport else {
            return;
        };
        for &pane in panes {
            match transport.send_command(&format!("refresh-client -t %{pane}")) {
                Ok(reply) => {
                    let mut bytes = b"\x1b[H\x1b[2J".to_vec();
                    bytes.extend_from_slice(reply.join("\n").as_bytes());
                    self.tmux_state.mux_screen_seeds.insert(pane, bytes);
                }
                Err(e) => log::warn!("par-mux: seed for moved pane %{pane} failed: {e}"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::mux::MuxAttachPending;
    use super::super::mux::tests::{manners_state, socket_path, spawn_daemon};
    use crate::app::window_state::WindowState;
    use std::time::{Duration, Instant};

    fn send(ws: &WindowState, cmd: &str) -> Vec<String> {
        ws.tmux_state
            .transport
            .as_ref()
            .expect("transport")
            .send_command(cmd)
            .unwrap_or_else(|e| panic!("{cmd}: {e}"))
    }

    /// `(pane, window)` for every daemon pane, via `pane-info`.
    fn daemon_panes(ws: &WindowState) -> Vec<(u64, u64)> {
        send(ws, "list-panes")
            .iter()
            .filter_map(|l| l.trim().strip_prefix('%')?.parse::<u64>().ok())
            .map(|pane| {
                let info = send(ws, &format!("pane-info -t %{pane}")).join(" ");
                let window = info
                    .split_whitespace()
                    .nth(1)
                    .and_then(|w| w.strip_prefix('@'))
                    .and_then(|w| w.parse().ok())
                    .unwrap_or_else(|| panic!("pane-info %{pane}: {info:?}"));
                (pane, window)
            })
            .collect()
    }

    /// Whether every daemon pane is mapped, and mapped into the tab that
    /// mirrors its own window — the daemon link a move must not break.
    fn link_problems(ws: &WindowState) -> Vec<String> {
        let mut problems = Vec::new();
        for (pane, window) in daemon_panes(ws) {
            let Some(tab) = ws.tmux_state.tmux_sync.get_tab(window) else {
                problems.push(format!("window @{window} (of %{pane}) has no tab"));
                continue;
            };
            match ws.tmux_state.tmux_pane_owner(pane) {
                None => problems.push(format!("%{pane} is unmapped")),
                Some((owner, native)) => {
                    if owner != tab {
                        problems.push(format!(
                            "%{pane} mapped to tab {owner}, its window @{window} is tab {tab}"
                        ));
                    }
                    let exists = ws
                        .tab_manager
                        .get_tab(owner)
                        .and_then(|t| t.pane_manager())
                        .and_then(|pm| pm.get_pane(native))
                        .is_some();
                    if !exists {
                        problems.push(format!("%{pane} maps to missing native pane {native}"));
                    }
                }
            }
        }
        problems
    }

    /// Link problems plus mirror drift: a tab holding a native pane for a
    /// daemon pane that left its window (an orphaned mirror).
    fn all_problems(ws: &WindowState) -> Vec<String> {
        let mut problems = link_problems(ws);
        let panes = daemon_panes(ws);
        for tab in ws.tab_manager.tabs() {
            let Some(window) = ws.tmux_state.tmux_sync.get_window(tab.id) else {
                continue;
            };
            let daemon = panes.iter().filter(|(_, w)| *w == window).count();
            let native = tab.pane_manager().map_or(0, |pm| pm.pane_count());
            if native != daemon {
                problems.push(format!(
                    "tab {} (@{window}) mirrors {native} panes, daemon has {daemon}",
                    tab.id
                ));
            }
        }
        problems
    }

    /// Drain ONLY — no test-side layout pump — until the link is whole:
    /// what a production client sees after a move made anywhere.
    fn drain_until_linked(ws: &mut WindowState, what: &str) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            ws.check_mux_notifications();
            let problems = all_problems(ws);
            if problems.is_empty() {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "{what}: daemon link broken: {problems:?}; owners {:?}; native ids {:?}",
                ws.tmux_state.tmux_pane_owners,
                ws.tab_manager
                    .tabs()
                    .iter()
                    .map(|t| (
                        t.id,
                        ws.tmux_state.tmux_sync.get_window(t.id),
                        t.pane_manager().map(|pm| pm
                            .all_panes()
                            .iter()
                            .map(|p| p.id)
                            .collect::<Vec<_>>())
                    ))
                    .collect::<Vec<_>>()
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn pane_text(ws: &WindowState, pane: u64) -> String {
        let Some((tab, native)) = ws.tmux_state.tmux_pane_owner(pane) else {
            return String::new();
        };
        ws.tab_manager
            .get_tab(tab)
            .and_then(|t| t.pane_manager())
            .and_then(|pm| pm.get_pane(native))
            .and_then(|p| p.terminal.try_read().ok())
            .map(|t| t.export_text())
            .unwrap_or_default()
    }

    fn attached(tag: &str) -> (WindowState, std::path::PathBuf) {
        let path = socket_path(tag);
        spawn_daemon(&path);
        let core_client = par_term_emu_core_rust::mux::MuxClient::connect(&path).expect("connect");
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(Ok(core_client)).unwrap();
        drop(tx);
        let mut ws = manners_state();
        ws.tmux_state.mux_attach_pending = Some(MuxAttachPending {
            name: tag.to_string(),
            rx,
        });
        ws.poll_mux_attach();
        assert!(ws.tmux_state.transport.is_some(), "attach must install");
        // The created session's %window-add waits in the client channel;
        // the drain turns it into a tab and the window-add adoption maps
        // %0 — no test-side pump (the other mux tests' manual
        // handle_tmux_window_add would add a duplicate @0 tab here).
        drain_until_linked(&mut ws, "initial attach");
        (ws, path)
    }

    /// Criterion 3: break/join/reorder/zoom driven daemon-side (as another
    /// client or the CLI would) keep every pane linked to the tab of its
    /// own window, leave no orphaned mirror, and re-seed a moved pane's new
    /// mirror with its screen — all from the daemon's broadcasts alone.
    #[test]
    #[ignore = "needs a fea7bdc+ core: scripts/with-local-core.sh cargo test --lib -- --ignored mux_pane_moves"]
    fn pane_moves_keep_every_pane_linked_to_its_windows_tab() {
        let (mut ws, path) = attached("pane-moves");

        // Two panes in @0; %1 prints a marker (a transformation of the
        // typed text, so the echo of the keystrokes cannot match).
        send(&ws, "split-window -h -t %0");
        drain_until_linked(&mut ws, "split");
        send(
            &ws,
            "send-keys -t %1 -l 'echo MOVED-PANE-MARK | tr A-Z a-z'",
        );
        send(&ws, "send-keys -t %1 Enter");
        let deadline = Instant::now() + Duration::from_secs(15);
        while !send(&ws, "capture-pane -t %1 -p")
            .iter()
            .any(|l| l.trim() == "moved-pane-mark")
        {
            assert!(Instant::now() < deadline, "marker never printed in %1");
            std::thread::sleep(Duration::from_millis(50));
        }

        // Break %1 out into its own window: a new tab, linked, re-seeded.
        let new_window = send(&ws, "break-pane -s %1");
        assert!(
            new_window.iter().any(|l| l.trim().starts_with('@')),
            "break-pane replies the new window id: {new_window:?}"
        );
        drain_until_linked(&mut ws, "break-pane");
        let (tab0, _) = ws.tmux_state.tmux_pane_owner(0).unwrap();
        let (tab1, _) = ws.tmux_state.tmux_pane_owner(1).unwrap();
        assert_ne!(tab0, tab1, "the broken-out pane lives in its own tab");
        let deadline = Instant::now() + Duration::from_secs(10);
        while !pane_text(&ws, 1).contains("moved-pane-mark") {
            assert!(
                Instant::now() < deadline,
                "the broken-out pane's new mirror was never seeded: {:?}",
                pane_text(&ws, 1)
            );
            ws.check_mux_notifications();
            std::thread::sleep(Duration::from_millis(50));
        }

        // Join it back beside %0: its window closes, both panes share a tab.
        let joined = send(&ws, "join-pane -s %1 -t %0 -h");
        assert!(joined.is_empty(), "join-pane accepted: {joined:?}");
        drain_until_linked(&mut ws, "join-pane (last pane of its window)");
        assert_eq!(
            ws.tmux_state.tmux_pane_owner(0).map(|o| o.0),
            ws.tmux_state.tmux_pane_owner(1).map(|o| o.0),
            "joined panes share one tab"
        );

        // A second window; then join ONE of @0's two panes into it — the
        // source window survives, so its mirror must drop the moved pane.
        send(&ws, "new-window");
        drain_until_linked(&mut ws, "new-window");
        let new_pane = daemon_panes(&ws)
            .into_iter()
            .map(|(p, _)| p)
            .max()
            .expect("the new window's pane");
        let joined = send(&ws, &format!("join-pane -s %1 -t %{new_pane} -v"));
        assert!(joined.is_empty(), "partial join accepted: {joined:?}");
        drain_until_linked(&mut ws, "join-pane (source window survives)");

        // Reorder the window list both ways.
        let windows = send(&ws, "list-windows");
        let ids: Vec<u64> = windows
            .iter()
            .filter_map(|l| l.strip_prefix('@')?.split(':').next()?.parse().ok())
            .collect();
        assert_eq!(ids.len(), 2, "two windows: {windows:?}");
        let swapped = send(&ws, &format!("swap-window -s @{} -t @{}", ids[0], ids[1]));
        assert!(swapped.is_empty(), "swap-window accepted: {swapped:?}");
        drain_until_linked(&mut ws, "swap-window");
        let moved = send(&ws, &format!("move-window -s @{} -t 0", ids[0]));
        assert!(moved.is_empty(), "move-window accepted: {moved:?}");
        drain_until_linked(&mut ws, "move-window");

        // Zoom and unzoom a pane: the true tree stays on the wire.
        let zoomed = send(&ws, "resize-pane -Z -t %1");
        assert!(zoomed.is_empty(), "resize-pane -Z accepted: {zoomed:?}");
        drain_until_linked(&mut ws, "zoom");
        send(&ws, "resize-pane -Z -t %1");
        drain_until_linked(&mut ws, "unzoom");

        let _ = std::fs::remove_file(&path);
    }

    /// Promote/demote on mux tabs go daemon-side: promote breaks the
    /// focused pane into its own daemon window (tab), demote joins a
    /// single-pane mux tab's pane into another mux tab, and the refused
    /// shapes (split mux source) never touch the daemon or the local tree.
    #[test]
    #[ignore = "needs a fea7bdc+ core: scripts/with-local-core.sh cargo test --lib -- --ignored mux_pane_moves"]
    fn promote_and_demote_on_mux_tabs_break_and_join_daemon_side() {
        let (mut ws, path) = attached("promote-demote");
        send(&ws, "split-window -h -t %0");
        drain_until_linked(&mut ws, "split");
        let windows_before = send(&ws, "list-windows").len();

        // Demote of a split mux tab is refused before pick mode.
        let tab0 = ws.tmux_state.tmux_pane_owner(0).unwrap().0;
        ws.tab_manager.switch_to(tab0);
        assert!(ws.mux_demote_refusal(tab0, None).is_some());
        ws.start_demote_tab();
        assert!(
            !ws.pane_transfer_state.is_active(),
            "a split mux tab must not enter demote pick mode"
        );

        // Promote the focused pane: break-pane, a new daemon window + tab.
        let focused = ws.focused_mux_pane_from_native().expect("mux pane focused");
        ws.promote_pane_to_tab();
        drain_until_linked(&mut ws, "promote (break-pane)");
        assert_eq!(
            send(&ws, "list-windows").len(),
            windows_before + 1,
            "promote made a daemon window"
        );
        let promoted_tab = ws.tmux_state.tmux_pane_owner(focused).unwrap().0;
        assert_ne!(promoted_tab, tab0, "the promoted pane has its own tab");

        // Demote it back: single-pane mux tab into the other mux tab.
        let other = if focused == 0 { 1 } else { 0 };
        let (target_tab, target_native) = ws.tmux_state.tmux_pane_owner(other).unwrap();
        assert_eq!(ws.mux_demote_refusal(promoted_tab, Some(target_tab)), None);
        ws.execute_demote(
            promoted_tab,
            target_tab,
            target_native,
            crate::pane::SplitDirection::Vertical,
        );
        drain_until_linked(&mut ws, "demote (join-pane)");
        assert_eq!(
            send(&ws, "list-windows").len(),
            windows_before,
            "the joined pane's window closed"
        );
        assert_eq!(
            ws.tmux_state.tmux_pane_owner(focused).map(|o| o.0),
            Some(target_tab),
            "the demoted pane now lives in the target tab"
        );
        assert!(ws.tab_manager.get_tab(promoted_tab).is_none());

        let _ = std::fs::remove_file(&path);
    }

    /// A LOCAL tab demoted into a mux tab is refused (a local PTY subtree
    /// inside a daemon mirror is dropped by the next layout push) and the
    /// local tab's tree is left untouched. Needs no daemon-side move, so it
    /// runs against the pinned core.
    #[test]
    fn demoting_a_local_tab_into_a_mux_tab_is_refused() {
        let (mut ws, path) = attached("demote-local-refused");
        let (mux_tab, mux_native) = ws.tmux_state.tmux_pane_owner(0).unwrap();
        let local_tab = ws
            .tab_manager
            .new_tab(
                &ws.config.load(),
                std::sync::Arc::clone(&ws.runtime),
                false,
                None,
            )
            .expect("local tab");
        ws.tab_manager
            .get_tab_mut(local_tab)
            .unwrap()
            .init_pane_manager();
        let local_panes = |ws: &WindowState| {
            ws.tab_manager
                .get_tab(local_tab)
                .and_then(|t| t.pane_manager())
                .map(|pm| pm.pane_count())
        };
        let before = local_panes(&ws);
        ws.overlay_state.toast_message = None;

        ws.execute_demote(
            local_tab,
            mux_tab,
            mux_native,
            crate::pane::SplitDirection::Vertical,
        );

        assert!(
            ws.overlay_state
                .toast_message
                .as_deref()
                .is_some_and(|t| t.contains("cannot merge")),
            "the refusal is surfaced: {:?}",
            ws.overlay_state.toast_message
        );
        assert!(
            ws.tab_manager.get_tab(local_tab).is_some(),
            "local tab kept"
        );
        assert_eq!(local_panes(&ws), before, "local tree untouched");
        assert_eq!(
            ws.tab_manager
                .get_tab(mux_tab)
                .and_then(|t| t.pane_manager())
                .map(|pm| pm.pane_count()),
            Some(1),
            "the mux mirror gained no local pane"
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn pane_info_reply_parses_window_and_size() {
        assert_eq!(
            super::parse_pane_info("%3 @2 80x24 cmd=enNo"),
            Some((2, (80, 24)))
        );
        assert_eq!(super::parse_pane_info("%3 @2 80x24"), Some((2, (80, 24))));
        assert_eq!(super::parse_pane_info("%3 2 80x24"), None);
        assert_eq!(super::parse_pane_info(""), None);
    }
}
