//! Cross-window navigation and the real close-window (UX.md A13, A14).
//!
//! These act on the whole `WindowManager`, so the per-window keybinding
//! handlers queue them on the menu bridge and the manager runs them here.

use winit::window::WindowId;

use super::WindowManager;

/// The window after (`step` = 1) or before (`step` = -1) `current` in
/// window-number order, wrapping. `numbered` is `(id, window number)`.
pub(crate) fn cycle_target(
    numbered: &[(WindowId, usize)],
    current: Option<WindowId>,
    step: i8,
) -> Option<WindowId> {
    if numbered.len() < 2 {
        return None;
    }
    let mut ordered = numbered.to_vec();
    ordered.sort_by_key(|(_, n)| *n);
    let pos = current
        .and_then(|c| ordered.iter().position(|(id, _)| *id == c))
        .unwrap_or(0);
    let len = ordered.len() as isize;
    let next = (pos as isize + step as isize).rem_euclid(len) as usize;
    Some(ordered[next].0)
}

/// The par-mux session to attach at launch (UX.md M15): the CLI
/// `--attach` name, else the `mux_auto_attach` config key; `None` when
/// neither is set or both are empty.
pub(crate) fn first_window_mux_attach(cli: Option<&str>, config: Option<&str>) -> Option<String> {
    cli.filter(|n| !n.is_empty())
        .or(config.filter(|n| !n.is_empty()))
        .map(str::to_string)
}

impl WindowManager {
    fn numbered_windows(&self) -> Vec<(WindowId, usize)> {
        self.windows
            .iter()
            .map(|(id, ws)| (*id, ws.window_index))
            .collect()
    }

    /// UX.md M15, once per launch after the first window(s) exist —
    /// whether a restore or a plain launch built them: attach to the
    /// `--attach` session (else `mux_auto_attach`). The target is the
    /// focused window unless a restore is already attaching it, then the
    /// first window that is not; with every window attaching, the restore
    /// wins and nothing is attached twice.
    pub(crate) fn launch_mux_auto_attach(&mut self) {
        if self.mux_auto_attach_done {
            return;
        }
        self.mux_auto_attach_done = true;
        #[cfg(feature = "mux")]
        {
            let config = self.config.load();
            let Some(name) = first_window_mux_attach(
                self.runtime_options.attach.as_deref(),
                config.tmux.mux_auto_attach.as_deref(),
            ) else {
                return;
            };
            let busy = |ws: &crate::app::window_state::WindowState| {
                ws.tmux_state.mux_attach_pending.is_some() || ws.tmux_state.transport.is_some()
            };
            let focused = self.get_focused_window_id();
            let target = focused
                .filter(|id| self.windows.get(id).is_some_and(|ws| !busy(ws)))
                .or_else(|| {
                    let mut free: Vec<_> = self
                        .windows
                        .iter()
                        .filter(|(_, ws)| !busy(ws))
                        .map(|(id, ws)| (*id, ws.window_index))
                        .collect();
                    free.sort_by_key(|(_, n)| *n);
                    free.first().map(|(id, _)| *id)
                });
            match target.and_then(|id| self.windows.get_mut(&id)) {
                Some(ws) => {
                    log::info!("par-mux auto-attach: '{name}'");
                    ws.attach_mux_session_by_name(&name);
                }
                None => log::info!(
                    "par-mux auto-attach '{name}' skipped: every window is already attaching"
                ),
            }
        }
    }

    /// Raise and focus `id`, un-minimizing it first.
    pub(crate) fn focus_window_by_id(&self, id: WindowId) {
        if let Some(window) = self.windows.get(&id).and_then(|ws| ws.window.as_ref()) {
            window.set_minimized(false);
            window.focus_window();
        }
    }

    /// A13 `next_window` / `prev_window`.
    pub(crate) fn cycle_window_focus(&self, focused: Option<WindowId>, step: i8) {
        if let Some(target) = cycle_target(&self.numbered_windows(), focused, step) {
            self.focus_window_by_id(target);
        }
    }

    /// A13 `switch_to_window_N`: the window holding number `n`.
    pub(crate) fn focus_window_number(&self, n: usize) {
        if let Some((id, _)) = self
            .numbered_windows()
            .into_iter()
            .find(|(_, number)| *number == n)
        {
            self.focus_window_by_id(id);
        }
    }

    /// A14 real `close_window`: the whole focused window, all tabs, through
    /// the same confirmation the title-bar close runs (D6). The request is
    /// the window event itself, so the one code path decides.
    pub(crate) fn close_whole_window(&mut self, focused: Option<WindowId>) {
        let Some(id) = focused else {
            return;
        };
        let close_now = self
            .windows
            .get_mut(&id)
            .is_some_and(|ws| ws.request_window_close());
        if close_now {
            self.close_window(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::cycle_target;
    use winit::window::WindowId;

    fn ids() -> (WindowId, WindowId, WindowId) {
        (
            WindowId::from(1u64),
            WindowId::from(2u64),
            WindowId::from(3u64),
        )
    }

    #[test]
    fn cycling_follows_window_numbers_and_wraps() {
        let (a, b, c) = ids();
        // HashMap order is arbitrary; the numbers decide.
        let numbered = [(c, 3), (a, 1), (b, 2)];
        assert_eq!(cycle_target(&numbered, Some(a), 1), Some(b));
        assert_eq!(cycle_target(&numbered, Some(c), 1), Some(a));
        assert_eq!(cycle_target(&numbered, Some(a), -1), Some(c));
    }

    #[test]
    fn the_cli_attach_wins_over_the_config_key() {
        use super::first_window_mux_attach as pick;
        assert_eq!(pick(Some("cli"), Some("cfg")).as_deref(), Some("cli"));
        assert_eq!(pick(None, Some("cfg")).as_deref(), Some("cfg"));
        assert_eq!(pick(Some(""), Some("cfg")).as_deref(), Some("cfg"));
        assert_eq!(pick(None, Some("")), None);
        assert_eq!(pick(None, None), None);
    }

    #[test]
    fn a_single_window_has_nothing_to_cycle_to() {
        let (a, _, _) = ids();
        assert_eq!(cycle_target(&[(a, 1)], Some(a), 1), None);
    }
}
