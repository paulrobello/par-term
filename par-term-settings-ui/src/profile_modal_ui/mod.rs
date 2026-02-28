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
//! | `edit_view.rs` | Profile edit/create view renderer, parent selector, badge/SSH sections |

mod edit_view;
mod form_helpers;
mod list_view;

use par_term_config::{Profile, ProfileId, ProfileManager};
use std::collections::HashSet;

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
        let action = match &self.mode.clone() {
            ModalMode::List => self.render_list_view(ui),
            ModalMode::Edit(_) | ModalMode::Create => {
                self.render_edit_view(ui, collapsed);
                ProfileModalAction::None
            }
        };

        // Render delete confirmation dialog on top
        if self.pending_delete.is_some() {
            self.render_delete_confirmation(ui.ctx());
        }

        action
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
                egui::Frame::window(&ctx.style())
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

        // Render delete confirmation dialog on top
        if self.pending_delete.is_some() {
            self.render_delete_confirmation(ctx);
        }

        action
    }
}

impl Default for ProfileModalUI {
    fn default() -> Self {
        Self::new()
    }
}
