//! Variables reference section — built-in snippet variable documentation.

use crate::SettingsUI;
use crate::section::collapsing_section;
use std::collections::HashSet;

pub(super) fn show_variables_reference_section(
    ui: &mut egui::Ui,
    _settings: &mut SettingsUI,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Variables Reference",
        "snippets_variables",
        false,
        collapsed,
        |ui| {
            ui.label("Built-in variables available for use in snippets:");
            ui.add_space(4.0);

            use par_term_config::snippets::BuiltInVariable;

            egui::Grid::new("snippet_variables_grid")
                .num_columns(2)
                .spacing([20.0, 4.0])
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Variable").strong());
                    ui.label(egui::RichText::new("Description").strong());
                    ui.end_row();

                    for (name, description) in BuiltInVariable::all() {
                        ui.label(egui::RichText::new(format!("\\({})", name)).monospace());
                        ui.label(
                            egui::RichText::new(*description)
                                .small()
                                .color(egui::Color32::GRAY),
                        );
                        ui.end_row();
                    }
                });

            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("Example: \"echo 'Report for \\(user) on \\(date)'\"")
                    .monospace()
                    .small(),
            );
        },
    );
}
