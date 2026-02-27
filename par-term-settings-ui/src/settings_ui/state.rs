//! SettingsUI state management and lifecycle methods.

use par_term_config::{
    BackgroundImageMode, Config, CursorShaderMetadataCache, Profile, ProfileId, ShaderMetadataCache,
};
use rfd::FileDialog;
use std::collections::HashSet;

use crate::profile_modal_ui::ProfileModalUI;
use crate::sidebar::SettingsTab;
use crate::{ArrangementManager, InstallationType, ShaderInstallResult, UpdateResult};

use super::SettingsUI;

impl SettingsUI {
    /// Create a new settings UI
    pub fn new(config: Config) -> Self {
        // Extract values before moving config
        let initial_cols = config.cols;
        let initial_rows = config.rows;
        let initial_collapsed: HashSet<String> =
            config.collapsed_settings_sections.iter().cloned().collect();

        Self {
            visible: false,
            temp_font_bold: config.font_family_bold.clone().unwrap_or_default(),
            temp_font_italic: config.font_family_italic.clone().unwrap_or_default(),
            temp_font_bold_italic: config.font_family_bold_italic.clone().unwrap_or_default(),
            temp_font_family: config.font_family.clone(),
            temp_font_size: config.font_size,
            temp_line_spacing: config.line_spacing,
            temp_char_spacing: config.char_spacing,
            temp_enable_text_shaping: config.enable_text_shaping,
            temp_enable_ligatures: config.enable_ligatures,
            temp_enable_kerning: config.enable_kerning,
            font_pending_changes: false,
            temp_custom_shell: config.custom_shell.clone().unwrap_or_default(),
            temp_shell_args: config
                .shell_args
                .as_ref()
                .map(|args| args.join(" "))
                .unwrap_or_default(),
            temp_working_directory: config.working_directory.clone().unwrap_or_default(),
            temp_startup_directory: config.startup_directory.clone().unwrap_or_default(),
            temp_initial_text: config.initial_text.clone(),
            temp_background_image: config.background_image.clone().unwrap_or_default(),
            temp_custom_shader: config.custom_shader.clone().unwrap_or_default(),
            temp_cursor_shader: config.cursor_shader.clone().unwrap_or_default(),
            temp_shader_channel0: config.custom_shader_channel0.clone().unwrap_or_default(),
            temp_shader_channel1: config.custom_shader_channel1.clone().unwrap_or_default(),
            temp_shader_channel2: config.custom_shader_channel2.clone().unwrap_or_default(),
            temp_shader_channel3: config.custom_shader_channel3.clone().unwrap_or_default(),
            temp_cubemap_path: config.custom_shader_cubemap.clone().unwrap_or_default(),
            temp_background_color: config.background_color,
            temp_pane_bg_path: String::new(),
            temp_pane_bg_mode: BackgroundImageMode::default(),
            temp_pane_bg_opacity: 1.0,
            temp_pane_bg_darken: 0.0,
            temp_pane_bg_index: None,
            last_live_opacity: config.window_opacity,
            current_cols: initial_cols,
            current_rows: initial_rows,
            supported_vsync_modes: vec![
                par_term_config::VsyncMode::Immediate,
                par_term_config::VsyncMode::Mailbox,
                par_term_config::VsyncMode::Fifo,
            ],
            vsync_warning: None,
            config,
            has_changes: false,
            search_query: String::new(),
            focus_search: true,
            shader_editor_visible: false,
            shader_editor_source: String::new(),
            shader_editor_error: None,
            shader_editor_original: String::new(),
            cursor_shader_editor_visible: false,
            cursor_shader_editor_source: String::new(),
            cursor_shader_editor_error: None,
            cursor_shader_editor_original: String::new(),
            available_agent_ids: Vec::new(),
            available_shaders: Self::scan_shaders_folder(),
            available_cubemaps: Self::scan_cubemaps_folder(),
            new_shader_name: String::new(),
            show_create_shader_dialog: false,
            show_delete_shader_dialog: false,
            shader_search_query: String::new(),
            shader_search_matches: Vec::new(),
            shader_search_current: 0,
            shader_search_visible: false,
            shader_metadata_cache: ShaderMetadataCache::with_shaders_dir(
                par_term_config::Config::shaders_dir(),
            ),
            cursor_shader_metadata_cache: CursorShaderMetadataCache::with_shaders_dir(
                par_term_config::Config::shaders_dir(),
            ),
            shader_settings_expanded: true,
            cursor_shader_settings_expanded: true,
            keybinding_recording_index: None,
            keybinding_recorded_combo: None,
            test_notification_requested: false,
            selected_tab: SettingsTab::default(),
            collapsed_sections: initial_collapsed,
            shell_integration_action: None,
            profile_modal_ui: ProfileModalUI::new(),
            profile_save_requested: false,
            profile_open_requested: None,
            shader_installing: false,
            shader_status: None,
            shader_error: None,
            shader_overwrite_prompt_visible: false,
            shader_conflicts: Vec::new(),
            shader_install_receiver: None,
            editing_trigger_index: None,
            temp_trigger_name: String::new(),
            temp_trigger_pattern: String::new(),
            temp_trigger_actions: Vec::new(),
            temp_trigger_require_user_action: true,
            adding_new_trigger: false,
            trigger_pattern_error: None,
            editing_coprocess_index: None,
            temp_coprocess_name: String::new(),
            temp_coprocess_command: String::new(),
            temp_coprocess_args: String::new(),
            temp_coprocess_auto_start: false,
            temp_coprocess_copy_output: true,
            temp_coprocess_restart_policy: par_term_config::automation::RestartPolicy::Never,
            temp_coprocess_restart_delay_ms: 0,
            adding_new_coprocess: false,
            trigger_resync_requested: false,
            pending_coprocess_actions: Vec::new(),
            coprocess_running: Vec::new(),
            coprocess_errors: Vec::new(),
            coprocess_output: Vec::new(),
            coprocess_output_expanded: Vec::new(),
            editing_script_index: None,
            temp_script_name: String::new(),
            temp_script_path: String::new(),
            temp_script_args: String::new(),
            temp_script_auto_start: false,
            temp_script_enabled: true,
            temp_script_restart_policy: par_term_config::automation::RestartPolicy::Never,
            temp_script_restart_delay_ms: 0,
            temp_script_subscriptions: String::new(),
            adding_new_script: false,
            pending_script_actions: Vec::new(),
            script_running: Vec::new(),
            script_errors: Vec::new(),
            script_output: Vec::new(),
            script_output_expanded: Vec::new(),
            script_panels: Vec::new(),
            open_log_requested: false,
            identify_panes_requested: false,
            update_install_requested: false,
            check_now_requested: false,
            update_status: None,
            update_result: None,
            last_update_result: None,
            update_installing: false,
            update_install_receiver: None,
            editing_snippet_index: None,
            temp_snippet_id: String::new(),
            temp_snippet_title: String::new(),
            temp_snippet_content: String::new(),
            temp_snippet_keybinding: String::new(),
            temp_snippet_folder: String::new(),
            temp_snippet_description: String::new(),
            temp_snippet_keybinding_enabled: true,
            temp_snippet_auto_execute: false,
            temp_snippet_variables: Vec::new(),
            adding_new_snippet: false,
            editing_action_index: None,
            temp_action_type: 0,
            temp_action_id: String::new(),
            temp_action_title: String::new(),
            temp_action_command: String::new(),
            temp_action_args: String::new(),
            temp_action_text: String::new(),
            temp_action_keys: String::new(),
            temp_action_keybinding: String::new(),
            adding_new_action: false,
            recording_snippet_keybinding: false,
            snippet_recorded_combo: None,
            recording_action_keybinding: false,
            action_recorded_combo: None,
            dynamic_source_editing: None,
            dynamic_source_edit_buffer: None,
            dynamic_source_new_header_key: String::new(),
            dynamic_source_new_header_value: String::new(),
            temp_import_url: String::new(),
            import_export_status: None,
            import_export_is_error: false,
            show_reset_defaults_dialog: false,
            arrangement_save_name: String::new(),
            arrangement_confirm_restore: None,
            arrangement_confirm_delete: None,
            arrangement_confirm_overwrite: None,
            arrangement_rename_id: None,
            arrangement_rename_text: String::new(),
            pending_arrangement_actions: Vec::new(),
            arrangement_manager: ArrangementManager::new(),
            app_version: "",
            installation_type: InstallationType::StandaloneBinary,
            shader_install_fn: None,
            shader_detect_modified_fn: None,
            shader_uninstall_fn: None,
            shader_has_files_fn: None,
            shader_count_files_fn: None,
            test_detection_content: String::new(),
            test_detection_command: String::new(),
            test_detection_requested: false,
            test_detection_result: None,
            shell_integration_is_installed_fn: None,
            shell_integration_detected_shell_fn: None,
            shell_integration_install_fn: None,
            shell_integration_uninstall_fn: None,
        }
    }

    /// Update the current terminal dimensions (called when window resizes)
    pub fn update_current_size(&mut self, cols: usize, rows: usize) {
        self.current_cols = cols;
        self.current_rows = rows;
    }

    /// Update the list of supported vsync modes (called when renderer is initialized)
    pub fn update_supported_vsync_modes(&mut self, modes: Vec<par_term_config::VsyncMode>) {
        self.supported_vsync_modes = modes;
        self.vsync_warning = None;
    }

    /// Check if a vsync mode is supported
    pub fn is_vsync_mode_supported(&self, mode: par_term_config::VsyncMode) -> bool {
        self.supported_vsync_modes.contains(&mode)
    }

    /// Set vsync warning message
    pub fn set_vsync_warning(&mut self, warning: Option<String>) {
        self.vsync_warning = warning;
    }

    pub fn pick_file_path(&self, title: &str) -> Option<String> {
        FileDialog::new()
            .set_title(title)
            .pick_file()
            .map(|p| p.display().to_string())
    }

    pub fn pick_folder_path(&self, title: &str) -> Option<String> {
        FileDialog::new()
            .set_title(title)
            .pick_folder()
            .map(|p| p.display().to_string())
    }

    /// Update the config copy (e.g., when config is reloaded).
    pub fn update_config(&mut self, config: Config) {
        if !self.has_changes {
            self.config = config;
            self.last_live_opacity = self.config.window_opacity;
            if !self.font_pending_changes {
                self.sync_font_temps_from_config();
            }
        }
    }

    /// Force-update the config copy, bypassing the `has_changes` guard.
    pub fn force_update_config(&mut self, config: Config) {
        self.config = config;
        self.sync_all_temps_from_config();
        self.has_changes = false;
    }

    pub(super) fn sync_font_temps_from_config(&mut self) {
        self.temp_font_family = self.config.font_family.clone();
        self.temp_font_size = self.config.font_size;
        self.temp_line_spacing = self.config.line_spacing;
        self.temp_char_spacing = self.config.char_spacing;
        self.temp_enable_text_shaping = self.config.enable_text_shaping;
        self.temp_enable_ligatures = self.config.enable_ligatures;
        self.temp_enable_kerning = self.config.enable_kerning;
        self.temp_font_bold = self.config.font_family_bold.clone().unwrap_or_default();
        self.temp_font_italic = self.config.font_family_italic.clone().unwrap_or_default();
        self.temp_font_bold_italic = self
            .config
            .font_family_bold_italic
            .clone()
            .unwrap_or_default();
        self.font_pending_changes = false;
    }

    /// Sync ALL temp fields from config
    pub fn sync_all_temps_from_config(&mut self) {
        self.sync_font_temps_from_config();
        self.temp_custom_shell = self.config.custom_shell.clone().unwrap_or_default();
        self.temp_shell_args = self
            .config
            .shell_args
            .as_ref()
            .map(|args| args.join(" "))
            .unwrap_or_default();
        self.temp_working_directory = self.config.working_directory.clone().unwrap_or_default();
        self.temp_startup_directory = self.config.startup_directory.clone().unwrap_or_default();
        self.temp_initial_text = self.config.initial_text.clone();
        self.temp_background_image = self.config.background_image.clone().unwrap_or_default();
        self.temp_background_color = self.config.background_color;
        self.temp_custom_shader = self.config.custom_shader.clone().unwrap_or_default();
        self.temp_cursor_shader = self.config.cursor_shader.clone().unwrap_or_default();
        self.temp_shader_channel0 = self
            .config
            .custom_shader_channel0
            .clone()
            .unwrap_or_default();
        self.temp_shader_channel1 = self
            .config
            .custom_shader_channel1
            .clone()
            .unwrap_or_default();
        self.temp_shader_channel2 = self
            .config
            .custom_shader_channel2
            .clone()
            .unwrap_or_default();
        self.temp_shader_channel3 = self
            .config
            .custom_shader_channel3
            .clone()
            .unwrap_or_default();
        self.temp_cubemap_path = self
            .config
            .custom_shader_cubemap
            .clone()
            .unwrap_or_default();
        self.last_live_opacity = self.config.window_opacity;
    }

    /// Reset all settings to their default values
    pub(super) fn reset_all_to_defaults(&mut self) {
        self.config = Config::default();
        self.sync_all_temps_from_config();
        self.has_changes = true;
        self.search_query.clear();
    }

    /// Show the reset to defaults confirmation dialog
    pub fn start_shader_install_with<F>(&mut self, force_overwrite: bool, install_fn: F)
    where
        F: FnOnce(bool) -> Result<ShaderInstallResult, String> + Send + 'static,
    {
        use std::sync::mpsc;

        if self.shader_installing {
            return;
        }

        self.shader_error = None;
        self.shader_status = Some(if force_overwrite {
            "Reinstalling shaders (overwriting modified files)...".to_string()
        } else {
            "Reinstalling shaders...".to_string()
        });
        self.shader_installing = true;

        let (tx, rx) = mpsc::channel();
        self.shader_install_receiver = Some(rx);

        std::thread::spawn(move || {
            let result = install_fn(force_overwrite);
            let _ = tx.send(result);
        });
    }

    /// Poll for completion of async shader install.
    pub fn poll_shader_install_status(&mut self) {
        if let Some(receiver) = &self.shader_install_receiver
            && let Ok(result) = receiver.try_recv()
        {
            self.shader_installing = false;
            self.shader_install_receiver = None;
            match result {
                Ok(res) => {
                    let detail = if res.skipped > 0 {
                        format!(
                            "Installed {} shaders ({} skipped, {} removed)",
                            res.installed, res.skipped, res.removed
                        )
                    } else {
                        format!(
                            "Installed {} shaders ({} removed)",
                            res.installed, res.removed
                        )
                    };
                    self.shader_status = Some(detail);
                    self.shader_error = None;
                    self.config.integration_versions.shaders_installed_version =
                        Some(self.app_version.to_string());
                }
                Err(e) => {
                    self.shader_error = Some(e);
                    self.shader_status = None;
                }
            }
        }
    }

    /// Begin self-update asynchronously.
    /// The caller must provide a function that performs the actual update.
    pub fn start_self_update_with<F>(&mut self, version: String, update_fn: F)
    where
        F: FnOnce(&str) -> Result<UpdateResult, String> + Send + 'static,
    {
        use std::sync::mpsc;

        if self.update_installing {
            return;
        }

        self.update_status = Some("Downloading and installing update...".to_string());
        self.update_result = None;
        self.update_installing = true;

        let (tx, rx) = mpsc::channel();
        self.update_install_receiver = Some(rx);

        std::thread::spawn(move || {
            let result = update_fn(&version);
            let _ = tx.send(result);
        });
    }

    /// Poll for completion of async self-update.
    pub fn poll_update_install_status(&mut self) {
        if let Some(receiver) = &self.update_install_receiver
            && let Ok(result) = receiver.try_recv()
        {
            self.update_installing = false;
            self.update_install_receiver = None;
            match &result {
                Ok(res) => {
                    self.update_status = Some(format!(
                        "Update installed! Restart par-term to use v{}",
                        res.new_version
                    ));
                }
                Err(e) => {
                    self.update_status = Some(format!("Update failed: {}", e));
                }
            }
            self.update_result = Some(result);
        }
    }

    /// Apply font changes from temp variables to config
    pub fn apply_font_changes(&mut self) {
        self.config.font_family = self.temp_font_family.clone();
        self.config.font_size = self.temp_font_size;
        self.config.line_spacing = self.temp_line_spacing;
        self.config.char_spacing = self.temp_char_spacing;
        self.config.enable_text_shaping = self.temp_enable_text_shaping;
        self.config.enable_ligatures = self.temp_enable_ligatures;
        self.config.enable_kerning = self.temp_enable_kerning;
        self.config.font_family_bold = if self.temp_font_bold.is_empty() {
            None
        } else {
            Some(self.temp_font_bold.clone())
        };
        self.config.font_family_italic = if self.temp_font_italic.is_empty() {
            None
        } else {
            Some(self.temp_font_italic.clone())
        };
        self.config.font_family_bold_italic = if self.temp_font_bold_italic.is_empty() {
            None
        } else {
            Some(self.temp_font_bold_italic.clone())
        };
        self.font_pending_changes = false;
    }

    /// Toggle settings window visibility
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        if self.visible {
            self.focus_search = true;
        }
    }

    /// Get a reference to the working config (for live sync)
    pub fn current_config(&self) -> &Config {
        &self.config
    }

    /// Sync the current collapsed sections state into the config's persisted field.
    pub(super) fn sync_collapsed_sections_to_config(&mut self) {
        self.config.collapsed_settings_sections = self.collapsed_sections.iter().cloned().collect();
    }

    /// Get a snapshot of the current collapsed section IDs for persistence on close.
    pub fn collapsed_sections_snapshot(&self) -> Vec<String> {
        self.collapsed_sections.iter().cloned().collect()
    }

    /// Check if a test notification was requested and clear the flag
    pub fn take_test_notification_request(&mut self) -> bool {
        let requested = self.test_notification_requested;
        self.test_notification_requested = false;
        requested
    }

    /// Sync profiles from the main window's profile manager into the inline editor.
    pub fn sync_profiles(&mut self, profiles: Vec<Profile>) {
        self.profile_modal_ui.load_profiles(profiles);
    }

    /// Take profile save request: returns working profiles if save was requested.
    pub fn take_profile_save_request(&mut self) -> Option<Vec<Profile>> {
        if self.profile_save_requested {
            self.profile_save_requested = false;
            Some(self.profile_modal_ui.get_working_profiles().to_vec())
        } else {
            None
        }
    }

    /// Take profile open request: returns and clears the profile ID to open.
    pub fn take_profile_open_request(&mut self) -> Option<ProfileId> {
        self.profile_open_requested.take()
    }
}
