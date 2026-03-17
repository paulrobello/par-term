//! Automatic profile switching logic for WindowState.
//!
//! Contains hostname-based, SSH-command-based, and directory-based automatic
//! profile switching triggered by OSC 7 / shell-integration events.

use std::sync::Arc;

use super::super::window_state::WindowState;

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

                // Execute profile command in the running shell if configured
                if let Some(cmd) = profile_command {
                    let mut full_cmd = cmd;
                    if let Some(args) = profile_command_args {
                        for arg in args {
                            full_cmd.push(' ');
                            full_cmd.push_str(&arg);
                        }
                    }
                    full_cmd.push('\n');

                    let terminal_clone = Arc::clone(&tab.terminal);
                    self.runtime.spawn(async move {
                        let term = terminal_clone.write().await;
                        if let Err(e) = term.write(full_cmd.as_bytes()) {
                            log::error!("Failed to execute profile command: {}", e);
                        }
                    });
                }
            }

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
            let cmd = if let Ok(term) = tab.terminal.try_write() {
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

                // Execute profile command in the running shell if configured
                if let Some(cmd) = profile_command {
                    let mut full_cmd = cmd;
                    if let Some(args) = profile_command_args {
                        for arg in args {
                            full_cmd.push(' ');
                            full_cmd.push_str(&arg);
                        }
                    }
                    full_cmd.push('\n');

                    let terminal_clone = Arc::clone(&tab.terminal);
                    self.runtime.spawn(async move {
                        let term = terminal_clone.write().await;
                        if let Err(e) = term.write(full_cmd.as_bytes()) {
                            log::error!("Failed to execute profile command: {}", e);
                        }
                    });
                }
            }

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
}
