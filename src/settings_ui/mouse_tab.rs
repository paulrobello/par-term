use super::SettingsUI;
use crate::config::DroppedFileQuoteStyle;

pub fn show_selection(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    ui.collapsing("Selection & Clipboard", |ui| {
        if ui
            .checkbox(
                &mut settings.config.auto_copy_selection,
                "Auto-copy selection",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if ui
            .checkbox(
                &mut settings.config.copy_trailing_newline,
                "Include trailing newline when copying",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if ui
            .checkbox(
                &mut settings.config.middle_click_paste,
                "Middle-click paste",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.separator();
        ui.label("Dropped Files");

        ui.horizontal(|ui| {
            ui.label("Quote style:");
            egui::ComboBox::from_id_salt("dropped_file_quote_style")
                .selected_text(settings.config.dropped_file_quote_style.display_name())
                .show_ui(ui, |ui| {
                    for style in DroppedFileQuoteStyle::all() {
                        if ui
                            .selectable_value(
                                &mut settings.config.dropped_file_quote_style,
                                *style,
                                style.display_name(),
                            )
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    }
                });
        })
        .response
        .on_hover_text(
            "How to quote file paths when dropped into the terminal:\n\
             - Single quotes: Safest for most shells\n\
             - Double quotes: Allows variable expansion\n\
             - Backslash: Escape individual characters\n\
             - None: Insert raw path (not recommended)",
        );

        ui.horizontal(|ui| {
            ui.label("Max clipboard sync events:");
            if ui
                .add(egui::Slider::new(
                    &mut settings.config.clipboard_max_sync_events,
                    8..=256,
                ))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Max clipboard event bytes:");
            if ui
                .add(egui::Slider::new(
                    &mut settings.config.clipboard_max_event_bytes,
                    512..=16384,
                ))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.separator();
        ui.label("Word Selection");

        ui.horizontal(|ui| {
            ui.label("Word characters:");
            if ui
                .add(
                    egui::TextEdit::singleline(&mut settings.config.word_characters)
                        .hint_text("/-+\\~_.")
                        .desired_width(150.0),
                )
                .on_hover_text(
                    "Characters considered part of a word (in addition to alphanumeric).\n\
                     Default: /-+\\~_. (matches iTerm2)",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        if ui
            .checkbox(
                &mut settings.config.smart_selection_enabled,
                "Enable smart selection",
            )
            .on_hover_text(
                "When enabled, double-click will try to match patterns like URLs, emails, paths\n\
                 before falling back to word boundary selection.",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        // Show smart selection rules if enabled
        if settings.config.smart_selection_enabled {
            ui.separator();
            ui.label("Smart Selection Rules");
            ui.label(
                egui::RichText::new("Higher precision rules are checked first")
                    .small()
                    .weak(),
            );

            egui::ScrollArea::vertical()
                .max_height(200.0)
                .show(ui, |ui| {
                    for rule in &mut settings.config.smart_selection_rules {
                        ui.horizontal(|ui| {
                            if ui.checkbox(&mut rule.enabled, "").changed() {
                                settings.has_changes = true;
                                *changes_this_frame = true;
                            }
                            let label = egui::RichText::new(&rule.name);
                            let label = if rule.enabled {
                                label
                            } else {
                                label.strikethrough().weak()
                            };
                            ui.label(label).on_hover_ui(|ui| {
                                ui.label(format!("Pattern: {}", rule.regex));
                                ui.label(format!("Precision: {:?}", rule.precision));
                            });
                        });
                    }
                });

            // Button to reset rules to defaults
            if ui
                .button("Reset rules to defaults")
                .on_hover_text("Replace all rules with the default set")
                .clicked()
            {
                settings.config.smart_selection_rules =
                    crate::config::default_smart_selection_rules();
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        }
    });
}

pub fn show_mouse_behavior(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
) {
    ui.collapsing("Mouse Behavior", |ui| {
        ui.horizontal(|ui| {
            ui.label("Scroll speed:");
            if ui
                .add(egui::Slider::new(
                    &mut settings.config.mouse_scroll_speed,
                    0.1..=10.0,
                ))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Double-click threshold (ms):");
            if ui
                .add(egui::Slider::new(
                    &mut settings.config.mouse_double_click_threshold,
                    100..=1000,
                ))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Triple-click threshold (ms):");
            if ui
                .add(egui::Slider::new(
                    &mut settings.config.mouse_triple_click_threshold,
                    100..=1000,
                ))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.separator();
        ui.label("Advanced Mouse Features");

        #[cfg(target_os = "macos")]
        let option_click_label = "Option+Click moves cursor";
        #[cfg(not(target_os = "macos"))]
        let option_click_label = "Alt+Click moves cursor";

        if ui
            .checkbox(
                &mut settings.config.option_click_moves_cursor,
                option_click_label,
            )
            .on_hover_text(
                "Position the text cursor at the clicked location.\n\
                 Useful for quick cursor positioning in shells and editors.\n\
                 Only works when viewing the current terminal (not scrolled back).",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if ui
            .checkbox(
                &mut settings.config.focus_follows_mouse,
                "Focus follows mouse",
            )
            .on_hover_text(
                "Automatically focus the terminal window when the mouse enters it.\n\
                 No click required - the window gains focus on mouse hover.",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if ui
            .checkbox(
                &mut settings.config.report_horizontal_scroll,
                "Report horizontal scroll events",
            )
            .on_hover_text(
                "Report horizontal scroll (trackpad swipe, tilt wheel) to applications.\n\
                 Uses mouse button codes 6 (left) and 7 (right) when mouse tracking is enabled.",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }
    });
}
