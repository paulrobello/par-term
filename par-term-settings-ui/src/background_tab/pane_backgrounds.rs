use crate::SettingsUI;
use crate::section::collapsing_section;
use par_term_config::BackgroundImageMode;
use std::collections::HashSet;

pub fn show_pane_backgrounds(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Per-Pane Background",
        "per_pane_background",
        false,
        collapsed,
        |ui| {
            ui.label("Override the global background for individual split panes.");
            ui.add_space(4.0);

            // Identify Panes button + Pane index selector
            ui.horizontal(|ui| {
                if ui
                    .button("Identify Panes")
                    .on_hover_text("Flash pane indices on the terminal window for 3 seconds")
                    .clicked()
                {
                    settings.identify_panes_requested = true;
                }
            });

            ui.add_space(4.0);

            // Initialize temp fields from pane 0 config on first render
            if settings.temp_pane_bg_index.is_none() {
                settings.temp_pane_bg_index = Some(0);
                if let Some((image_path, mode, opacity, darken)) =
                    settings.config.get_pane_background(0)
                {
                    settings.temp_pane_bg_path = image_path;
                    settings.temp_pane_bg_mode = mode;
                    settings.temp_pane_bg_opacity = opacity;
                    settings.temp_pane_bg_darken = darken;
                }
            }

            // Pane index selector with prev/next buttons
            ui.horizontal(|ui| {
                ui.label("Pane index:");

                let mut index = settings.temp_pane_bg_index.unwrap_or(0);
                let mut changed = false;

                if ui.add_enabled(index > 0, egui::Button::new("<")).clicked() {
                    index = index.saturating_sub(1);
                    changed = true;
                }

                if ui
                    .add(egui::DragValue::new(&mut index).range(0..=9))
                    .changed()
                {
                    changed = true;
                }

                if ui.add_enabled(index < 9, egui::Button::new(">")).clicked() {
                    index = index.saturating_add(1).min(9);
                    changed = true;
                }

                if changed {
                    settings.temp_pane_bg_index = Some(index);
                    if let Some((image_path, mode, opacity, darken)) =
                        settings.config.get_pane_background(index)
                    {
                        settings.temp_pane_bg_path = image_path;
                        settings.temp_pane_bg_mode = mode;
                        settings.temp_pane_bg_opacity = opacity;
                        settings.temp_pane_bg_darken = darken;
                    } else {
                        settings.temp_pane_bg_path.clear();
                        settings.temp_pane_bg_mode = BackgroundImageMode::default();
                        settings.temp_pane_bg_opacity = 1.0;
                        settings.temp_pane_bg_darken = 0.0;
                    }
                }
            });

            // Track whether any pane background field changed this frame
            let mut pane_bg_changed = false;

            // Image path
            ui.horizontal(|ui| {
                ui.label("Image path:");
                if ui
                    .text_edit_singleline(&mut settings.temp_pane_bg_path)
                    .changed()
                {
                    pane_bg_changed = true;
                }

                if ui.button("Browse\u{2026}").clicked()
                    && let Some(path) = settings.pick_file_path("Select pane background image")
                {
                    settings.temp_pane_bg_path = path;
                    pane_bg_changed = true;
                }
            });

            // Mode dropdown
            ui.horizontal(|ui| {
                ui.label("Mode:");
                let current = settings.temp_pane_bg_mode as usize;
                let mut selected = current;
                egui::ComboBox::from_id_salt("pane_bg_mode")
                    .selected_text(match current {
                        0 => "Fit",
                        1 => "Fill",
                        2 => "Stretch",
                        3 => "Tile",
                        4 => "Center",
                        _ => "Stretch",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut selected, 0, "Fit");
                        ui.selectable_value(&mut selected, 1, "Fill");
                        ui.selectable_value(&mut selected, 2, "Stretch");
                        ui.selectable_value(&mut selected, 3, "Tile");
                        ui.selectable_value(&mut selected, 4, "Center");
                    });
                if selected != current {
                    settings.temp_pane_bg_mode = match selected {
                        0 => BackgroundImageMode::Fit,
                        1 => BackgroundImageMode::Fill,
                        2 => BackgroundImageMode::Stretch,
                        3 => BackgroundImageMode::Tile,
                        4 => BackgroundImageMode::Center,
                        _ => BackgroundImageMode::default(),
                    };
                    pane_bg_changed = true;
                }
            });

            // Opacity slider
            ui.horizontal(|ui| {
                ui.label("Opacity:");
                if ui
                    .add(egui::Slider::new(
                        &mut settings.temp_pane_bg_opacity,
                        0.0..=1.0,
                    ))
                    .changed()
                {
                    pane_bg_changed = true;
                }
            });

            // Darken slider
            ui.horizontal(|ui| {
                ui.label("Darken:");
                if ui
                    .add(egui::Slider::new(
                        &mut settings.temp_pane_bg_darken,
                        0.0..=1.0,
                    ))
                    .on_hover_text(
                        "Darken the background image (0.0 = original, 1.0 = fully black)",
                    )
                    .changed()
                {
                    pane_bg_changed = true;
                }
            });

            // Auto-apply changes to config in real-time
            if pane_bg_changed {
                let index = settings.temp_pane_bg_index.unwrap_or(0);
                settings
                    .config
                    .pane_backgrounds
                    .retain(|pb| pb.index != index);
                if !settings.temp_pane_bg_path.is_empty() {
                    settings
                        .config
                        .pane_backgrounds
                        .push(par_term_config::PaneBackgroundConfig {
                            index,
                            image: settings.temp_pane_bg_path.clone(),
                            mode: settings.temp_pane_bg_mode,
                            opacity: settings.temp_pane_bg_opacity,
                            darken: settings.temp_pane_bg_darken,
                        });
                }
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            ui.add_space(4.0);

            // Clear pane background button
            if ui.button("Clear pane background").clicked() {
                let index = settings.temp_pane_bg_index.unwrap_or(0);
                settings
                    .config
                    .pane_backgrounds
                    .retain(|pb| pb.index != index);
                settings.temp_pane_bg_path.clear();
                settings.temp_pane_bg_mode = BackgroundImageMode::default();
                settings.temp_pane_bg_opacity = 1.0;
                settings.temp_pane_bg_darken = 0.0;
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            // Show configured pane backgrounds
            if !settings.config.pane_backgrounds.is_empty() {
                ui.add_space(4.0);
                ui.label("Configured pane backgrounds:");
                for pb in &settings.config.pane_backgrounds {
                    ui.label(format!(
                        "  Pane {}: {} ({:?}, opacity: {:.1}, darken: {:.1})",
                        pb.index, pb.image, pb.mode, pb.opacity, pb.darken
                    ));
                }
            }
        },
    );
}
