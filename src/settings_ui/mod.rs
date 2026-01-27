//! Settings UI for the terminal emulator.
//!
//! This module provides an egui-based settings window for configuring
//! terminal options at runtime.

use crate::config::{Config, ShaderMetadataCache};
use egui::{Color32, Context, Frame, Window, epaint::Shadow};
use rfd::FileDialog;

pub mod background_tab;
pub mod bell_tab;
mod cursor_shader_editor;
pub mod cursor_tab;
pub mod font_tab;
pub mod mouse_tab;
pub mod screenshot_tab;
pub mod scrollbar_tab;
mod shader_dialogs;
mod shader_editor;
mod shader_utils;
pub mod shell_tab;
pub mod tab_bar_tab;
pub mod terminal_tab;
pub mod theme_tab;
pub mod window_tab;

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
    /// Whether the per-shader settings section is expanded
    pub(crate) shader_settings_expanded: bool,
}

impl SettingsUI {
    /// Create a new settings UI
    pub fn new(config: Config) -> Self {
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
            temp_background_image: config.background_image.clone().unwrap_or_default(),
            temp_custom_shader: config.custom_shader.clone().unwrap_or_default(),
            temp_cursor_shader: config.cursor_shader.clone().unwrap_or_default(),
            temp_shader_channel0: config.custom_shader_channel0.clone().unwrap_or_default(),
            temp_shader_channel1: config.custom_shader_channel1.clone().unwrap_or_default(),
            temp_shader_channel2: config.custom_shader_channel2.clone().unwrap_or_default(),
            temp_shader_channel3: config.custom_shader_channel3.clone().unwrap_or_default(),
            temp_cubemap_path: config.custom_shader_cubemap.clone().unwrap_or_default(),
            last_live_opacity: config.window_opacity,
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
            shader_settings_expanded: true,
        }
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
    pub fn current_config(&self) -> &Config {
        &self.config
    }

    /// Show the settings window and return results
    /// - First Option: Some(config) if save was clicked (persist to disk)
    /// - Second Option: Some(config) if any changes were made (apply immediately)
    /// - Third Option: Some(ShaderEditorResult) if background shader Apply was clicked
    /// - Fourth Option: Some(CursorShaderEditorResult) if cursor shader Apply was clicked
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

    /// Show all settings sections filtered by search query
    fn show_settings_sections(&mut self, ui: &mut egui::Ui, changes_this_frame: &mut bool) {
        let query = self.search_query.trim().to_lowercase();
        let mut matches_found = false;
        let mut section_shown = false;

        let insert_section_separator = |ui: &mut egui::Ui, shown: &mut bool| {
            if *shown {
                ui.separator();
            } else {
                *shown = true;
            }
        };

        let section_matches = |title: &str, fields: &[&str]| -> bool {
            if query.is_empty() {
                return true;
            }

            let q = query.as_str();
            title.to_lowercase().contains(q) || fields.iter().any(|f| f.to_lowercase().contains(q))
        };

        // Window & Display
        if section_matches(
            "Window & Display",
            &[
                "Title",
                "Width",
                "Height",
                "Padding",
                "Opacity",
                "Decorations",
                "Always on top",
                "Max FPS",
                "VSync",
                "Power Saving",
                "Pause shaders",
                "Unfocused",
                "blur",
            ],
        ) {
            insert_section_separator(ui, &mut section_shown);
            matches_found = true;
            window_tab::show(ui, self, changes_this_frame);
        }

        // Terminal
        if section_matches(
            "Terminal",
            &["Columns", "Rows", "Scrollback", "Exit when shell exits"],
        ) {
            insert_section_separator(ui, &mut section_shown);
            matches_found = true;
            terminal_tab::show(ui, self, changes_this_frame);
        }

        // Font Settings
        if section_matches(
            "Font",
            &[
                "Family",
                "Bold",
                "Italic",
                "Size",
                "Line spacing",
                "Char spacing",
                "Text shaping",
                "Ligatures",
                "Kerning",
            ],
        ) {
            insert_section_separator(ui, &mut section_shown);
            matches_found = true;
            font_tab::show(ui, self, changes_this_frame);
        }

        // Theme & Colors
        if section_matches("Theme & Colors", &["Theme"]) {
            insert_section_separator(ui, &mut section_shown);
            matches_found = true;
            theme_tab::show(ui, self, changes_this_frame);
        }

        // Background & Effects
        if section_matches(
            "Background & Effects",
            &[
                "Background image",
                "Enable background image",
                "Shader",
                "Enable shader",
                "Opacity",
                "Animation",
                "Mode",
                "Text opacity",
            ],
        ) {
            insert_section_separator(ui, &mut section_shown);
            matches_found = true;
            background_tab::show_background(ui, self, changes_this_frame);
        }

        // Cursor Shader (separate from background shader)
        if section_matches(
            "Cursor Shader",
            &["Cursor shader", "Cursor effect", "Cursor animation"],
        ) {
            insert_section_separator(ui, &mut section_shown);
            matches_found = true;
            background_tab::show_cursor_shader(ui, self, changes_this_frame);
        }

        // Cursor
        if section_matches("Cursor", &["Style", "Blink", "Blink interval", "Color"]) {
            insert_section_separator(ui, &mut section_shown);
            matches_found = true;
            cursor_tab::show(ui, self, changes_this_frame);
        }

        // Selection & Clipboard
        if section_matches(
            "Selection & Clipboard",
            &[
                "Auto-copy",
                "Trailing newline",
                "Middle-click",
                "Max clipboard",
            ],
        ) {
            insert_section_separator(ui, &mut section_shown);
            matches_found = true;
            mouse_tab::show_selection(ui, self, changes_this_frame);
        }

        // Mouse Behavior
        if section_matches(
            "Mouse Behavior",
            &["Scroll speed", "Double-click", "Triple-click"],
        ) {
            insert_section_separator(ui, &mut section_shown);
            matches_found = true;
            mouse_tab::show_mouse_behavior(ui, self, changes_this_frame);
        }

        // Scrollbar
        if section_matches(
            "Scrollbar",
            &[
                "Width",
                "Autohide",
                "Position",
                "Thumb color",
                "Track color",
            ],
        ) {
            insert_section_separator(ui, &mut section_shown);
            matches_found = true;
            scrollbar_tab::show(ui, self, changes_this_frame);
        }

        // Bell & Notifications
        if section_matches(
            "Bell & Notifications",
            &[
                "Visual bell",
                "Audio bell",
                "Desktop notifications",
                "Activity",
                "Silence",
                "Notification buffer",
            ],
        ) {
            insert_section_separator(ui, &mut section_shown);
            matches_found = true;
            bell_tab::show(ui, self, changes_this_frame);
        }

        // Shell Configuration
        if section_matches(
            "Shell Configuration",
            &[
                "Custom shell",
                "Shell args",
                "Working directory",
                "Login shell",
            ],
        ) {
            insert_section_separator(ui, &mut section_shown);
            matches_found = true;
            shell_tab::show(ui, self, changes_this_frame);
        }

        // Tab Bar
        if section_matches(
            "Tab Bar",
            &[
                "Tab bar",
                "Tab background",
                "Tab text",
                "Tab indicator",
                "Tab close",
                "Active tab",
                "Inactive tab",
                "Bell",
                "Activity",
            ],
        ) {
            insert_section_separator(ui, &mut section_shown);
            matches_found = true;
            tab_bar_tab::show(ui, self, changes_this_frame);
        }

        // Screenshot
        if section_matches("Screenshot", &["Format", "png", "jpeg", "svg", "html"]) {
            insert_section_separator(ui, &mut section_shown);
            matches_found = true;
            screenshot_tab::show(ui, self, changes_this_frame);
        }

        if !matches_found && !query.is_empty() {
            ui.label(format!("No settings match \"{}\"", self.search_query));
        }
    }
}
