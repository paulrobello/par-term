//! Profile management modal UI using egui
//!
//! Provides a modal dialog for creating, editing, and managing profiles.

use crate::section::collapsing_section;
use crate::shell_detection;
use par_term_config::{Profile, ProfileId, ProfileManager};
use std::collections::HashSet;

use crate::nerd_font::NERD_FONT_PRESETS;

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
enum ModalMode {
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
    mode: ModalMode,
    /// Working copy of profiles being edited
    working_profiles: Vec<Profile>,
    /// ID of profile being edited/created
    editing_id: Option<ProfileId>,

    // Temporary form fields
    temp_name: String,
    temp_working_dir: String,
    temp_shell: Option<String>,
    temp_login_shell: Option<bool>,
    temp_command: String,
    temp_args: String,
    temp_tab_name: String,
    temp_icon: String,
    // New fields for enhanced profile system (issue #78)
    temp_tags: String,
    temp_parent_id: Option<ProfileId>,
    temp_keyboard_shortcut: String,
    temp_hostname_patterns: String,
    temp_tmux_session_patterns: String,
    temp_directory_patterns: String,
    temp_badge_text: String,
    // Badge appearance settings
    temp_badge_color: Option<[u8; 3]>,
    temp_badge_color_alpha: Option<f32>,
    temp_badge_font: String,
    temp_badge_font_bold: Option<bool>,
    temp_badge_top_margin: Option<f32>,
    temp_badge_right_margin: Option<f32>,
    temp_badge_max_width: Option<f32>,
    temp_badge_max_height: Option<f32>,
    // SSH temp fields
    temp_ssh_host: String,
    temp_ssh_user: String,
    temp_ssh_port: String,
    temp_ssh_identity_file: String,
    temp_ssh_extra_args: String,

    /// Selected profile in list view
    selected_id: Option<ProfileId>,
    /// Whether there are unsaved changes
    has_changes: bool,
    /// Validation error message
    validation_error: Option<String>,
    /// Profile pending deletion (for confirmation)
    pending_delete: Option<(ProfileId, String)>,
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
    fn clear_form(&mut self) {
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
    fn load_profile_to_form(&mut self, profile: &Profile) {
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
    fn form_to_profile(&self, id: ProfileId, order: usize) -> Profile {
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
    fn validate_form(&self) -> Option<String> {
        if self.temp_name.trim().is_empty() {
            return Some("Profile name is required".to_string());
        }
        None
    }

    /// Start editing an existing profile
    fn start_edit(&mut self, id: ProfileId) {
        if let Some(profile) = self.working_profiles.iter().find(|p| p.id == id).cloned() {
            self.load_profile_to_form(&profile);
            self.editing_id = Some(id);
            self.mode = ModalMode::Edit(id);
        }
    }

    /// Start creating a new profile
    fn start_create(&mut self) {
        self.clear_form();
        self.temp_name = "New Profile".to_string();
        let new_id = uuid::Uuid::new_v4();
        self.editing_id = Some(new_id);
        self.mode = ModalMode::Create;
    }

    /// Save the current form (either update existing or create new)
    fn save_form(&mut self) {
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
    fn cancel_edit(&mut self) {
        self.mode = ModalMode::List;
        self.editing_id = None;
        self.clear_form();
    }

    /// Request deletion of a profile (shows confirmation)
    fn request_delete(&mut self, id: ProfileId, name: String) {
        self.pending_delete = Some((id, name));
    }

    /// Confirm and execute profile deletion
    fn confirm_delete(&mut self) {
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
    fn cancel_delete(&mut self) {
        self.pending_delete = None;
    }

    /// Move a profile up in the list
    fn move_up(&mut self, id: ProfileId) {
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
    fn move_down(&mut self, id: ProfileId) {
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

    /// Render the profile list/edit UI inline (no egui::Window wrapper).
    ///
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

    /// Render delete confirmation dialog
    fn render_delete_confirmation(&mut self, ctx: &egui::Context) {
        let (_, profile_name) = self
            .pending_delete
            .as_ref()
            .expect("render_delete_confirmation called only when pending_delete is Some");
        let name = profile_name.clone();

        egui::Window::new("Confirm Delete")
            .collapsible(false)
            .resizable(false)
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .frame(
                egui::Frame::window(&ctx.style())
                    .fill(egui::Color32::from_rgba_unmultiplied(40, 40, 40, 255))
                    .inner_margin(egui::Margin::same(20)),
            )
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(format!("Delete profile \"{}\"?", name));
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("This action cannot be undone.")
                            .small()
                            .color(egui::Color32::GRAY),
                    );
                    ui.add_space(16.0);
                    ui.horizontal(|ui| {
                        if ui.button("Delete").clicked() {
                            self.confirm_delete();
                        }
                        if ui.button("Cancel").clicked() {
                            self.cancel_delete();
                        }
                    });
                });
            });
    }

    /// Render the list view
    pub(crate) fn render_list_view(&mut self, ui: &mut egui::Ui) -> ProfileModalAction {
        let mut action = ProfileModalAction::None;

        // Header with create button
        ui.horizontal(|ui| {
            ui.heading("Profiles");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("+ New Profile").clicked() {
                    self.start_create();
                }
            });
        });
        ui.separator();

        // Profile list
        let available_height = ui.available_height() - 50.0; // Reserve space for footer
        egui::ScrollArea::vertical()
            .max_height(available_height)
            .show(ui, |ui| {
                if self.working_profiles.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(
                            egui::RichText::new("No profiles yet")
                                .italics()
                                .color(egui::Color32::GRAY),
                        );
                        ui.add_space(10.0);
                        ui.label("Click '+ New Profile' to create one");
                    });
                } else {
                    for (idx, profile) in self.working_profiles.clone().iter().enumerate() {
                        let is_selected = self.selected_id == Some(profile.id);

                        // Use push_id with profile.id to ensure stable widget ID for double-click detection
                        ui.push_id(profile.id, |ui| {
                            let bg_color = if is_selected {
                                egui::Color32::from_rgba_unmultiplied(70, 100, 140, 150)
                            } else {
                                egui::Color32::TRANSPARENT
                            };

                            let frame = egui::Frame::NONE
                                .fill(bg_color)
                                .inner_margin(egui::Margin::symmetric(8, 4))
                                .corner_radius(4.0);

                            frame.show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    // Reorder buttons
                                    ui.add_enabled_ui(idx > 0, |ui| {
                                        if ui.small_button("Up").clicked() {
                                            self.move_up(profile.id);
                                        }
                                    });
                                    ui.add_enabled_ui(
                                        idx < self.working_profiles.len() - 1,
                                        |ui| {
                                            if ui.small_button("Dn").clicked() {
                                                self.move_down(profile.id);
                                            }
                                        },
                                    );

                                    // Icon and name
                                    if let Some(icon) = &profile.icon {
                                        ui.label(icon);
                                    }
                                    let name_response =
                                        ui.selectable_label(is_selected, &profile.name);
                                    if name_response.clicked() {
                                        self.selected_id = Some(profile.id);
                                    }
                                    if name_response.double_clicked() {
                                        self.start_edit(profile.id);
                                    }

                                    // Dynamic profile indicator
                                    if profile.source.is_dynamic() {
                                        ui.label(
                                            egui::RichText::new("[dynamic]")
                                                .color(egui::Color32::from_rgb(100, 180, 255))
                                                .small(),
                                        );
                                    }

                                    // Spacer
                                    let is_dynamic = profile.source.is_dynamic();
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            // Delete button (disabled for dynamic profiles)
                                            ui.add_enabled_ui(!is_dynamic, |ui| {
                                                if ui.small_button("🗑").clicked() {
                                                    self.request_delete(
                                                        profile.id,
                                                        profile.name.clone(),
                                                    );
                                                }
                                            });
                                            // Edit/View button
                                            let edit_label =
                                                if is_dynamic { "👁" } else { "✏" };
                                            if ui.small_button(edit_label).clicked() {
                                                self.start_edit(profile.id);
                                            }
                                        },
                                    );
                                });
                            });
                        });
                    }
                }
            });

        // Footer buttons
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("Save").clicked() {
                action = ProfileModalAction::Save;
                // Don't call close() here - the caller needs to get working_profiles first
                // The caller will close the modal after retrieving the profiles
                self.visible = false;
            }
            if ui.button("Cancel").clicked() {
                action = ProfileModalAction::Cancel;
                self.close();
            }

            if self.has_changes {
                ui.colored_label(egui::Color32::YELLOW, "* Unsaved changes");
            }
        });

        action
    }

    /// Render the edit/create view
    pub(crate) fn render_edit_view(&mut self, ui: &mut egui::Ui, collapsed: &mut HashSet<String>) {
        // Check if the profile being edited is a dynamic profile
        let is_dynamic_profile = self
            .editing_id
            .and_then(|id| self.working_profiles.iter().find(|p| p.id == id))
            .is_some_and(|p| p.source.is_dynamic());

        let title = match &self.mode {
            ModalMode::Create => "Create Profile",
            ModalMode::Edit(_) => {
                if is_dynamic_profile {
                    "View Profile"
                } else {
                    "Edit Profile"
                }
            }
            _ => "Profile",
        };

        ui.heading(title);

        // Show read-only notice for dynamic profiles
        if is_dynamic_profile {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("ℹ").color(egui::Color32::from_rgb(100, 180, 255)));
                ui.colored_label(
                    egui::Color32::from_rgb(100, 180, 255),
                    "This profile is managed by a remote source and cannot be edited locally.",
                );
            });
        }

        ui.separator();

        // Form in a scrollable area to handle many fields
        egui::ScrollArea::vertical()
            .max_height(ui.available_height() - 60.0)
            .show(ui, |ui| {
                // Disable all form fields for dynamic (read-only) profiles
                if is_dynamic_profile {
                    ui.disable();
                }

                egui::Grid::new("profile_form")
                    .num_columns(2)
                    .spacing([10.0, 8.0])
                    .show(ui, |ui| {
                        // === Basic Settings ===
                        ui.label("Name:");
                        ui.text_edit_singleline(&mut self.temp_name);
                        ui.end_row();

                        ui.label("Icon:");
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.temp_icon);
                            let picker_label = if self.temp_icon.is_empty() {
                                "\u{ea7b}" // Nerd Font file icon
                            } else {
                                &self.temp_icon
                            };
                            let picker_btn = ui.button(picker_label);
                            egui::Popup::from_toggle_button_response(&picker_btn)
                                .close_behavior(
                                    egui::PopupCloseBehavior::CloseOnClickOutside,
                                )
                                .show(|ui| {
                                    ui.set_min_width(280.0);
                                    egui::ScrollArea::vertical()
                                        .max_height(300.0)
                                        .show(ui, |ui| {
                                            for (category, icons) in NERD_FONT_PRESETS {
                                                ui.label(
                                                    egui::RichText::new(*category)
                                                        .small()
                                                        .strong(),
                                                );
                                                ui.horizontal_wrapped(|ui| {
                                                    for (icon, label) in *icons {
                                                        let btn = ui.add_sized(
                                                            [28.0, 28.0],
                                                            egui::Button::new(
                                                                egui::RichText::new(*icon)
                                                                    .size(16.0),
                                                            )
                                                            .frame(false),
                                                        );
                                                        if btn
                                                            .on_hover_text(*label)
                                                            .clicked()
                                                        {
                                                            self.temp_icon =
                                                                icon.to_string();
                                                            egui::Popup::close_all(
                                                                ui.ctx(),
                                                            );
                                                        }
                                                    }
                                                });
                                                ui.add_space(2.0);
                                            }
                                            ui.add_space(4.0);
                                            if ui.button("Clear icon").clicked() {
                                                self.temp_icon.clear();
                                                egui::Popup::close_all(ui.ctx());
                                            }
                                        });
                                });
                        });
                        ui.end_row();

                        ui.label("Working Directory:");
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.temp_working_dir);
                            if ui.small_button("Browse...").clicked()
                                && let Some(path) = rfd::FileDialog::new().pick_folder()
                            {
                                self.temp_working_dir = path.display().to_string();
                            }
                        });
                        ui.end_row();

                        // Shell selection dropdown
                        ui.label("Shell:");
                        ui.horizontal(|ui| {
                            let shells = shell_detection::detected_shells();
                            let selected_label = self
                                .temp_shell
                                .as_ref()
                                .map(|path| {
                                    // Find display name for selected shell
                                    shells
                                        .iter()
                                        .find(|s| s.path == *path)
                                        .map(|s| s.name.clone())
                                        .unwrap_or_else(|| path.clone())
                                })
                                .unwrap_or_else(|| "Default (inherit global)".to_string());

                            egui::ComboBox::from_id_salt("shell_selector")
                                .selected_text(&selected_label)
                                .show_ui(ui, |ui| {
                                    // Default option (inherit global)
                                    if ui
                                        .selectable_label(
                                            self.temp_shell.is_none(),
                                            "Default (inherit global)",
                                        )
                                        .clicked()
                                    {
                                        self.temp_shell = None;
                                    }
                                    ui.separator();
                                    // Detected shells
                                    for shell in shells {
                                        let is_selected = self
                                            .temp_shell
                                            .as_ref()
                                            .is_some_and(|s| s == &shell.path);
                                        if ui
                                            .selectable_label(
                                                is_selected,
                                                format!("{} ({})", shell.name, shell.path),
                                            )
                                            .clicked()
                                        {
                                            self.temp_shell = Some(shell.path.clone());
                                        }
                                    }
                                });
                        });
                        ui.end_row();

                        // Login shell toggle
                        ui.label("Login Shell:");
                        ui.horizontal(|ui| {
                            let mut use_custom = self.temp_login_shell.is_some();
                            if ui.checkbox(&mut use_custom, "").changed() {
                                if use_custom {
                                    self.temp_login_shell = Some(true);
                                } else {
                                    self.temp_login_shell = None;
                                }
                            }
                            if let Some(ref mut login) = self.temp_login_shell {
                                ui.checkbox(login, "Use login shell (-l)");
                            } else {
                                ui.label(
                                    egui::RichText::new("(inherit global)")
                                        .small()
                                        .color(egui::Color32::GRAY),
                                );
                            }
                        });
                        ui.end_row();

                        ui.label("Command:");
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.temp_command);
                            ui.label(
                                egui::RichText::new("(overrides shell)")
                                    .small()
                                    .color(egui::Color32::GRAY),
                            );
                        });
                        ui.end_row();

                        ui.label("Arguments:");
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.temp_args);
                            ui.label(
                                egui::RichText::new("(space-separated)")
                                    .small()
                                    .color(egui::Color32::GRAY),
                            );
                        });
                        ui.end_row();

                        ui.label("Tab Name:");
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.temp_tab_name);
                            ui.label(
                                egui::RichText::new("(optional)")
                                    .small()
                                    .color(egui::Color32::GRAY),
                            );
                        });
                        ui.end_row();
                    });

                // === Enhanced Features Section (issue #78) ===
                ui.add_space(12.0);
                ui.separator();
                ui.label(
                    egui::RichText::new("Enhanced Features")
                        .strong()
                        .color(egui::Color32::LIGHT_BLUE),
                );
                ui.add_space(4.0);

                egui::Grid::new("profile_form_enhanced")
                    .num_columns(2)
                    .spacing([10.0, 8.0])
                    .show(ui, |ui| {
                        // Tags
                        ui.label("Tags:");
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.temp_tags);
                            ui.label(
                                egui::RichText::new("(comma-separated)")
                                    .small()
                                    .color(egui::Color32::GRAY),
                            );
                        });
                        ui.end_row();

                        // Parent profile (inheritance)
                        ui.label("Inherit From:");
                        self.render_parent_selector(ui);
                        ui.end_row();

                        // Keyboard shortcut
                        ui.label("Keyboard Shortcut:");
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.temp_keyboard_shortcut);
                            ui.label(
                                egui::RichText::new({
                                    #[cfg(target_os = "macos")]
                                    { "(e.g. Cmd+1)" }
                                    #[cfg(not(target_os = "macos"))]
                                    { "(e.g. Ctrl+Shift+1)" }
                                })
                                    .small()
                                    .color(egui::Color32::GRAY),
                            );
                        });
                        ui.end_row();

                        // Hostname patterns for auto-switching
                        ui.label("Auto-Switch Hosts:");
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.temp_hostname_patterns);
                            ui.label(
                                egui::RichText::new("(*.example.com)")
                                    .small()
                                    .color(egui::Color32::GRAY),
                            );
                        });
                        ui.end_row();

                        // Tmux session patterns for auto-switching
                        ui.label("Auto-Switch Tmux:");
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.temp_tmux_session_patterns);
                            ui.label(
                                egui::RichText::new("(work-*, *-dev)")
                                    .small()
                                    .color(egui::Color32::GRAY),
                            );
                        });
                        ui.end_row();

                        // Directory patterns for auto-switching
                        ui.label("Auto-Switch Dirs:");
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.temp_directory_patterns);
                            ui.label(
                                egui::RichText::new("(~/projects/work-*)")
                                    .small()
                                    .color(egui::Color32::GRAY),
                            );
                        });
                        ui.end_row();

                        // Badge text
                        ui.label("Badge Text:");
                        ui.horizontal(|ui| {
                            ui.text_edit_singleline(&mut self.temp_badge_text);
                            ui.label(
                                egui::RichText::new("(overrides global)")
                                    .small()
                                    .color(egui::Color32::GRAY),
                            );
                        });
                        ui.end_row();
                    });

                // Badge Appearance section (collapsible)
                ui.add_space(8.0);
                collapsing_section(ui, "Badge Appearance", "profile_badge_appearance", false, collapsed, |ui| {
                    egui::Grid::new("profile_form_badge_appearance")
                        .num_columns(2)
                        .spacing([10.0, 8.0])
                        .show(ui, |ui| {
                            // Badge color
                            ui.label("Color:");
                            ui.horizontal(|ui| {
                                let mut use_custom = self.temp_badge_color.is_some();
                                if ui.checkbox(&mut use_custom, "").changed() {
                                    if use_custom {
                                        self.temp_badge_color = Some([255, 0, 0]); // Default red
                                    } else {
                                        self.temp_badge_color = None;
                                    }
                                }
                                if let Some(ref mut color) = self.temp_badge_color {
                                    let mut egui_color =
                                        egui::Color32::from_rgb(color[0], color[1], color[2]);
                                    if egui::color_picker::color_edit_button_srgba(
                                        ui,
                                        &mut egui_color,
                                        egui::color_picker::Alpha::Opaque,
                                    )
                                    .changed()
                                    {
                                        *color = [egui_color.r(), egui_color.g(), egui_color.b()];
                                    }
                                } else {
                                    ui.label(
                                        egui::RichText::new("(use global)")
                                            .small()
                                            .color(egui::Color32::GRAY),
                                    );
                                }
                            });
                            ui.end_row();

                            // Badge alpha/opacity
                            ui.label("Opacity:");
                            ui.horizontal(|ui| {
                                let mut use_custom = self.temp_badge_color_alpha.is_some();
                                if ui.checkbox(&mut use_custom, "").changed() {
                                    if use_custom {
                                        self.temp_badge_color_alpha = Some(0.5);
                                    } else {
                                        self.temp_badge_color_alpha = None;
                                    }
                                }
                                if let Some(ref mut alpha) = self.temp_badge_color_alpha {
                                    ui.add(egui::Slider::new(alpha, 0.0..=1.0).step_by(0.05));
                                } else {
                                    ui.label(
                                        egui::RichText::new("(use global)")
                                            .small()
                                            .color(egui::Color32::GRAY),
                                    );
                                }
                            });
                            ui.end_row();

                            // Badge font
                            ui.label("Font:");
                            ui.horizontal(|ui| {
                                ui.text_edit_singleline(&mut self.temp_badge_font);
                                ui.label(
                                    egui::RichText::new("(blank = global)")
                                        .small()
                                        .color(egui::Color32::GRAY),
                                );
                            });
                            ui.end_row();

                            // Badge font bold
                            ui.label("Bold:");
                            ui.horizontal(|ui| {
                                let mut use_custom = self.temp_badge_font_bold.is_some();
                                if ui.checkbox(&mut use_custom, "").changed() {
                                    if use_custom {
                                        self.temp_badge_font_bold = Some(true);
                                    } else {
                                        self.temp_badge_font_bold = None;
                                    }
                                }
                                if let Some(ref mut bold) = self.temp_badge_font_bold {
                                    ui.checkbox(bold, "Bold text");
                                } else {
                                    ui.label(
                                        egui::RichText::new("(use global)")
                                            .small()
                                            .color(egui::Color32::GRAY),
                                    );
                                }
                            });
                            ui.end_row();

                            // Badge top margin
                            ui.label("Top Margin:");
                            ui.horizontal(|ui| {
                                let mut use_custom = self.temp_badge_top_margin.is_some();
                                if ui.checkbox(&mut use_custom, "").changed() {
                                    if use_custom {
                                        self.temp_badge_top_margin = Some(0.0);
                                    } else {
                                        self.temp_badge_top_margin = None;
                                    }
                                }
                                if let Some(ref mut margin) = self.temp_badge_top_margin {
                                    ui.add(egui::DragValue::new(margin).range(0.0..=100.0).suffix(" px"));
                                } else {
                                    ui.label(
                                        egui::RichText::new("(use global)")
                                            .small()
                                            .color(egui::Color32::GRAY),
                                    );
                                }
                            });
                            ui.end_row();

                            // Badge right margin
                            ui.label("Right Margin:");
                            ui.horizontal(|ui| {
                                let mut use_custom = self.temp_badge_right_margin.is_some();
                                if ui.checkbox(&mut use_custom, "").changed() {
                                    if use_custom {
                                        self.temp_badge_right_margin = Some(16.0);
                                    } else {
                                        self.temp_badge_right_margin = None;
                                    }
                                }
                                if let Some(ref mut margin) = self.temp_badge_right_margin {
                                    ui.add(egui::DragValue::new(margin).range(0.0..=100.0).suffix(" px"));
                                } else {
                                    ui.label(
                                        egui::RichText::new("(use global)")
                                            .small()
                                            .color(egui::Color32::GRAY),
                                    );
                                }
                            });
                            ui.end_row();

                            // Badge max width
                            ui.label("Max Width:");
                            ui.horizontal(|ui| {
                                let mut use_custom = self.temp_badge_max_width.is_some();
                                if ui.checkbox(&mut use_custom, "").changed() {
                                    if use_custom {
                                        self.temp_badge_max_width = Some(0.5);
                                    } else {
                                        self.temp_badge_max_width = None;
                                    }
                                }
                                if let Some(ref mut width) = self.temp_badge_max_width {
                                    ui.add(
                                        egui::Slider::new(width, 0.1..=1.0)
                                            .step_by(0.05)
                                            .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
                                    );
                                } else {
                                    ui.label(
                                        egui::RichText::new("(use global)")
                                            .small()
                                            .color(egui::Color32::GRAY),
                                    );
                                }
                            });
                            ui.end_row();

                            // Badge max height
                            ui.label("Max Height:");
                            ui.horizontal(|ui| {
                                let mut use_custom = self.temp_badge_max_height.is_some();
                                if ui.checkbox(&mut use_custom, "").changed() {
                                    if use_custom {
                                        self.temp_badge_max_height = Some(0.2);
                                    } else {
                                        self.temp_badge_max_height = None;
                                    }
                                }
                                if let Some(ref mut height) = self.temp_badge_max_height {
                                    ui.add(
                                        egui::Slider::new(height, 0.05..=0.5)
                                            .step_by(0.05)
                                            .custom_formatter(|v, _| format!("{:.0}%", v * 100.0)),
                                    );
                                } else {
                                    ui.label(
                                        egui::RichText::new("(use global)")
                                            .small()
                                            .color(egui::Color32::GRAY),
                                    );
                                }
                            });
                            ui.end_row();
                        });

                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("Check boxes to override global badge settings for this profile.")
                            .small()
                            .color(egui::Color32::GRAY),
                    );
                });

                // SSH Connection section
                ui.add_space(8.0);
                collapsing_section(ui, "SSH Connection", "profile_ssh_connection", false, collapsed, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Host:");
                        ui.text_edit_singleline(&mut self.temp_ssh_host);
                    });
                    ui.horizontal(|ui| {
                        ui.label("User:");
                        ui.text_edit_singleline(&mut self.temp_ssh_user);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Port:");
                        ui.add(egui::TextEdit::singleline(&mut self.temp_ssh_port).desired_width(60.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Identity File:");
                        ui.text_edit_singleline(&mut self.temp_ssh_identity_file);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Extra Args:");
                        ui.text_edit_singleline(&mut self.temp_ssh_extra_args);
                    });
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("When SSH Host is set, opening this profile connects via SSH instead of launching a shell.")
                            .weak()
                            .size(11.0),
                    );
                });

                // Validation error
                if let Some(error) = &self.validation_error {
                    ui.add_space(8.0);
                    ui.colored_label(egui::Color32::RED, error);
                }

                // Help text
                ui.add_space(16.0);
                ui.label(
                    egui::RichText::new(
                        "Note: Inherited settings from parent profiles are used when this profile's field is empty.",
                    )
                    .small()
                    .color(egui::Color32::GRAY),
                );
            });

        // Footer buttons
        ui.add_space(8.0);
        ui.separator();
        ui.horizontal(|ui| {
            if is_dynamic_profile {
                // Dynamic profiles are read-only; only show Back button
                if ui.button("Back").clicked() {
                    self.cancel_edit();
                }
            } else {
                if ui.button("Save Profile").clicked() {
                    self.save_form();
                }
                if ui.button("Cancel").clicked() {
                    self.cancel_edit();
                }
            }
        });
    }

    /// Check if `ancestor_id` appears in the parent chain of `profile_id`
    fn has_ancestor(&self, profile_id: ProfileId, ancestor_id: ProfileId) -> bool {
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

    /// Render the parent profile selector dropdown
    fn render_parent_selector(&mut self, ui: &mut egui::Ui) {
        // Get valid parents (excludes self and profiles that would create cycles)
        let current_id = self.editing_id;
        let valid_parents: Vec<_> = self
            .working_profiles
            .iter()
            .filter(|p| {
                // Cannot select self as parent
                if Some(p.id) == current_id {
                    return false;
                }
                // Prevent cycles: reject if this candidate has current profile as ancestor
                if let Some(cid) = current_id
                    && self.has_ancestor(p.id, cid)
                {
                    return false;
                }
                true
            })
            .map(|p| (p.id, p.display_label()))
            .collect();

        let selected_label = self
            .temp_parent_id
            .and_then(|id| self.working_profiles.iter().find(|p| p.id == id))
            .map(|p| p.display_label())
            .unwrap_or_else(|| "(None)".to_string());

        egui::ComboBox::from_id_salt("parent_profile_selector")
            .selected_text(&selected_label)
            .show_ui(ui, |ui| {
                // Option to clear parent
                if ui
                    .selectable_label(self.temp_parent_id.is_none(), "(None)")
                    .clicked()
                {
                    self.temp_parent_id = None;
                }
                // List valid parents
                for (id, label) in valid_parents {
                    if ui
                        .selectable_label(self.temp_parent_id == Some(id), &label)
                        .clicked()
                    {
                        self.temp_parent_id = Some(id);
                    }
                }
            });
    }
}

impl Default for ProfileModalUI {
    fn default() -> Self {
        Self::new()
    }
}
