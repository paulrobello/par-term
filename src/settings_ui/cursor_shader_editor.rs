//! Cursor shader editor window for the settings UI.

use arboard::Clipboard;
use egui::{Color32, Context, Frame, Window, epaint::Shadow};

use super::{CursorShaderEditorResult, SettingsUI};

impl SettingsUI {
    /// Show the cursor shader editor window
    /// Returns Some(CursorShaderEditorResult) if Apply was clicked
    pub(super) fn show_cursor_shader_editor_window(
        &mut self,
        ctx: &Context,
    ) -> Option<CursorShaderEditorResult> {
        if !self.cursor_shader_editor_visible {
            return None;
        }

        let mut cursor_shader_editor_open = true;
        let mut cursor_apply_clicked = false;
        let mut cursor_cancel_clicked = false;
        let mut cursor_save_to_file_clicked = false;

        // Calculate 90% of viewport height
        let viewport = ctx.input(|i| i.viewport_rect());
        let window_height = viewport.height() * 0.9;

        let cursor_shader_filename = &self.temp_cursor_shader.clone();
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
                self.show_cursor_shader_error(ui);

                // Cursor shader source editor
                let available_height = ui.available_height() - 60.0; // Reserve space for buttons

                let cursor_editor_id = egui::Id::new("cursor_shader_editor_textedit");

                // Handle keyboard shortcuts
                // macOS menu system consumes key-press events, so we detect key-release with cmd held
                let (select_all, copy, paste, cut) = ui.input(|i| {
                    // Check raw events for key releases with command modifier
                    let mut a = false;
                    let mut c = false;
                    let mut v = false;
                    let mut x = false;
                    for event in &i.raw.events {
                        if let egui::Event::Key {
                            key,
                            pressed: false, // Key release
                            modifiers,
                            ..
                        } = event
                            && modifiers.command
                        {
                            match key {
                                egui::Key::A => a = true,
                                egui::Key::C => c = true,
                                egui::Key::V => v = true,
                                egui::Key::X => x = true,
                                _ => {}
                            }
                        }
                    }
                    (a, c, v, x)
                });

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

                        // Handle select all
                        if select_all {
                            if let Some(mut state) =
                                egui::TextEdit::load_state(ui.ctx(), cursor_editor_id)
                            {
                                let len = self.cursor_shader_editor_source.len();
                                let ccursor_range = egui::text::CCursorRange::two(
                                    egui::text::CCursor::new(0),
                                    egui::text::CCursor::new(len),
                                );
                                state.cursor.set_char_range(Some(ccursor_range));
                                state.store(ui.ctx(), cursor_editor_id);
                            }
                        }
                        // Handle copy
                        else if copy {
                            if let Some(state) =
                                egui::TextEdit::load_state(ui.ctx(), cursor_editor_id)
                                && let Some(range) = state.cursor.char_range()
                            {
                                let start = range.primary.index.min(range.secondary.index);
                                let end = range.primary.index.max(range.secondary.index);
                                if start != end {
                                    let selected_text =
                                        &self.cursor_shader_editor_source[start..end];
                                    if let Ok(mut clipboard) = Clipboard::new() {
                                        let _ = clipboard.set_text(selected_text);
                                    }
                                }
                            }
                        }
                        // Handle cut
                        else if cut {
                            if let Some(mut state) =
                                egui::TextEdit::load_state(ui.ctx(), cursor_editor_id)
                                && let Some(range) = state.cursor.char_range()
                            {
                                let start = range.primary.index.min(range.secondary.index);
                                let end = range.primary.index.max(range.secondary.index);
                                if start != end {
                                    let selected_text =
                                        &self.cursor_shader_editor_source[start..end];
                                    if let Ok(mut clipboard) = Clipboard::new() {
                                        let _ = clipboard.set_text(selected_text);
                                    }
                                    // Delete the selected text
                                    self.cursor_shader_editor_source
                                        .replace_range(start..end, "");
                                    // Move cursor to start of deleted range
                                    let ccursor = egui::text::CCursor::new(start);
                                    state.cursor.set_char_range(Some(
                                        egui::text::CCursorRange::one(ccursor),
                                    ));
                                    state.store(ui.ctx(), cursor_editor_id);
                                }
                            }
                        }
                        // Handle paste
                        else if paste
                            && let Ok(mut clipboard) = Clipboard::new()
                            && let Ok(text) = clipboard.get_text()
                            && let Some(mut state) =
                                egui::TextEdit::load_state(ui.ctx(), cursor_editor_id)
                        {
                            let insert_pos = if let Some(range) = state.cursor.char_range() {
                                let start = range.primary.index.min(range.secondary.index);
                                let end = range.primary.index.max(range.secondary.index);
                                // Delete selection if any
                                if start != end {
                                    self.cursor_shader_editor_source
                                        .replace_range(start..end, "");
                                }
                                start
                            } else {
                                self.cursor_shader_editor_source.len()
                            };
                            // Insert pasted text
                            self.cursor_shader_editor_source
                                .insert_str(insert_pos, &text);
                            // Move cursor to end of pasted text
                            let new_pos = insert_pos + text.len();
                            let ccursor = egui::text::CCursor::new(new_pos);
                            state
                                .cursor
                                .set_char_range(Some(egui::text::CCursorRange::one(ccursor)));
                            state.store(ui.ctx(), cursor_editor_id);
                        }
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
        let cursor_shader_apply_result = if cursor_apply_clicked {
            Some(CursorShaderEditorResult {
                source: self.cursor_shader_editor_source.clone(),
            })
        } else {
            None
        };

        if cursor_save_to_file_clicked {
            self.save_cursor_shader_to_file();
        }

        if cursor_cancel_clicked || !cursor_shader_editor_open {
            self.close_cursor_shader_editor();
        }

        cursor_shader_apply_result
    }

    /// Show the cursor shader error panel
    fn show_cursor_shader_error(&mut self, ui: &mut egui::Ui) {
        let mut dismiss_error = false;

        if let Some(error) = &self.cursor_shader_editor_error {
            let error_text = error.clone();
            let shader_path = crate::config::Config::shader_path(&self.temp_cursor_shader);
            let full_error = format!("File: {}\n\n{}", shader_path.display(), error_text);

            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(
                        Color32::from_rgb(255, 100, 100),
                        "Cursor Shader Compilation Error",
                    );
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
            self.cursor_shader_editor_error = None;
        }
    }

    /// Save cursor shader source to file
    fn save_cursor_shader_to_file(&mut self) {
        let shader_path = crate::config::Config::shader_path(&self.temp_cursor_shader);
        match std::fs::write(&shader_path, &self.cursor_shader_editor_source) {
            Ok(()) => {
                self.cursor_shader_editor_original = self.cursor_shader_editor_source.clone();
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

    /// Close the cursor shader editor and clear state
    fn close_cursor_shader_editor(&mut self) {
        self.cursor_shader_editor_visible = false;
        self.cursor_shader_editor_source.clear();
        self.cursor_shader_editor_original.clear();
        self.cursor_shader_editor_error = None;
    }
}
