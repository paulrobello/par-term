//! SC1 gate: with every parent setting off, each dependent group is still
//! drawn (not hidden) and is disabled.

use std::collections::HashSet;

use par_term_config::Config;

use crate::settings_ui::SettingsUI;
use crate::sidebar::SettingsTab;

/// The UX.md SC1 spot-test list: `(tab, parent label)`. Each parent is
/// turned off by [`config_with_parents_off`].
const SPOT_TESTS: &[(SettingsTab, &str)] = &[
    (SettingsTab::StatusBar, "Enable status bar"),
    (SettingsTab::StatusBar, "Hide on mouse inactivity"),
    (SettingsTab::Appearance, "Enable badge"),
    (SettingsTab::Appearance, "Cursor blink"),
    (SettingsTab::Appearance, "Cursor guide (horizontal line)"),
    (SettingsTab::Appearance, "Cursor shadow"),
    (SettingsTab::Appearance, "Cursor boost (glow)"),
    (SettingsTab::Notifications, "Visual bell"),
    (
        SettingsTab::Notifications,
        "Notify on activity after inactivity",
    ),
    (SettingsTab::Notifications, "Notify after prolonged silence"),
    (SettingsTab::Notifications, "Send code when idle"),
    (SettingsTab::Window, "Show focus indicator"),
    (SettingsTab::Window, "Dim inactive panes"),
    (SettingsTab::Window, "Show pane titles"),
    (SettingsTab::Window, "Dim inactive tabs"),
    (SettingsTab::Input, "Enable smart selection"),
    (SettingsTab::Terminal, "Underline highlighted links"),
    (
        SettingsTab::Terminal,
        "Confirm before closing tabs with running jobs",
    ),
    (SettingsTab::Terminal, "Mode: Custom"),
    (SettingsTab::Advanced, "Enable tmux integration"),
    (SettingsTab::Advanced, "Auto-attach on startup"),
    (SettingsTab::Advanced, "Show tmux status bar"),
    (SettingsTab::Effects, "Shader: a background shader selected"),
    (SettingsTab::Effects, "Shader: a cursor shader selected"),
    (SettingsTab::Notifications, "Bell"),
    (SettingsTab::Notifications, "Command Complete"),
    (SettingsTab::Window, "Tab style: Automatic"),
    (SettingsTab::Window, "Position: Left"),
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

fn drawn_on(tab: SettingsTab, settings: &mut SettingsUI) -> Vec<(String, bool)> {
    settings.selected_tab = tab;
    crate::dependent::take_drawn();
    let ctx = egui::Context::default();
    let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            settings.show_as_panel(ui);
        });
    });
    output.textures_delta.clear();
    crate::dependent::take_drawn()
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
        (SettingsTab::StatusBar, "Enable status bar"),
        (SettingsTab::Appearance, "Enable badge"),
    ] {
        let drawn = drawn_on(tab, &mut settings);
        assert!(
            drawn.iter().any(|(p, on)| p == parent && *on),
            "{parent} group should be enabled"
        );
    }
}
