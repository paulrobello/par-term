//! tmux session profile auto-application for the gateway tab.
//!
//! Extracted from `gateway` to keep session lifecycle I/O separate from
//! profile matching logic.
//!
//! Contains:
//! - `apply_tmux_session_profile` — match session name against configured profiles
//! - `apply_profile_to_gateway_tab` — apply a matched profile's visual settings

use crate::app::window_state::WindowState;

impl WindowState {
    /// Apply a profile based on tmux session name
    ///
    /// This checks for profiles that match the session name pattern and applies
    /// them to the gateway tab. Profile matching uses glob patterns (e.g., "work-*",
    /// "*-production").
    pub(crate) fn apply_tmux_session_profile(&mut self, session_name: &str) {
        // First, check if there's a fixed tmux_profile configured
        if let Some(ref profile_name) = self.config.load().tmux_profile {
            if let Some(profile) = self.overlay_ui.profile_manager.find_by_name(profile_name) {
                let profile_id = profile.id;
                let profile_display = profile.name.clone();
                crate::debug_info!(
                    "TMUX",
                    "Applying configured tmux_profile '{}' for session '{}'",
                    profile_display,
                    session_name
                );
                self.apply_profile_to_gateway_tab(profile_id, &profile_display);
                return;
            } else {
                crate::debug_info!(
                    "TMUX",
                    "Configured tmux_profile '{}' not found",
                    profile_name
                );
            }
        }

        // Then, check for pattern-based matching
        if let Some(profile) = self
            .overlay_ui
            .profile_manager
            .find_by_tmux_session(session_name)
        {
            let profile_id = profile.id;
            let profile_display = profile.name.clone();
            crate::debug_info!(
                "TMUX",
                "Auto-switching to profile '{}' for tmux session '{}'",
                profile_display,
                session_name
            );
            self.apply_profile_to_gateway_tab(profile_id, &profile_display);
        } else {
            crate::debug_info!(
                "TMUX",
                "No profile matches tmux session '{}' - consider adding tmux_session_patterns to a profile",
                session_name
            );
        }
    }

    /// Apply a profile to the tmux gateway tab
    pub(crate) fn apply_profile_to_gateway_tab(
        &mut self,
        profile_id: crate::profile::ProfileId,
        profile_name: &str,
    ) {
        // Extract profile settings before borrowing tab_manager
        let profile_settings = self.overlay_ui.profile_manager.get(&profile_id).map(|p| {
            (
                p.tab_name.clone(),
                p.icon.clone(),
                p.badge_text.clone(),
                p.command.clone(),
                p.command_args.clone(),
                p.source.is_dynamic(),
            )
        });

        if let Some(gateway_tab_id) = self.tmux_state.tmux_gateway_tab_id
            && let Some(tab) = self.tab_manager.get_tab_mut(gateway_tab_id)
        {
            // Mark the auto-applied profile
            tab.profile.auto_applied_profile_id = Some(profile_id);

            let mut pending_command: Option<(String, Option<Vec<String>>, bool)> = None;
            if let Some((tab_name, icon, badge_text, command, command_args, remote_origin)) =
                profile_settings
            {
                // Apply profile icon
                tab.profile.profile_icon = icon;

                // Save original title before overriding (only if not already saved)
                if tab.profile.pre_profile_title.is_none() {
                    tab.profile.pre_profile_title = Some(tab.title.clone());
                }
                // Apply profile tab name (fall back to profile name)
                tab.set_title(&tab_name.unwrap_or_else(|| profile_name.to_string()));

                // Apply badge text override if configured
                if let Some(badge_text) = badge_text {
                    tab.profile.badge_override = Some(badge_text.clone());
                    crate::debug_info!(
                        "TMUX",
                        "Applied badge text '{}' from profile '{}'",
                        badge_text,
                        profile_name
                    );
                }

                // Deferred rather than run here: the tmux session name that selected
                // this profile is pattern-matched, so the command needs the same
                // confirmation the hostname path uses. Dispatched once `tab` is free.
                pending_command = command.map(|cmd| (cmd, command_args, remote_origin));
            }

            if let Some((command, command_args, remote_origin)) = pending_command {
                self.dispatch_profile_command(
                    profile_id,
                    profile_name,
                    Some(&command),
                    command_args.as_deref(),
                    remote_origin,
                    "a tmux session pattern",
                );
            }

            // Show notification about profile switch
            self.show_toast(format!("tmux: Profile '{}' applied", profile_name));
            log::info!(
                "Applied profile '{}' for tmux session (gateway tab {})",
                profile_name,
                gateway_tab_id
            );
        }

        // Apply profile badge settings (color, font, margins, etc.)
        if let Some(profile) = self.overlay_ui.profile_manager.get(&profile_id) {
            let profile_clone = profile.clone();
            self.apply_profile_badge(&profile_clone);
        }
    }
}
