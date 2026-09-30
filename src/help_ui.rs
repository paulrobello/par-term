use crate::command_palette::catalog::chord_display;
use crate::config::Config;
use crate::help_content::{MODAL_SECTIONS, shortcut_sections};
use crate::ui_constants::{HELP_WINDOW_DEFAULT_HEIGHT, HELP_WINDOW_DEFAULT_WIDTH};
use egui::{Color32, Context, Frame, RichText, Window, epaint::Shadow};
use par_term_keybindings::KeybindingRegistry;
use std::cell::Cell;

/// The chord the registry currently binds to `action`, formatted the way the
/// palette formats it, or `"unbound"`.
fn live_chord(registry: &KeybindingRegistry, action: &str) -> String {
    registry
        .chord_for_action(action)
        .map(|combo| chord_display(&combo))
        .unwrap_or_else(|| "unbound".to_string())
}

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

    /// Show the help window. Every chord comes from `registry`, the live
    /// bindings, and `keybindings`, the list it was built from (UX.md V12).
    pub fn show(
        &mut self,
        ctx: &Context,
        registry: &KeybindingRegistry,
        keybindings: &[par_term_config::KeyBinding],
    ) {
        if !self.visible {
            return;
        }

        // Fully opaque regardless of terminal opacity — scoped to this
        // panel (OV1: no global style writes).
        let solid_bg = Color32::from_rgba_unmultiplied(24, 24, 24, 255);

        let mut open = true;
        let close_requested = Cell::new(false);

        let viewport = ctx.input(|i| i.viewport_rect());
        Window::new("Help")
            .resizable(true)
            .default_width(HELP_WINDOW_DEFAULT_WIDTH)
            .default_height(HELP_WINDOW_DEFAULT_HEIGHT)
            .default_pos(viewport.center())
            .pivot(egui::Align2::CENTER_CENTER)
            .open(&mut open)
            .frame(
                Frame::window(&ctx.global_style())
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
                crate::app::overlay::theme::solid_panel(ui, solid_bg);
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
                        ui.hyperlink_to(env!("CARGO_PKG_REPOSITORY"), env!("CARGO_PKG_REPOSITORY"));
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

                    // One-sentence model (UX.md 2.3)
                    ui.heading("Windows, Tabs, Panes, Sessions");
                    ui.separator();
                    ui.label(
                        "A window holds tabs; a tab holds panes. A window can be attached \
                         to one par-mux session, in which case its tabs live in the daemon \
                         and survive quitting par-term. Detach leaves them running; End \
                         session ends them.",
                    );

                    ui.add_space(12.0);

                    // Keyboard Shortcuts Section: generated (UX.md V12)
                    ui.heading("Keyboard Shortcuts");
                    ui.separator();

                    ui.label(
                        RichText::new(
                            "Generated from your current bindings; change them in \
                             Settings ▸ Input ▸ Keybindings. Unbound actions are not \
                             listed; the command palette reaches every action.",
                        )
                        .weak(),
                    );
                    ui.add_space(4.0);

                    egui::Grid::new("shortcuts_grid")
                        .num_columns(2)
                        .spacing([20.0, 4.0])
                        .striped(true)
                        .show(ui, |ui| {
                            for section in shortcut_sections(registry, keybindings) {
                                ui.label(RichText::new(section.title).strong().underline());
                                ui.end_row();
                                for row in &section.rows {
                                    shortcut_row(ui, &row.chord, &row.label);
                                }
                                ui.end_row();
                            }
                            for (title, rows) in MODAL_SECTIONS {
                                ui.label(RichText::new(*title).strong().underline());
                                ui.end_row();
                                for (keys, description) in *rows {
                                    shortcut_row(ui, keys, description);
                                }
                                ui.end_row();
                            }
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
                            shortcut_row(ui, "Click + drag", "Select text");
                            shortcut_row(ui, "Double / triple-click", "Select word / line");
                            shortcut_row(ui, "Middle-click", "Paste (if enabled)");
                            shortcut_row(ui, "Cmd/Ctrl+Click URL", "Open URL in browser");
                            shortcut_row(ui, "Scrollbar drag / click", "Scroll / jump");
                            shortcut_row(ui, "Middle-click a tab", "Close the tab");
                            shortcut_row(ui, "Double-click empty tab bar", "New tab");
                        });

                    ui.add_space(12.0);

                    // Tips Section
                    ui.heading("Tips");
                    ui.separator();

                    ui.label(
                        "• Configuration changes made in Settings are saved to the config file.",
                    );
                    ui.label(format!(
                        "• Press {} to reload config without restarting the terminal.",
                        live_chord(registry, "reload_config")
                    ));
                    ui.label("• Custom shaders can be placed in the shaders folder.");
                    ui.label(format!(
                        "• {} opens the command palette: every action, searchable by name.",
                        live_chord(registry, "toggle_command_palette")
                    ));

                    ui.add_space(12.0);

                    // Close button
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("Close").clicked() {
                            close_requested.set(true);
                        }
                        ui.label(
                            RichText::new(format!(
                                "Press {} or Escape to close",
                                live_chord(registry, "toggle_help")
                            ))
                            .weak(),
                        );
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A rebind shows the new chord; an unbound action says so.
    #[test]
    fn live_chord_reads_the_registry() {
        let rebound = KeybindingRegistry::from_config(&[par_term_config::KeyBinding {
            key: "F9".to_string(),
            action: "split_right".to_string(),
        }]);
        assert_eq!(live_chord(&rebound, "split_right"), "F9");
        assert_eq!(live_chord(&rebound, "split_down"), "unbound");
    }

    /// V12: the panel holds no hand-maintained chord table. Its shortcut
    /// rows come from `help_content`, whose tests prove they match the
    /// shipped defaults.
    #[test]
    fn the_panel_renders_generated_sections() {
        let source = include_str!("help_ui.rs");
        let body = &source[..source.find("#[cfg(test)]").unwrap()];
        assert!(body.contains("shortcut_sections(registry, keybindings)"));
        assert!(!body.contains("REGISTRY_SHORTCUTS"));
        assert!(!body.contains("\"Cmd+C"));
    }
}
