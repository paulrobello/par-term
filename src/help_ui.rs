use crate::config::Config;
use egui::{Color32, Context, Frame, RichText, Window, epaint::Shadow};
use std::cell::Cell;

/// Help UI manager using egui
pub struct HelpUI {
    /// Whether the help window is currently visible
    pub visible: bool,
}

impl HelpUI {
    /// Create a new help UI
    pub fn new() -> Self {
        Self { visible: false }
    }

    /// Toggle help window visibility
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Show the help window
    pub fn show(&mut self, ctx: &Context) {
        if !self.visible {
            return;
        }

        // Ensure help panel is fully opaque regardless of terminal opacity
        let mut style = (*ctx.style()).clone();
        let solid_bg = Color32::from_rgba_unmultiplied(24, 24, 24, 255);
        style.visuals.window_fill = solid_bg;
        style.visuals.panel_fill = solid_bg;
        style.visuals.widgets.noninteractive.bg_fill = solid_bg;
        ctx.set_style(style);

        let mut open = true;
        let close_requested = Cell::new(false);

        let viewport = ctx.input(|i| i.viewport_rect());
        Window::new("Help")
            .resizable(true)
            .default_width(550.0)
            .default_height(600.0)
            .default_pos(viewport.center())
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
                egui::ScrollArea::vertical().show(ui, |ui| {
                    // About Section
                    ui.heading("About par-term");
                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("Version:");
                        ui.label(RichText::new(env!("CARGO_PKG_VERSION")).strong());
                    });

                    ui.add_space(4.0);
                    ui.label(env!("CARGO_PKG_DESCRIPTION"));

                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label("Author:");
                        ui.label(env!("CARGO_PKG_AUTHORS"));
                    });

                    ui.horizontal(|ui| {
                        ui.label("License:");
                        ui.label(env!("CARGO_PKG_LICENSE"));
                    });

                    ui.horizontal(|ui| {
                        ui.label("Repository:");
                        ui.hyperlink_to(
                            env!("CARGO_PKG_REPOSITORY"),
                            env!("CARGO_PKG_REPOSITORY"),
                        );
                    });

                    ui.add_space(12.0);

                    // Configuration Paths Section
                    ui.heading("Configuration Paths");
                    ui.separator();

                    let config_path = Config::config_path();
                    let shaders_dir = Config::shaders_dir();

                    ui.horizontal(|ui| {
                        ui.label("Config file:");
                        ui.label(RichText::new(config_path.display().to_string()).monospace());
                    });

                    ui.horizontal(|ui| {
                        ui.label("Shaders folder:");
                        ui.label(RichText::new(shaders_dir.display().to_string()).monospace());
                    });

                    ui.add_space(12.0);

                    // Keyboard Shortcuts Section
                    ui.heading("Keyboard Shortcuts");
                    ui.separator();

                    // Use a grid for clean alignment
                    egui::Grid::new("shortcuts_grid")
                        .num_columns(2)
                        .spacing([20.0, 4.0])
                        .striped(true)
                        .show(ui, |ui| {
                            // Navigation
                            ui.label(RichText::new("Navigation").strong().underline());
                            ui.end_row();

                            shortcut_row(ui, "PageUp", "Scroll up one page");
                            shortcut_row(ui, "PageDown", "Scroll down one page");
                            shortcut_row(ui, "Shift+Home", "Scroll to top");
                            shortcut_row(ui, "Shift+End", "Scroll to bottom");
                            shortcut_row(ui, "Mouse wheel", "Scroll up/down");

                            ui.end_row();

                            // Window & Display
                            ui.label(RichText::new("Window & Display").strong().underline());
                            ui.end_row();

                            shortcut_row(ui, "F1", "Toggle this help panel");
                            shortcut_row(ui, "F3", "Toggle FPS overlay");
                            shortcut_row(ui, "F5", "Reload configuration");
                            shortcut_row(ui, "F11", "Toggle fullscreen / Shader editor");
                            shortcut_row(ui, "F12", "Toggle settings panel");

                            ui.end_row();

                            // Font & Text
                            ui.label(RichText::new("Font & Text").strong().underline());
                            ui.end_row();

                            shortcut_row(ui, "Ctrl++", "Increase font size");
                            shortcut_row(ui, "Ctrl+-", "Decrease font size");
                            shortcut_row(ui, "Ctrl+0", "Reset font size to default");

                            ui.end_row();

                            // Selection & Clipboard
                            ui.label(RichText::new("Selection & Clipboard").strong().underline());
                            ui.end_row();

                            shortcut_row(ui, "Click + Drag", "Select text");
                            shortcut_row(ui, "Double-click", "Select word");
                            shortcut_row(ui, "Triple-click", "Select line");
                            shortcut_row(ui, "Ctrl+Shift+C", "Copy selection");
                            shortcut_row(ui, "Ctrl+Shift+V", "Paste from clipboard");
                            shortcut_row(ui, "Ctrl+Shift+H", "Toggle clipboard history");
                            shortcut_row(ui, "Cmd/Ctrl+R", "Fuzzy command history search");
                            shortcut_row(ui, "Middle-click", "Paste (if enabled)");

                            ui.end_row();

                            // Search
                            ui.label(RichText::new("Search").strong().underline());
                            ui.end_row();

                            shortcut_row(ui, "Cmd/Ctrl+F", "Open search");
                            shortcut_row(ui, "Enter", "Find next match");
                            shortcut_row(ui, "Shift+Enter", "Find previous match");
                            shortcut_row(ui, "Escape", "Close search");

                            ui.end_row();

                            // Terminal
                            ui.label(RichText::new("Terminal").strong().underline());
                            ui.end_row();

                            shortcut_row(ui, "Ctrl+L", "Clear screen");
                            shortcut_row(ui, "Ctrl+Shift+S", "Take screenshot");
                            shortcut_row(ui, "Ctrl+Shift+R", "Toggle session recording");
                            shortcut_row(ui, "Ctrl+Shift+F5", "Fix rendering (after monitor change)");

                            ui.end_row();

                            // URL Handling
                            ui.label(RichText::new("URL Handling").strong().underline());
                            ui.end_row();

                            shortcut_row(ui, "Ctrl+Click URL", "Open URL in browser");
                        });

                    ui.add_space(12.0);

                    // Copy Mode Section
                    ui.heading("Copy Mode (Vi-Style)");
                    ui.separator();

                    ui.label("Copy Mode provides keyboard-driven text selection and navigation through the terminal buffer, including scrollback history.");

                    ui.add_space(4.0);

                    egui::Grid::new("copy_mode_grid")
                        .num_columns(2)
                        .spacing([20.0, 4.0])
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label(RichText::new("Enter / Exit").strong().underline());
                            ui.end_row();

                            #[cfg(target_os = "macos")]
                            shortcut_row(ui, "Cmd+Shift+C", "Toggle copy mode");
                            #[cfg(not(target_os = "macos"))]
                            shortcut_row(ui, "Ctrl+Shift+Space", "Toggle copy mode");
                            shortcut_row(ui, "q / Escape", "Exit copy mode");

                            ui.end_row();

                            ui.label(RichText::new("Navigation").strong().underline());
                            ui.end_row();

                            shortcut_row(ui, "h j k l", "Left / Down / Up / Right");
                            shortcut_row(ui, "w / b / e", "Word forward / back / end");
                            shortcut_row(ui, "W / B / E", "WORD forward / back / end");
                            shortcut_row(ui, "0", "Start of line");
                            shortcut_row(ui, "$", "End of line");
                            shortcut_row(ui, "^", "First non-blank character");
                            shortcut_row(ui, "gg", "Top of scrollback");
                            shortcut_row(ui, "G", "Bottom of buffer");
                            shortcut_row(ui, "Ctrl+U / Ctrl+D", "Half page up / down");
                            shortcut_row(ui, "Ctrl+B / Ctrl+F", "Full page up / down");

                            ui.end_row();

                            ui.label(RichText::new("Selection & Yank").strong().underline());
                            ui.end_row();

                            shortcut_row(ui, "v", "Character selection");
                            shortcut_row(ui, "V", "Line selection");
                            shortcut_row(ui, "y", "Yank (copy) selection to clipboard");
                            shortcut_row(ui, "1-9", "Count prefix (e.g. 5j = down 5 lines)");

                            ui.end_row();

                            ui.label(RichText::new("Search").strong().underline());
                            ui.end_row();

                            shortcut_row(ui, "/", "Search forward");
                            shortcut_row(ui, "?", "Search backward");
                            shortcut_row(ui, "n", "Next match");
                            shortcut_row(ui, "N", "Previous match");

                            ui.end_row();

                            ui.label(RichText::new("Marks").strong().underline());
                            ui.end_row();

                            shortcut_row(ui, "m + char", "Set mark at current position");
                            shortcut_row(ui, "' + char", "Jump to mark");
                        });

                    ui.add_space(12.0);

                    // Mouse Actions Section
                    ui.heading("Mouse Actions");
                    ui.separator();

                    egui::Grid::new("mouse_grid")
                        .num_columns(2)
                        .spacing([20.0, 4.0])
                        .striped(true)
                        .show(ui, |ui| {
                            shortcut_row(ui, "Scrollbar drag", "Scroll through history");
                            shortcut_row(ui, "Scrollbar click", "Jump to position");
                        });

                    ui.add_space(12.0);

                    // Tips Section
                    ui.heading("Tips");
                    ui.separator();

                    ui.label("• Configuration changes made via F12 settings are saved to the config file.");
                    ui.label("• Press F5 to reload config without restarting the terminal.");
                    ui.label("• Custom shaders can be placed in the shaders folder.");
                    ui.label("• The shader editor (F11) allows live editing when a shader is configured.");
                    ui.label("• If display looks corrupted after moving between monitors, press Ctrl+Shift+F5.");

                    ui.add_space(12.0);

                    // Close button
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Close").clicked() {
                            close_requested.set(true);
                        }
                        ui.label(RichText::new("Press F1 or Escape to close").weak());
                    });
                });
            });

        // Update visibility based on window state
        if !open || close_requested.get() {
            self.visible = false;
        }
    }
}

impl Default for HelpUI {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper function to add a shortcut row to the grid
fn shortcut_row(ui: &mut egui::Ui, shortcut: &str, description: &str) {
    ui.label(RichText::new(shortcut).monospace().strong());
    ui.label(description);
    ui.end_row();
}
