//! Status bar settings tab.
//!
//! Contains:
//! - `general`: Enable/disable, position, height
//! - `styling`: Colors, font size, separator
//! - `auto_hide`: Fullscreen and mouse-inactivity auto-hide
//! - `widget_options`: Time format, git status display
//! - `poll_intervals`: System monitor and git branch poll rates
//! - `widgets`: Three-column widget layout with toggle/reorder/move controls

mod agent_usage;
mod auto_hide;
mod general;
mod poll_intervals;
mod styling;
mod widget_options;
mod widgets;

use super::SettingsUI;
use std::collections::HashSet;

/// Show the status bar tab content.
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    // General section
    general::show_general_section(ui, settings, changes_this_frame, collapsed);

    // Styling section
    styling::show_styling_section(ui, settings, changes_this_frame, collapsed);

    // Auto-Hide section
    auto_hide::show_auto_hide_section(ui, settings, changes_this_frame, collapsed);

    // Widget Options section
    widget_options::show_widget_options_section(ui, settings, changes_this_frame, collapsed);

    // Agent Usage section
    agent_usage::show_agent_usage_section(ui, settings, changes_this_frame, collapsed);

    // Poll Intervals section
    poll_intervals::show_poll_intervals_section(ui, settings, changes_this_frame, collapsed);

    // Widgets section
    widgets::show_widgets_section(ui, settings, changes_this_frame, collapsed);
}
