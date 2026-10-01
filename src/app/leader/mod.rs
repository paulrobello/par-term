//! The par-term leader key (UX.md K4–K9a and the L-table).
//!
//! Pressing the leader (`leader_key`: Cmd+B on macOS, Ctrl+Shift+B
//! elsewhere) arms a one-key table of window, tab, pane, and par-mux
//! session actions. The table is the same in a local tab, an attached
//! par-mux tab, and a tmux gateway tab: every entry dispatches through
//! `execute_keybinding_action`, which already routes each action to the
//! daemon or the local tree.
//!
//! - After `leader_overlay_delay_ms` the which-key overlay lists the table
//!   with each action's live chord ([`which_key`]).
//! - The leader pressed again sends the literal chord to the pane (K8).
//! - Keys marked *repeat* re-arm instead of disarming (K9); Escape, an
//!   unbound key, and `leader_timeout_ms` cancel.
//! - In a tmux gateway tab the tmux prefix (`tmux_prefix_key`) arms the
//!   same state machine, and a key tmux's own prefix table knows goes to
//!   tmux (see [`table::tmux_key`]), so tmux muscle memory carries
//!   over.
//!
//! While armed the leader is the [`crate::app::overlay::OverlayId::Leader`]
//! Mode in the window's overlay stack: the stack hands it every key,
//! Escape included, and its handler here decides.
//!
//! This module is the pure half: state, timing, and the per-key decision,
//! tested without a window. [`window`] applies decisions to a
//! `WindowState`.

pub(crate) mod table;
pub(crate) mod which_key;
mod window;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod window_tests;

pub(crate) use table::TableKey;
pub(crate) use which_key::WhichKey;
pub(crate) use window::LeaderPress;

use par_term_keybindings::KeyCombo;
use std::time::{Duration, Instant};

/// Which chord armed the leader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArmedBy {
    /// `leader_key`.
    Leader,
    /// `tmux_prefix_key`, in a tmux gateway tab.
    TmuxPrefix,
}

/// The two configured durations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LeaderTiming {
    pub(crate) timeout: Duration,
    pub(crate) overlay_delay: Duration,
}

impl LeaderTiming {
    pub(crate) fn from_config(input: &par_term_config::InputConfig) -> Self {
        Self {
            timeout: Duration::from_millis(input.leader_timeout_ms),
            overlay_delay: Duration::from_millis(input.leader_overlay_delay_ms),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Idle,
    Armed {
        by: ArmedBy,
        deadline: Instant,
        overlay_at: Instant,
    },
}

/// One window's leader state.
#[derive(Debug)]
pub(crate) struct LeaderState {
    phase: Phase,
    /// The configured chord string and its parse, re-parsed only when the
    /// string changes (a config reload).
    combo: Option<(String, Option<KeyCombo>)>,
}

impl Default for LeaderState {
    fn default() -> Self {
        Self {
            phase: Phase::Idle,
            combo: None,
        }
    }
}

impl LeaderState {
    pub(crate) fn is_armed(&self) -> bool {
        matches!(self.phase, Phase::Armed { .. })
    }

    pub(crate) fn armed_by(&self) -> Option<ArmedBy> {
        match self.phase {
            Phase::Armed { by, .. } => Some(by),
            Phase::Idle => None,
        }
    }

    pub(crate) fn arm(&mut self, now: Instant, by: ArmedBy, timing: LeaderTiming) {
        self.phase = Phase::Armed {
            by,
            deadline: now + timing.timeout,
            overlay_at: now + timing.overlay_delay,
        };
    }

    pub(crate) fn disarm(&mut self) {
        self.phase = Phase::Idle;
    }

    /// A repeat key ran: stay armed for another full timeout (K9).
    pub(crate) fn refresh(&mut self, now: Instant, timing: LeaderTiming) {
        if let Phase::Armed { deadline, .. } = &mut self.phase {
            *deadline = now + timing.timeout;
        }
    }

    pub(crate) fn expired(&self, now: Instant) -> bool {
        matches!(self.phase, Phase::Armed { deadline, .. } if now >= deadline)
    }

    /// Whether the which-key overlay should be showing.
    pub(crate) fn overlay_due(&self, now: Instant) -> bool {
        matches!(self.phase, Phase::Armed { overlay_at, .. } if now >= overlay_at)
    }

    /// When the event loop must next wake for the leader: the overlay's
    /// delay while it is pending, then the timeout.
    pub(crate) fn next_wake(&self, now: Instant) -> Option<Instant> {
        match self.phase {
            Phase::Armed {
                deadline,
                overlay_at,
                ..
            } if now < overlay_at => Some(overlay_at.min(deadline)),
            Phase::Armed { deadline, .. } => Some(deadline),
            Phase::Idle => None,
        }
    }

    /// The parsed leader chord for `source` (the configured string). An
    /// empty or unparsable string disables the leader.
    pub(crate) fn combo_for(&mut self, source: &str) -> Option<&KeyCombo> {
        if self
            .combo
            .as_ref()
            .is_none_or(|(cached, _)| cached != source)
        {
            let trimmed = source.trim();
            let parsed = if trimmed.is_empty() {
                None
            } else {
                match par_term_keybindings::parser::parse_key_combo(trimmed) {
                    Ok(combo) => Some(combo),
                    Err(e) => {
                        log::warn!("Invalid leader_key '{source}': {e}; the leader is off");
                        None
                    }
                }
            };
            self.combo = Some((source.to_string(), parsed));
        }
        self.combo.as_ref().and_then(|(_, combo)| combo.as_ref())
    }
}

/// One key press as the leader sees it.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct PressFacts {
    pub(crate) pressed: bool,
    /// An OS auto-repeat of a held key.
    pub(crate) os_repeat: bool,
    /// Shift, Ctrl, Alt, or Super alone.
    pub(crate) modifier_only: bool,
    pub(crate) escape: bool,
    /// The press is the leader chord.
    pub(crate) is_leader: bool,
    /// The press is the tmux prefix, in a tmux gateway tab.
    pub(crate) is_tmux_prefix: bool,
    /// The press as an L-table key, when it can be one.
    pub(crate) table_key: Option<TableKey>,
}

/// What the leader does with a key press it owns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LeaderStep {
    Arm(ArmedBy),
    /// Owned and ignored: a release, a modifier, or an auto-repeat.
    Swallow,
    /// The leader again: send the chord to the pane and disarm (K8).
    Literal,
    /// Escape: disarm.
    Cancel,
    /// A key the table does not bind: disarm and say so.
    Unbound,
    /// Run a registry action; a repeat key stays armed (K9).
    Run {
        action: &'static str,
        repeat: bool,
    },
    /// Run the key through tmux's prefix table (gateway tabs).
    Tmux {
        key: TableKey,
        repeat: bool,
    },
}

/// Decide what one key press does. `None` when the leader does not own it
/// (disarmed, and not the leader chord): normal key handling continues.
pub(crate) fn decide(
    armed: bool,
    press: &PressFacts,
    vim_keys: bool,
    tmux_tab: bool,
) -> Option<LeaderStep> {
    if !armed {
        if !press.pressed || press.os_repeat {
            return None;
        }
        if press.is_leader {
            return Some(LeaderStep::Arm(ArmedBy::Leader));
        }
        if press.is_tmux_prefix {
            return Some(LeaderStep::Arm(ArmedBy::TmuxPrefix));
        }
        return None;
    }

    // Holding the leader auto-repeats it; that must not count as the
    // double press that sends it to the pane.
    if !press.pressed || press.modifier_only || press.os_repeat {
        return Some(LeaderStep::Swallow);
    }
    if press.is_leader || press.is_tmux_prefix {
        return Some(LeaderStep::Literal);
    }
    if press.escape {
        return Some(LeaderStep::Cancel);
    }
    let Some(key) = press.table_key else {
        return Some(LeaderStep::Unbound);
    };
    let binding = table::lookup(key, vim_keys);
    if tmux_tab && table::tmux_key(key, vim_keys).is_some() {
        return Some(LeaderStep::Tmux {
            key,
            repeat: binding.is_some_and(|b| b.repeat),
        });
    }
    Some(match binding {
        Some(b) => LeaderStep::Run {
            action: b.action,
            repeat: b.repeat,
        },
        None => LeaderStep::Unbound,
    })
}
