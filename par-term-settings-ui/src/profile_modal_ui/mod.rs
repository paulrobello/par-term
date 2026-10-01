//! Profile management modal UI using egui
//!
//! Provides a modal dialog for creating, editing, and managing profiles.
//!
//! ## Sub-module layout
//!
//! | File | Contents |
//! |------|----------|
//! | `mod.rs` (this file) | Type definitions, lifecycle methods, public entry points (`show`, `show_inline`) |
//! | `form_helpers.rs` | Private form field helpers (clear, load, save, validate, move up/down) |
//! | `list_view.rs` | Profile list view renderer and delete confirmation dialog |
//! | `edit_view.rs` | Profile edit/create view: header, sub-tab bar, footer, icon and shell pickers |
//! | `edit_tabs.rs` | Editor sub-tabs (General, Session, Text & Badge, Shader, SSH, Auto-Switch) |
//! | `shortcut.rs` | Keyboard Shortcut recorder, bound to the profile's `open_profile:<id>` action |
//! | `badge_section.rs` | Badge, shader, tmux, par-mux, and SSH sections the sub-tabs draw |

mod badge_section;
mod edit_tabs;
mod edit_view;
mod form_helpers;
mod list_view;
mod parent_selector;
pub(crate) mod shortcut;

pub use edit_tabs::ProfileEditTab;
use par_term_config::{Profile, ProfileId, ProfileManager};
use std::collections::HashSet;

/// True when two profile lists serialize identically. `Profile` has no
/// `PartialEq`; a serialization failure counts as a difference.
fn profiles_equal(a: &[Profile], b: &[Profile]) -> bool {
    a.len() == b.len()
        && match (serde_yaml_ng::to_value(a), serde_yaml_ng::to_value(b)) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        }
}

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
    // Shader override temp fields
    pub(super) temp_shader: String,
    pub(super) temp_shader_brightness: Option<f32>,
    pub(super) temp_shader_text_opacity: Option<f32>,
    pub(super) temp_shader_animation_speed: Option<f32>,
    pub(super) temp_shader_channels: [String; 4],
    // SSH temp fields
    pub(super) temp_ssh_host: String,
    pub(super) temp_ssh_user: String,
    pub(super) temp_ssh_port: String,
    pub(super) temp_ssh_identity_file: String,
    pub(super) temp_ssh_extra_args: String,
    // tmux auto-connect fields
    pub(super) temp_tmux_session_name: String,
    pub(super) temp_tmux_connection_mode: par_term_config::TmuxConnectionMode,
    pub(super) temp_mux_session_name: String,

    /// Selected profile in list view
    pub(super) selected_id: Option<ProfileId>,
    /// Whether there are unsaved changes
    pub(super) has_changes: bool,
    /// Validation error message
    pub(super) validation_error: Option<String>,
    /// Profiles as last loaded or saved. List Cancel restores this; a Save
    /// that would empty a non-empty baseline needs explicit confirmation.
    pub(super) baseline_profiles: Vec<Profile>,
    /// Set while the "save an empty profile list?" confirmation is showing.
    pub(super) confirm_empty_save: bool,
    /// The empty-list confirmation was accepted; the next save request goes
    /// through without asking again.
    pub(super) empty_save_confirmed: bool,
    /// Global `tmux_enabled`, set by the Settings window each frame; the
    /// per-profile tmux section warns when it is off (UX.md B57).
    pub global_tmux_enabled: bool,
    /// Selected sub-tab of the profile editor (UX.md 15.2).
    pub edit_tab: ProfileEditTab,
    /// Row armed for delete by the list's first Delete click (UX.md SC4).
    pub(super) pending_row_delete: crate::delete_confirm::PendingDelete,
    /// The chord bound to the edited profile's `open_profile:<id>` action
    /// in the working config, set by the Settings window each frame.
    pub(super) config_shortcut: Option<String>,
    /// Shortcut edit staged in the open editor: `Some(Some(chord))` records,
    /// `Some(None)` clears. Applied to the config when Done saves the form.
    pub(super) staged_shortcut: Option<Option<String>>,
    /// The Keyboard Shortcut recorder is waiting for a chord.
    pub(super) shortcut_recording: bool,
    /// A recorded chord awaiting its conflict check by the config owner.
    pub(super) unchecked_chord: Option<String>,
    /// Result of the last conflict check, shown under the field.
    pub(super) shortcut_conflict: Option<String>,
    /// A saved binding change for the config owner to apply.
    pub(super) pending_binding: Option<shortcut::PendingBinding>,
}

impl ProfileModalUI {
    // =========================================================================
    // Lifecycle & State Management
    // =========================================================================

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
            temp_shader: String::new(),
            temp_shader_brightness: None,
            temp_shader_text_opacity: None,
            temp_shader_animation_speed: None,
            temp_shader_channels: Default::default(),
            temp_ssh_host: String::new(),
            temp_ssh_user: String::new(),
            temp_ssh_port: String::new(),
            temp_ssh_identity_file: String::new(),
            temp_ssh_extra_args: String::new(),
            temp_tmux_session_name: String::new(),
            temp_tmux_connection_mode: par_term_config::TmuxConnectionMode::default(),
            temp_mux_session_name: String::new(),
            selected_id: None,
            has_changes: false,
            validation_error: None,
            baseline_profiles: Vec::new(),
            confirm_empty_save: false,
            empty_save_confirmed: false,
            global_tmux_enabled: true,
            edit_tab: ProfileEditTab::default(),
            pending_row_delete: None,
            config_shortcut: None,
            staged_shortcut: None,
            shortcut_recording: false,
            unchecked_chord: None,
            shortcut_conflict: None,
            pending_binding: None,
        }
    }

    /// Open the modal with current profiles
    pub fn open(&mut self, manager: &ProfileManager) {
        self.visible = true;
        self.mode = ModalMode::List;
        self.working_profiles = manager.to_vec();
        self.baseline_profiles = self.working_profiles.clone();
        self.confirm_empty_save = false;
        self.editing_id = None;
        self.selected_id = None;
        self.has_changes = false;
        self.validation_error = None;
        self.pending_row_delete = None;
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
        self.pending_row_delete = None;
        self.clear_form();
    }

    /// Load profiles into the working copy without toggling visibility.
    ///
    /// Used by the settings window to populate the inline profile editor
    /// without opening a modal window.
    pub fn load_profiles(&mut self, profiles: Vec<Profile>) {
        self.baseline_profiles = profiles.clone();
        self.working_profiles = profiles;
        self.confirm_empty_save = false;
        self.mode = ModalMode::List;
        self.editing_id = None;
        self.selected_id = None;
        self.has_changes = false;
        self.validation_error = None;
        self.pending_row_delete = None;
        self.clear_form();
    }

    /// Get the working profiles (for saving)
    pub fn get_working_profiles(&self) -> &[Profile] {
        &self.working_profiles
    }

    /// Mark the working set as delivered to the consumer (which persists it),
    /// clearing the list footer's "* Unsaved changes" marker.
    pub fn mark_saved(&mut self) {
        self.has_changes = false;
        self.baseline_profiles = self.working_profiles.clone();
    }

    /// Whether the working set differs from what was last loaded or saved,
    /// including an edit form that is still open. Viewing a read-only
    /// dynamic profile is not an edit.
    pub fn has_unsaved_changes(&self) -> bool {
        self.has_changes
            || self.is_editing_local_profile()
            || !profiles_equal(&self.working_profiles, &self.baseline_profiles)
    }

    fn is_editing_local_profile(&self) -> bool {
        match self.mode {
            ModalMode::List => false,
            ModalMode::Create => true,
            ModalMode::Edit(id) => !self
                .working_profiles
                .iter()
                .any(|p| p.id == id && p.source.is_dynamic()),
        }
    }

    /// Profiles as last loaded or saved.
    pub fn baseline_profiles(&self) -> &[Profile] {
        &self.baseline_profiles
    }

    /// Put back a baseline that a failed save moved: the working set stays,
    /// so the edits show as unsaved again and Save can be retried.
    pub fn restore_baseline(&mut self, baseline: Vec<Profile>) {
        self.baseline_profiles = baseline;
    }

    /// List-view Cancel: drop unsaved edits by restoring the baseline.
    ///
    /// Unlike [`Self::close`], the working set is never emptied, so a later
    /// Save cannot persist an empty list the user never asked for.
    pub fn cancel_list_changes(&mut self) {
        self.working_profiles = self.baseline_profiles.clone();
        self.mode = ModalMode::List;
        self.editing_id = None;
        self.selected_id = None;
        self.has_changes = false;
        self.validation_error = None;
        self.pending_row_delete = None;
        self.confirm_empty_save = false;
        self.clear_form();
    }

    /// List-view Save. Returns [`ProfileModalAction::Save`] when the working
    /// set may be persisted. Saving an empty set over a non-empty baseline
    /// arms a confirmation instead and returns [`ProfileModalAction::None`].
    pub fn request_list_save(&mut self) -> ProfileModalAction {
        let confirmed = std::mem::take(&mut self.empty_save_confirmed);
        if self.working_profiles.is_empty() && !self.baseline_profiles.is_empty() && !confirmed {
            self.confirm_empty_save = true;
            return ProfileModalAction::None;
        }
        self.confirm_empty_save = false;
        ProfileModalAction::Save
    }

    /// Confirm the "save an empty profile list?" prompt. The next
    /// [`Self::request_list_save`] goes through without asking again.
    pub fn confirm_empty_list_save(&mut self) -> ProfileModalAction {
        self.confirm_empty_save = false;
        self.empty_save_confirmed = true;
        ProfileModalAction::Save
    }

    /// Whether the empty-save confirmation is showing.
    pub fn is_confirming_empty_save(&self) -> bool {
        self.confirm_empty_save
    }

    /// Fold an open edit form into the working set before a save. A form
    /// that fails validation stays open and its message is returned, so the
    /// caller can block the save instead of dropping the edit. A read-only
    /// dynamic profile view is simply closed.
    pub fn finish_open_edit(&mut self) -> Result<(), String> {
        if matches!(self.mode, ModalMode::List) {
            return Ok(());
        }
        if !self.is_editing_local_profile() {
            self.cancel_edit();
            return Ok(());
        }
        self.save_form();
        match &self.validation_error {
            Some(error) if !matches!(self.mode, ModalMode::List) => Err(error.clone()),
            _ => Ok(()),
        }
    }

    #[cfg(test)]
    pub(crate) fn clear_working_profiles_for_test(&mut self) {
        self.working_profiles.clear();
    }

    #[cfg(test)]
    pub(crate) fn add_profile_for_test(&mut self, profile: Profile) {
        self.working_profiles.push(profile);
    }

    #[cfg(test)]
    pub(crate) fn start_create_with_name_for_test(&mut self, name: &str) {
        self.start_create();
        self.temp_name = name.to_string();
    }

    #[cfg(test)]
    pub(crate) fn start_edit_for_test(&mut self, id: ProfileId) {
        self.start_edit(id);
    }

    // =========================================================================
    // Public UI Entry Points (modal window + inline embed)
    // =========================================================================

    /// Used inside the settings window's Profiles tab to embed the profile
    /// management UI directly. Returns `ProfileModalAction` to communicate
    /// save/cancel/open-profile requests to the caller.
    pub fn show_inline(
        &mut self,
        ui: &mut egui::Ui,
        collapsed: &mut HashSet<String>,
    ) -> ProfileModalAction {
        match &self.mode.clone() {
            ModalMode::List => self.render_list_view(ui),
            ModalMode::Edit(_) | ModalMode::Create => {
                self.render_edit_view(ui, collapsed);
                ProfileModalAction::None
            }
        }
    }

    /// Render the modal and return any action triggered
    pub fn show(&mut self, ctx: &egui::Context) -> ProfileModalAction {
        if !self.visible {
            return ProfileModalAction::None;
        }

        let mut action = ProfileModalAction::None;

        // Handle Escape key
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            match &self.mode {
                ModalMode::Edit(_) | ModalMode::Create => {
                    self.cancel_edit();
                }
                ModalMode::List => {
                    self.close();
                    return ProfileModalAction::Cancel;
                }
            }
        }

        let modal_size = egui::vec2(550.0, 580.0);

        egui::Window::new("Manage Profiles")
            .collapsible(false)
            .resizable(false)
            .order(egui::Order::Foreground)
            .default_size(modal_size)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .frame(
                egui::Frame::window(&ctx.global_style())
                    .fill(egui::Color32::from_rgba_unmultiplied(30, 30, 30, 250))
                    .inner_margin(egui::Margin::same(16)),
            )
            .show(ctx, |ui| match &self.mode.clone() {
                ModalMode::List => {
                    action = self.render_list_view(ui);
                }
                ModalMode::Edit(_) | ModalMode::Create => {
                    let mut modal_collapsed = HashSet::new();
                    self.render_edit_view(ui, &mut modal_collapsed);
                }
            });

        action
    }
}

impl Default for ProfileModalUI {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod edit_tabs_tests;
