//! Settings UI for the terminal emulator.
//!
//! This module provides an egui-based settings window for configuring
//! terminal options at runtime.

use crate::config::{Config, CursorShaderMetadataCache, ShaderMetadataCache};
use egui::{Color32, Context, Frame, Window, epaint::Shadow};
use rfd::FileDialog;
use std::collections::HashSet;

// Reorganized settings tabs (8 consolidated tabs)
pub mod advanced_tab;
pub mod appearance_tab;
pub mod effects_tab;
pub mod input_tab;
pub mod integrations_tab;
pub mod notifications_tab;
pub mod quick_settings;
pub mod section;
pub mod sidebar;
pub mod terminal_tab;
pub mod window_tab;

// Background tab is still needed by effects_tab for delegation
pub mod background_tab;

// Shader editor components (used by background_tab)
mod cursor_shader_editor;
mod shader_dialogs;
mod shader_editor;
mod shader_utils;

pub use sidebar::SettingsTab;

/// Result of shader editor actions
#[derive(Debug, Clone)]
pub struct ShaderEditorResult {
    /// New shader source code to compile and apply
    pub source: String,
}

/// Result of cursor shader editor actions
#[derive(Debug, Clone)]
pub struct CursorShaderEditorResult {
    /// New cursor shader source code to compile and apply
    pub source: String,
}

/// Settings UI manager using egui
pub struct SettingsUI {
    /// Whether the settings window is currently visible
    pub visible: bool,

    /// Working copy of config being edited
    pub(crate) config: Config,

    /// Last opacity value that was forwarded for live updates
    pub(crate) last_live_opacity: f32,

    /// Whether config has unsaved changes
    pub(crate) has_changes: bool,

    /// Temp strings for optional fields (for UI editing)
    pub(crate) temp_font_bold: String,
    pub(crate) temp_font_italic: String,
    pub(crate) temp_font_bold_italic: String,
    pub(crate) temp_font_family: String,
    pub(crate) temp_font_size: f32,
    pub(crate) temp_line_spacing: f32,
    pub(crate) temp_char_spacing: f32,
    pub(crate) temp_enable_text_shaping: bool,
    pub(crate) temp_enable_ligatures: bool,
    pub(crate) temp_enable_kerning: bool,
    pub(crate) font_pending_changes: bool,
    pub(crate) temp_custom_shell: String,
    pub(crate) temp_shell_args: String,
    pub(crate) temp_working_directory: String,
    pub(crate) temp_initial_text: String,
    pub(crate) temp_background_image: String,
    pub(crate) temp_custom_shader: String,
    pub(crate) temp_cursor_shader: String,

    /// Temporary strings for shader channel texture paths (iChannel0-3)
    pub(crate) temp_shader_channel0: String,
    pub(crate) temp_shader_channel1: String,
    pub(crate) temp_shader_channel2: String,
    pub(crate) temp_shader_channel3: String,

    /// Temporary string for cubemap path prefix (iCubemap)
    pub(crate) temp_cubemap_path: String,

    /// Temporary color for solid background color editing
    pub(crate) temp_background_color: [u8; 3],

    /// Search query used to filter settings sections
    pub(crate) search_query: String,

    // Background shader editor state
    /// Whether the shader editor window is visible
    pub(crate) shader_editor_visible: bool,
    /// The shader source code being edited
    pub(crate) shader_editor_source: String,
    /// Shader compilation error message (if any)
    pub(crate) shader_editor_error: Option<String>,
    /// Original source when editor was opened (for cancel)
    pub(crate) shader_editor_original: String,

    // Cursor shader editor state
    /// Whether the cursor shader editor window is visible
    pub(crate) cursor_shader_editor_visible: bool,
    /// The cursor shader source code being edited
    pub(crate) cursor_shader_editor_source: String,
    /// Cursor shader compilation error message (if any)
    pub(crate) cursor_shader_editor_error: Option<String>,
    /// Original cursor shader source when editor was opened (for cancel)
    pub(crate) cursor_shader_editor_original: String,

    // Shader management state
    /// List of available shader files in the shaders folder
    pub(crate) available_shaders: Vec<String>,
    /// List of available cubemap prefixes (e.g., "textures/cubemaps/env-outside")
    pub(crate) available_cubemaps: Vec<String>,
    /// Name for new shader (in create dialog)
    pub(crate) new_shader_name: String,
    /// Whether to show the create shader dialog
    pub(crate) show_create_shader_dialog: bool,
    /// Whether to show the delete confirmation dialog
    pub(crate) show_delete_shader_dialog: bool,

    // Shader editor search state
    /// Search query for shader editor
    pub(crate) shader_search_query: String,
    /// Byte positions of search matches
    pub(crate) shader_search_matches: Vec<usize>,
    /// Current match index (0-based)
    pub(crate) shader_search_current: usize,
    /// Whether search bar is visible
    pub(crate) shader_search_visible: bool,

    // Per-shader configuration state
    /// Cache for parsed shader metadata
    pub(crate) shader_metadata_cache: ShaderMetadataCache,
    /// Cache for parsed cursor shader metadata
    pub(crate) cursor_shader_metadata_cache: CursorShaderMetadataCache,
    /// Whether the per-shader settings section is expanded
    pub(crate) shader_settings_expanded: bool,
    /// Whether the per-cursor-shader settings section is expanded
    pub(crate) cursor_shader_settings_expanded: bool,

    // Current window state (for "Use Current Size" button)
    /// Current terminal columns (actual rendered size, may differ from config)
    pub(crate) current_cols: usize,
    /// Current terminal rows (actual rendered size, may differ from config)
    pub(crate) current_rows: usize,

    // VSync mode support (for runtime validation)
    /// Supported vsync modes for the current display
    pub(crate) supported_vsync_modes: Vec<crate::config::VsyncMode>,
    /// Warning message when an unsupported vsync mode is selected
    pub(crate) vsync_warning: Option<String>,

    // Keybinding recording state
    /// Index of the keybinding currently being recorded (None = not recording)
    pub(crate) keybinding_recording_index: Option<usize>,
    /// The recorded key combination string (displayed during recording)
    pub(crate) keybinding_recorded_combo: Option<String>,

    // Notification test state
    /// Flag to request sending a test notification
    pub(crate) test_notification_requested: bool,

    // New UI state for reorganized settings
    /// Currently selected settings tab (new sidebar navigation)
    pub(crate) selected_tab: SettingsTab,
    /// Set of collapsed section IDs (sections start open by default, collapsed when user collapses them)
    #[allow(dead_code)]
    pub(crate) collapsed_sections: HashSet<String>,

    // Integrations tab action state
    /// Pending shell integration action (install/uninstall)
    pub(crate) shell_integration_action: Option<integrations_tab::ShellIntegrationAction>,
    /// Pending shader action (install/uninstall)
    pub(crate) shader_action: Option<integrations_tab::ShaderAction>,

    // Reset to defaults dialog state
    /// Whether to show the reset to defaults confirmation dialog
    pub(crate) show_reset_defaults_dialog: bool,
}

impl SettingsUI {
    /// Create a new settings UI
    pub fn new(config: Config) -> Self {
        // Extract values before moving config
        let initial_cols = config.cols;
        let initial_rows = config.rows;

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
            last_live_opacity: config.window_opacity,
            current_cols: initial_cols,
            current_rows: initial_rows,
            supported_vsync_modes: vec![
                crate::config::VsyncMode::Immediate,
                crate::config::VsyncMode::Mailbox,
                crate::config::VsyncMode::Fifo,
            ], // Will be updated when renderer is available
            vsync_warning: None,
            config,
            has_changes: false,
            search_query: String::new(),
            shader_editor_visible: false,
            shader_editor_source: String::new(),
            shader_editor_error: None,
            shader_editor_original: String::new(),
            cursor_shader_editor_visible: false,
            cursor_shader_editor_source: String::new(),
            cursor_shader_editor_error: None,
            cursor_shader_editor_original: String::new(),
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
                crate::config::Config::shaders_dir(),
            ),
            cursor_shader_metadata_cache: CursorShaderMetadataCache::with_shaders_dir(
                crate::config::Config::shaders_dir(),
            ),
            shader_settings_expanded: true,
            cursor_shader_settings_expanded: true,
            keybinding_recording_index: None,
            keybinding_recorded_combo: None,
            test_notification_requested: false,
            selected_tab: SettingsTab::default(),
            collapsed_sections: HashSet::new(),
            shell_integration_action: None,
            shader_action: None,
            show_reset_defaults_dialog: false,
        }
    }

    /// Update the current terminal dimensions (called when window resizes)
    #[allow(dead_code)]
    pub fn update_current_size(&mut self, cols: usize, rows: usize) {
        self.current_cols = cols;
        self.current_rows = rows;
    }

    /// Update the list of supported vsync modes (called when renderer is initialized)
    pub fn update_supported_vsync_modes(&mut self, modes: Vec<crate::config::VsyncMode>) {
        self.supported_vsync_modes = modes;
        // Clear any previous warning
        self.vsync_warning = None;
    }

    /// Check if a vsync mode is supported
    pub fn is_vsync_mode_supported(&self, mode: crate::config::VsyncMode) -> bool {
        self.supported_vsync_modes.contains(&mode)
    }

    /// Set vsync warning message (called when an unsupported mode is detected)
    #[allow(dead_code)]
    pub fn set_vsync_warning(&mut self, warning: Option<String>) {
        self.vsync_warning = warning;
    }

    pub(crate) fn pick_file_path(&self, title: &str) -> Option<String> {
        FileDialog::new()
            .set_title(title)
            .pick_file()
            .map(|p| p.display().to_string())
    }

    pub(crate) fn pick_folder_path(&self, title: &str) -> Option<String> {
        FileDialog::new()
            .set_title(title)
            .pick_folder()
            .map(|p| p.display().to_string())
    }

    /// Update the config copy (e.g., when config is reloaded)
    pub fn update_config(&mut self, config: Config) {
        if !self.has_changes {
            self.config = config;
            self.last_live_opacity = self.config.window_opacity;

            // Refresh staged font values only if there aren't unsaved font edits
            if !self.font_pending_changes {
                self.sync_font_temps_from_config();
            }
        }
    }

    fn sync_font_temps_from_config(&mut self) {
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

    /// Sync ALL temp fields from config (used when resetting to defaults)
    fn sync_all_temps_from_config(&mut self) {
        // Font temps
        self.sync_font_temps_from_config();

        // Shell/terminal temps
        self.temp_custom_shell = self.config.custom_shell.clone().unwrap_or_default();
        self.temp_shell_args = self
            .config
            .shell_args
            .as_ref()
            .map(|args| args.join(" "))
            .unwrap_or_default();
        self.temp_working_directory = self.config.working_directory.clone().unwrap_or_default();
        self.temp_initial_text = self.config.initial_text.clone();

        // Background temps
        self.temp_background_image = self.config.background_image.clone().unwrap_or_default();
        self.temp_background_color = self.config.background_color;

        // Shader temps
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

        // Update live opacity tracking
        self.last_live_opacity = self.config.window_opacity;
    }

    /// Reset all settings to their default values
    fn reset_all_to_defaults(&mut self) {
        self.config = Config::default();
        self.sync_all_temps_from_config();
        self.has_changes = true;
        self.search_query.clear();
    }

    /// Show the reset to defaults confirmation dialog
    fn show_reset_defaults_dialog_window(&mut self, ctx: &Context) {
        if !self.show_reset_defaults_dialog {
            return;
        }

        let mut close_dialog = false;
        let mut do_reset = false;

        egui::Window::new("Reset to Defaults")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new("⚠ Warning")
                            .color(egui::Color32::YELLOW)
                            .size(18.0)
                            .strong(),
                    );
                    ui.add_space(10.0);
                    ui.label("This will reset ALL settings to their default values.");
                    ui.add_space(5.0);
                    ui.label(
                        egui::RichText::new("Unsaved changes will be lost. This cannot be undone.")
                            .color(egui::Color32::GRAY),
                    );
                    ui.add_space(15.0);

                    ui.horizontal(|ui| {
                        // Reset button with danger styling
                        let reset_button = egui::Button::new(
                            egui::RichText::new("Reset").color(egui::Color32::WHITE),
                        )
                        .fill(egui::Color32::from_rgb(180, 50, 50));

                        if ui.add(reset_button).clicked() {
                            do_reset = true;
                            close_dialog = true;
                        }

                        ui.add_space(10.0);

                        if ui.button("Cancel").clicked() {
                            close_dialog = true;
                        }
                    });
                    ui.add_space(10.0);
                });
            });

        if do_reset {
            self.reset_all_to_defaults();
        }

        if close_dialog {
            self.show_reset_defaults_dialog = false;
        }
    }

    /// Apply font changes from temp variables to config
    pub(crate) fn apply_font_changes(&mut self) {
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
    /// Note: This is kept for the overlay mode in WindowState, but the primary
    /// settings interface is now a separate window managed by WindowManager.
    #[allow(dead_code)]
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Get a reference to the working config (for live sync)
    #[allow(dead_code)]
    pub fn current_config(&self) -> &Config {
        &self.config
    }

    /// Check if a test notification was requested and clear the flag
    pub fn take_test_notification_request(&mut self) -> bool {
        let requested = self.test_notification_requested;
        self.test_notification_requested = false;
        requested
    }

    /// Show the settings window and return results
    /// - First Option: Some(config) if save was clicked (persist to disk)
    /// - Second Option: Some(config) if any changes were made (apply immediately)
    /// - Third Option: Some(ShaderEditorResult) if background shader Apply was clicked
    /// - Fourth Option: Some(CursorShaderEditorResult) if cursor shader Apply was clicked
    #[allow(dead_code)]
    pub fn show(
        &mut self,
        ctx: &Context,
    ) -> (
        Option<Config>,
        Option<Config>,
        Option<ShaderEditorResult>,
        Option<CursorShaderEditorResult>,
    ) {
        if !self.visible && !self.shader_editor_visible && !self.cursor_shader_editor_visible {
            return (None, None, None, None);
        }

        log::info!("SettingsUI.show() called - visible: true");

        // Handle Escape key to close settings window
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.cursor_shader_editor_visible {
                // Close cursor shader editor first if open
                self.cursor_shader_editor_visible = false;
                self.cursor_shader_editor_error = None;
            } else if self.shader_editor_visible {
                // Close background shader editor first if open
                self.shader_editor_visible = false;
                self.shader_editor_error = None;
            } else if self.visible {
                // Close settings window
                self.visible = false;
                return (None, None, None, None);
            }
        }

        // Ensure settings panel is fully opaque regardless of terminal opacity
        let mut style = (*ctx.style()).clone();
        let solid_bg = Color32::from_rgba_unmultiplied(24, 24, 24, 255);
        style.visuals.window_fill = solid_bg;
        style.visuals.panel_fill = solid_bg;
        style.visuals.widgets.noninteractive.bg_fill = solid_bg;
        ctx.set_style(style);

        let mut save_requested = false;
        let mut discard_requested = false;
        let mut close_requested = false;
        let mut open = true;
        let mut changes_this_frame = false;

        // Only show the main settings window if visible
        if self.visible {
            let settings_viewport = ctx.input(|i| i.viewport_rect());
            Window::new("Settings")
                .resizable(true)
                .default_width(650.0)
                .default_height(700.0)
                .default_pos(settings_viewport.center())
                .pivot(egui::Align2::CENTER_CENTER)
                .open(&mut open)
                .frame(
                    Frame::window(&ctx.style())
                        .fill(solid_bg)
                        .stroke(egui::Stroke::NONE)
                        .shadow(Shadow {
                            offset: [0, 0],
                            blur: 0,
                            spread: 0,
                            color: Color32::TRANSPARENT,
                        }),
                )
                .show(ctx, |ui| {
                    // Reserve space for fixed footer buttons
                    let available_height = ui.available_height();
                    let footer_height = 45.0;

                    // Scrollable content area (takes remaining space above footer)
                    egui::ScrollArea::vertical()
                        .max_height(available_height - footer_height)
                        .show(ui, |ui| {
                            ui.heading("Terminal Settings");
                            ui.horizontal(|ui| {
                                ui.label("Quick search:");
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.search_query)
                                        .hint_text("Type to filter settings"),
                                );
                            });
                            ui.separator();

                            self.show_settings_sections(ui, &mut changes_this_frame);
                        });

                    // Fixed footer with action buttons (outside ScrollArea)
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() {
                            save_requested = true;
                        }

                        if ui.button("Discard").clicked() {
                            discard_requested = true;
                        }

                        if ui.button("Close").clicked() {
                            close_requested = true;
                        }

                        ui.separator();

                        if ui
                            .button("Edit Config File")
                            .on_hover_text("Open config.yaml in your default editor")
                            .clicked()
                        {
                            let config_path = Config::config_path();
                            if let Err(e) = open::that(&config_path) {
                                log::error!("Failed to open config file: {}", e);
                            }
                        }

                        if ui
                            .button("Reset to Defaults")
                            .on_hover_text("Reset all settings to their default values")
                            .clicked()
                        {
                            self.show_reset_defaults_dialog = true;
                        }

                        if self.has_changes {
                            ui.colored_label(egui::Color32::YELLOW, "* Unsaved changes");
                        }
                    });
                });
        }

        // Show shader editor windows (delegated to separate modules)
        let shader_apply_result = self.show_shader_editor_window(ctx);
        let cursor_shader_apply_result = self.show_cursor_shader_editor_window(ctx);

        // Show shader dialogs
        self.show_create_shader_dialog_window(ctx);
        self.show_delete_shader_dialog_window(ctx);

        // Show reset to defaults confirmation dialog
        self.show_reset_defaults_dialog_window(ctx);

        // Update visibility based on window state (only if settings window is being shown)
        if self.visible && (!open || close_requested) {
            self.visible = false;
        }

        // Handle save request
        let config_to_save = if save_requested {
            if self.font_pending_changes {
                self.apply_font_changes();
            }
            self.has_changes = false;
            Some(self.config.clone())
        } else {
            None
        };

        // Handle discard request
        if discard_requested {
            self.has_changes = false;
            self.sync_font_temps_from_config();
        }

        // Push live config while the settings window is open to guarantee real-time updates.
        let config_for_live_update = if self.visible {
            // Only log when the value actually changes to avoid spam
            if (self.config.window_opacity - self.last_live_opacity).abs() > f32::EPSILON {
                log::info!(
                    "SettingsUI: live opacity {:.3} (last {:.3})",
                    self.config.window_opacity,
                    self.last_live_opacity
                );
                self.last_live_opacity = self.config.window_opacity;
            }
            Some(self.config.clone())
        } else {
            None
        };

        (
            config_to_save,
            config_for_live_update,
            shader_apply_result,
            cursor_shader_apply_result,
        )
    }

    /// Show the settings UI as a full-window panel (for standalone settings window)
    /// This renders directly to a CentralPanel instead of creating an egui::Window
    pub fn show_as_panel(
        &mut self,
        ctx: &Context,
    ) -> (
        Option<Config>,
        Option<Config>,
        Option<ShaderEditorResult>,
        Option<CursorShaderEditorResult>,
    ) {
        // Handle Escape key to close shader editors (not the window itself - that's handled by winit)
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.cursor_shader_editor_visible {
                self.cursor_shader_editor_visible = false;
                self.cursor_shader_editor_error = None;
            } else if self.shader_editor_visible {
                self.shader_editor_visible = false;
                self.shader_editor_error = None;
            }
        }

        // Ensure settings panel is fully opaque
        let mut style = (*ctx.style()).clone();
        let solid_bg = Color32::from_rgba_unmultiplied(24, 24, 24, 255);
        style.visuals.window_fill = solid_bg;
        style.visuals.panel_fill = solid_bg;
        style.visuals.widgets.noninteractive.bg_fill = solid_bg;
        ctx.set_style(style);

        let mut save_requested = false;
        let mut discard_requested = false;
        let mut changes_this_frame = false;

        // Render directly to CentralPanel (no egui::Window wrapper)
        egui::CentralPanel::default()
            .frame(Frame::central_panel(&ctx.style()).fill(solid_bg))
            .show(ctx, |ui| {
                // Reserve space for fixed footer buttons
                let available_height = ui.available_height();
                let footer_height = 45.0;

                // Scrollable content area (takes remaining space above footer)
                egui::ScrollArea::vertical()
                    .max_height(available_height - footer_height)
                    .show(ui, |ui| {
                        ui.heading("Terminal Settings");
                        ui.horizontal(|ui| {
                            ui.label("Quick search:");
                            ui.add(
                                egui::TextEdit::singleline(&mut self.search_query)
                                    .hint_text("Type to filter settings"),
                            );
                        });
                        ui.separator();

                        self.show_settings_sections(ui, &mut changes_this_frame);
                    });

                // Fixed footer with action buttons (outside ScrollArea)
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        save_requested = true;
                    }

                    if ui.button("Discard").clicked() {
                        discard_requested = true;
                    }

                    ui.separator();

                    if ui
                        .button("Edit Config File")
                        .on_hover_text("Open config.yaml in your default editor")
                        .clicked()
                    {
                        let config_path = Config::config_path();
                        if let Err(e) = open::that(&config_path) {
                            log::error!("Failed to open config file: {}", e);
                        }
                    }

                    if ui
                        .button("Reset to Defaults")
                        .on_hover_text("Reset all settings to their default values")
                        .clicked()
                    {
                        self.show_reset_defaults_dialog = true;
                    }

                    if self.has_changes {
                        ui.colored_label(egui::Color32::YELLOW, "* Unsaved changes");
                    }
                });
            });

        // Show shader editor windows (delegated to separate modules)
        let shader_apply_result = self.show_shader_editor_window(ctx);
        let cursor_shader_apply_result = self.show_cursor_shader_editor_window(ctx);

        // Show shader dialogs
        self.show_create_shader_dialog_window(ctx);
        self.show_delete_shader_dialog_window(ctx);

        // Show reset to defaults confirmation dialog
        self.show_reset_defaults_dialog_window(ctx);

        // Handle save request
        let config_to_save = if save_requested {
            if self.font_pending_changes {
                self.apply_font_changes();
            }
            self.has_changes = false;
            Some(self.config.clone())
        } else {
            None
        };

        // Handle discard request
        if discard_requested {
            self.has_changes = false;
            self.sync_font_temps_from_config();
        }

        // Push live config for real-time updates
        let config_for_live_update = {
            if (self.config.window_opacity - self.last_live_opacity).abs() > f32::EPSILON {
                log::info!(
                    "SettingsUI: live opacity {:.3} (last {:.3})",
                    self.config.window_opacity,
                    self.last_live_opacity
                );
                self.last_live_opacity = self.config.window_opacity;
            }
            Some(self.config.clone())
        };

        (
            config_to_save,
            config_for_live_update,
            shader_apply_result,
            cursor_shader_apply_result,
        )
    }

    /// Show all settings sections using the new sidebar + tab layout.
    fn show_settings_sections(&mut self, ui: &mut egui::Ui, changes_this_frame: &mut bool) {
        // Quick settings strip at the top
        quick_settings::show(ui, self, changes_this_frame);
        ui.separator();

        // Get available dimensions for the main content area
        let available_width = ui.available_width();
        let available_height = ui.available_height();
        let sidebar_width = 150.0;
        let content_width = (available_width - sidebar_width - 15.0).max(300.0);

        // Main content area with sidebar and tab content
        // Use allocate_ui_with_layout to ensure the horizontal layout fills available height
        let layout = egui::Layout::left_to_right(egui::Align::Min);
        ui.allocate_ui_with_layout(
            egui::vec2(available_width, available_height),
            layout,
            |ui| {
                // Left sidebar for tab navigation (fixed width)
                ui.allocate_ui_with_layout(
                    egui::vec2(sidebar_width, available_height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        sidebar::show(ui, &mut self.selected_tab, &self.search_query);
                    },
                );

                ui.separator();

                // Right content area for selected tab (fills remaining space)
                ui.allocate_ui_with_layout(
                    egui::vec2(content_width, available_height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("settings_tab_content")
                            .max_height(available_height)
                            .show(ui, |ui| {
                                ui.set_min_width(content_width - 20.0);
                                self.show_tab_content(ui, changes_this_frame);
                            });
                    },
                );
            },
        );
    }

    /// Show the content for the currently selected tab.
    fn show_tab_content(&mut self, ui: &mut egui::Ui, changes_this_frame: &mut bool) {
        match self.selected_tab {
            SettingsTab::Appearance => {
                appearance_tab::show(ui, self, changes_this_frame);
            }
            SettingsTab::Window => {
                window_tab::show(ui, self, changes_this_frame);
            }
            SettingsTab::Input => {
                input_tab::show(ui, self, changes_this_frame);
            }
            SettingsTab::Terminal => {
                terminal_tab::show(ui, self, changes_this_frame);
            }
            SettingsTab::Effects => {
                effects_tab::show(ui, self, changes_this_frame);
            }
            SettingsTab::Notifications => {
                notifications_tab::show(ui, self, changes_this_frame);
            }
            SettingsTab::Integrations => {
                self.show_integrations_tab(ui, changes_this_frame);
            }
            SettingsTab::Advanced => {
                advanced_tab::show(ui, self, changes_this_frame);
            }
        }
    }
}
