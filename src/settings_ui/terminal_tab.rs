use super::SettingsUI;

pub fn show(ui: &mut egui::Ui, settings: &mut SettingsUI, changes_this_frame: &mut bool) {
    ui.collapsing("Terminal", |ui| {
        ui.horizontal(|ui| {
            ui.label("Columns:");
            if ui
                .add(egui::Slider::new(&mut settings.config.cols, 40..=300))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Rows:");
            if ui
                .add(egui::Slider::new(&mut settings.config.rows, 10..=100))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Scrollback lines:");
            if ui
                .add(egui::Slider::new(
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
