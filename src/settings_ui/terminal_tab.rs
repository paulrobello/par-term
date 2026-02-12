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

use super::SettingsUI;
use super::section::{INPUT_WIDTH, SLIDER_WIDTH, collapsing_section};
use std::collections::HashSet;

const SLIDER_HEIGHT: f32 = 18.0;

/// Show the terminal tab content.
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    let query = settings.search_query.trim().to_lowercase();

    // Behavior section
    if section_matches(
        &query,
        "Behavior",
        &[
            "scrollback",
            "exit",
            "shell exit",
            "jobs",
            "confirm",
            "close",
        ],
    ) {
        show_behavior_section(ui, settings, changes_this_frame, collapsed);
    }

    // Unicode section (collapsed by default)
    if section_matches(
        &query,
        "Unicode",
        &["unicode", "width", "answerback", "ambiguous"],
    ) {
        show_unicode_section(ui, settings, changes_this_frame, collapsed);
    }

    // Shell section
    if section_matches(
        &query,
        "Shell",
        &[
            "shell",
            "custom shell",
            "working directory",
            "login",
            "startup",
            "previous",
            "home",
        ],
    ) {
        show_shell_section(ui, settings, changes_this_frame, collapsed);
    }

    // Startup section (collapsed by default)
    if section_matches(
        &query,
        "Startup",
        &["initial text", "startup", "delay", "newline"],
    ) {
        show_startup_section(ui, settings, changes_this_frame, collapsed);
    }

    // Search section
    if section_matches(
        &query,
        "Search",
        &["search", "highlight", "case sensitive", "regex", "wrap"],
    ) {
        show_search_section(ui, settings, changes_this_frame, collapsed);
    }

    // Semantic History section
    if section_matches(
        &query,
        "Semantic History",
        &["semantic", "history", "file", "editor", "path", "click"],
    ) {
        show_semantic_history_section(ui, settings, changes_this_frame, collapsed);
    }

    // Scrollbar section
    if section_matches(
        &query,
        "Scrollbar",
        &["scrollbar", "thumb", "track", "autohide", "marker"],
    ) {
        show_scrollbar_section(ui, settings, changes_this_frame, collapsed);
    }

    // Command Separators section
    if section_matches(
        &query,
        "Command Separators",
        &["separator", "command", "line", "divider", "prompt"],
    ) {
        show_command_separator_section(ui, settings, changes_this_frame, collapsed);
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
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "Behavior", "terminal_behavior", true, collapsed, |ui| {
        ui.horizontal(|ui| {
            ui.label("Scrollback lines:");
            if ui
                .add_sized(
                    [SLIDER_WIDTH, SLIDER_HEIGHT],
                    egui::Slider::new(&mut settings.config.scrollback_lines, 1000..=100000),
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Shell exit action:");
            egui::ComboBox::from_id_salt("shell_exit_action")
                .selected_text(settings.config.shell_exit_action.display_name())
                .show_ui(ui, |ui| {
                    for action in crate::config::ShellExitAction::all() {
                        if ui
                            .selectable_value(
                                &mut settings.config.shell_exit_action,
                                *action,
                                action.display_name(),
                            )
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    }
                });
        });

        ui.add_space(8.0);
        ui.label(egui::RichText::new("Close Confirmation").strong());

        if ui
            .checkbox(
                &mut settings.config.prompt_on_quit,
                "Confirm before quitting with open sessions",
            )
            .on_hover_text(
                "When enabled, closing the window will show a confirmation dialog\n\
                 if there are any open terminal sessions.",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if ui
            .checkbox(
                &mut settings.config.confirm_close_running_jobs,
                "Confirm before closing tabs with running jobs",
            )
            .on_hover_text(
                "When enabled, closing a tab with a running command will show a confirmation dialog.\n\
                 Requires shell integration to detect running commands.",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        // Jobs to ignore list (only shown when confirmation is enabled)
        if settings.config.confirm_close_running_jobs {
            ui.horizontal(|ui| {
                ui.add_space(20.0);
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new("Jobs to ignore (won't trigger confirmation):").small(),
                    );
                    ui.horizontal(|ui| {
                        // Show current list as comma-separated
                        let mut jobs_text = settings.config.jobs_to_ignore.join(", ");
                        let response = ui
                            .add(
                                egui::TextEdit::singleline(&mut jobs_text)
                                    .desired_width(INPUT_WIDTH)
                                    .hint_text("bash, zsh, cat, sleep"),
                            )
                            .on_hover_text(
                                "Comma-separated list of process names.\n\
                                 These processes won't trigger the close confirmation.\n\
                                 Common shells and pagers are ignored by default.",
                            );
                        if response.changed() {
                            // Parse comma-separated list
                            settings.config.jobs_to_ignore = jobs_text
                                .split(',')
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect();
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    });

                    // Reset to defaults button
                    if ui
                        .small_button("Reset to defaults")
                        .on_hover_text("Restore the default list of ignored jobs")
                        .clicked()
                    {
                        settings.config.jobs_to_ignore = crate::config::defaults::jobs_to_ignore();
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });
            });
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
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "Unicode", "terminal_unicode", false, collapsed, |ui| {
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

        ui.horizontal(|ui| {
            ui.label("Normalization:");
            let norm_text = match settings.config.normalization_form {
                par_term_emu_core_rust::NormalizationForm::None => "None",
                par_term_emu_core_rust::NormalizationForm::NFC => "NFC (default)",
                par_term_emu_core_rust::NormalizationForm::NFD => "NFD",
                par_term_emu_core_rust::NormalizationForm::NFKC => "NFKC",
                par_term_emu_core_rust::NormalizationForm::NFKD => "NFKD",
            };
            egui::ComboBox::from_id_salt("terminal_normalization_form")
                .selected_text(norm_text)
                .show_ui(ui, |ui| {
                    let forms = [
                        (
                            par_term_emu_core_rust::NormalizationForm::NFC,
                            "NFC (default)",
                        ),
                        (par_term_emu_core_rust::NormalizationForm::NFD, "NFD"),
                        (par_term_emu_core_rust::NormalizationForm::NFKC, "NFKC"),
                        (par_term_emu_core_rust::NormalizationForm::NFKD, "NFKD"),
                        (
                            par_term_emu_core_rust::NormalizationForm::None,
                            "None (disabled)",
                        ),
                    ];
                    for (value, label) in forms {
                        if ui
                            .selectable_value(&mut settings.config.normalization_form, value, label)
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    }
                })
                .response
                .on_hover_text(
                    "Unicode normalization form for text processing.\n\
                     - NFC: Canonical composition (default, most compatible)\n\
                     - NFD: Canonical decomposition (macOS HFS+ style)\n\
                     - NFKC: Compatibility composition (resolves ligatures)\n\
                     - NFKD: Compatibility decomposition\n\
                     - None: No normalization (store text as-is)",
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
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "Shell", "terminal_shell", true, collapsed, |ui| {
        ui.horizontal(|ui| {
            ui.label("Custom shell (optional):");
            if ui
                .add(
                    egui::TextEdit::singleline(&mut settings.temp_custom_shell)
                        .desired_width(INPUT_WIDTH),
                )
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
                .add(
                    egui::TextEdit::singleline(&mut settings.temp_shell_args)
                        .desired_width(INPUT_WIDTH),
                )
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

        ui.add_space(8.0);
        ui.label(egui::RichText::new("Startup Directory").strong());

        // Startup directory mode dropdown
        ui.horizontal(|ui| {
            ui.label("Mode:");
            let mode_text = settings.config.startup_directory_mode.display_name();
            egui::ComboBox::from_id_salt("startup_directory_mode")
                .selected_text(mode_text)
                .show_ui(ui, |ui| {
                    use crate::config::StartupDirectoryMode;
                    for mode in StartupDirectoryMode::all() {
                        if ui
                            .selectable_value(
                                &mut settings.config.startup_directory_mode,
                                *mode,
                                mode.display_name(),
                            )
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    }
                })
                .response
                .on_hover_text(
                    "Controls where new terminal sessions start:\n\
                     • Home: Start in your home directory\n\
                     • Previous Session: Remember and restore the last working directory\n\
                     • Custom: Start in a specific directory",
                );
        });

        // Custom directory path (only shown when mode is Custom)
        if settings.config.startup_directory_mode == crate::config::StartupDirectoryMode::Custom {
            ui.horizontal(|ui| {
                ui.label("Custom directory:");
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut settings.temp_startup_directory)
                            .desired_width(INPUT_WIDTH),
                    )
                    .changed()
                {
                    settings.config.startup_directory =
                        if settings.temp_startup_directory.is_empty() {
                            None
                        } else {
                            Some(settings.temp_startup_directory.clone())
                        };
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }

                if ui.button("Browse...").clicked()
                    && let Some(path) = settings.pick_folder_path("Select startup directory")
                {
                    settings.temp_startup_directory = path.clone();
                    settings.config.startup_directory = Some(path);
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        }

        // Show last working directory info when in Previous mode
        if settings.config.startup_directory_mode == crate::config::StartupDirectoryMode::Previous {
            if let Some(ref last_dir) = settings.config.last_working_directory {
                ui.label(
                    egui::RichText::new(format!("Last session: {}", last_dir))
                        .small()
                        .weak(),
                );
            } else {
                ui.label(
                    egui::RichText::new("No previous session directory saved yet")
                        .small()
                        .weak(),
                );
            }
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
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "Startup", "terminal_startup", false, collapsed, |ui| {
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
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "Search", "terminal_search", true, collapsed, |ui| {
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
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Scrollbar",
        "terminal_scrollbar",
        true,
        collapsed,
        |ui| {
            if ui
                .checkbox(
                    &mut settings.config.scrollbar_command_marks,
                    "Show command markers (requires shell integration)",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            // Indent the tooltip option under command markers
            ui.horizontal(|ui| {
                ui.add_space(20.0);
                ui.add_enabled_ui(settings.config.scrollbar_command_marks, |ui| {
                    if ui
                        .checkbox(
                            &mut settings.config.scrollbar_mark_tooltips,
                            "Show tooltips on hover",
                        )
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });
            });

            ui.horizontal(|ui| {
                ui.label("Width:");
                if ui
                    .add_sized(
                        [SLIDER_WIDTH, SLIDER_HEIGHT],
                        egui::Slider::new(&mut settings.config.scrollbar_width, 4.0..=50.0),
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Autohide delay (ms, 0=never):");
                if ui
                    .add_sized(
                        [SLIDER_WIDTH, SLIDER_HEIGHT],
                        egui::Slider::new(&mut settings.config.scrollbar_autohide_delay, 0..=5000),
                    )
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
        },
    );
}

// ============================================================================
// Command Separator Section
// ============================================================================

fn show_command_separator_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Command Separators",
        "terminal_command_separator",
        false,
        collapsed,
        |ui| {
            if ui
                .checkbox(
                    &mut settings.config.command_separator_enabled,
                    "Show separator lines between commands (requires shell integration)",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            ui.add_enabled_ui(settings.config.command_separator_enabled, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Thickness (px):");
                    if ui
                        .add_sized(
                            [SLIDER_WIDTH, SLIDER_HEIGHT],
                            egui::Slider::new(
                                &mut settings.config.command_separator_thickness,
                                0.5..=5.0,
                            ),
                        )
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Opacity:");
                    if ui
                        .add_sized(
                            [SLIDER_WIDTH, SLIDER_HEIGHT],
                            egui::Slider::new(
                                &mut settings.config.command_separator_opacity,
                                0.0..=1.0,
                            ),
                        )
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });

                if ui
                    .checkbox(
                        &mut settings.config.command_separator_exit_color,
                        "Color by exit code (green=success, red=failure)",
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }

                // Custom color picker (only when exit-code coloring is off)
                ui.add_enabled_ui(!settings.config.command_separator_exit_color, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Custom color:");
                        let mut color = egui::Color32::from_rgb(
                            settings.config.command_separator_color[0],
                            settings.config.command_separator_color[1],
                            settings.config.command_separator_color[2],
                        );
                        if egui::color_picker::color_edit_button_srgba(
                            ui,
                            &mut color,
                            egui::color_picker::Alpha::Opaque,
                        )
                        .changed()
                        {
                            settings.config.command_separator_color =
                                [color.r(), color.g(), color.b()];
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    });
                });
            });
        },
    );
}

// ============================================================================
// Semantic History Section
// ============================================================================

fn show_semantic_history_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Semantic History",
        "terminal_semantic_history",
        true,
        collapsed,
        |ui| {
            ui.label(
                egui::RichText::new(
                    "Click file paths in terminal output to open them in your editor.",
                )
                .weak(),
            );

            ui.add_space(4.0);

            if ui
                .checkbox(
                    &mut settings.config.semantic_history_enabled,
                    "Enable file path detection",
                )
                .on_hover_text(
                    "Detect file paths in terminal output.\n\
                 Cmd+Click (macOS) or Ctrl+Click (Windows/Linux) to open.",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label("Editor mode:");
                egui::ComboBox::from_id_salt("semantic_history_editor_mode")
                    .selected_text(settings.config.semantic_history_editor_mode.display_name())
                    .show_ui(ui, |ui| {
                        for mode in crate::config::SemanticHistoryEditorMode::all() {
                            if ui
                                .selectable_value(
                                    &mut settings.config.semantic_history_editor_mode,
                                    *mode,
                                    mode.display_name(),
                                )
                                .changed()
                            {
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                        }
                    });
            });

            // Show description based on selected mode
            let mode_description = match settings.config.semantic_history_editor_mode {
                crate::config::SemanticHistoryEditorMode::Custom => {
                    "Use the custom editor command configured below"
                }
                crate::config::SemanticHistoryEditorMode::EnvironmentVariable => {
                    "Use the $EDITOR environment variable"
                }
                crate::config::SemanticHistoryEditorMode::SystemDefault => {
                    "Use the system default application for each file type"
                }
            };
            ui.label(egui::RichText::new(mode_description).small().weak());

            // Only show custom editor command when mode is Custom
            if settings.config.semantic_history_editor_mode
                == crate::config::SemanticHistoryEditorMode::Custom
            {
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label("Editor command:");
                    if ui
                        .add(
                            egui::TextEdit::singleline(
                                &mut settings.config.semantic_history_editor,
                            )
                            .desired_width(INPUT_WIDTH),
                        )
                        .on_hover_text(
                            "Command to open files.\n\n\
                         Placeholders:\n\
                         • {file} - file path\n\
                         • {line} - line number (if available)\n\
                         • {col} - column number (if available)\n\n\
                         Examples:\n\
                         • code -g {file}:{line} (VS Code)\n\
                         • subl {file}:{line} (Sublime Text)\n\
                         • vim +{line} {file} (Vim)",
                        )
                        .changed()
                    {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                });

                if settings.config.semantic_history_editor.is_empty() {
                    ui.label(
                        egui::RichText::new(
                            "Note: When custom command is empty, falls back to system default",
                        )
                        .small()
                        .weak(),
                    );
                }
            }
        },
    );
}
