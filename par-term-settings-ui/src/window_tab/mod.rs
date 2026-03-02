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

/// Search keywords for the Window settings tab.
pub fn keywords() -> &'static [&'static str] {
    &[
        // Display
        "window",
        "title",
        "size",
        "columns",
        "rows",
        "padding",
        "hide padding on split",
        "allow title change",
        // Transparency
        "opacity",
        "transparency",
        "transparent",
        "blur",
        "blur radius",
        "keep text opaque",
        // Performance
        "fps",
        "max fps",
        "vsync",
        "refresh",
        "power",
        "gpu",
        "unfocused",
        "inactive tab",
        "inactive tab fps",
        "pause shaders",
        "reduce flicker",
        "flicker",
        "maximize throughput",
        "throughput",
        "render interval",
        // Window behavior
        "decorations",
        "always on top",
        "lock window size",
        "window number",
        "window type",
        "monitor",
        "target monitor",
        "space",
        "spaces",
        "mission control",
        "virtual desktop",
        "macos space",
        "target space",
        // Tab bar
        "tab bar",
        "tabs",
        "tab bar mode",
        "tab title mode",
        "tab title",
        "osc only",
        "cwd title",
        "rename tab",
        "tab height",
        "tab index",
        "close button",
        "stretch",
        "html titles",
        "inherit cwd",
        "inherit directory",
        "profile drawer",
        "new tab shortcut",
        "profile picker",
        "new tab profile",
        "max tabs",
        // Tab bar appearance
        "tab min width",
        "tab border",
        "tab color",
        "inactive tab",
        "outline only",
        "outline tab",
        "dimming",
        "dim inactive",
        "tab background",
        "tab text",
        "tab indicator",
        "activity indicator",
        "bell indicator",
        "close button color",
        "tab style",
        "auto tab style",
        "automatic tab",
        "system tab style",
        // Tab bar layout
        "tab bar position",
        "tab bar width",
        // Split panes
        "panes",
        "split",
        "divider",
        "divider width",
        "hit width",
        "pane padding",
        "divider style",
        "focus indicator",
        "focus indicator color",
        "focus indicator width",
        "pane focus",
        "max panes",
        "min pane size",
        // Pane appearance
        "divider color",
        "hover color",
        "dim inactive panes",
        "inactive pane",
        "pane opacity",
        "pane title",
        "pane title height",
        "pane title position",
        "pane title color",
        "pane background",
        // Performance extras
        "latency",
    ]
}
