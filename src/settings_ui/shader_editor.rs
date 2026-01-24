//! Background shader editor window for the settings UI.

use arboard::Clipboard;
use egui::{Color32, Context, Frame, Window, epaint::Shadow};

use super::{SettingsUI, ShaderEditorResult};

impl SettingsUI {
    /// Show the background shader editor window
    /// Returns Some(ShaderEditorResult) if Apply was clicked
    pub(super) fn show_shader_editor_window(
        &mut self,
        ctx: &Context,
    ) -> Option<ShaderEditorResult> {
        if !self.shader_editor_visible {
            return None;
        }

        let mut shader_editor_open = true;
        let mut apply_clicked = false;
        let mut cancel_clicked = false;
        let mut save_to_file_clicked = false;

        // Calculate 90% of viewport height
        let viewport = ctx.input(|i| i.viewport_rect());
        let window_height = viewport.height() * 0.9;

        let bg_shader_filename = self.temp_custom_shader.clone();
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

                // Handle search bar toggle
                self.handle_shader_search_toggle(ui);

                // Show search bar if visible
                self.show_shader_search_bar(ui);

                // Show error dialog if there's an error
                self.show_shader_error(ui);

                // Shader source editor
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
                            && let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), editor_id)
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
        let shader_apply_result = if apply_clicked {
            Some(ShaderEditorResult {
                source: self.shader_editor_source.clone(),
            })
        } else {
            None
        };

        if save_to_file_clicked {
            self.save_shader_to_file();
        }

        if cancel_clicked || !shader_editor_open {
            self.close_shader_editor();
        }

        shader_apply_result
    }

    /// Handle Ctrl+F and Escape for search bar toggle
    fn handle_shader_search_toggle(&mut self, ui: &egui::Ui) {
        let ctrl_f = ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::F));
        let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));

        if ctrl_f {
            self.shader_search_visible = !self.shader_search_visible;
        }
        if escape && self.shader_search_visible {
            self.shader_search_visible = false;
        }
    }

    /// Show the shader search bar
    fn show_shader_search_bar(&mut self, ui: &mut egui::Ui) {
        if !self.shader_search_visible {
            return;
        }

        let ctrl_f = ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::F));

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
                .add_enabled(has_matches, egui::Button::new("<"))
                .on_hover_text("Previous (Shift+Enter)")
                .clicked()
                || (enter_pressed && shift_held && has_matches)
            {
                self.shader_search_previous();
            }
            if ui
                .add_enabled(has_matches, egui::Button::new(">"))
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
            if ui.button("X").on_hover_text("Close (Esc)").clicked() {
                self.shader_search_visible = false;
            }
        });
        ui.separator();
    }

    /// Show the shader error panel
    fn show_shader_error(&mut self, ui: &mut egui::Ui) {
        let mut dismiss_error = false;

        if let Some(error) = &self.shader_editor_error {
            let error_text = error.clone();
            let shader_path = crate::config::Config::shader_path(&self.temp_custom_shader);
            let full_error = format!("File: {}\n\n{}", shader_path.display(), error_text);

            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(Color32::from_rgb(255, 100, 100), "Shader Compilation Error");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Dismiss").clicked() {
                            dismiss_error = true;
                        }
                        if ui.button("Copy").clicked()
                            && let Ok(mut clipboard) = Clipboard::new()
                        {
                            let _ = clipboard.set_text(full_error.clone());
                        }
                    });
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
    }

    /// Save shader source to file
    fn save_shader_to_file(&mut self) {
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

    /// Close the shader editor and clear state
    fn close_shader_editor(&mut self) {
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
