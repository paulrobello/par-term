//! Terminal settings tab.
//!
//! Consolidates: terminal_tab (original), shell_tab, search_tab, scrollbar_tab
//!
//! Contains:
//! - Behavior settings (scrollback, exit behavior)
//! - Unicode settings (version, ambiguous width, answerback)
//! - Shell settings (custom shell, args, working directory)
//! - Startup settings (initial text)
//! - Search settings (highlight colors, defaults)
//! - Scrollbar settings (width, colors, autohide)

use super::section::{collapsing_section, INPUT_WIDTH, SLIDER_WIDTH};
use super::SettingsUI;

const SLIDER_HEIGHT: f32 = 18.0;

/// Show the terminal tab content.
pub fn show(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    let query = settings.search_query.trim().to_lowercase();

    // Behavior section
    if section_matches(&query, "Behavior", &["scrollback", "exit", "shell exit"]) {
        show_behavior_section(ui, settings, changes_this_frame);
    }

    // Unicode section (collapsed by default)
    if section_matches(
        &query,
        "Unicode",
        &["unicode", "width", "answerback", "ambiguous"],
    ) {
        show_unicode_section(ui, settings, changes_this_frame);
    }

    // Shell section
    if section_matches(
        &query,
        "Shell",
        &["shell", "custom shell", "working directory", "login"],
    ) {
        show_shell_section(ui, settings, changes_this_frame);
    }

    // Startup section (collapsed by default)
    if section_matches(
        &query,
        "Startup",
        &["initial text", "startup", "delay", "newline"],
    ) {
        show_startup_section(ui, settings, changes_this_frame);
    }

    // Search section
    if section_matches(
        &query,
        "Search",
        &["search", "highlight", "case sensitive", "regex", "wrap"],
    ) {
        show_search_section(ui, settings, changes_this_frame);
    }

    // Scrollbar section
    if section_matches(
        &query,
        "Scrollbar",
        &["scrollbar", "thumb", "track", "autohide"],
    ) {
        show_scrollbar_section(ui, settings, changes_this_frame);
    }
}

fn section_matches(query: &str, title: &str, keywords: &[&str]) -> bool {
    if query.is_empty() {
        return true;
    }
    if title.to_lowercase().contains(query) {
        return true;
    }
    keywords.iter().any(|k| k.to_lowercase().contains(query))
}

// ============================================================================
// Behavior Section
// ============================================================================

fn show_behavior_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    collapsing_section(ui, "Behavior", "terminal_behavior", true, |ui| {
        ui.horizontal(|ui| {
            ui.label("Scrollback lines:");
            if ui
                .add_sized([SLIDER_WIDTH, SLIDER_HEIGHT], egui::Slider::new(
                    &mut settings.config.scrollback_lines,
                    1000..=100000,
                ))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        if ui
            .checkbox(
                &mut settings.config.exit_on_shell_exit,
                "Exit when shell exits",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }
    });
}

// ============================================================================
// Unicode Section
// ============================================================================

fn show_unicode_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    collapsing_section(ui, "Unicode", "terminal_unicode", false, |ui| {
        ui.horizontal(|ui| {
            ui.label("Unicode version:");
            let version_text = match settings.config.unicode_version {
                par_term_emu_core_rust::UnicodeVersion::Unicode9 => "9.0",
                par_term_emu_core_rust::UnicodeVersion::Unicode10 => "10.0",
                par_term_emu_core_rust::UnicodeVersion::Unicode11 => "11.0",
                par_term_emu_core_rust::UnicodeVersion::Unicode12 => "12.0",
                par_term_emu_core_rust::UnicodeVersion::Unicode13 => "13.0",
                par_term_emu_core_rust::UnicodeVersion::Unicode14 => "14.0",
                par_term_emu_core_rust::UnicodeVersion::Unicode15 => "15.0",
                par_term_emu_core_rust::UnicodeVersion::Unicode15_1 => "15.1",
                par_term_emu_core_rust::UnicodeVersion::Unicode16 => "16.0",
                par_term_emu_core_rust::UnicodeVersion::Auto => "Auto (latest)",
            };
            egui::ComboBox::from_id_salt("terminal_unicode_version")
                .selected_text(version_text)
                .show_ui(ui, |ui| {
                    let versions = [
                        (
                            par_term_emu_core_rust::UnicodeVersion::Auto,
                            "Auto (latest)",
                        ),
                        (par_term_emu_core_rust::UnicodeVersion::Unicode16, "16.0"),
                        (par_term_emu_core_rust::UnicodeVersion::Unicode15_1, "15.1"),
                        (par_term_emu_core_rust::UnicodeVersion::Unicode15, "15.0"),
                        (par_term_emu_core_rust::UnicodeVersion::Unicode14, "14.0"),
                        (par_term_emu_core_rust::UnicodeVersion::Unicode13, "13.0"),
                        (par_term_emu_core_rust::UnicodeVersion::Unicode12, "12.0"),
                        (par_term_emu_core_rust::UnicodeVersion::Unicode11, "11.0"),
                        (par_term_emu_core_rust::UnicodeVersion::Unicode10, "10.0"),
                        (par_term_emu_core_rust::UnicodeVersion::Unicode9, "9.0"),
                    ];
                    for (value, label) in versions {
                        if ui
                            .selectable_value(&mut settings.config.unicode_version, value, label)
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    }
                })
                .response
                .on_hover_text(
                    "Unicode version for character width calculations.\n\
                     Different versions have different width tables for emoji.\n\
                     Use older versions for compatibility with legacy systems.",
                );
        });

        ui.horizontal(|ui| {
            ui.label("Ambiguous width:");
            let width_text = match settings.config.ambiguous_width {
                par_term_emu_core_rust::AmbiguousWidth::Narrow => "Narrow (1 cell)",
                par_term_emu_core_rust::AmbiguousWidth::Wide => "Wide (2 cells)",
            };
            egui::ComboBox::from_id_salt("terminal_ambiguous_width")
                .selected_text(width_text)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_value(
                            &mut settings.config.ambiguous_width,
                            par_term_emu_core_rust::AmbiguousWidth::Narrow,
                            "Narrow (1 cell)",
                        )
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                    if ui
                        .selectable_value(
                            &mut settings.config.ambiguous_width,
                            par_term_emu_core_rust::AmbiguousWidth::Wide,
                            "Wide (2 cells)",
                        )
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                })
                .response
                .on_hover_text(
                    "Treatment of East Asian Ambiguous width characters.\n\
                     - Narrow: 1 cell (Western default)\n\
                     - Wide: 2 cells (CJK default, use for Chinese/Japanese/Korean)",
                );
        });

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.label("Answerback string:");
            if ui
                .text_edit_singleline(&mut settings.config.answerback_string)
                .on_hover_text(
                    "String sent in response to ENQ (0x05) control character.\n\
                     Used for legacy terminal identification.\n\
                     Leave empty (default) for security.\n\
                     Common values: \"par-term\", \"vt100\"",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.label(
            egui::RichText::new(
                "Warning: Setting this may expose terminal identification to applications",
            )
            .small()
            .color(egui::Color32::YELLOW),
        );
    });
}

// ============================================================================
// Shell Section
// ============================================================================

fn show_shell_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    collapsing_section(ui, "Shell", "terminal_shell", true, |ui| {
        ui.horizontal(|ui| {
            ui.label("Custom shell (optional):");
            if ui
                .add(egui::TextEdit::singleline(&mut settings.temp_custom_shell).desired_width(INPUT_WIDTH))
                .changed()
            {
                settings.config.custom_shell = if settings.temp_custom_shell.is_empty() {
                    None
                } else {
                    Some(settings.temp_custom_shell.clone())
                };
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if ui.button("Browse...").clicked()
                && let Some(path) = settings.pick_file_path("Select shell binary")
            {
                settings.temp_custom_shell = path.clone();
                settings.config.custom_shell = Some(path);
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Shell args (space-separated):");
            if ui
                .add(egui::TextEdit::singleline(&mut settings.temp_shell_args).desired_width(INPUT_WIDTH))
                .changed()
            {
                settings.config.shell_args = if settings.temp_shell_args.is_empty() {
                    None
                } else {
                    Some(
                        settings
                            .temp_shell_args
                            .split_whitespace()
                            .map(String::from)
                            .collect(),
                    )
                };
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Working directory (optional):");
            if ui
                .add(egui::TextEdit::singleline(&mut settings.temp_working_directory).desired_width(INPUT_WIDTH))
                .changed()
            {
                settings.config.working_directory = if settings.temp_working_directory.is_empty() {
                    None
                } else {
                    Some(settings.temp_working_directory.clone())
                };
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if ui.button("Browse...").clicked()
                && let Some(path) = settings.pick_folder_path("Select working directory")
            {
                settings.temp_working_directory = path.clone();
                settings.config.working_directory = Some(path);
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        if ui
            .checkbox(&mut settings.config.login_shell, "Login shell (-l)")
            .on_hover_text(
                "Spawn shell as login shell. This ensures PATH is properly initialized from /etc/paths, ~/.zprofile, etc. Recommended on macOS.",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }
    });
}

// ============================================================================
// Startup Section
// ============================================================================

fn show_startup_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    collapsing_section(ui, "Startup", "terminal_startup", false, |ui| {
        ui.label("Initial text to send when a session starts:");
        if ui
            .text_edit_multiline(&mut settings.temp_initial_text)
            .changed()
        {
            settings.config.initial_text = settings.temp_initial_text.clone();
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.horizontal(|ui| {
            ui.label("Delay (ms):");
            if ui
                .add(
                    egui::DragValue::new(&mut settings.config.initial_text_delay_ms)
                        .range(0..=5000),
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if ui
                .checkbox(
                    &mut settings.config.initial_text_send_newline,
                    "Append newline after text",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.label(
            egui::RichText::new("Supports \\n, \\r, \\t, \\xHH, \\e escape sequences.")
                .small()
                .weak(),
        );
    });
}

// ============================================================================
// Search Section
// ============================================================================

fn show_search_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    collapsing_section(ui, "Search", "terminal_search", true, |ui| {
        ui.label(egui::RichText::new("Highlight Colors").strong());

        // Match highlight color
        ui.horizontal(|ui| {
            ui.label("Match highlight:");
            let mut color = egui::Color32::from_rgba_unmultiplied(
                settings.config.search_highlight_color[0],
                settings.config.search_highlight_color[1],
                settings.config.search_highlight_color[2],
                settings.config.search_highlight_color[3],
            );
            if ui.color_edit_button_srgba(&mut color).changed() {
                settings.config.search_highlight_color =
                    [color.r(), color.g(), color.b(), color.a()];
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        // Current match highlight color
        ui.horizontal(|ui| {
            ui.label("Current match:");
            let mut color = egui::Color32::from_rgba_unmultiplied(
                settings.config.search_current_highlight_color[0],
                settings.config.search_current_highlight_color[1],
                settings.config.search_current_highlight_color[2],
                settings.config.search_current_highlight_color[3],
            );
            if ui.color_edit_button_srgba(&mut color).changed() {
                settings.config.search_current_highlight_color =
                    [color.r(), color.g(), color.b(), color.a()];
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.add_space(8.0);
        ui.label(egui::RichText::new("Default Options").strong());

        // Case sensitivity default
        if ui
            .checkbox(
                &mut settings.config.search_case_sensitive,
                "Case sensitive by default",
            )
            .on_hover_text("When enabled, search will be case-sensitive by default")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        // Regex default
        if ui
            .checkbox(&mut settings.config.search_regex, "Use regex by default")
            .on_hover_text(
                "When enabled, search patterns will be treated as regular expressions by default",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        // Wrap around
        if ui
            .checkbox(
                &mut settings.config.search_wrap_around,
                "Wrap around when navigating",
            )
            .on_hover_text("When enabled, navigating past the last match wraps to the first match")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.add_space(8.0);
        ui.label(egui::RichText::new("Keyboard Shortcuts").weak().small());
        ui.label(
            egui::RichText::new("  Cmd/Ctrl+F: Open search, Enter: Next, Shift+Enter: Previous")
                .weak()
                .small(),
        );
    });
}

// ============================================================================
// Scrollbar Section
// ============================================================================

fn show_scrollbar_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    collapsing_section(ui, "Scrollbar", "terminal_scrollbar", true, |ui| {
        ui.horizontal(|ui| {
            ui.label("Width:");
            if ui
                .add_sized([SLIDER_WIDTH, SLIDER_HEIGHT], egui::Slider::new(
                    &mut settings.config.scrollbar_width,
                    4.0..=50.0,
                ))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Autohide delay (ms, 0=never):");
            if ui
                .add_sized([SLIDER_WIDTH, SLIDER_HEIGHT], egui::Slider::new(
                    &mut settings.config.scrollbar_autohide_delay,
                    0..=5000,
                ))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.add_space(8.0);
        ui.label(egui::RichText::new("Colors").strong());

        ui.horizontal(|ui| {
            ui.label("Thumb color:");
            let mut thumb = egui::Color32::from_rgba_unmultiplied(
                (settings.config.scrollbar_thumb_color[0] * 255.0) as u8,
                (settings.config.scrollbar_thumb_color[1] * 255.0) as u8,
                (settings.config.scrollbar_thumb_color[2] * 255.0) as u8,
                (settings.config.scrollbar_thumb_color[3] * 255.0) as u8,
            );
            if egui::color_picker::color_edit_button_srgba(
                ui,
                &mut thumb,
                egui::color_picker::Alpha::Opaque,
            )
            .changed()
            {
                settings.config.scrollbar_thumb_color = [
                    thumb.r() as f32 / 255.0,
                    thumb.g() as f32 / 255.0,
                    thumb.b() as f32 / 255.0,
                    thumb.a() as f32 / 255.0,
                ];
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Track color:");
            let mut track = egui::Color32::from_rgba_unmultiplied(
                (settings.config.scrollbar_track_color[0] * 255.0) as u8,
                (settings.config.scrollbar_track_color[1] * 255.0) as u8,
                (settings.config.scrollbar_track_color[2] * 255.0) as u8,
                (settings.config.scrollbar_track_color[3] * 255.0) as u8,
            );
            if egui::color_picker::color_edit_button_srgba(
                ui,
                &mut track,
                egui::color_picker::Alpha::Opaque,
            )
            .changed()
            {
                settings.config.scrollbar_track_color = [
                    track.r() as f32 / 255.0,
                    track.g() as f32 / 255.0,
                    track.b() as f32 / 255.0,
                    track.a() as f32 / 255.0,
                ];
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });
    });
}
