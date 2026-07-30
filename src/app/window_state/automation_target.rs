//! Tab-targeted automation actions.
//!
//! Most producers that queue onto `TriggerState::pending_trigger_actions` are
//! implicitly about the tab the user is looking at: an output trigger fires on
//! the active tab's output, and a profile auto-switch matches on the active
//! tab's hostname or directory. Script `WriteText` is not. A script keeps
//! running in the tab it was started on, so its write belongs to *that* tab
//! whether or not the user is looking at it.
//!
//! The queued action is an `ActionResult` from the external
//! `par-term-emu-core-rust` crate, which carries no tab identity, so the target
//! rides alongside it as an [`AutomationTarget`]. Three rules follow:
//!
//! 1. An approved targeted action is written to its own tab
//!    ([`WindowState::execute_approved_targeted_actions`]) rather than to
//!    whichever tab is active when the user clicks Allow.
//! 2. The dialog names that tab ([`WindowState::pending_action_target_note`]),
//!    so approving a write into a tab the user is not looking at is a visible
//!    choice rather than a surprise.
//! 3. A queued action whose target is gone is discarded, never retargeted
//!    ([`WindowState::prune_orphaned_pending_actions`]).
//!
//! An `AutomationTarget` of `None` on a pending action keeps the original
//! meaning — the active tab, resolved at execution time by
//! `check_trigger_actions`.

use par_term_emu_core_rust::terminal::ActionResult;
use winit::window::WindowId;

use crate::tab::TabId;

use super::WindowState;

/// Longest tab title shown in the confirmation dialog's target line.
const MAX_TARGET_TITLE_CHARS: usize = 48;

/// The tab a queued automation action must be resolved against.
///
/// Tab ids are allocated per window (`TabManager::next_tab_id` starts at 1 in
/// every window), so the window is part of the identity, not decoration.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct AutomationTarget {
    /// The window whose `TabManager` owns `tab_id`.
    pub(crate) window_id: WindowId,
    /// The tab within that window.
    pub(crate) tab_id: TabId,
}

/// The dialog line naming the tab an approved write will land in.
///
/// `index` is zero-based; tabs are numbered from 1 in the UI. Kept separate
/// from the lookup so the wording can be tested without a `TabManager`.
pub(crate) fn describe_write_target(index: usize, title: &str, is_active: bool) -> String {
    let title = title.trim();
    let named = if title.is_empty() {
        format!("tab {}", index + 1)
    } else {
        let mut shown: String = title.chars().take(MAX_TARGET_TITLE_CHARS).collect();
        if shown.chars().count() < title.chars().count() {
            shown.push('…');
        }
        format!("tab {} \"{}\"", index + 1, shown)
    };

    if is_active {
        format!("Writes into {} — the tab you are looking at.", named)
    } else {
        format!("Writes into {} — NOT the tab you are looking at.", named)
    }
}

impl WindowState {
    /// This window's id, once its `Arc<Window>` exists.
    fn automation_window_id(&self) -> Option<WindowId> {
        self.window.as_ref().map(|w| w.id())
    }

    /// Discard queued confirmations whose target tab no longer exists.
    ///
    /// A tab can be closed between the moment a background-tab script queues a
    /// write and the moment the user answers the dialog. Approving then would
    /// either write nothing or — if the id were ever reused — write into an
    /// unrelated terminal, so the prompt is withdrawn instead.
    pub(crate) fn prune_orphaned_pending_actions(&mut self) {
        if self.trigger_state.pending_trigger_actions.is_empty() {
            return;
        }
        // `window` is `None` only before this window's surface exists. Reading
        // that as "every target is gone" would discard legitimate prompts.
        let Some(window_id) = self.automation_window_id() else {
            return;
        };

        let orphaned: Vec<usize> = self
            .trigger_state
            .pending_trigger_actions
            .iter()
            .enumerate()
            .filter(|(_, pending)| match pending.target {
                Some(target) => {
                    target.window_id != window_id
                        || self.tab_manager.get_tab(target.tab_id).is_none()
                }
                None => false,
            })
            .map(|(index, _)| index)
            .collect();

        if orphaned.is_empty() {
            return;
        }

        let head_removed = orphaned.contains(&0);
        for index in orphaned.into_iter().rev() {
            let pending = self.trigger_state.pending_trigger_actions.remove(index);
            self.trigger_state
                .automation_action_notes
                .remove(&pending.trigger_id);
            log::warn!(
                "Automation confirmation '{}' withdrawn: the tab it was aimed at is gone",
                pending.trigger_name
            );
        }

        // The dialog's flicker guard is keyed to whatever sat at the head when
        // it opened. Removing that entry would slide a different action under a
        // guard that has already elapsed, so the click meant for the withdrawn
        // action would resolve its replacement. Re-arm the guard, as the
        // approve/deny path does when it uncovers the next action.
        if head_removed {
            self.trigger_state.trigger_prompt_dialog_open = false;
            self.trigger_state.trigger_prompt_activated_frame = None;
        }
        self.request_redraw();
    }

    /// The dialog's target sentence for the confirmation at the head of the
    /// queue, or `None` when that action targets the active tab implicitly.
    ///
    /// Resolved per frame rather than captured at queue time so the dialog
    /// names the tab as it is *now* — titles track the running command.
    pub(crate) fn pending_action_target_note(&self) -> Option<String> {
        let target = self.trigger_state.pending_trigger_actions.first()?.target?;
        if self.automation_window_id() != Some(target.window_id) {
            return None;
        }

        let index = self
            .tab_manager
            .tabs()
            .iter()
            .position(|tab| tab.id == target.tab_id)?;
        let title = self.tab_manager.tabs()[index].title.as_str();
        let is_active = self.tab_manager.active_tab_id() == Some(target.tab_id);

        Some(describe_write_target(index, title, is_active))
    }

    /// Write dialog-approved actions that name their own tab.
    ///
    /// Separate from `check_trigger_actions`, which resolves everything against
    /// the active tab and gives up early when that tab's terminal lock is
    /// contended. Neither is true of a targeted action.
    pub(crate) fn execute_approved_targeted_actions(&mut self) {
        if self.trigger_state.approved_targeted_actions.is_empty() {
            return;
        }
        // Leave the queue alone until the surface exists: these actions were
        // approved by the user and must not be dropped on a transient `None`.
        let Some(window_id) = self.automation_window_id() else {
            return;
        };

        let approved = std::mem::take(&mut self.trigger_state.approved_targeted_actions);
        for (target, action) in approved {
            let ActionResult::SendText { text, delay_ms, .. } = action else {
                log::error!(
                    "Targeted automation action for tab {} is not SendText; discarding it",
                    target.tab_id
                );
                continue;
            };

            if delay_ms != 0 {
                // No producer queues a delayed targeted write. The delayed path
                // writes from a spawned thread, which would have to re-resolve
                // the target after the sleep; refuse rather than write early.
                log::error!(
                    "Targeted automation write for tab {} requested a {}ms delay, \
                     which this path does not support; discarding it",
                    target.tab_id,
                    delay_ms
                );
                continue;
            }

            if target.window_id != window_id {
                log::warn!(
                    "Approved automation write discarded: it targets window {:?}, \
                     not the window holding it",
                    target.window_id
                );
                continue;
            }

            let Some(tab) = self.tab_manager.get_tab(target.tab_id) else {
                log::warn!(
                    "Approved automation write discarded: tab {} closed before it ran",
                    target.tab_id
                );
                continue;
            };

            // blocking_read: the user approved this write in a dialog, so
            // dropping it on a contended lock would silently discard an
            // explicit decision. Mutating terminal methods take `&self` and
            // serialize internally, so a read lock is the correct one.
            let term = tab.terminal.blocking_read();
            match term.write_str(&text) {
                Ok(()) => crate::debug_info!(
                    "SCRIPT",
                    "AUDIT approved automation write tab={} bytes={}",
                    target.tab_id,
                    text.len()
                ),
                Err(e) => log::error!(
                    "Approved automation write to tab {} failed: {}",
                    target.tab_id,
                    e
                ),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_TARGET_TITLE_CHARS, describe_write_target};

    #[test]
    fn target_line_numbers_tabs_from_one() {
        assert_eq!(
            describe_write_target(0, "zsh", false),
            "Writes into tab 1 \"zsh\" — NOT the tab you are looking at."
        );
        assert_eq!(
            describe_write_target(4, "zsh", false),
            "Writes into tab 5 \"zsh\" — NOT the tab you are looking at."
        );
    }

    #[test]
    fn target_line_distinguishes_the_active_tab() {
        // The whole point of naming the tab is that a background write reads
        // differently from a foreground one.
        let background = describe_write_target(2, "build", false);
        let foreground = describe_write_target(2, "build", true);
        assert_ne!(background, foreground);
        assert!(background.contains("NOT the tab you are looking at"));
        assert!(!foreground.contains("NOT"));
    }

    #[test]
    fn target_line_survives_an_empty_title() {
        let described = describe_write_target(1, "   ", false);
        assert!(described.contains("tab 2"));
        assert!(!described.contains('"'));
    }

    #[test]
    fn a_long_title_is_truncated() {
        let title = "x".repeat(MAX_TARGET_TITLE_CHARS + 20);
        let described = describe_write_target(0, &title, true);
        assert!(described.contains('…'));
        assert!(described.chars().count() < title.chars().count() + 60);
    }
}
