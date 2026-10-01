//! SC1 gate: with every parent setting off, each dependent group is still
//! drawn (not hidden) and is disabled.

use std::collections::HashSet;

use par_term_config::Config;

use crate::settings_ui::SettingsUI;
use crate::sidebar::SettingsTab;

/// The UX.md SC1 spot-test list: `(tab, parent label)`. Each parent is
/// turned off by [`config_with_parents_off`].
const SPOT_TESTS: &[(SettingsTab, &str)] = &[
    (SettingsTab::Advanced, "Enable status bar"),
    (SettingsTab::Advanced, "Hide on mouse inactivity"),
    (SettingsTab::Appearance, "Enable badge"),
    (SettingsTab::Appearance, "Cursor blink"),
    (SettingsTab::Appearance, "Cursor guide (horizontal line)"),
    (SettingsTab::Appearance, "Cursor shadow"),
    (SettingsTab::Appearance, "Cursor boost (glow)"),
    (SettingsTab::Advanced, "Visual bell"),
    (SettingsTab::Advanced, "Notify on activity after inactivity"),
    (SettingsTab::Advanced, "Notify after prolonged silence"),
    (SettingsTab::Advanced, "Send code when idle"),
    (SettingsTab::Panes, "Show focus indicator"),
    (SettingsTab::Panes, "Dim inactive panes"),
    (SettingsTab::Panes, "Show pane titles"),
    (SettingsTab::WindowsAndTabs, "Dim inactive tabs"),
    (SettingsTab::Pointer, "Enable smart selection"),
    (SettingsTab::General, "Underline highlighted links"),
    (
        SettingsTab::General,
        "Confirm before closing tabs with running jobs",
    ),
    (SettingsTab::General, "Mode: Custom"),
    (SettingsTab::Sessions, "Enable tmux integration"),
    (SettingsTab::Sessions, "Auto-attach on startup"),
    (SettingsTab::Sessions, "Show tmux status bar"),
    (SettingsTab::Effects, "Shader: a background shader selected"),
    (SettingsTab::Appearance, "Shader: a cursor shader selected"),
    (SettingsTab::Advanced, "Bell"),
    (SettingsTab::Advanced, "Command Complete"),
    (SettingsTab::WindowsAndTabs, "Tab style: Automatic"),
    (SettingsTab::WindowsAndTabs, "Position: Left"),
    (SettingsTab::Keys, "Leader chord: a chord recorded"),
];

fn config_with_parents_off() -> Config {
    let mut c = Config::default();
    c.status_bar.status_bar_enabled = false;
    c.status_bar.status_bar_auto_hide_mouse_inactive = false;
    c.badge.badge_enabled = false;
    c.cursor.cursor_blink = false;
    c.cursor.cursor_guide_enabled = false;
    c.cursor.cursor_shadow_enabled = false;
    c.cursor.cursor_boost = 0.0;
    c.notifications.notification_bell_visual = false;
    c.notifications.notification_activity_enabled = false;
    c.notifications.notification_silence_enabled = false;
    c.notifications.anti_idle_enabled = false;
    c.panes.pane_focus_indicator = false;
    c.panes.dim_inactive_panes = false;
    c.panes.show_pane_titles = false;
    c.tab_colors.dim_inactive_tabs = false;
    c.word_selection.smart_selection_enabled = false;
    c.semantic_history.link_highlight_underline = false;
    c.shell.confirm_close_running_jobs = false;
    c.shell.startup_directory_mode = par_term_config::StartupDirectoryMode::Home;
    c.tmux.tmux_enabled = false;
    c.tmux.tmux_auto_attach = false;
    c.tmux.tmux_show_status_bar = false;
    c.shader.custom_shader = None;
    c.shader.cursor_shader = None;
    c.notifications.alert_sounds.clear();
    c.tabs.tab_style = par_term_config::TabStyle::Dark;
    c.tabs.tab_bar_position = par_term_config::TabBarPosition::Top;
    c.input.leader_key = String::new();
    c.collapsed_settings_sections = Vec::new();
    c
}

/// Every section that starts collapsed, marked as toggled so it renders open.
fn expand_every_section(settings: &mut SettingsUI) {
    let ids: HashSet<String> = [
        "appearance_cursor_effects",
        "badge_position",
        "notifications_anti_idle",
        "status_bar_auto_hide",
        "status_bar_poll_intervals",
        "window_pane_appearance",
        "window_tab_bar_appearance",
        "input_word_selection",
        "notifications_alert_sounds",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    settings.collapsed_sections = ids;
}

/// Dependent groups drawn on every page of `tab`.
fn drawn_on(tab: SettingsTab, settings: &mut SettingsUI) -> Vec<(String, bool)> {
    let mut drawn = Vec::new();
    for page in 0..crate::layout::pages(tab).len() {
        settings.select_page(tab, page);
        crate::dependent::take_drawn();
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                settings.show_as_panel(ui);
            });
        });
        output.textures_delta.clear();
        drawn.extend(crate::dependent::take_drawn());
    }
    drawn
}

#[test]
fn every_dependent_is_drawn_disabled_when_its_parent_is_off() {
    let mut settings = SettingsUI::new_for_tests(config_with_parents_off());
    expand_every_section(&mut settings);

    let mut missing = Vec::new();
    let mut enabled = Vec::new();
    for (tab, parent) in SPOT_TESTS {
        let drawn = drawn_on(*tab, &mut settings);
        let groups: Vec<&(String, bool)> = drawn.iter().filter(|(p, _)| p == parent).collect();
        if groups.is_empty() {
            missing.push(format!("{tab:?} › {parent}"));
        } else if groups.iter().any(|(_, on)| *on) {
            enabled.push(format!("{tab:?} › {parent}"));
        }
    }
    assert!(
        missing.is_empty(),
        "hidden instead of disabled: {missing:?}"
    );
    assert!(
        enabled.is_empty(),
        "still enabled with parent off: {enabled:?}"
    );
}

#[test]
fn dependents_are_enabled_when_their_parent_is_on() {
    let mut config = Config::default();
    config.status_bar.status_bar_enabled = true;
    config.badge.badge_enabled = true;
    let mut settings = SettingsUI::new_for_tests(config);
    expand_every_section(&mut settings);

    for (tab, parent) in [
        (SettingsTab::Advanced, "Enable status bar"),
        (SettingsTab::Appearance, "Enable badge"),
    ] {
        let drawn = drawn_on(tab, &mut settings);
        assert!(
            drawn.iter().any(|(p, on)| p == parent && *on),
            "{parent} group should be enabled"
        );
    }
}
