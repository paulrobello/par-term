//! Window settings tab.
//!
//! Consolidates: window_tab (original), tab_bar_tab, panes_tab
//!
//! Contains:
//! - Display settings (title, dimensions, padding)
//! - Transparency settings (opacity, blur)
//! - Performance settings (FPS, VSync, power saving)
//! - Window behavior (decorations, always on top, etc.)
//! - Tab bar settings
//! - Split panes settings

use crate::SettingsUI;
use crate::section::section_matches;
use std::collections::HashSet;

mod behavior;
mod display;
mod panes;
mod performance;
mod tab_bar;
mod transparency;

/// Show the window tab content.
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    let query = settings.search_query.trim().to_lowercase();

    // Display section
    if section_matches(
        &query,
        "Display",
        &[
            "title",
            "columns",
            "rows",
            "padding",
            "size",
            "window title",
            "allow title change",
        ],
    ) {
        display::show_display_section(ui, settings, changes_this_frame, collapsed);
    }

    // Transparency section
    if section_matches(
        &query,
        "Transparency",
        &[
            "opacity",
            "blur",
            "transparent",
            "background",
            "default background",
            "text opaque",
        ],
    ) {
        transparency::show_transparency_section(ui, settings, changes_this_frame, collapsed);
    }

    // Performance section (collapsed by default)
    if section_matches(
        &query,
        "Performance",
        &[
            "fps",
            "vsync",
            "refresh",
            "power",
            "unfocused",
            "gpu",
            "flicker",
            "reduce",
            "throughput",
            "render interval",
            "batch",
            "mailbox",
            "fifo",
            "gpu preference",
            "power saving",
        ],
    ) {
        performance::show_performance_section(ui, settings, changes_this_frame, collapsed);
    }

    // Window Behavior section (collapsed by default)
    if section_matches(
        &query,
        "Window Behavior",
        &[
            "decorations",
            "always on top",
            "window type",
            "monitor",
            "lock",
            "edge-anchored",
            "primary monitor",
            "window number",
        ],
    ) {
        behavior::show_behavior_section(ui, settings, changes_this_frame, collapsed);
    }

    // Tab Bar section
    if section_matches(
        &query,
        "Tab Bar",
        &[
            "tab",
            "tabs",
            "bar",
            "index",
            "close button",
            "profile drawer",
            "stretch",
            "html titles",
            "inherit directory",
            "max tabs",
        ],
    ) {
        tab_bar::show_tab_bar_section(ui, settings, changes_this_frame, collapsed);
    }

    // Tab Bar Appearance section (collapsed by default)
    if section_matches(
        &query,
        "Tab Bar Appearance",
        &[
            "tab color",
            "tab border",
            "inactive tab",
            "dimming",
            "tab style",
            "minimum tab width",
            "active indicator",
            "activity indicator",
            "bell indicator",
        ],
    ) {
        tab_bar::show_tab_bar_appearance_section(ui, settings, changes_this_frame, collapsed);
    }

    // Split Panes section
    if section_matches(
        &query,
        "Split Panes",
        &[
            "pane",
            "split",
            "divider",
            "focus indicator",
            "hit width",
            "drag area",
            "max panes",
            "min pane size",
            "pane padding",
        ],
    ) {
        panes::show_panes_section(ui, settings, changes_this_frame, collapsed);
    }

    // Pane Appearance section (collapsed by default)
    if section_matches(
        &query,
        "Pane Appearance",
        &[
            "pane color",
            "pane title",
            "inactive pane",
            "pane opacity",
            "hover color",
            "dim inactive",
            "title height",
            "title position",
            "pane background",
        ],
    ) {
        panes::show_pane_appearance_section(ui, settings, changes_this_frame, collapsed);
    }
}
