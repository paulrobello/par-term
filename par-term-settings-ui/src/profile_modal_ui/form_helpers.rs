//! Private form field helpers for `ProfileModalUI`.
//!
//! Covers: clear_form, load_profile_to_form, form_to_profile, validate_form,
//! start_edit, start_create, save_form, cancel_edit, request_delete,
//! confirm_delete, cancel_delete, move_up, move_down.

use super::{ModalMode, ProfileModalUI};
use par_term_config::{Profile, ProfileId};

impl ProfileModalUI {
    // =========================================================================
    // Form Field Helpers (private)
    // =========================================================================

    /// Clear form fields
    pub(super) fn clear_form(&mut self) {
        self.temp_name.clear();
        self.temp_working_dir.clear();
        self.temp_shell = None;
        self.temp_login_shell = None;
        self.temp_command.clear();
        self.temp_args.clear();
        self.temp_tab_name.clear();
        self.temp_icon.clear();
        self.temp_tags.clear();
        self.temp_parent_id = None;
        self.temp_keyboard_shortcut.clear();
        self.temp_hostname_patterns.clear();
        self.temp_tmux_session_patterns.clear();
        self.temp_directory_patterns.clear();
        self.temp_badge_text.clear();
        self.temp_badge_color = None;
        self.temp_badge_color_alpha = None;
        self.temp_badge_font.clear();
        self.temp_badge_font_bold = None;
        self.temp_badge_top_margin = None;
        self.temp_badge_right_margin = None;
        self.temp_badge_max_width = None;
        self.temp_badge_max_height = None;
        self.temp_ssh_host.clear();
        self.temp_ssh_user.clear();
        self.temp_ssh_port.clear();
        self.temp_ssh_identity_file.clear();
        self.temp_ssh_extra_args.clear();
        self.validation_error = None;
    }

    /// Load a profile into the form
    pub(super) fn load_profile_to_form(&mut self, profile: &Profile) {
        self.temp_name = profile.name.clone();
        self.temp_working_dir = profile.working_directory.clone().unwrap_or_default();
        self.temp_shell = profile.shell.clone();
        self.temp_login_shell = profile.login_shell;
        self.temp_command = profile.command.clone().unwrap_or_default();
        self.temp_args = profile
            .command_args
            .as_ref()
            .map(|args| args.join(" "))
            .unwrap_or_default();
        self.temp_tab_name = profile.tab_name.clone().unwrap_or_default();
        self.temp_icon = profile.icon.clone().unwrap_or_default();
        // New fields
        self.temp_tags = profile.tags.join(", ");
        self.temp_parent_id = profile.parent_id;
        self.temp_keyboard_shortcut = profile.keyboard_shortcut.clone().unwrap_or_default();
        self.temp_hostname_patterns = profile.hostname_patterns.join(", ");
        self.temp_tmux_session_patterns = profile.tmux_session_patterns.join(", ");
        self.temp_directory_patterns = profile.directory_patterns.join(", ");
        self.temp_badge_text = profile.badge_text.clone().unwrap_or_default();
        // Badge appearance settings
        self.temp_badge_color = profile.badge_color;
        self.temp_badge_color_alpha = profile.badge_color_alpha;
        self.temp_badge_font = profile.badge_font.clone().unwrap_or_default();
        self.temp_badge_font_bold = profile.badge_font_bold;
        self.temp_badge_top_margin = profile.badge_top_margin;
        self.temp_badge_right_margin = profile.badge_right_margin;
        self.temp_badge_max_width = profile.badge_max_width;
        self.temp_badge_max_height = profile.badge_max_height;
        // SSH fields
        self.temp_ssh_host = profile.ssh_host.clone().unwrap_or_default();
        self.temp_ssh_user = profile.ssh_user.clone().unwrap_or_default();
        self.temp_ssh_port = profile.ssh_port.map(|p| p.to_string()).unwrap_or_default();
        self.temp_ssh_identity_file = profile.ssh_identity_file.clone().unwrap_or_default();
        self.temp_ssh_extra_args = profile.ssh_extra_args.clone().unwrap_or_default();
    }

    /// Create a profile from form fields
    pub(super) fn form_to_profile(&self, id: ProfileId, order: usize) -> Profile {
        let mut profile = Profile::with_id(id, self.temp_name.trim());
        profile.order = order;

        if !self.temp_working_dir.is_empty() {
            profile.working_directory = Some(self.temp_working_dir.clone());
        }
        profile.shell = self.temp_shell.clone();
        profile.login_shell = self.temp_login_shell;
        if !self.temp_command.is_empty() {
            profile.command = Some(self.temp_command.clone());
        }
        if !self.temp_args.is_empty() {
            // Parse space-separated arguments
            profile.command_args = Some(
                self.temp_args
                    .split_whitespace()
                    .map(String::from)
                    .collect(),
            );
        }
        if !self.temp_tab_name.is_empty() {
            profile.tab_name = Some(self.temp_tab_name.clone());
        }
        if !self.temp_icon.is_empty() {
            profile.icon = Some(self.temp_icon.clone());
        }
        // New fields
        if !self.temp_tags.is_empty() {
            profile.tags = self
                .temp_tags
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
        profile.parent_id = self.temp_parent_id;
        if !self.temp_keyboard_shortcut.is_empty() {
            profile.keyboard_shortcut = Some(self.temp_keyboard_shortcut.clone());
        }
        if !self.temp_hostname_patterns.is_empty() {
            profile.hostname_patterns = self
                .temp_hostname_patterns
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
        if !self.temp_tmux_session_patterns.is_empty() {
            profile.tmux_session_patterns = self
                .temp_tmux_session_patterns
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
        if !self.temp_directory_patterns.is_empty() {
            profile.directory_patterns = self
                .temp_directory_patterns
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
        if !self.temp_badge_text.is_empty() {
            profile.badge_text = Some(self.temp_badge_text.clone());
        }
        // Badge appearance settings
        profile.badge_color = self.temp_badge_color;
        profile.badge_color_alpha = self.temp_badge_color_alpha;
        if !self.temp_badge_font.is_empty() {
            profile.badge_font = Some(self.temp_badge_font.clone());
        }
        profile.badge_font_bold = self.temp_badge_font_bold;
        profile.badge_top_margin = self.temp_badge_top_margin;
        profile.badge_right_margin = self.temp_badge_right_margin;
        profile.badge_max_width = self.temp_badge_max_width;
        profile.badge_max_height = self.temp_badge_max_height;
        // SSH fields
        if !self.temp_ssh_host.is_empty() {
            profile.ssh_host = Some(self.temp_ssh_host.clone());
        }
        if !self.temp_ssh_user.is_empty() {
            profile.ssh_user = Some(self.temp_ssh_user.clone());
        }
        if !self.temp_ssh_port.is_empty() {
            profile.ssh_port = self.temp_ssh_port.parse().ok();
        }
        if !self.temp_ssh_identity_file.is_empty() {
            profile.ssh_identity_file = Some(self.temp_ssh_identity_file.clone());
        }
        if !self.temp_ssh_extra_args.is_empty() {
            profile.ssh_extra_args = Some(self.temp_ssh_extra_args.clone());
        }

        profile
    }

    /// Validate form fields
    pub(super) fn validate_form(&self) -> Option<String> {
        if self.temp_name.trim().is_empty() {
            return Some("Profile name is required".to_string());
        }
        None
    }

    /// Start editing an existing profile
    pub(super) fn start_edit(&mut self, id: ProfileId) {
        if let Some(profile) = self.working_profiles.iter().find(|p| p.id == id).cloned() {
            self.load_profile_to_form(&profile);
            self.editing_id = Some(id);
            self.mode = ModalMode::Edit(id);
        }
    }

    /// Start creating a new profile
    pub(super) fn start_create(&mut self) {
        self.clear_form();
        self.temp_name = "New Profile".to_string();
        let new_id = uuid::Uuid::new_v4();
        self.editing_id = Some(new_id);
        self.mode = ModalMode::Create;
    }

    /// Save the current form (either update existing or create new)
    pub(super) fn save_form(&mut self) {
        if let Some(error) = self.validate_form() {
            self.validation_error = Some(error);
            return;
        }

        if let Some(id) = self.editing_id {
            match &self.mode {
                ModalMode::Create => {
                    let order = self.working_profiles.len();
                    let profile = self.form_to_profile(id, order);
                    self.working_profiles.push(profile);
                    log::info!("Created new profile: {}", self.temp_name);
                }
                ModalMode::Edit(edit_id) => {
                    if let Some(existing) =
                        self.working_profiles.iter().position(|p| p.id == *edit_id)
                    {
                        let order = self.working_profiles[existing].order;
                        let profile = self.form_to_profile(id, order);
                        self.working_profiles[existing] = profile;
                        log::info!("Updated profile: {}", self.temp_name);
                    }
                }
                ModalMode::List => {}
            }
            self.has_changes = true;
        }

        self.mode = ModalMode::List;
        self.editing_id = None;
        self.clear_form();
    }

    /// Cancel editing and return to list view
    pub(super) fn cancel_edit(&mut self) {
        self.mode = ModalMode::List;
        self.editing_id = None;
        self.clear_form();
    }

    /// Request deletion of a profile (shows confirmation)
    pub(super) fn request_delete(&mut self, id: ProfileId, name: String) {
        self.pending_delete = Some((id, name));
    }

    /// Confirm and execute profile deletion
    pub(super) fn confirm_delete(&mut self) {
        if let Some((id, name)) = self.pending_delete.take() {
            self.working_profiles.retain(|p| p.id != id);
            self.has_changes = true;
            if self.selected_id == Some(id) {
                self.selected_id = None;
            }
            log::info!("Deleted profile: {}", name);
        }
    }

    /// Cancel pending deletion
    pub(super) fn cancel_delete(&mut self) {
        self.pending_delete = None;
    }

    /// Move a profile up in the list
    pub(super) fn move_up(&mut self, id: ProfileId) {
        if let Some(pos) = self.working_profiles.iter().position(|p| p.id == id)
            && pos > 0
        {
            self.working_profiles.swap(pos, pos - 1);
            // Update order values
            for (i, p) in self.working_profiles.iter_mut().enumerate() {
                p.order = i;
            }
            self.has_changes = true;
        }
    }

    /// Move a profile down in the list
    pub(super) fn move_down(&mut self, id: ProfileId) {
        if let Some(pos) = self.working_profiles.iter().position(|p| p.id == id)
            && pos < self.working_profiles.len() - 1
        {
            self.working_profiles.swap(pos, pos + 1);
            // Update order values
            for (i, p) in self.working_profiles.iter_mut().enumerate() {
                p.order = i;
            }
            self.has_changes = true;
        }
    }
}
