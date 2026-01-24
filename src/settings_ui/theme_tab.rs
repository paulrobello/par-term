use super::SettingsUI;
use crate::themes::Theme;

pub fn show(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    ui.collapsing("Theme & Colors", |ui| {
        let available = Theme::available_themes();
        let mut selected = settings.config.theme.clone();

        ui.horizontal(|ui| {
            ui.label("Theme:");
            egui::ComboBox::from_id_salt("theme_select")
                .width(220.0)
                .selected_text(selected.clone())
                .show_ui(ui, |ui| {
                    for theme in &available {
                        ui.selectable_value(&mut selected, theme.to_string(), *theme);
                    }
                });
        });

        if selected != settings.config.theme {
            settings.config.theme = selected;
            settings.has_changes = true;
            *changes_this_frame = true;
        }
    });
}
