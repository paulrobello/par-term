//! Confirmation queueing for the script `WriteText` command.
//!
//! `WriteText` types straight into the active PTY. Sanitising it strips escape
//! sequences but not the printable characters and newline that make up a
//! command line, so — as with triggers — the control on this path is the user
//! seeing the text before it runs. The decision logic (id derivation, the
//! pre-approval predicate, the display escaping) lives in
//! `par_term_scripting::confirm` where it is unit-tested; this module is the
//! frontend half that touches `WindowState`.

use par_term_emu_core_rust::terminal::ActionResult;
use par_term_scripting::confirm::{MAX_PENDING_WRITE_TEXT_PROMPTS, describe_write_text};

use super::WindowManager;

impl WindowManager {
    /// Queue a sanitised `WriteText` payload behind the automation confirmation
    /// dialog.
    ///
    /// The write never happens here. The payload is pushed onto
    /// `TriggerState::pending_trigger_actions` as a `SendText` action and
    /// `check_trigger_actions` performs the single write once the user approves,
    /// which is the same execution sink profile auto-switch commands use.
    ///
    /// `text` must already be sanitised: it is both what the dialog shows and
    /// what reaches the PTY, and a dialog that shows one thing while another is
    /// written would be worse than no dialog at all.
    pub(super) fn queue_script_write_text(
        ws: &mut crate::app::window_state::WindowState,
        config_index: usize,
        script_name: &str,
        action_id: u64,
        text: String,
    ) {
        // A script repeating one payload would otherwise stack an identical
        // dialog per event cycle.
        if ws
            .trigger_state
            .pending_trigger_actions
            .iter()
            .any(|pending| pending.trigger_id == action_id)
        {
            return;
        }

        // The dialog resolves one action at a time, so an unbounded queue is a
        // denial of service against the user rather than a safeguard.
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
            "AUDIT Script[{}] WriteText queued for confirmation script={:?} text={:?}",
            config_index,
            script_name,
            text
        );

        let description = format!("Type into the active tab: {}", describe_write_text(&text));

        ws.trigger_state.automation_action_notes.insert(
            action_id,
            format!(
                "Script '{}' asked to type this into the active tab. \
                 Approving writes exactly the text shown above.",
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
            },
        );

        // Script output arrives with no terminal activity behind it, so nothing
        // else would schedule the frame that draws the dialog.
        ws.request_redraw();
    }
}
