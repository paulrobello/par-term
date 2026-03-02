//! Status bar settings tab.
//!
//! Contains:
//! - [`general`]: Enable/disable, position, height
//! - [`styling`]: Colors, font size, separator
//! - [`auto_hide`]: Fullscreen and mouse-inactivity auto-hide
//! - [`widget_options`]: Time format, git status display
//! - [`poll_intervals`]: System monitor and git branch poll rates
//! - [`widgets`]: Three-column widget layout with toggle/reorder/move controls

mod auto_hide;
mod general;
mod poll_intervals;
mod styling;
mod widget_options;
mod widgets;

use super::SettingsUI;
use super::section::section_matches;
use std::collections::HashSet;

/// Show the status bar tab content.
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    let query = settings.search_query.trim().to_lowercase();

    // General section
    if section_matches(
        &query,
        "General",
        &["enable", "status bar", "position", "height"],
    ) {
        general::show_general_section(ui, settings, changes_this_frame, collapsed);
    }

    // Styling section
    if section_matches(
        &query,
        "Styling",
        &[
            "color",
            "background",
            "foreground",
            "font",
            "separator",
            "opacity",
        ],
    ) {
        styling::show_styling_section(ui, settings, changes_this_frame, collapsed);
    }

    // Auto-Hide section
    if section_matches(
        &query,
        "Auto-Hide",
        &["auto hide", "fullscreen", "mouse", "inactivity", "timeout"],
    ) {
        auto_hide::show_auto_hide_section(ui, settings, changes_this_frame, collapsed);
    }

    // Widget Options section
    if section_matches(
        &query,
        "Widget Options",
        &[
            "time", "format", "clock", "git", "ahead", "behind", "dirty", "status",
        ],
    ) {
        widget_options::show_widget_options_section(ui, settings, changes_this_frame, collapsed);
    }

    // Poll Intervals section
    if section_matches(
        &query,
        "Poll Intervals",
        &["poll", "interval", "system", "git", "refresh"],
    ) {
        poll_intervals::show_poll_intervals_section(ui, settings, changes_this_frame, collapsed);
    }

    // Widgets section
    if section_matches(
        &query,
        "Widgets",
        &[
            "widget",
            "clock",
            "cpu",
            "memory",
            "network",
            "git",
            "bell",
            "command",
            "directory",
            "hostname",
            "left",
            "center",
            "right",
            "custom",
            "update",
        ],
    ) {
        widgets::show_widgets_section(ui, settings, changes_this_frame, collapsed);
    }
}

/// Search keywords for the Status Bar settings tab.
pub fn keywords() -> &'static [&'static str] {
    &[
        "status",
        "status bar",
        "widget",
        "widgets",
        "cpu",
        "memory",
        "network",
        "git branch",
        "git status",
        "ahead",
        "behind",
        "dirty",
        "clock",
        "time",
        "time format",
        "hostname",
        "username",
        "auto hide",
        "poll interval",
        "separator",
        "bell indicator",
        "current command",
        "directory",
        "section",
        "left",
        "center",
        "right",
        // Position and size
        "position",
        "height",
        // Styling
        "background",
        "background color",
        "background opacity",
        "text color",
        "foreground",
        "font size",
        // Auto-hide extras
        "fullscreen",
        "inactivity",
        "inactivity timeout",
        // Custom widgets
        "custom text",
        "custom widget",
        // Time format
        "strftime",
    ]
}
