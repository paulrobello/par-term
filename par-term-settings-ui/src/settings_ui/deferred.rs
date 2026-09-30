//! Settings that do not apply to open windows (UX.md SS8).
//!
//! Most edits apply live. The few that cannot carry one consistent badge next
//! to their control, and after a Save that changed any of them the banner
//! lists what is still waiting for a restart or a new window.

use par_term_config::Config;

use super::SettingsUI;

/// When a deferred setting takes effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deferred {
    /// Only after par-term is restarted.
    Restart,
    /// Only for windows opened after the change.
    NewWindows,
}

impl Deferred {
    fn badge_text(self) -> &'static str {
        match self {
            Deferred::Restart => "⟳ Restart required",
            Deferred::NewWindows => "⟳ New windows only",
        }
    }

    fn explanation(self) -> &'static str {
        match self {
            Deferred::Restart => "This setting applies after par-term is restarted.",
            Deferred::NewWindows => {
                "This setting applies to windows opened after it changes; \
                 open windows keep their current value."
            }
        }
    }
}

struct DeferredSetting {
    label: &'static str,
    when: Deferred,
    changed: fn(&Config, &Config) -> bool,
}

const DEFERRED_SETTINGS: &[DeferredSetting] = &[
    DeferredSetting {
        label: "GPU power preference",
        when: Deferred::Restart,
        changed: |a, b| a.rendering.power_preference != b.rendering.power_preference,
    },
    DeferredSetting {
        label: "Window type",
        when: Deferred::NewWindows,
        changed: |a, b| a.placement.window_type != b.placement.window_type,
    },
    DeferredSetting {
        label: "Target monitor",
        when: Deferred::NewWindows,
        changed: |a, b| a.placement.target_monitor != b.placement.target_monitor,
    },
    DeferredSetting {
        label: "Target Space",
        when: Deferred::NewWindows,
        changed: |a, b| a.placement.target_space != b.placement.target_space,
    },
];

/// Labels of deferred settings that differ between two configs, split into
/// (needs a restart, needs a new window).
pub(crate) fn deferred_changes(
    before: &Config,
    after: &Config,
) -> (Vec<&'static str>, Vec<&'static str>) {
    let mut restart = Vec::new();
    let mut new_windows = Vec::new();
    for setting in DEFERRED_SETTINGS {
        if (setting.changed)(before, after) {
            match setting.when {
                Deferred::Restart => restart.push(setting.label),
                Deferred::NewWindows => new_windows.push(setting.label),
            }
        }
    }
    (restart, new_windows)
}

/// The badge shown next to a deferred setting's control.
pub fn deferred_badge(ui: &mut egui::Ui, when: Deferred) {
    ui.label(
        egui::RichText::new(when.badge_text())
            .small()
            .color(egui::Color32::from_rgb(255, 193, 7)),
    )
    .on_hover_text(when.explanation());
}

impl SettingsUI {
    /// After a successful Save, list deferred settings the save changed.
    pub(super) fn announce_restart_needed(&mut self, before: &Config) {
        let (restart, new_windows) = deferred_changes(before, &self.baseline_config);
        let mut parts = Vec::new();
        if !restart.is_empty() {
            parts.push(format!(
                "Restart par-term to apply: {}.",
                restart.join(", ")
            ));
        }
        if !new_windows.is_empty() {
            parts.push(format!(
                "Applies to new windows: {}.",
                new_windows.join(", ")
            ));
        }
        if !parts.is_empty() {
            self.show_info_banner(format!("Saved. {}", parts.join(" ")));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings_ui::BannerKind;

    #[test]
    fn save_that_changes_power_preference_lists_a_restart() {
        let mut settings = SettingsUI::new_for_tests(Config::default());
        settings.config.rendering.power_preference =
            match settings.config.rendering.power_preference {
                par_term_config::PowerPreference::HighPerformance => {
                    par_term_config::PowerPreference::LowPower
                }
                _ => par_term_config::PowerPreference::HighPerformance,
            };
        settings.request_save();
        settings.report_config_save(Ok(()));

        let banner = settings.banner().expect("restart notice");
        assert_eq!(banner.kind, BannerKind::Info);
        assert!(banner.text.contains("Restart par-term"));
        assert!(banner.text.contains("GPU power preference"));
    }

    #[test]
    fn save_of_live_settings_shows_no_restart_notice() {
        let mut settings = SettingsUI::new_for_tests(Config::default());
        settings.config.window.window_opacity = 0.5;
        settings.request_save();
        settings.report_config_save(Ok(()));
        assert!(settings.banner().is_none());
    }
}
