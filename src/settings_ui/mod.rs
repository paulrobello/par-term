use crate::config::Config;
use arboard::Clipboard;
use egui::{Color32, Context, Frame, Window, epaint::Shadow};
use rfd::FileDialog;

pub mod background_tab;
pub mod bell_tab;
pub mod cursor_tab;
pub mod font_tab;
pub mod mouse_tab;
pub mod screenshot_tab;
pub mod scrollbar_tab;
pub mod shell_tab;
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
            new_shader_name: String::new(),
            show_create_shader_dialog: false,
            show_delete_shader_dialog: false,
            shader_search_query: String::new(),
            shader_search_matches: Vec::new(),
            shader_search_current: 0,
            shader_search_visible: false,
        }
    }

    /// Scan the shaders folder and return a list of shader filenames
    fn scan_shaders_folder() -> Vec<String> {
        let shaders_dir = crate::config::Config::shaders_dir();
        let mut shaders = Vec::new();

        // Create the shaders directory if it doesn't exist
        if !shaders_dir.exists()
            && let Err(e) = std::fs::create_dir_all(&shaders_dir)
        {
            log::warn!("Failed to create shaders directory: {}", e);
            return shaders;
        }

        // Read all .glsl files from the shaders directory
        if let Ok(entries) = std::fs::read_dir(&shaders_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file()
                    && let Some(ext) = path.extension()
                    && (ext == "glsl" || ext == "frag" || ext == "shader")
                    && let Some(name) = path.file_name()
                {
                    shaders.push(name.to_string_lossy().to_string());
                }
            }
        }

        shaders.sort();
        shaders
    }

    /// Refresh the list of available shaders
    pub fn refresh_shaders(&mut self) {
        self.available_shaders = Self::scan_shaders_folder();
    }

    /// Get background shaders (excludes cursor_* shaders)
    pub(crate) fn background_shaders(&self) -> Vec<String> {
        self.available_shaders
            .iter()
            .filter(|s| !s.starts_with("cursor_"))
            .cloned()
            .collect()
    }

    /// Get cursor shaders (only cursor_* shaders)
    pub(crate) fn cursor_shaders(&self) -> Vec<String> {
        self.available_shaders
            .iter()
            .filter(|s| s.starts_with("cursor_"))
            .cloned()
            .collect()
    }

    /// Set shader compilation error (called from app when shader fails to compile)
    pub fn set_shader_error(&mut self, error: Option<String>) {
        self.shader_editor_error = error;
    }

    /// Clear shader error
    pub fn clear_shader_error(&mut self) {
        self.shader_editor_error = None;
    }

    /// Set cursor shader compilation error
    pub fn set_cursor_shader_error(&mut self, error: Option<String>) {
        self.cursor_shader_editor_error = error;
    }

    /// Clear cursor shader error
    pub fn clear_cursor_shader_error(&mut self) {
        self.cursor_shader_editor_error = None;
    }

    /// Check if cursor shader editor is visible
    #[allow(dead_code)]
    pub fn is_cursor_shader_editor_visible(&self) -> bool {
        self.cursor_shader_editor_visible
    }

    /// Open the shader editor directly (without opening settings)
    ///
    /// Returns true if the editor was opened, false if no shader path is configured
    pub fn open_shader_editor(&mut self) -> bool {
        if self.temp_custom_shader.is_empty() {
            log::warn!("Cannot open shader editor: no shader path configured");
            return false;
        }

        // Load shader source from file
        let shader_path = crate::config::Config::shader_path(&self.temp_custom_shader);
        match std::fs::read_to_string(&shader_path) {
            Ok(source) => {
                self.shader_editor_source = source.clone();
                self.shader_editor_original = source;
                self.shader_editor_error = None;
                self.shader_editor_visible = true;
                log::info!("Shader editor opened for: {}", shader_path.display());
                true
            }
            Err(e) => {
                self.shader_editor_error = Some(format!(
                    "Failed to read shader file '{}': {}",
                    shader_path.display(),
                    e
                ));
                self.shader_editor_visible = true; // Show editor with error
                log::error!("Failed to load shader: {}", e);
                true
            }
        }
    }

    /// Update search matches based on current query
    fn update_shader_search_matches(&mut self) {
        self.shader_search_matches.clear();
        self.shader_search_current = 0;

        if self.shader_search_query.is_empty() {
            return;
        }

        let query_lower = self.shader_search_query.to_lowercase();
        let source_lower = self.shader_editor_source.to_lowercase();

        let mut start = 0;
        while let Some(pos) = source_lower[start..].find(&query_lower) {
            self.shader_search_matches.push(start + pos);
            start += pos + query_lower.len();
        }
    }

    /// Move to next search match
    fn shader_search_next(&mut self) {
        if !self.shader_search_matches.is_empty() {
            self.shader_search_current =
                (self.shader_search_current + 1) % self.shader_search_matches.len();
        }
    }

    /// Move to previous search match
    fn shader_search_previous(&mut self) {
        if !self.shader_search_matches.is_empty() {
            if self.shader_search_current == 0 {
                self.shader_search_current = self.shader_search_matches.len() - 1;
            } else {
                self.shader_search_current -= 1;
            }
        }
    }

    /// Get the current match position (byte offset) if any
    fn shader_search_current_pos(&self) -> Option<usize> {
        if self.shader_search_matches.is_empty() {
            None
        } else {
            Some(self.shader_search_matches[self.shader_search_current])
        }
    }

    /// Check if shader editor is visible
    pub fn is_shader_editor_visible(&self) -> bool {
        self.shader_editor_visible
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
        let mut shader_apply_result: Option<ShaderEditorResult> = None;
        let mut cursor_shader_apply_result: Option<CursorShaderEditorResult> = None;

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
                                title.to_lowercase().contains(q)
                                    || fields.iter().any(|f| f.to_lowercase().contains(q))
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
                                ],
                            ) {
                                insert_section_separator(ui, &mut section_shown);
                                matches_found = true;
                                window_tab::show(ui, self, &mut changes_this_frame);
                            }

                            // Terminal
                            if section_matches(
                                "Terminal",
                                &["Columns", "Rows", "Scrollback", "Exit when shell exits"],
                            ) {
                                insert_section_separator(ui, &mut section_shown);
                                matches_found = true;
                                terminal_tab::show(ui, self, &mut changes_this_frame);
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
                                font_tab::show(ui, self, &mut changes_this_frame);
                            }

                            // Theme & Colors
                            if section_matches("Theme & Colors", &["Theme"]) {
                                insert_section_separator(ui, &mut section_shown);
                                matches_found = true;
                                theme_tab::show(ui, self, &mut changes_this_frame);
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
                                background_tab::show_background(ui, self, &mut changes_this_frame);
                            }

                            // Cursor Shader (separate from background shader)
                            if section_matches(
                                "Cursor Shader",
                                &["Cursor shader", "Cursor effect", "Cursor animation"],
                            ) {
                                insert_section_separator(ui, &mut section_shown);
                                matches_found = true;
                                background_tab::show_cursor_shader(
                                    ui,
                                    self,
                                    &mut changes_this_frame,
                                );
                            }

                            // Cursor
                            if section_matches(
                                "Cursor",
                                &["Style", "Blink", "Blink interval", "Color"],
                            ) {
                                insert_section_separator(ui, &mut section_shown);
                                matches_found = true;
                                cursor_tab::show(ui, self, &mut changes_this_frame);
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
                                mouse_tab::show_selection(ui, self, &mut changes_this_frame);
                            }

                            // Mouse Behavior
                            if section_matches(
                                "Mouse Behavior",
                                &["Scroll speed", "Double-click", "Triple-click"],
                            ) {
                                insert_section_separator(ui, &mut section_shown);
                                matches_found = true;
                                mouse_tab::show_mouse_behavior(ui, self, &mut changes_this_frame);
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
                                scrollbar_tab::show(ui, self, &mut changes_this_frame);
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
                                bell_tab::show(ui, self, &mut changes_this_frame);
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
                                shell_tab::show(ui, self, &mut changes_this_frame);
                            }

                            // Screenshot
                            if section_matches(
                                "Screenshot",
                                &["Format", "png", "jpeg", "svg", "html"],
                            ) {
                                insert_section_separator(ui, &mut section_shown);
                                matches_found = true;
                                screenshot_tab::show(ui, self, &mut changes_this_frame);
                            }
                            if !matches_found && !query.is_empty() {
                                ui.label(format!("No settings match \"{}\"", self.search_query));
                            }
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

        // Show shader editor window if visible
        if self.shader_editor_visible {
            let mut shader_editor_open = true;
            let mut apply_clicked = false;
            let mut cancel_clicked = false;
            let mut save_to_file_clicked = false;

            // Calculate 90% of viewport height
            let viewport = ctx.input(|i| i.viewport_rect());
            let window_height = viewport.height() * 0.9;

            let bg_shader_filename = &self.temp_custom_shader;
            Window::new(format!("Background Shader Editor - {}", bg_shader_filename))
                .resizable(true)
                .default_width(900.0)
                .default_height(window_height)
                .default_pos(viewport.center())
                .pivot(egui::Align2::CENTER_CENTER)
                .open(&mut shader_editor_open)
                .frame(
                    Frame::window(&ctx.style())
                        .fill(Color32::from_rgba_unmultiplied(20, 20, 20, 255))
                        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(60, 60, 60)))
                        .shadow(Shadow {
                            offset: [2, 2],
                            blur: 8,
                            spread: 0,
                            color: Color32::from_black_alpha(128),
                        }),
                )
                .show(ctx, |ui| {
                    ui.heading("GLSL Shader Editor (F11 to toggle)");
                    ui.horizontal(|ui| {
                        ui.label("Edit your custom shader below. Click Apply to test changes.");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.small("Ctrl+F to search");
                        });
                    });
                    ui.separator();

                    // Search bar (Ctrl+F to toggle)
                    let ctrl_f = ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::F));
                    let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));

                    if ctrl_f {
                        self.shader_search_visible = !self.shader_search_visible;
                        if self.shader_search_visible {
                            // Focus will be requested below
                        }
                    }
                    if escape && self.shader_search_visible {
                        self.shader_search_visible = false;
                    }

                    if self.shader_search_visible {
                        ui.horizontal(|ui| {
                            ui.label("Find:");
                            let search_field = ui.add(
                                egui::TextEdit::singleline(&mut self.shader_search_query)
                                    .desired_width(200.0)
                                    .hint_text("Search..."),
                            );

                            // Focus search field when first shown
                            if ctrl_f {
                                search_field.request_focus();
                            }

                            // Update matches when query changes
                            if search_field.changed() {
                                self.update_shader_search_matches();
                            }

                            // Handle Enter for next match, Shift+Enter for previous
                            let enter_pressed = ui.input(|i| i.key_pressed(egui::Key::Enter));
                            let shift_held = ui.input(|i| i.modifiers.shift);

                            // Previous/Next buttons
                            let has_matches = !self.shader_search_matches.is_empty();
                            if ui
                                .add_enabled(has_matches, egui::Button::new("◀"))
                                .on_hover_text("Previous (Shift+Enter)")
                                .clicked()
                                || (enter_pressed && shift_held && has_matches)
                            {
                                self.shader_search_previous();
                            }
                            if ui
                                .add_enabled(has_matches, egui::Button::new("▶"))
                                .on_hover_text("Next (Enter)")
                                .clicked()
                                || (enter_pressed && !shift_held && has_matches)
                            {
                                self.shader_search_next();
                            }

                            // Match count
                            if self.shader_search_query.is_empty() {
                                ui.label("");
                            } else if self.shader_search_matches.is_empty() {
                                ui.colored_label(Color32::from_rgb(255, 100, 100), "No matches");
                            } else {
                                ui.label(format!(
                                    "{} / {}",
                                    self.shader_search_current + 1,
                                    self.shader_search_matches.len()
                                ));
                            }

                            // Close button
                            if ui.button("✕").on_hover_text("Close (Esc)").clicked() {
                                self.shader_search_visible = false;
                            }
                        });
                        ui.separator();
                    }

                    // Show error dialog if there's an error
                    let mut dismiss_error = false;
                    if let Some(error) = &self.shader_editor_error {
                        let error_text = error.clone();
                        let shader_path =
                            crate::config::Config::shader_path(&self.temp_custom_shader);
                        let full_error =
                            format!("File: {}\n\n{}", shader_path.display(), error_text);

                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.colored_label(
                                    Color32::from_rgb(255, 100, 100),
                                    "⚠ Shader Compilation Error",
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.button("Dismiss").clicked() {
                                            dismiss_error = true;
                                        }
                                        if ui.button("Copy").clicked()
                                            && let Ok(mut clipboard) = Clipboard::new()
                                        {
                                            let _ = clipboard.set_text(full_error.clone());
                                        }
                                    },
                                );
                            });
                            ui.label(format!("File: {}", shader_path.display()));
                            ui.separator();
                            // Multiline selectable text for copying
                            egui::ScrollArea::vertical()
                                .max_height(120.0)
                                .show(ui, |ui| {
                                    ui.add(
                                        egui::TextEdit::multiline(&mut error_text.as_str())
                                            .font(egui::TextStyle::Monospace)
                                            .desired_width(f32::INFINITY)
                                            .interactive(true),
                                    );
                                });
                        });
                        ui.separator();
                    }
                    if dismiss_error {
                        self.shader_editor_error = None;
                    }

                    // Shader source editor
                    // Note: code_editor() provides a dark theme optimized for code
                    let available_height = ui.available_height() - 60.0; // Reserve space for buttons

                    // Get current search match position before rendering
                    let search_selection = self.shader_search_current_pos().map(|pos| {
                        let end = pos + self.shader_search_query.len();
                        (pos, end)
                    });

                    let editor_id = egui::Id::new("shader_editor_textedit");

                    egui::ScrollArea::both()
                        .auto_shrink([false, false])
                        .max_height(available_height)
                        .show(ui, |ui| {
                            let response = ui.add(
                                egui::TextEdit::multiline(&mut self.shader_editor_source)
                                    .id(editor_id)
                                    .font(egui::TextStyle::Monospace)
                                    .code_editor()
                                    .desired_width(f32::INFINITY)
                                    .min_size(egui::vec2(
                                        ui.available_width(),
                                        available_height - 20.0,
                                    )),
                            );

                            // If we have a search match, select it and scroll to it
                            if let Some((start, end)) = search_selection
                                && let Some(mut state) =
                                    egui::TextEdit::load_state(ui.ctx(), editor_id)
                            {
                                // Create a cursor range that selects the match
                                let ccursor_range = egui::text::CCursorRange::two(
                                    egui::text::CCursor::new(start),
                                    egui::text::CCursor::new(end),
                                );
                                state.cursor.set_char_range(Some(ccursor_range));
                                state.store(ui.ctx(), editor_id);

                                // Request scroll to cursor
                                ui.scroll_to_rect(response.rect, Some(egui::Align::Center));
                            }
                        });

                    ui.separator();

                    // Action buttons
                    ui.horizontal(|ui| {
                        if ui.button("Apply").clicked() {
                            apply_clicked = true;
                        }
                        ui.label("|");
                        if ui.button("Save to File").clicked() {
                            save_to_file_clicked = true;
                        }
                        ui.label("|");
                        if ui.button("Find").on_hover_text("Ctrl+F").clicked() {
                            self.shader_search_visible = !self.shader_search_visible;
                        }
                        ui.label("|");
                        if ui.button("Revert").clicked() {
                            self.shader_editor_source = self.shader_editor_original.clone();
                            self.shader_editor_error = None;
                        }
                        ui.label("|");
                        if ui.button("Close").clicked() {
                            cancel_clicked = true;
                        }
                    });
                });

            // Handle shader editor actions
            if apply_clicked {
                shader_apply_result = Some(ShaderEditorResult {
                    source: self.shader_editor_source.clone(),
                });
                // Don't close editor - let user see if it worked or get error
            }

            if save_to_file_clicked {
                // Save current source to the shader file
                let shader_path = crate::config::Config::shader_path(&self.temp_custom_shader);
                match std::fs::write(&shader_path, &self.shader_editor_source) {
                    Ok(()) => {
                        self.shader_editor_original = self.shader_editor_source.clone();
                        log::info!("Shader saved to {}", shader_path.display());
                    }
                    Err(e) => {
                        self.shader_editor_error = Some(format!(
                            "Failed to save shader file '{}': {}",
                            shader_path.display(),
                            e
                        ));
                    }
                }
            }

            if cancel_clicked || !shader_editor_open {
                self.shader_editor_visible = false;
                self.shader_editor_source.clear();
                self.shader_editor_original.clear();
                self.shader_editor_error = None;
                // Clear search state
                self.shader_search_query.clear();
                self.shader_search_matches.clear();
                self.shader_search_current = 0;
                self.shader_search_visible = false;
            }
        }

        // Show cursor shader editor window if visible
        if self.cursor_shader_editor_visible {
            let mut cursor_shader_editor_open = true;
            let mut cursor_apply_clicked = false;
            let mut cursor_cancel_clicked = false;
            let mut cursor_save_to_file_clicked = false;

            // Calculate 90% of viewport height
            let viewport = ctx.input(|i| i.viewport_rect());
            let window_height = viewport.height() * 0.9;

            let cursor_shader_filename = &self.temp_cursor_shader;
            Window::new(format!("Cursor Shader Editor - {}", cursor_shader_filename))
                .resizable(true)
                .default_width(900.0)
                .default_height(window_height)
                .default_pos(viewport.center())
                .pivot(egui::Align2::CENTER_CENTER)
                .open(&mut cursor_shader_editor_open)
                .frame(
                    Frame::window(&ctx.style())
                        .fill(Color32::from_rgba_unmultiplied(20, 20, 20, 255))
                        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(60, 60, 60)))
                        .shadow(Shadow {
                            offset: [2, 2],
                            blur: 8,
                            spread: 0,
                            color: Color32::from_black_alpha(128),
                        }),
                )
                .show(ctx, |ui| {
                    ui.heading("Cursor GLSL Shader Editor");
                    ui.label("Edit your cursor shader below. Click Apply to test changes.");
                    ui.separator();

                    // Show error dialog if there's an error
                    let mut dismiss_error = false;
                    if let Some(error) = &self.cursor_shader_editor_error {
                        let error_text = error.clone();
                        let shader_path =
                            crate::config::Config::shader_path(&self.temp_cursor_shader);
                        let full_error =
                            format!("File: {}\n\n{}", shader_path.display(), error_text);

                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.colored_label(
                                    Color32::from_rgb(255, 100, 100),
                                    "⚠ Cursor Shader Compilation Error",
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.button("Dismiss").clicked() {
                                            dismiss_error = true;
                                        }
                                        if ui.button("Copy").clicked()
                                            && let Ok(mut clipboard) = Clipboard::new()
                                        {
                                            let _ = clipboard.set_text(full_error.clone());
                                        }
                                    },
                                );
                            });
                            ui.label(format!("File: {}", shader_path.display()));
                            ui.separator();
                            // Multiline selectable text for copying
                            egui::ScrollArea::vertical()
                                .max_height(120.0)
                                .show(ui, |ui| {
                                    ui.add(
                                        egui::TextEdit::multiline(&mut error_text.as_str())
                                            .font(egui::TextStyle::Monospace)
                                            .desired_width(f32::INFINITY)
                                            .interactive(true),
                                    );
                                });
                        });
                        ui.separator();
                    }
                    if dismiss_error {
                        self.cursor_shader_editor_error = None;
                    }

                    // Cursor shader source editor
                    let available_height = ui.available_height() - 60.0; // Reserve space for buttons

                    let cursor_editor_id = egui::Id::new("cursor_shader_editor_textedit");

                    egui::ScrollArea::both()
                        .auto_shrink([false, false])
                        .max_height(available_height)
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut self.cursor_shader_editor_source)
                                    .id(cursor_editor_id)
                                    .font(egui::TextStyle::Monospace)
                                    .code_editor()
                                    .desired_width(f32::INFINITY)
                                    .min_size(egui::vec2(
                                        ui.available_width(),
                                        available_height - 20.0,
                                    )),
                            );
                        });

                    ui.separator();

                    // Action buttons
                    ui.horizontal(|ui| {
                        if ui.button("Apply").clicked() {
                            cursor_apply_clicked = true;
                        }
                        ui.label("|");
                        if ui.button("Save to File").clicked() {
                            cursor_save_to_file_clicked = true;
                        }
                        ui.label("|");
                        if ui.button("Revert").clicked() {
                            self.cursor_shader_editor_source =
                                self.cursor_shader_editor_original.clone();
                            self.cursor_shader_editor_error = None;
                        }
                        ui.label("|");
                        if ui.button("Close").clicked() {
                            cursor_cancel_clicked = true;
                        }
                    });
                });

            // Handle cursor shader editor actions
            if cursor_apply_clicked {
                cursor_shader_apply_result = Some(CursorShaderEditorResult {
                    source: self.cursor_shader_editor_source.clone(),
                });
                // Don't close editor - let user see if it worked or get error
            }

            if cursor_save_to_file_clicked {
                // Save current source to the cursor shader file
                let shader_path = crate::config::Config::shader_path(&self.temp_cursor_shader);
                match std::fs::write(&shader_path, &self.cursor_shader_editor_source) {
                    Ok(()) => {
                        self.cursor_shader_editor_original =
                            self.cursor_shader_editor_source.clone();
                        log::info!("Cursor shader saved to {}", shader_path.display());
                    }
                    Err(e) => {
                        self.cursor_shader_editor_error = Some(format!(
                            "Failed to save cursor shader file '{}': {}",
                            shader_path.display(),
                            e
                        ));
                    }
                }
            }

            if cursor_cancel_clicked || !cursor_shader_editor_open {
                self.cursor_shader_editor_visible = false;
                self.cursor_shader_editor_source.clear();
                self.cursor_shader_editor_original.clear();
                self.cursor_shader_editor_error = None;
            }
        }

        // Create Shader Dialog
        if self.show_create_shader_dialog {
            let mut close_dialog = false;
            let mut create_shader = false;

            Window::new("Create New Shader")
                .collapsible(false)
                .resizable(false)
                .default_width(400.0)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("Enter a name for the new shader file:");
                    ui.label("(will be saved as .glsl in the shaders folder)");
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        ui.label("Name:");
                        let response = ui.text_edit_singleline(&mut self.new_shader_name);
                        if response.lost_focus()
                            && ui.input(|i| i.key_pressed(egui::Key::Enter))
                            && !self.new_shader_name.is_empty()
                        {
                            create_shader = true;
                        }
                    });

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Create").clicked() && !self.new_shader_name.is_empty() {
                            create_shader = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close_dialog = true;
                        }
                    });
                });

            if create_shader {
                // Ensure filename ends with .glsl
                let mut filename = self.new_shader_name.clone();
                if !filename.ends_with(".glsl")
                    && !filename.ends_with(".frag")
                    && !filename.ends_with(".shader")
                {
                    filename.push_str(".glsl");
                }

                let shader_path = crate::config::Config::shaders_dir().join(&filename);

                // Check if file already exists
                if shader_path.exists() {
                    self.shader_editor_error =
                        Some(format!("Shader '{}' already exists!", filename));
                } else {
                    // Create the shader with a basic template
                    let template = r#"// Custom shader for par-term
// Available uniforms:
//   iTime       - Time in seconds (when animation enabled)
//   iResolution - Viewport resolution (vec2)
//   iChannel0   - Terminal content texture (sampler2D)
//   iOpacity    - Window opacity (float)
//   iTextOpacity - Text opacity (float)

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;

    // Sample terminal content
    vec4 terminal = texture(iChannel0, uv);

    // Example: simple color tint based on position
    vec3 tint = vec3(0.8, 0.9, 1.0);

    // Mix terminal content with effect
    vec3 color = terminal.rgb * tint;

    fragColor = vec4(color, terminal.a);
}
"#;

                    match std::fs::write(&shader_path, template) {
                        Ok(()) => {
                            log::info!("Created new shader: {}", shader_path.display());
                            // Update the shader list
                            self.refresh_shaders();
                            // Select the new shader
                            self.temp_custom_shader = filename.clone();
                            self.config.custom_shader = Some(filename);
                            self.has_changes = true;
                            // Open the shader editor with the new shader
                            self.shader_editor_source = template.to_string();
                            self.shader_editor_original = template.to_string();
                            self.shader_editor_error = None;
                            self.shader_editor_visible = true;
                            close_dialog = true;
                        }
                        Err(e) => {
                            self.shader_editor_error =
                                Some(format!("Failed to create shader: {}", e));
                        }
                    }
                }
            }

            if close_dialog {
                self.show_create_shader_dialog = false;
                self.new_shader_name.clear();
            }
        }

        // Delete Shader Confirmation Dialog
        if self.show_delete_shader_dialog {
            let mut close_dialog = false;
            let mut delete_shader = false;
            let shader_name = self.temp_custom_shader.clone();

            Window::new("Delete Shader")
                .collapsible(false)
                .resizable(false)
                .default_width(350.0)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(format!(
                        "Are you sure you want to delete '{}'?",
                        shader_name
                    ));
                    ui.label("This action cannot be undone.");
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if ui.button("Delete").clicked() {
                            delete_shader = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close_dialog = true;
                        }
                    });
                });

            if delete_shader {
                let shader_path = crate::config::Config::shader_path(&shader_name);
                match std::fs::remove_file(&shader_path) {
                    Ok(()) => {
                        log::info!("Deleted shader: {}", shader_path.display());
                        // Clear the selection
                        self.temp_custom_shader.clear();
                        self.config.custom_shader = None;
                        self.has_changes = true;
                        // Refresh the shader list
                        self.refresh_shaders();
                        close_dialog = true;
                    }
                    Err(e) => {
                        self.shader_editor_error = Some(format!("Failed to delete shader: {}", e));
                        close_dialog = true;
                    }
                }
            }

            if close_dialog {
                self.show_delete_shader_dialog = false;
            }
        }

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
            // No-op for live updates when discarded
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
}

// Note: Syntax highlighting for shader editor could be added via egui_extras::syntax_highlighting
// when the API stabilizes. The code_editor() mode provides a dark theme suitable for code editing.
