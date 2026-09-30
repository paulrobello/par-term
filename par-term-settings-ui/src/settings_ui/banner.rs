//! Inline banner under the Settings header (UX.md SS7, SS8).
//!
//! Failures the user must act on (a config or profile save that did not
//! reach disk, an import that could not be read, an editor that would not
//! open) used to reach only the debug log. They now show here until
//! dismissed or replaced. Informational results (an import summary, the
//! settings a save needs a restart for) use the same banner.

use par_term_config::Config;

use super::SettingsUI;

/// Severity of a banner message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BannerKind {
    /// Something failed; the user's edit or file did not go where they asked.
    Error,
    /// A result worth reading (import summary, restart notice).
    Info,
}

/// One banner message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Banner {
    /// Severity, which picks the color.
    pub kind: BannerKind,
    /// Text shown to the user.
    pub text: String,
}

impl SettingsUI {
    /// Show an error in the banner, replacing whatever it held.
    pub fn show_error_banner(&mut self, text: impl Into<String>) {
        self.banner = Some(Banner {
            kind: BannerKind::Error,
            text: text.into(),
        });
    }

    /// Show an informational message in the banner.
    pub fn show_info_banner(&mut self, text: impl Into<String>) {
        self.banner = Some(Banner {
            kind: BannerKind::Info,
            text: text.into(),
        });
    }

    /// The banner currently showing, if any.
    pub fn banner(&self) -> Option<&Banner> {
        self.banner.as_ref()
    }

    /// Report how writing the config handed over by Save went.
    ///
    /// On failure the baseline moved by Save is put back, so the edits show
    /// as unsaved again, Revert still targets what is on disk, and a pending
    /// close is cancelled so the error stays on screen.
    pub fn report_config_save(&mut self, result: Result<(), String>) {
        let previous = self.config_save_rollback.take();
        match result {
            Ok(()) => {
                if let Some(previous) = previous {
                    self.announce_restart_needed(&previous);
                }
            }
            Err(error) => {
                if let Some(previous) = previous {
                    self.baseline_config = previous;
                }
                self.close_pending = false;
                self.show_error_banner(format!(
                    "Could not save {}: {error}. Your changes are still unsaved.",
                    Config::config_path().display()
                ));
            }
        }
    }

    /// Report how writing the profiles handed over by Save went. Same
    /// rollback as [`Self::report_config_save`].
    pub fn report_profile_save(&mut self, result: Result<(), String>) {
        let previous = self.profile_save_rollback.take();
        if let Err(error) = result {
            if let Some(previous) = previous {
                self.profile_modal_ui.restore_baseline(previous);
            }
            self.close_pending = false;
            self.show_error_banner(format!(
                "Could not save profiles: {error}. Your profile changes are still unsaved."
            ));
        }
    }

    /// config.yaml changed on disk (UX.md SS10). With no unsaved edits the
    /// on-disk config becomes the working config and the baseline. With
    /// unsaved edits it is held back and a banner offers to load it; Save
    /// writes over it instead.
    pub fn config_changed_on_disk(&mut self, on_disk: Config) {
        if super::configs_equal(&on_disk, &self.baseline_config) {
            return;
        }
        if self.has_unsaved_changes() {
            self.disk_config_pending = Some(on_disk);
            self.show_info_banner(
                "config.yaml changed on disk. Load it to drop your unsaved edits, \
                 or Save to write over it.",
            );
        } else {
            self.force_update_config(on_disk);
            self.show_info_banner("Loaded config.yaml, which changed on disk.");
        }
    }

    /// Load the on-disk config held back by [`Self::config_changed_on_disk`],
    /// dropping unsaved config edits.
    pub fn load_pending_disk_config(&mut self) {
        if let Some(on_disk) = self.disk_config_pending.take() {
            self.force_update_config(on_disk);
            self.banner = None;
        }
    }

    pub(super) fn render_banner(&mut self, ui: &mut egui::Ui) {
        let Some(banner) = &self.banner else {
            return;
        };
        let (fill, stroke) = match banner.kind {
            BannerKind::Error => (
                egui::Color32::from_rgb(70, 24, 24),
                egui::Color32::from_rgb(244, 67, 54),
            ),
            BannerKind::Info => (
                egui::Color32::from_rgb(20, 44, 70),
                egui::Color32::from_rgb(33, 150, 243),
            ),
        };
        let text = banner.text.clone();
        let mut dismiss = false;
        let mut load = false;
        egui::Frame::NONE
            .fill(fill)
            .stroke(egui::Stroke::new(1.0, stroke))
            .corner_radius(4.0)
            .inner_margin(egui::Margin::symmetric(8, 6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Label::new(egui::RichText::new(text).color(egui::Color32::WHITE))
                            .wrap(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("✕").on_hover_text("Dismiss").clicked() {
                            dismiss = true;
                        }
                        if self.disk_config_pending.is_some()
                            && ui
                                .small_button("Load from disk")
                                .on_hover_text("Replace your unsaved edits with config.yaml")
                                .clicked()
                        {
                            load = true;
                        }
                    });
                });
            });
        if load {
            self.load_pending_disk_config();
        } else if dismiss {
            self.banner = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use par_term_config::Profile;

    #[test]
    fn failed_config_save_restores_the_baseline_and_shows_an_error() {
        let mut settings = SettingsUI::new_for_tests(Config::default());
        let original = settings.config.window.window_opacity;
        settings.config.window.window_opacity = 0.3;
        settings.request_save().expect("Save hands over the config");
        assert!(!settings.has_unsaved_changes());

        settings.report_config_save(Err("disk full".to_string()));

        assert_eq!(settings.baseline_config().window.window_opacity, original);
        assert!(settings.has_unsaved_changes(), "the edit is unsaved again");
        let banner = settings.banner().expect("banner shown");
        assert_eq!(banner.kind, BannerKind::Error);
        assert!(banner.text.contains("disk full"));
    }

    #[test]
    fn failed_save_during_close_keeps_the_window_open() {
        let mut settings = SettingsUI::new_for_tests(Config::default());
        settings.config.window.window_opacity = 0.3;
        assert!(!settings.request_close());
        settings.resolve_close_prompt(crate::settings_ui::ClosePromptChoice::Save);

        settings.report_config_save(Err("read-only".to_string()));

        assert!(!settings.take_close_ready());
    }

    #[test]
    fn failed_profile_save_marks_profiles_unsaved_again() {
        let mut settings = SettingsUI::new_for_tests(Config::default());
        settings.sync_profiles(vec![Profile::new("a")]);
        settings
            .profile_modal_ui
            .add_profile_for_test(Profile::new("b"));
        settings.request_save();
        settings.take_profile_save_request().expect("queued");
        assert!(!settings.has_unsaved_changes());

        settings.report_profile_save(Err("permission denied".to_string()));

        assert!(settings.has_unsaved_changes());
        assert_eq!(settings.banner().map(|b| b.kind), Some(BannerKind::Error));
    }

    #[test]
    fn disk_change_without_edits_moves_the_baseline() {
        let mut settings = SettingsUI::new_for_tests(Config::default());
        let on_disk = Config {
            font_size: 31.0,
            ..Config::default()
        };

        settings.config_changed_on_disk(on_disk);

        assert_eq!(settings.config.font_size, 31.0);
        assert_eq!(settings.baseline_config().font_size, 31.0);
        assert!(!settings.has_unsaved_changes());
    }

    #[test]
    fn disk_change_with_edits_is_held_until_loaded() {
        let mut settings = SettingsUI::new_for_tests(Config::default());
        settings.config.window.window_opacity = 0.2;
        let on_disk = Config {
            font_size: 31.0,
            ..Config::default()
        };

        settings.config_changed_on_disk(on_disk);
        assert_eq!(settings.config.window.window_opacity, 0.2, "edits kept");
        assert_ne!(settings.config.font_size, 31.0);
        assert_eq!(settings.banner().map(|b| b.kind), Some(BannerKind::Info));

        settings.load_pending_disk_config();
        assert_eq!(settings.config.font_size, 31.0);
        assert!(!settings.has_unsaved_changes());
    }

    #[test]
    fn successful_save_shows_no_error() {
        let mut settings = SettingsUI::new_for_tests(Config::default());
        settings.config.window.window_opacity = 0.3;
        settings.request_save();
        settings.report_config_save(Ok(()));
        assert!(
            settings
                .banner()
                .is_none_or(|b| b.kind != BannerKind::Error)
        );
    }
}
