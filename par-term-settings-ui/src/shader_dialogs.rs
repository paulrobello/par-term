//! Create and delete shader dialogs for the settings UI.

use egui::{Context, Window};

use super::SettingsUI;

impl SettingsUI {
    /// Show the create shader dialog
    pub(super) fn show_create_shader_dialog_window(&mut self, ctx: &Context) {
        if !self.show_create_shader_dialog {
            return;
        }

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
            close_dialog = self.create_shader_file();
        }

        if close_dialog {
            self.show_create_shader_dialog = false;
            self.new_shader_name.clear();
        }
    }

    /// Create a new shader file from the template
    fn create_shader_file(&mut self) -> bool {
        // Ensure filename ends with .glsl
        let mut filename = self.new_shader_name.clone();
        if !filename.ends_with(".glsl")
            && !filename.ends_with(".frag")
            && !filename.ends_with(".shader")
        {
            filename.push_str(".glsl");
        }

        let shader_path = par_term_config::Config::shaders_dir().join(&filename);

        // Check if file already exists
        if shader_path.exists() {
            self.shader_editor_error = Some(format!("Shader '{}' already exists!", filename));
            return false;
        }

        // Create the shader with a basic template
        let template = r#"// Custom shader for par-term
// Available uniforms:
//   iTime       - Time in seconds (when animation enabled)
//   iResolution - Viewport resolution (vec2)
//   iChannel0-3 - User texture channels (sampler2D, Shadertoy compatible)
//   iChannel4   - Terminal content texture (sampler2D)
//   iOpacity    - Window opacity (float)
//   iTextOpacity - Text opacity (float)

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;

    // Sample terminal content
    vec4 terminal = texture(iChannel4, uv);

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
                true
            }
            Err(e) => {
                self.shader_editor_error = Some(format!("Failed to create shader: {}", e));
                false
            }
        }
    }

    /// Show the delete shader confirmation dialog
    pub(super) fn show_delete_shader_dialog_window(&mut self, ctx: &Context) {
        if !self.show_delete_shader_dialog {
            return;
        }

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
            let shader_path = par_term_config::Config::shader_path(&shader_name);
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
}
