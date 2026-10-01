//! Profile identity and auto-switching state for a terminal tab.
//!
//! Groups the profile a tab was opened from (UX.md PR5), the profile an
//! automatic switch applied on top of it and why, the per-tab pin that stops
//! auto-switching (PR6), and the per-tab badge override (B60).

/// Profile identity and auto-switching state for a terminal tab.
#[derive(Default)]
pub(crate) struct TabProfileState {
    /// The profile the tab was opened from, or the one the user explicitly
    /// applied to it (Change Tab Profile). `None` for a plain new tab.
    pub(crate) source_profile_id: Option<crate::profile::ProfileId>,
    /// Profile ID that was auto-applied based on hostname detection
    pub(crate) auto_applied_profile_id: Option<crate::profile::ProfileId>,
    /// Profile ID that was auto-applied based on directory pattern matching
    pub(crate) auto_applied_dir_profile_id: Option<crate::profile::ProfileId>,
    /// Why the auto-applied profile was chosen ("host example.com",
    /// "directory ~/work"), for the tab tooltip and the switch toast.
    pub(crate) auto_reason: Option<String>,
    /// Pinned: automatic profile switching leaves this tab alone (PR6). An
    /// explicit Change Tab Profile pins the tab too ("explicit user
    /// selection wins", PROFILES.md).
    pub(crate) pinned: bool,
    /// Icon from auto-applied profile (displayed in tab bar)
    pub(crate) profile_icon: Option<String>,
    /// Original tab title saved before auto-profile override (restored when profile clears)
    pub(crate) pre_profile_title: Option<String>,
    /// Icon to restore when an auto-applied profile clears or is undone.
    pub(crate) pre_profile_icon: Option<Option<String>>,
    /// Badge format for this tab (a profile's `badge_text`, or a script's
    /// SetBadge). Read by the badge renderer for the active tab, so a badge
    /// follows its tab instead of staying on the window (B60).
    pub(crate) badge_override: Option<String>,
    /// Badge format to restore when an auto-applied profile clears.
    pub(crate) pre_badge_override: Option<Option<String>>,
    /// A profile the user undid on this tab: the rule that matched it does
    /// not re-apply it until the rule stops matching (PR6 Undo).
    pub(crate) declined_profile_id: Option<crate::profile::ProfileId>,
    /// Confirmation id of the profile command the last auto-switch queued,
    /// withdrawn if the switch is undone or reverts first.
    pub(crate) pending_command_id: Option<u64>,
    /// Whether current profile was auto-applied due to SSH hostname detection
    pub(crate) ssh_auto_switched: bool,
    /// Terminal update generation the auto-switch rules last read for this
    /// tab while it was in the background (re-read only on new output).
    pub(crate) evaluated_generation: u64,
}

impl TabProfileState {
    /// The profile this tab runs as now: an auto-applied profile over the
    /// source profile.
    pub(crate) fn effective_profile_id(&self) -> Option<crate::profile::ProfileId> {
        self.auto_applied_profile_id
            .or(self.auto_applied_dir_profile_id)
            .or(self.source_profile_id)
    }
}

/// Which rule applied an automatic profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AutoRule {
    /// A hostname pattern (OSC 7, usually over SSH). Outranks directory.
    Host,
    /// A directory pattern.
    Directory,
}

/// The profile fields an automatic switch applies to a tab.
#[derive(Debug, Clone, Default)]
pub(crate) struct AutoApply {
    pub(crate) profile_id: crate::profile::ProfileId,
    /// Title to set (the profile's `tab_name`, else its name).
    pub(crate) title: String,
    pub(crate) icon: Option<String>,
    pub(crate) badge_text: Option<String>,
    /// "host example.com" / "directory ~/work".
    pub(crate) reason: String,
}

/// What a tab must do to its title after a state change. Title lives on
/// `Tab` (it syncs the focused pane), so the state hands it back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TitleChange {
    Keep,
    Set(String),
}

impl TabProfileState {
    /// Apply an automatic profile to this tab's state, saving the title,
    /// icon, and badge it overrides on the first switch so a revert
    /// restores them. `current_title` is the tab's title now.
    pub(crate) fn apply_auto(
        &mut self,
        rule: AutoRule,
        apply: AutoApply,
        current_title: &str,
    ) -> TitleChange {
        if self.pre_profile_title.is_none() {
            self.pre_profile_title = Some(current_title.to_string());
        }
        if self.pre_profile_icon.is_none() {
            self.pre_profile_icon = Some(self.profile_icon.clone());
        }
        if self.pre_badge_override.is_none() {
            self.pre_badge_override = Some(self.badge_override.clone());
        }
        match rule {
            AutoRule::Host => self.auto_applied_profile_id = Some(apply.profile_id),
            AutoRule::Directory => self.auto_applied_dir_profile_id = Some(apply.profile_id),
        }
        self.auto_reason = Some(apply.reason);
        self.profile_icon = apply.icon;
        if apply.badge_text.is_some() {
            self.badge_override = apply.badge_text;
        }
        TitleChange::Set(apply.title)
    }

    /// Clear every automatic profile, restoring what the first switch
    /// overrode. Returns the title to restore, if one was saved.
    pub(crate) fn revert_auto(&mut self) -> TitleChange {
        self.auto_applied_profile_id = None;
        self.auto_applied_dir_profile_id = None;
        self.auto_reason = None;
        self.ssh_auto_switched = false;
        if let Some(icon) = self.pre_profile_icon.take() {
            self.profile_icon = icon;
        }
        if let Some(badge) = self.pre_badge_override.take() {
            self.badge_override = badge;
        }
        match self.pre_profile_title.take() {
            Some(title) => TitleChange::Set(title),
            None => TitleChange::Keep,
        }
    }
}

/// The tab tooltip's profile line (UX.md PR5): `Profile: X`, or
/// `Profile: X (auto: host example.com)` while an automatic switch is
/// applied. `name_of` resolves a profile id to its live name; a deleted
/// profile drops out rather than showing a stale id.
pub(crate) fn profile_tooltip(
    state: &TabProfileState,
    name_of: impl Fn(crate::profile::ProfileId) -> Option<String>,
) -> Option<String> {
    let auto = state
        .auto_applied_profile_id
        .or(state.auto_applied_dir_profile_id)
        .and_then(&name_of);
    let source = state.source_profile_id.and_then(&name_of);
    let pin = if state.pinned { " · pinned" } else { "" };
    match (auto, source) {
        (Some(auto), _) => {
            let reason = state.auto_reason.as_deref().unwrap_or("rule");
            Some(format!("Profile: {auto} (auto: {reason}){pin}"))
        }
        (None, Some(source)) => Some(format!("Profile: {source}{pin}")),
        (None, None) if state.pinned => Some("Profile: Default · pinned".to_string()),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(id: crate::profile::ProfileId) -> Option<String> {
        if id == uuid::Uuid::from_u128(1) {
            Some("Work".to_string())
        } else if id == uuid::Uuid::from_u128(2) {
            Some("Prod SSH".to_string())
        } else {
            None
        }
    }

    #[test]
    fn the_tooltip_names_the_source_profile() {
        let state = TabProfileState {
            source_profile_id: Some(uuid::Uuid::from_u128(1)),
            ..Default::default()
        };
        assert_eq!(
            profile_tooltip(&state, names).as_deref(),
            Some("Profile: Work")
        );
    }

    #[test]
    fn the_tooltip_names_an_auto_applied_profile_and_its_rule() {
        let state = TabProfileState {
            source_profile_id: Some(uuid::Uuid::from_u128(1)),
            auto_applied_profile_id: Some(uuid::Uuid::from_u128(2)),
            auto_reason: Some("host prod.example.com".to_string()),
            ..Default::default()
        };
        assert_eq!(
            profile_tooltip(&state, names).as_deref(),
            Some("Profile: Prod SSH (auto: host prod.example.com)")
        );
        assert_eq!(state.effective_profile_id(), Some(uuid::Uuid::from_u128(2)));
    }

    #[test]
    fn a_plain_tab_has_no_profile_line_and_a_deleted_profile_drops_out() {
        assert_eq!(profile_tooltip(&TabProfileState::default(), names), None);
        let state = TabProfileState {
            source_profile_id: Some(uuid::Uuid::from_u128(99)),
            ..Default::default()
        };
        assert_eq!(profile_tooltip(&state, names), None);
    }

    #[test]
    fn apply_then_revert_restores_title_icon_and_badge() {
        let mut state = TabProfileState {
            profile_icon: Some("📁".to_string()),
            badge_override: Some("mine".to_string()),
            ..Default::default()
        };
        let change = state.apply_auto(
            AutoRule::Host,
            AutoApply {
                profile_id: uuid::Uuid::from_u128(2),
                title: "Prod".to_string(),
                icon: Some("🔥".to_string()),
                badge_text: Some("PROD".to_string()),
                reason: "host prod".to_string(),
            },
            "zsh",
        );
        assert_eq!(change, TitleChange::Set("Prod".to_string()));
        assert_eq!(state.badge_override.as_deref(), Some("PROD"));
        // A second switch keeps the ORIGINAL saved values.
        state.apply_auto(
            AutoRule::Directory,
            AutoApply {
                profile_id: uuid::Uuid::from_u128(1),
                title: "Work".to_string(),
                ..Default::default()
            },
            "Prod",
        );
        assert_eq!(state.revert_auto(), TitleChange::Set("zsh".to_string()));
        assert_eq!(state.profile_icon.as_deref(), Some("📁"));
        assert_eq!(state.badge_override.as_deref(), Some("mine"));
        assert_eq!(state.effective_profile_id(), None);
        assert_eq!(state.revert_auto(), TitleChange::Keep, "nothing left");
    }

    #[test]
    fn a_pinned_tab_says_so() {
        let state = TabProfileState {
            source_profile_id: Some(uuid::Uuid::from_u128(1)),
            pinned: true,
            ..Default::default()
        };
        assert_eq!(
            profile_tooltip(&state, names).as_deref(),
            Some("Profile: Work · pinned")
        );
    }
}
