//! Window settings tab.
//!
//! Consolidates: window_tab (original), tab_bar_tab, panes_tab, arrangements_tab
//!
//! Contains:
//! - Display settings (title, dimensions, padding)
//! - Transparency settings (opacity, blur)
//! - Performance settings (FPS, VSync, power saving)
//! - Window behavior (decorations, always on top, etc.)
//! - Tab bar settings
//! - Split panes settings
//! - Window arrangements (save and restore layouts)

use crate::SettingsUI;
use std::collections::HashSet;

mod behavior;
mod display;
mod panes;
mod performance;
mod scrollbar;
mod tab_bar;
mod tab_bar_appearance;
mod tab_bar_behavior;
mod transparency;

/// Show the window tab content.
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    // Display section
    display::show_display_section(ui, settings, changes_this_frame, collapsed);

    // Transparency section
    transparency::show_transparency_section(ui, settings, changes_this_frame, collapsed);

    // Performance section (collapsed by default)
    performance::show_performance_section(ui, settings, changes_this_frame, collapsed);

    // Window Behavior section (collapsed by default)
    behavior::show_behavior_section(ui, settings, changes_this_frame, collapsed);

    // Tab Bar section
    tab_bar::show_tab_bar_section(ui, settings, changes_this_frame, collapsed);

    // Tab Bar Appearance section (collapsed by default)
    tab_bar::show_tab_bar_appearance_section(ui, settings, changes_this_frame, collapsed);

    // Split Panes section
    panes::show_panes_section(ui, settings, changes_this_frame, collapsed);

    // Pane Appearance section (collapsed by default)
    panes::show_pane_appearance_section(ui, settings, changes_this_frame, collapsed);

    // Scrollbar section
    scrollbar::show_scrollbar_section(ui, settings, changes_this_frame, collapsed);

    // Arrangements section (absorbed from arrangements_tab)
    crate::arrangements_tab::show(ui, settings, changes_this_frame, collapsed);
}
