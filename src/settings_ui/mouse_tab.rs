use super::SettingsUI;

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
    });
}
