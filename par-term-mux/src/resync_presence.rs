//! The pane-info presence tail (core 0.72+): `PanePresence` and its
//! parser, split from `resync.rs` when the file crossed the 800-line gate.
//! ENH-045 rides this tail as the `reflow=` token.

use par_term_tmux::TmuxPaneId;

/// The pane-info presence tail (core 0.72): the pane's human and process
/// signal, as far as the daemon reported it. Each field is `None` when the
/// row carries no token (a pre-0.72 daemon, `last_viewed_at` on a pane
/// never viewed, `cpu_ms` on a dead pane) or when the token's value does
/// not parse — a missing or malformed tail never fails the parse (ARC-060).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PanePresence {
    /// Attached clients whose focused window shows this pane
    /// zoomed-or-active (stream mirrors included).
    pub viewer_count: Option<u32>,
    /// Epoch-ms the pane was last viewed: `now` while a viewer holds it,
    /// otherwise the moment the last viewer left; absent when never viewed.
    pub last_viewed_at: Option<u64>,
    /// Cumulative CPU-milliseconds of the pane's child plus its live
    /// descendants; absent for a dead pane, whose child is reaped.
    pub cpu_ms: Option<u64>,
    /// ENH-045: whether the pane terminal runs xterm-style resize reflow
    /// (`reflow=1` in the presence tail). `None` when the token is absent
    /// (pre-0.84 daemon, or the daemon's reflow config off) — a streaming
    /// mirror keeps its legacy reflow then, matching what the daemon does.
    pub xterm_resize_reflow: Option<bool>,
}

/// Parse the pane-info presence tail from a `pane-info -t %N` reply, the
/// same reply [`pane_foreground_command`] reads the `cmd=` token from:
/// one line, `%N @W COLSxROWS [cmd=…] [exited=…]` with zero or more
/// trailing `key=value` tokens. Known tokens (`viewer_count=`,
/// `last_viewed_at=`, `cpu_ms=`, `reflow=`) are consumed; a malformed
/// value degrades
/// to `None` with a warn, its siblings keep theirs; unknown tokens
/// (`last_input_at=`, `viewers=`, `constraint_*=`, …) are tolerated and
/// ignored. `None` means no line names `pane` — an `%error` reply from a
/// daemon without `pane-info` never starts with the pane id, so it reads
/// the same way.
pub fn pane_presence(reply: &[String], pane: TmuxPaneId) -> Option<PanePresence> {
    let prefix = format!("%{pane} ");
    let line = reply.iter().find(|line| line.starts_with(&prefix))?;
    let mut presence = PanePresence::default();
    for token in line.split_whitespace().skip(1) {
        if let Some(value) = token.strip_prefix("viewer_count=") {
            presence.viewer_count = match value.parse() {
                Ok(count) => Some(count),
                Err(e) => {
                    log::warn!("[MUX] unparseable viewer_count token {token:?}: {e}");
                    None
                }
            };
        } else if let Some(value) = token.strip_prefix("last_viewed_at=") {
            presence.last_viewed_at = match value.parse() {
                Ok(ms) => Some(ms),
                Err(e) => {
                    log::warn!("[MUX] unparseable last_viewed_at token {token:?}: {e}");
                    None
                }
            };
        } else if let Some(value) = token.strip_prefix("cpu_ms=") {
            presence.cpu_ms = match value.parse() {
                Ok(ms) => Some(ms),
                Err(e) => {
                    log::warn!("[MUX] unparseable cpu_ms token {token:?}: {e}");
                    None
                }
            };
        } else if let Some(value) = token.strip_prefix("reflow=") {
            presence.xterm_resize_reflow = match value {
                "1" => Some(true),
                "0" => Some(false),
                other => {
                    log::warn!(
                        "[MUX] unparseable reflow token {token:?} (want 0|1, got {other:?})"
                    );
                    None
                }
            };
        }
    }
    Some(presence)
}
