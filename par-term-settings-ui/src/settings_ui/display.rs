//! Display methods for SettingsUI.
//
//! Contains: show(), show_as_panel(), show_settings_sections(),
//! show_tab_content(), show_reset_defaults_dialog_window(),
//! and check_keybinding_conflict().

use egui::{Color32, Context, Frame, Window, epaint::Shadow};
use par_term_config::Config;

use crate::sidebar::SettingsTab;
use crate::{CursorShaderEditorResult, ShaderEditorResult};

use super::SettingsUI;

impl SettingsUI {
    fn show_reset_defaults_dialog_window(&mut self, ctx: &Context) {
        if !self.show_reset_defaults_dialog {
            return;
        }

        let mut close_dialog = false;
        let mut do_reset = false;

        egui::Window::new("Reset to Defaults")
            .collapsible(false)
            .resizable(false)
            .order(egui::Order::Foreground)
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

    /// Begin shader install asynchronously with optional force overwrite.
    /// The caller must provide a function that performs the actual installation.
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

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.cursor_shader_editor_visible {
                self.cursor_shader_editor_visible = false;
                self.cursor_shader_editor_error = None;
            } else if self.shader_editor_visible {
                self.shader_editor_visible = false;
                self.shader_editor_error = None;
            } else if self.visible {
                self.visible = false;
                return (None, None, None, None);
            }
        }

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
                    // Fixed header area (never scrolls)
                    ui.heading("Terminal Settings");
                    ui.horizontal(|ui| {
                        ui.label("Quick search:");
                        let response = ui.add(
                            egui::TextEdit::singleline(&mut self.search_query)
                                .hint_text("Type to filter settings"),
                        );
                        if self.focus_search {
                            self.focus_search = false;
                            response.request_focus();
                        }
                    });
                    ui.separator();

                    // Settings sections (sidebar + content) fill remaining space
                    // Each has its own scroll area internally
                    self.show_settings_sections(ui, &mut changes_this_frame);

                    // Footer
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

        let shader_apply_result = self.show_shader_editor_window(ctx);
        let cursor_shader_apply_result = self.show_cursor_shader_editor_window(ctx);

        self.show_create_shader_dialog_window(ctx);
        self.show_delete_shader_dialog_window(ctx);
        self.show_reset_defaults_dialog_window(ctx);

        if self.visible && (!open || close_requested) {
            self.visible = false;
        }

        let config_to_save = if save_requested {
            if self.font_pending_changes {
                self.apply_font_changes();
            }
            self.has_changes = false;
            self.sync_collapsed_sections_to_config();
            let mut config = self.config.clone();
            config.generate_snippet_action_keybindings();
            Some(config)
        } else {
            None
        };

        if discard_requested {
            self.has_changes = false;
            self.sync_font_temps_from_config();
        }

        let config_for_live_update = if self.visible {
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
    pub fn show_as_panel(
        &mut self,
        ctx: &Context,
    ) -> (
        Option<Config>,
        Option<Config>,
        Option<ShaderEditorResult>,
        Option<CursorShaderEditorResult>,
    ) {
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.cursor_shader_editor_visible {
                self.cursor_shader_editor_visible = false;
                self.cursor_shader_editor_error = None;
            } else if self.shader_editor_visible {
                self.shader_editor_visible = false;
                self.shader_editor_error = None;
            }
        }

        let mut style = (*ctx.style()).clone();
        let solid_bg = Color32::from_rgba_unmultiplied(24, 24, 24, 255);
        style.visuals.window_fill = solid_bg;
        style.visuals.panel_fill = solid_bg;
        style.visuals.widgets.noninteractive.bg_fill = solid_bg;
        ctx.set_style(style);

        let mut save_requested = false;
        let mut discard_requested = false;
        let mut changes_this_frame = false;

        egui::CentralPanel::default()
            .frame(Frame::central_panel(&ctx.style()).fill(solid_bg))
            .show(ctx, |ui| {
                // Fixed header area (never scrolls)
                ui.heading("Terminal Settings");
                ui.horizontal(|ui| {
                    ui.label("Quick search:");
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut self.search_query)
                            .hint_text("Type to filter settings"),
                    );
                    if self.focus_search {
                        self.focus_search = false;
                        response.request_focus();
                    }
                });
                ui.separator();

                // Settings sections (sidebar + content) fill remaining space
                // Each has its own scroll area internally
                self.show_settings_sections(ui, &mut changes_this_frame);

                // Footer
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

        let shader_apply_result = self.show_shader_editor_window(ctx);
        let cursor_shader_apply_result = self.show_cursor_shader_editor_window(ctx);

        self.show_create_shader_dialog_window(ctx);
        self.show_delete_shader_dialog_window(ctx);
        self.show_reset_defaults_dialog_window(ctx);

        let config_to_save = if save_requested {
            if self.font_pending_changes {
                self.apply_font_changes();
            }
            self.has_changes = false;
            self.sync_collapsed_sections_to_config();
            let mut config = self.config.clone();
            config.generate_snippet_action_keybindings();
            Some(config)
        } else {
            None
        };

        if discard_requested {
            self.has_changes = false;
            self.sync_font_temps_from_config();
        }

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
        crate::quick_settings::show(ui, self, changes_this_frame);
        ui.separator();

        let available_width = ui.available_width();
        // Reserve space for the footer (separator + button row)
        let footer_height = 45.0;
        let available_height = (ui.available_height() - footer_height).max(100.0);
        let sidebar_width = 150.0;
        let content_width = (available_width - sidebar_width - 15.0).max(300.0);

        let layout = egui::Layout::left_to_right(egui::Align::Min);
        ui.allocate_ui_with_layout(
            egui::vec2(available_width, available_height),
            layout,
            |ui| {
                // Sidebar with its own scroll area
                ui.allocate_ui_with_layout(
                    egui::vec2(sidebar_width, available_height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("settings_sidebar")
                            .max_height(available_height)
                            .show(ui, |ui| {
                                crate::sidebar::show(
                                    ui,
                                    &mut self.selected_tab,
                                    &self.search_query,
                                );
                            });
                    },
                );

                ui.separator();

                // Content area with its own scroll area
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
        let mut collapsed = std::mem::take(&mut self.collapsed_sections);

        match self.selected_tab {
            SettingsTab::Appearance => {
                crate::appearance_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Window => {
                crate::window_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Input => {
                crate::input_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Terminal => {
                crate::terminal_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Effects => {
                crate::effects_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Badge => {
                crate::badge_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::ProgressBar => {
                crate::progress_bar_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::StatusBar => {
                crate::status_bar_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Profiles => {
                crate::profiles_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Ssh => {
                self.show_ssh_tab(ui, changes_this_frame);
            }
            SettingsTab::Notifications => {
                crate::notifications_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Integrations => {
                self.show_integrations_tab(ui, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Automation => {
                crate::automation_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Scripts => {
                crate::scripts_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Snippets => {
                crate::snippets_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Actions => {
                crate::actions_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::ContentPrettifier => {
                crate::prettifier_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Arrangements => {
                crate::arrangements_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::AiInspector => {
                crate::ai_inspector_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
            SettingsTab::Advanced => {
                crate::advanced_tab::show(ui, self, changes_this_frame, &mut collapsed);
            }
        }

        self.collapsed_sections = collapsed;
    }

    /// Check if a keybinding conflicts with existing keybindings.
    pub fn check_keybinding_conflict(&self, key: &str, exclude_id: Option<&str>) -> Option<String> {
        for binding in &self.config.keybindings {
            if binding.key == key {
                return Some(format!("Already bound to: {}", binding.action));
            }
        }

        for snippet in &self.config.snippets {
            if let Some(snippet_key) = &snippet.keybinding
                && snippet_key == key
            {
                if exclude_id == Some(&snippet.id) {
                    continue;
                }
                return Some(format!("Already bound to snippet: {}", snippet.title));
            }
        }

        None
    }
}
