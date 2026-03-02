//! Tab bar sections of the window settings tab.
//!
//! Delegates to focused sub-files:
//! - `tab_bar_behavior.rs` — Tab Bar section (mode, position, toggles, sizing)
//! - `tab_bar_appearance.rs` — Tab Bar Appearance section (colors, borders, dimming)

use crate::SettingsUI;
use std::collections::HashSet;

pub(super) fn show_tab_bar_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    super::tab_bar_behavior::show_tab_bar_section(ui, settings, changes_this_frame, collapsed);
}

pub(super) fn show_tab_bar_appearance_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    super::tab_bar_appearance::show_tab_bar_appearance_section(
        ui,
        settings,
        changes_this_frame,
        collapsed,
    );
}
