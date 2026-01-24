use super::SettingsUI;

pub fn show(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    ui.collapsing("Screenshot", |ui| {
        ui.horizontal(|ui| {
            ui.label("Format:");

            let options = ["png", "jpeg", "svg", "html"];
            let mut selected = settings.config.screenshot_format.clone();

            egui::ComboBox::from_id_salt("screenshot_format")
                .width(140.0)
                .selected_text(selected.as_str())
                .show_ui(ui, |ui| {
                    for opt in options {
                        ui.selectable_value(&mut selected, opt.to_string(), opt);
                    }
                });

            if selected != settings.config.screenshot_format {
                settings.config.screenshot_format = selected;
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });
        ui.label("Supported: png, jpeg, svg, html");
    });
}
