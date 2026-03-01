//! Profile auto-switching state for a terminal tab.
//!
//! Groups all fields related to automatic profile switching based on
//! hostname detection, directory patterns, and SSH connections.

/// Profile auto-switching state for a terminal tab.
#[derive(Default)]
pub(crate) struct TabProfileState {
    /// Profile ID that was auto-applied based on hostname detection
    pub(crate) auto_applied_profile_id: Option<crate::profile::ProfileId>,
    /// Profile ID that was auto-applied based on directory pattern matching
    pub(crate) auto_applied_dir_profile_id: Option<crate::profile::ProfileId>,
    /// Icon from auto-applied profile (displayed in tab bar)
    pub(crate) profile_icon: Option<String>,
    /// Original tab title saved before auto-profile override (restored when profile clears)
    pub(crate) pre_profile_title: Option<String>,
    /// Badge text override from auto-applied profile (overrides global badge_format)
    pub(crate) badge_override: Option<String>,
    /// Profile saved before SSH auto-switch (for revert on disconnect)
    pub(crate) pre_ssh_switch_profile: Option<crate::profile::ProfileId>,
    /// Whether current profile was auto-applied due to SSH hostname detection
    pub(crate) ssh_auto_switched: bool,
}
