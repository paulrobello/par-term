//! Selection, clipboard, and dropped-files settings sections.

use crate::SettingsUI;
use crate::section::{SLIDER_WIDTH, collapsing_section};
use par_term_config::DroppedFileQuoteStyle;
use std::collections::HashSet;

const SLIDER_HEIGHT: f32 = 18.0;

// ============================================================================
// Selection & Clipboard Section
// ============================================================================

pub(super) fn show_selection_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Selection & Clipboard",
        "input_selection",
        true,
        collapsed,
        |ui| {
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
                ui.label("Paste delay (ms):");
                if ui
                    .add_sized(
                        [SLIDER_WIDTH, SLIDER_HEIGHT],
                        egui::Slider::new(&mut settings.config.paste_delay_ms, 0..=500),
                    )
                    .on_hover_text(
                        "Delay between pasted lines in milliseconds (0 = no delay). \
                     Useful for slow terminals or remote connections.",
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.separator();
            ui.label("Dropped Files");

            ui.horizontal(|ui| {
                ui.label("Quote style:");
                egui::ComboBox::from_id_salt("input_dropped_file_quote_style")
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
            .on_hover_text("How to quote file paths when dropped into the terminal");
        },
    );
}

// ============================================================================
// Clipboard Limits Section
// ============================================================================

pub(super) fn show_clipboard_limits_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Clipboard Limits",
        "input_clipboard_limits",
        false,
        collapsed,
        |ui| {
            ui.horizontal(|ui| {
                ui.label("Max clipboard sync events:");
                if ui
                    .add_sized(
                        [SLIDER_WIDTH, SLIDER_HEIGHT],
                        egui::Slider::new(&mut settings.config.clipboard_max_sync_events, 8..=256),
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Max clipboard event bytes:");
                if ui
                    .add_sized(
                        [SLIDER_WIDTH, SLIDER_HEIGHT],
                        egui::Slider::new(
                            &mut settings.config.clipboard_max_event_bytes,
                            512..=16384,
                        ),
                    )
                    .changed()
                {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        },
    );
}
