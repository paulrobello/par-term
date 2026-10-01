//! Automatic profile switching (UX.md PR6).
//!
//! Hostname rules (OSC 7, usually over SSH) and directory rules (OSC 7 cwd)
//! apply a matching profile to a tab. The rules:
//!
//! - **Every tab is evaluated**, not only the active one: a background tab
//!   that SSHes somewhere switches too.
//! - **A pinned tab never switches.** Pin profile on the tab context menu
//!   sets it, and an explicit Change Tab Profile pins the tab — explicit
//!   user selection wins (PROFILES.md).
//! - **Every switch is announced** with a toast carrying an Undo button.
//!   Undo reverts the tab and declines that profile until its rule stops
//!   matching, so the next frame does not re-apply it.
//! - **Hostname outranks directory.** A host match replaces a directory
//!   match; a directory match never replaces a host match.
//!
//! The per-tab badge (B60): the switched profile's `badge_text` becomes the
//! tab's `badge_override`, which the badge renderer reads for the active
//! tab; a revert restores what the tab had.
//!
//! Profile commands go through the confirmation queue, targeted at the
//! matched tab (`profile_command`).

use crate::tab::{AutoApply, AutoRule, TabId, TitleChange};

use super::super::window_state::WindowState;
use super::profile_command::ProfileCommand;

/// One tab's rule inputs for this frame.
#[derive(Debug, Clone, Default)]
pub(crate) struct TabRuleInputs {
    /// A hostname change this frame: `Some(Some(host))` remote,
    /// `Some(None)` back to local, `None` unchanged.
    pub(crate) hostname: Option<Option<String>>,
    /// A cwd change this frame.
    pub(crate) cwd: Option<String>,
}

/// What the evaluator decided for one tab.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum AutoDecision {
    Nothing,
    Apply {
        rule: AutoRule,
        profile_id: crate::profile::ProfileId,
        reason: String,
    },
    Revert,
}

/// Decide what one tab's rule inputs do, from the tab's state and the
/// matching profile for each input. Pure, so tests drive it without a PTY.
pub(crate) fn decide(
    state: &crate::tab::TabProfileState,
    inputs: &TabRuleInputs,
    host_match: Option<crate::profile::ProfileId>,
    dir_match: Option<crate::profile::ProfileId>,
    host_switch_enabled: bool,
) -> AutoDecision {
    if state.pinned {
        return AutoDecision::Nothing;
    }
    // Hostname rules first.
    match &inputs.hostname {
        Some(None) if state.auto_applied_profile_id.is_some() => return AutoDecision::Revert,
        Some(Some(host)) if host_switch_enabled => match host_match {
            Some(id) if Some(id) == state.declined_profile_id => {}
            Some(id) if Some(id) != state.auto_applied_profile_id => {
                return AutoDecision::Apply {
                    rule: AutoRule::Host,
                    profile_id: id,
                    reason: format!("host {host}"),
                };
            }
            Some(_) => {}
            None if state.auto_applied_profile_id.is_some() => return AutoDecision::Revert,
            None => {}
        },
        _ => {}
    }
    // A host-applied profile outranks every directory rule.
    if state.auto_applied_profile_id.is_some() {
        return AutoDecision::Nothing;
    }
    if let Some(cwd) = &inputs.cwd {
        return match dir_match {
            Some(id) if Some(id) == state.declined_profile_id => AutoDecision::Nothing,
            Some(id) if Some(id) != state.auto_applied_dir_profile_id => AutoDecision::Apply {
                rule: AutoRule::Directory,
                profile_id: id,
                reason: format!("directory {cwd}"),
            },
            Some(_) => AutoDecision::Nothing,
            None if state.auto_applied_dir_profile_id.is_some() => AutoDecision::Revert,
            None => AutoDecision::Nothing,
        };
    }
    AutoDecision::Nothing
}

impl WindowState {
    /// Evaluate automatic profile switching for every tab (UX.md PR6).
    /// Returns true when any tab changed, triggering a redraw.
    pub fn check_auto_profile_switch(&mut self) -> bool {
        if self.overlay_ui.profile_manager.is_empty() {
            return false;
        }
        let host_switch_enabled = self.config.load().ssh.ssh_auto_profile_switch;
        let active = self.tab_manager.active_tab_id();
        let tab_ids: Vec<TabId> = self.tab_manager.tabs().iter().map(|t| t.id).collect();
        let mut changed = false;
        for tab_id in tab_ids {
            let Some(tab) = self.tab_manager.get_tab_mut(tab_id) else {
                continue;
            };
            // Tmux gateway tabs switch through the gateway's session rule.
            if tab.tmux.tmux_gateway_active {
                continue;
            }
            // A background tab is re-read only after new output: its OSC 7
            // cannot change otherwise, and the reads below lock its PTY
            // session on the event loop. try_read: contention skips a frame.
            if Some(tab_id) != active {
                let Some(generation) = tab.terminal.try_read().ok().map(|t| t.update_generation())
                else {
                    continue;
                };
                if generation == tab.profile.evaluated_generation {
                    continue;
                }
                tab.profile.evaluated_generation = generation;
            }
            let inputs = TabRuleInputs {
                hostname: tab.check_hostname_change().map(Some).or_else(|| {
                    // `check_hostname_change` reports only remote hosts; a
                    // return to localhost reads as no hostname with a
                    // host-applied profile still set.
                    (tab.detected_hostname.is_none()
                        && tab.profile.auto_applied_profile_id.is_some())
                    .then_some(None)
                }),
                cwd: tab.check_cwd_change(),
            };
            let manager = &self.overlay_ui.profile_manager;
            let host_match = match &inputs.hostname {
                Some(Some(host)) => manager.find_by_hostname(host).map(|p| p.id),
                _ => None,
            };
            let dir_match = inputs
                .cwd
                .as_deref()
                .and_then(|cwd| manager.find_by_directory(cwd))
                .map(|p| p.id);
            // A declined profile is honoured only while its rule matches.
            if tab.profile.declined_profile_id.is_some()
                && (inputs.hostname.is_some() || inputs.cwd.is_some())
                && tab.profile.declined_profile_id != host_match
                && tab.profile.declined_profile_id != dir_match
            {
                tab.profile.declined_profile_id = None;
            }
            match decide(
                &tab.profile,
                &inputs,
                host_match,
                dir_match,
                host_switch_enabled,
            ) {
                AutoDecision::Nothing => {}
                AutoDecision::Revert => {
                    self.revert_auto_profile(tab_id);
                    changed = true;
                }
                AutoDecision::Apply {
                    rule,
                    profile_id,
                    reason,
                } => {
                    self.apply_auto_profile(tab_id, rule, profile_id, reason);
                    changed = true;
                }
            }
        }
        changed
    }

    /// Apply `profile_id` to tab `tab_id` as an automatic switch: tab state,
    /// title, per-tab badge, the profile command (targeted, confirmed), and
    /// the toast with Undo.
    pub(crate) fn apply_auto_profile(
        &mut self,
        tab_id: TabId,
        rule: AutoRule,
        profile_id: crate::profile::ProfileId,
        reason: String,
    ) {
        let Some(profile) = self.overlay_ui.profile_manager.get(&profile_id).cloned() else {
            return;
        };
        let is_active = self.tab_manager.active_tab_id() == Some(tab_id);
        let Some(tab) = self.tab_manager.get_tab_mut(tab_id) else {
            return;
        };
        if let Some(stale) = tab.profile.pending_command_id.take() {
            self.withdraw_profile_command(stale);
        }
        let Some(tab) = self.tab_manager.get_tab_mut(tab_id) else {
            return;
        };
        let title = profile
            .tab_name
            .clone()
            .unwrap_or_else(|| profile.name.clone());
        let change = tab.profile.apply_auto(
            rule,
            AutoApply {
                profile_id,
                title,
                icon: profile.icon.clone(),
                badge_text: profile.badge_text.clone(),
                reason: reason.clone(),
            },
            &tab.title.clone(),
        );
        if let TitleChange::Set(title) = change {
            tab.set_title(&title);
        }
        if rule == AutoRule::Host {
            tab.profile.ssh_auto_switched = true;
        }
        crate::debug_info!(
            "PROFILE",
            "Auto-applied profile '{}' to tab {} ({})",
            profile.name,
            tab_id,
            reason
        );
        let command_id = self.queue_profile_command(
            ProfileCommand {
                profile_id,
                profile_name: &profile.name,
                command: profile.command.as_deref(),
                command_args: profile.command_args.as_deref(),
                remote_origin: profile.source.is_dynamic(),
                match_reason: &reason,
            },
            Some(tab_id),
        );
        if let Some(tab) = self.tab_manager.get_tab_mut(tab_id) {
            tab.profile.pending_command_id = command_id;
        }
        if is_active {
            self.apply_profile_badge(&profile);
        }
        self.post_toast(
            crate::app::overlay::toast::ToastKind::Info,
            format!("Profile {} applied ({reason})", profile.name),
            Some(crate::app::overlay::toast::ToastAction {
                label: "Undo".to_string(),
                action_id: format!("{UNDO_PROFILE_SWITCH}{tab_id}"),
            }),
        );
        log::info!(
            "Auto-applied profile '{}' to tab {tab_id} ({reason})",
            profile.name
        );
    }

    /// Revert tab `tab_id`'s automatic profile, restoring its title, icon,
    /// badge, and withdrawing a still-pending profile command.
    pub(crate) fn revert_auto_profile(&mut self, tab_id: TabId) {
        let Some(tab) = self.tab_manager.get_tab_mut(tab_id) else {
            return;
        };
        let pending = tab.profile.pending_command_id.take();
        if let TitleChange::Set(title) = tab.profile.revert_auto() {
            tab.set_title(&title);
        }
        let source = tab.profile.source_profile_id;
        crate::debug_info!("PROFILE", "Reverted auto-applied profile on tab {tab_id}");
        if let Some(id) = pending {
            self.withdraw_profile_command(id);
        }
        if self.tab_manager.active_tab_id() == Some(tab_id) {
            self.sync_active_tab_profile(source);
        }
    }

    /// The toast's Undo (`undo_profile_switch:<tab id>`): revert the tab and
    /// decline the profile until its rule stops matching.
    pub(crate) fn undo_profile_switch(&mut self, tab_id: TabId) -> bool {
        let Some(tab) = self.tab_manager.get_tab_mut(tab_id) else {
            return false;
        };
        let undone = tab
            .profile
            .auto_applied_profile_id
            .or(tab.profile.auto_applied_dir_profile_id);
        if undone.is_none() {
            return false;
        }
        tab.profile.declined_profile_id = undone;
        self.revert_auto_profile(tab_id);
        self.request_redraw();
        true
    }

    /// Re-point the window-wide profile surfaces (badge settings,
    /// `session.profile_name`) at the active tab's profile after it changed
    /// or reverted. `None` restores the configured badge.
    pub(crate) fn sync_active_tab_profile(&mut self, profile: Option<crate::profile::ProfileId>) {
        let effective = self
            .tab_manager
            .active_tab()
            .and_then(|t| t.profile.effective_profile_id())
            .or(profile);
        match effective.and_then(|id| self.overlay_ui.profile_manager.get(&id).cloned()) {
            Some(profile) => self.apply_profile_badge(&profile),
            None => {
                let config = self.config.load_full();
                self.badge_state.update_config(&config);
                self.badge_state
                    .set_format(config.badge.badge_format.clone());
                self.badge_state.variables_mut().profile_name = "Default".to_string();
                self.badge_state.mark_dirty();
            }
        }
    }
}

/// The toast Undo action prefix; the rest is the tab id.
pub(crate) const UNDO_PROFILE_SWITCH: &str = "undo_profile_switch:";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tab::TabProfileState;

    fn host(h: &str) -> TabRuleInputs {
        TabRuleInputs {
            hostname: Some(Some(h.to_string())),
            cwd: None,
        }
    }

    fn dir(d: &str) -> TabRuleInputs {
        TabRuleInputs {
            hostname: None,
            cwd: Some(d.to_string()),
        }
    }

    const P1: uuid::Uuid = uuid::Uuid::from_u128(1);
    const P2: uuid::Uuid = uuid::Uuid::from_u128(2);

    #[test]
    fn a_host_match_applies_with_its_reason() {
        let d = decide(
            &TabProfileState::default(),
            &host("prod"),
            Some(P1),
            None,
            true,
        );
        assert_eq!(
            d,
            AutoDecision::Apply {
                rule: AutoRule::Host,
                profile_id: P1,
                reason: "host prod".to_string()
            }
        );
    }

    #[test]
    fn a_pinned_tab_does_not_auto_switch() {
        // UX.md MP3 acceptance: a pinned tab does not auto-switch.
        let pinned = TabProfileState {
            pinned: true,
            ..Default::default()
        };
        assert_eq!(
            decide(&pinned, &host("prod"), Some(P1), None, true),
            AutoDecision::Nothing
        );
        assert_eq!(
            decide(&pinned, &dir("/work"), None, Some(P1), true),
            AutoDecision::Nothing
        );
    }

    #[test]
    fn host_switching_off_leaves_hosts_alone_but_not_directories() {
        let s = TabProfileState::default();
        assert_eq!(
            decide(&s, &host("prod"), Some(P1), None, false),
            AutoDecision::Nothing
        );
        assert!(matches!(
            decide(&s, &dir("/w"), None, Some(P2), false),
            AutoDecision::Apply { .. }
        ));
    }

    #[test]
    fn a_host_profile_outranks_directory_rules() {
        let s = TabProfileState {
            auto_applied_profile_id: Some(P1),
            ..Default::default()
        };
        assert_eq!(
            decide(&s, &dir("/w"), None, Some(P2), true),
            AutoDecision::Nothing
        );
    }

    #[test]
    fn returning_to_localhost_or_leaving_a_matching_directory_reverts() {
        let host_applied = TabProfileState {
            auto_applied_profile_id: Some(P1),
            ..Default::default()
        };
        let back_home = TabRuleInputs {
            hostname: Some(None),
            cwd: None,
        };
        assert_eq!(
            decide(&host_applied, &back_home, None, None, true),
            AutoDecision::Revert
        );
        let dir_applied = TabProfileState {
            auto_applied_dir_profile_id: Some(P2),
            ..Default::default()
        };
        assert_eq!(
            decide(&dir_applied, &dir("/elsewhere"), None, None, true),
            AutoDecision::Revert
        );
    }

    #[test]
    fn an_undone_profile_is_not_reapplied_while_its_rule_matches() {
        let declined = TabProfileState {
            declined_profile_id: Some(P1),
            ..Default::default()
        };
        assert_eq!(
            decide(&declined, &host("prod"), Some(P1), None, true),
            AutoDecision::Nothing
        );
        // A different profile still applies.
        assert!(matches!(
            decide(&declined, &host("other"), Some(P2), None, true),
            AutoDecision::Apply { profile_id: P2, .. }
        ));
    }

    #[test]
    fn the_same_profile_is_not_reapplied() {
        let s = TabProfileState {
            auto_applied_profile_id: Some(P1),
            ..Default::default()
        };
        assert_eq!(
            decide(&s, &host("prod"), Some(P1), None, true),
            AutoDecision::Nothing
        );
    }
}
