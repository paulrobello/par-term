//! Profile modal UI state definitions and data management methods.

use crate::profile::{Profile, ProfileId, ProfileManager};

/// Actions that can be triggered from the profile modal
#[derive(Debug, Clone, PartialEq)]
pub enum ProfileModalAction {
    /// No action
    None,
    /// Save changes to profiles and close modal
    Save,
    /// Cancel and discard changes
    Cancel,
    /// Open a profile immediately (after creation)
    OpenProfile(ProfileId),
}

/// Modal display mode
#[derive(Debug, Clone, PartialEq)]
pub(super) enum ModalMode {
    /// Viewing the list of profiles
    List,
    /// Editing an existing profile
    Edit(ProfileId),
    /// Creating a new profile
    Create,
}

/// Profile modal UI state
pub struct ProfileModalUI {
    /// Whether the modal is visible
    pub visible: bool,
    /// Current display mode
    pub(super) mode: ModalMode,
    /// Working copy of profiles being edited
    pub(super) working_profiles: Vec<Profile>,
    /// ID of profile being edited/created
    pub(super) editing_id: Option<ProfileId>,

    // Temporary form fields
    pub(super) temp_name: String,
    pub(super) temp_working_dir: String,
    pub(super) temp_shell: Option<String>,
    pub(super) temp_login_shell: Option<bool>,
    pub(super) temp_command: String,
    pub(super) temp_args: String,
    pub(super) temp_tab_name: String,
    pub(super) temp_icon: String,
    // New fields for enhanced profile system (issue #78)
    pub(super) temp_tags: String,
    pub(super) temp_parent_id: Option<ProfileId>,
    pub(super) temp_keyboard_shortcut: String,
    pub(super) temp_hostname_patterns: String,
    pub(super) temp_tmux_session_patterns: String,
    pub(super) temp_directory_patterns: String,
    pub(super) temp_badge_text: String,
    // Badge appearance settings
    pub(super) temp_badge_color: Option<[u8; 3]>,
    pub(super) temp_badge_color_alpha: Option<f32>,
    pub(super) temp_badge_font: String,
    pub(super) temp_badge_font_bold: Option<bool>,
    pub(super) temp_badge_top_margin: Option<f32>,
    pub(super) temp_badge_right_margin: Option<f32>,
    pub(super) temp_badge_max_width: Option<f32>,
    pub(super) temp_badge_max_height: Option<f32>,
    /// Whether badge settings section is expanded
    pub(super) badge_section_expanded: bool,
    /// Whether SSH settings section is expanded
    pub(super) ssh_section_expanded: bool,
    // SSH temp fields
    pub(super) temp_ssh_host: String,
    pub(super) temp_ssh_user: String,
    pub(super) temp_ssh_port: String,
    pub(super) temp_ssh_identity_file: String,
    pub(super) temp_ssh_extra_args: String,

    /// Selected profile in list view
    pub(super) selected_id: Option<ProfileId>,
    /// Whether there are unsaved changes
    pub(super) has_changes: bool,
    /// Validation error message
    pub(super) validation_error: Option<String>,
    /// Profile pending deletion (for confirmation)
    pub(super) pending_delete: Option<(ProfileId, String)>,
}

impl ProfileModalUI {
    /// Create a new profile modal UI
    pub fn new() -> Self {
        Self {
            visible: false,
            mode: ModalMode::List,
            working_profiles: Vec::new(),
            editing_id: None,
            temp_name: String::new(),
            temp_working_dir: String::new(),
            temp_shell: None,
            temp_login_shell: None,
            temp_command: String::new(),
            temp_args: String::new(),
            temp_tab_name: String::new(),
            temp_icon: String::new(),
            temp_tags: String::new(),
            temp_parent_id: None,
            temp_keyboard_shortcut: String::new(),
            temp_hostname_patterns: String::new(),
            temp_tmux_session_patterns: String::new(),
            temp_directory_patterns: String::new(),
            temp_badge_text: String::new(),
            temp_badge_color: None,
            temp_badge_color_alpha: None,
            temp_badge_font: String::new(),
            temp_badge_font_bold: None,
            temp_badge_top_margin: None,
            temp_badge_right_margin: None,
            temp_badge_max_width: None,
            temp_badge_max_height: None,
            badge_section_expanded: false,
            ssh_section_expanded: false,
            temp_ssh_host: String::new(),
            temp_ssh_user: String::new(),
            temp_ssh_port: String::new(),
            temp_ssh_identity_file: String::new(),
            temp_ssh_extra_args: String::new(),
            selected_id: None,
            has_changes: false,
            validation_error: None,
            pending_delete: None,
        }
    }

    /// Open the modal with current profiles
    pub fn open(&mut self, manager: &ProfileManager) {
        self.visible = true;
        self.mode = ModalMode::List;
        self.working_profiles = manager.to_vec();
        self.editing_id = None;
        self.selected_id = None;
        self.has_changes = false;
        self.validation_error = None;
        self.pending_delete = None;
        self.clear_form();
        log::info!(
            "Profile modal opened with {} profiles",
            self.working_profiles.len()
        );
    }

    /// Close the modal
    pub fn close(&mut self) {
        self.visible = false;
        self.mode = ModalMode::List;
        self.working_profiles.clear();
        self.editing_id = None;
        self.pending_delete = None;
        self.clear_form();
    }

    /// Load profiles into the working copy without toggling visibility.
    ///
    /// Used by the settings window to populate the inline profile editor
    /// without opening a modal window.
    pub fn load_profiles(&mut self, profiles: Vec<Profile>) {
        self.working_profiles = profiles;
        self.mode = ModalMode::List;
        self.editing_id = None;
        self.selected_id = None;
        self.has_changes = false;
        self.validation_error = None;
        self.pending_delete = None;
        self.clear_form();
    }

    /// Get the working profiles (for saving)
    pub fn get_working_profiles(&self) -> &[Profile] {
        &self.working_profiles
    }

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

    /// Check if `ancestor_id` appears in the parent chain of `profile_id`
    pub(super) fn has_ancestor(&self, profile_id: ProfileId, ancestor_id: ProfileId) -> bool {
        let mut current_id = profile_id;
        let mut visited = vec![current_id];
        while let Some(parent_id) = self
            .working_profiles
            .iter()
            .find(|p| p.id == current_id)
            .and_then(|p| p.parent_id)
        {
            if parent_id == ancestor_id {
                return true;
            }
            if visited.contains(&parent_id) {
                return false;
            }
            visited.push(parent_id);
            current_id = parent_id;
        }
        false
    }
}

impl Default for ProfileModalUI {
    fn default() -> Self {
        Self::new()
    }
}
