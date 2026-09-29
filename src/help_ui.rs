use crate::command_palette::catalog::chord_display;
use crate::config::Config;
use crate::ui_constants::{HELP_WINDOW_DEFAULT_HEIGHT, HELP_WINDOW_DEFAULT_WIDTH};
use egui::{Color32, Context, Frame, RichText, Window, epaint::Shadow};
use par_term_keybindings::KeybindingRegistry;
use std::cell::Cell;

/// Registry-bound shortcuts the help panel lists, as `(section, rows)` with
/// each row `(action id, description)`. The chord column is read from the
/// live registry when the panel draws (UX.md K6), so a rebind or a default
/// change shows here without editing this table.
const REGISTRY_SHORTCUTS: &[(&str, &[(&str, &str)])] = &[
    (
        "Windows & Tabs",
        &[
            ("new_window", "New window"),
            ("new_tab", "New tab"),
            ("duplicate_tab", "Duplicate tab"),
            ("close_tab", "Close tab"),
            ("reopen_closed_tab", "Reopen closed tab"),
            ("next_tab", "Next tab"),
            ("prev_tab", "Previous tab"),
            ("move_tab_left", "Move tab left"),
            ("move_tab_right", "Move tab right"),
            ("switch_to_tab_1", "Go to tab 1 (2–9 likewise)"),
            ("quit", "Quit"),
        ],
    ),
    (
        "Panes",
        &[
            ("split_right", "Split right"),
            ("split_down", "Split down"),
            ("close_pane", "Close pane (then tab, then window)"),
            ("navigate_pane_left", "Focus pane left (arrows likewise)"),
            ("resize_pane_left", "Resize pane left (arrows likewise)"),
            ("swap_pane_left", "Swap pane left (arrows likewise)"),
            ("select_pane_hint", "Select pane by letter"),
            ("toggle_broadcast_input", "Toggle broadcast input"),
        ],
    ),
    (
        "Sessions & Profiles",
        &[
            ("toggle_command_palette", "Command palette"),
            ("toggle_profile_drawer", "Profile drawer"),
            ("toggle_tmux_session_picker", "tmux session picker"),
            ("ssh_quick_connect", "SSH quick connect"),
        ],
    ),
    (
        "Search & History",
        &[
            ("toggle_search", "Open search"),
            ("toggle_command_history", "Fuzzy command history"),
            ("toggle_clipboard_history", "Clipboard history"),
            ("paste_special", "Paste special"),
            ("toggle_copy_mode", "Toggle copy mode"),
        ],
    ),
    (
        "Scrolling",
        &[
            ("scroll_up_page", "Scroll up one page"),
            ("scroll_down_page", "Scroll down one page"),
            ("scroll_to_top", "Scroll to top"),
            ("scroll_to_bottom", "Scroll to bottom"),
            ("scroll_to_previous_mark", "Previous command mark"),
            ("scroll_to_next_mark", "Next command mark"),
        ],
    ),
    (
        "Window & Display",
        &[
            ("toggle_help", "Toggle this help panel"),
            ("toggle_fps_overlay", "Toggle FPS overlay"),
            ("reload_config", "Reload configuration"),
            ("toggle_fullscreen", "Toggle fullscreen"),
            ("maximize_vertically", "Maximize vertically"),
            ("open_settings", "Open settings"),
            ("increase_font_size", "Increase font size"),
            ("decrease_font_size", "Decrease font size"),
            ("reset_font_size", "Reset font size"),
            ("toggle_background_shader", "Toggle background shader"),
            ("toggle_cursor_shader", "Toggle cursor shader"),
        ],
    ),
    (
        "Terminal",
        &[
            ("clear_screen", "Clear screen"),
            ("clear_scrollback", "Clear scrollback"),
            ("toggle_session_logging", "Toggle session logging"),
            ("toggle_ai_inspector", "Toggle assistant panel"),
        ],
    ),
];

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

    /// Show the help window. Chords come from `registry`, the live bindings.
    pub fn show(&mut self, ctx: &Context, registry: &KeybindingRegistry) {
        if !self.visible {
            return;
        }

        // Ensure help panel is fully opaque regardless of terminal opacity
        let mut style = (*ctx.global_style()).clone();
        let solid_bg = Color32::from_rgba_unmultiplied(24, 24, 24, 255);
        style.visuals.window_fill = solid_bg;
        style.visuals.panel_fill = solid_bg;
        style.visuals.widgets.noninteractive.bg_fill = solid_bg;
        ctx.set_global_style(style);

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

                    ui.label(
                        RichText::new(
                            "Chords shown are your current bindings; change them in \
                             Settings ▸ Input ▸ Keybindings.",
                        )
                        .weak(),
                    );
                    ui.add_space(4.0);

                    // Use a grid for clean alignment
                    egui::Grid::new("shortcuts_grid")
                        .num_columns(2)
                        .spacing([20.0, 4.0])
                        .striped(true)
                        .show(ui, |ui| {
                            for (section, rows) in REGISTRY_SHORTCUTS {
                                ui.label(RichText::new(*section).strong().underline());
                                ui.end_row();
                                for (action, description) in *rows {
                                    shortcut_row(ui, &live_chord(registry, action), description);
                                }
                                ui.end_row();
                            }

                            // Built-in keys that are not registry bindings.
                            ui.label(RichText::new("Selection & Clipboard").strong().underline());
                            ui.end_row();

                            #[cfg(target_os = "macos")]
                            shortcut_row(ui, "Cmd+C / Cmd+V", "Copy / paste");
                            #[cfg(not(target_os = "macos"))]
                            shortcut_row(ui, "Ctrl+Shift+C / Ctrl+Shift+V", "Copy / paste");
                            shortcut_row(ui, "Click + Drag", "Select text");
                            shortcut_row(ui, "Double-click", "Select word");
                            shortcut_row(ui, "Triple-click", "Select line");
                            shortcut_row(ui, "Middle-click", "Paste (if enabled)");
                            shortcut_row(ui, "Mouse wheel", "Scroll up/down");
                            shortcut_row(ui, "Cmd/Ctrl+Click URL", "Open URL in browser");

                            ui.end_row();

                            ui.label(RichText::new("In the search bar").strong().underline());
                            ui.end_row();

                            shortcut_row(ui, "Enter", "Find next match");
                            shortcut_row(ui, "Shift+Enter", "Find previous match");
                            shortcut_row(ui, "Escape", "Close search");
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

                            shortcut_row(
                                ui,
                                &live_chord(registry, "toggle_copy_mode"),
                                "Toggle copy mode",
                            );
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

                    ui.label("• Configuration changes made in Settings are saved to the config file.");
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Every action the help panel lists must be dispatchable, or its row is
    /// a shortcut that does nothing.
    #[test]
    fn every_listed_action_has_a_handler() {
        use crate::app::input_events::keybinding_actions::ACTION_HANDLERS;
        use crate::app::input_events::keybinding_display_actions::DISPLAY_ACTION_HANDLERS;
        let live: Vec<&str> = ACTION_HANDLERS
            .iter()
            .map(|(id, _)| *id)
            .chain(DISPLAY_ACTION_HANDLERS.iter().map(|(id, _)| *id))
            .collect();
        let dead: Vec<&str> = REGISTRY_SHORTCUTS
            .iter()
            .flat_map(|(_, rows)| rows.iter().map(|(id, _)| *id))
            .filter(|id| !live.contains(id))
            .collect();
        assert!(dead.is_empty(), "help rows with no handler: {dead:?}");
    }

    /// The chord column is the live registry binding: with the shipped
    /// defaults each listed action shows its primary (first) default chord,
    /// and a rebind shows the new chord.
    #[test]
    fn chords_come_from_the_live_registry() {
        let defaults = par_term_config::defaults::keybindings();
        let registry = KeybindingRegistry::from_config(&defaults);
        let primary = |action: &str| {
            defaults
                .iter()
                .find(|kb| kb.action == action)
                .map(|kb| {
                    chord_display(
                        &par_term_keybindings::parser::parse_key_combo(&kb.key)
                            .expect("default parses")
                            .platform_normalized(),
                    )
                })
                .unwrap_or_else(|| "unbound".to_string())
        };
        for (_, rows) in REGISTRY_SHORTCUTS {
            for (action, _) in *rows {
                assert_eq!(live_chord(&registry, action), primary(action), "{action}");
            }
        }

        let rebound = KeybindingRegistry::from_config(&[par_term_config::KeyBinding {
            key: "F9".to_string(),
            action: "split_right".to_string(),
        }]);
        assert_eq!(live_chord(&rebound, "split_right"), "F9");
        assert_eq!(live_chord(&rebound, "split_down"), "unbound");
    }
}
