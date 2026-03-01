//! Search keyword dispatch for settings sidebar.
//!
//! Each settings tab module exposes a `keywords()` function that returns the
//! list of search terms for that tab.  This module collects them into a single
//! dispatch so `sidebar.rs` can stay focused on rendering.

use crate::sidebar::SettingsTab;

/// Return the search keywords for the given settings tab.
///
/// Delegates to each tab module's `keywords()` function so that keyword lists
/// live next to the UI code they describe.
pub fn tab_search_keywords(tab: SettingsTab) -> &'static [&'static str] {
    match tab {
        SettingsTab::Appearance => crate::appearance_tab::keywords(),
        SettingsTab::Window => crate::window_tab::keywords(),
        SettingsTab::Input => crate::input_tab::keywords(),
        SettingsTab::Terminal => crate::terminal_tab::keywords(),
        SettingsTab::Effects => crate::effects_tab::keywords(),
        SettingsTab::Badge => crate::badge_tab::keywords(),
        SettingsTab::ProgressBar => crate::progress_bar_tab::keywords(),
        SettingsTab::StatusBar => crate::status_bar_tab::keywords(),
        SettingsTab::Profiles => crate::profiles_tab::keywords(),
        SettingsTab::Ssh => crate::ssh_tab::keywords(),
        SettingsTab::Notifications => crate::notifications_tab::keywords(),
        SettingsTab::Integrations => crate::integrations_tab::keywords(),
        SettingsTab::Automation => crate::automation_tab::keywords(),
        SettingsTab::Scripts => crate::scripts_tab::keywords(),
        SettingsTab::Snippets => crate::snippets_tab::keywords(),
        SettingsTab::Actions => crate::actions_tab::keywords(),
        SettingsTab::ContentPrettifier => crate::prettifier_tab::keywords(),
        SettingsTab::Arrangements => crate::arrangements_tab::keywords(),
        SettingsTab::AiInspector => crate::ai_inspector_tab::keywords(),
        SettingsTab::Advanced => crate::advanced_tab::keywords(),
    }
}
