//! Agent Usage settings section (subsystem enable, update command, refresh
//! interval, hidden agents).

use crate::SettingsUI;
use crate::section::collapsing_section;
use std::collections::HashSet;

pub fn show_agent_usage_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Agent Usage",
        "status_bar_agent_usage",
        false,
        collapsed,
        |ui| {
            let agent_usage = &mut settings.config.agent_usage;

            // Subsystem enable
            if ui
                .checkbox(
                    &mut agent_usage.agent_usage_enabled,
                    "Enable agent usage panel",
                )
                .on_hover_text(
                    "Watch the usage records directory and show the Agent Usage \
                     status-bar widget and popup panel. The widget self-hides when \
                     no agent has anything to show.",
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            // Update command
            let mut command = agent_usage
                .agent_usage_update_command
                .clone()
                .unwrap_or_default();
            ui.horizontal(|ui| {
                ui.label("Update command:");
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut command)
                            .hint_text("path/to/collector --update (empty = watch only)")
                            .desired_width(240.0),
                    )
                    .on_hover_text(
                        "Optional command run through `sh -c` on the refresh interval and on \
                         manual refresh (panel `r`). It should rewrite the records directory. \
                         Runs off the UI thread, one instance at a time, killed after 60s. \
                         par-term ships no collectors — point this at your own.",
                    )
                    .changed()
                {
                    agent_usage.agent_usage_update_command =
                        (!command.trim().is_empty()).then(|| command.clone());
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            // Refresh interval
            let mut interval = agent_usage.agent_usage_refresh_interval_sec as f32;
            ui.horizontal(|ui| {
                ui.label("Refresh interval (s):");
                if ui
                    .add(egui::Slider::new(&mut interval, 30.0..=3600.0))
                    .on_hover_text(
                        "How often the records directory is rescanned when no file change \
                         arrives, and how often the update command runs when configured. \
                         Clamped to at least 30s.",
                    )
                    .changed()
                {
                    agent_usage.agent_usage_refresh_interval_sec = interval.round() as u64;
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            // Hidden agents
            let mut hidden = agent_usage.agent_usage_hidden_agents.join(", ");
            ui.horizontal(|ui| {
                ui.label("Hidden agents:");
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut hidden)
                            .hint_text("claude, codex (comma-separated agent ids)")
                            .desired_width(240.0),
                    )
                    .on_hover_text(
                        "Agent ids whose records should not render, matching the record \
                         filename stem (e.g. `claude` for claude.json).",
                    )
                    .changed()
                {
                    agent_usage.agent_usage_hidden_agents = hidden
                        .split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string)
                        .collect();
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            // Extra records directories (v2 merge: synced machines)
            let mut extra_dirs = agent_usage.agent_usage_extra_records_dirs.join(", ");
            ui.horizontal(|ui| {
                ui.label("Extra records dirs:");
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut extra_dirs)
                            .hint_text("~/Sync/agent-usage (comma-separated paths)")
                            .desired_width(240.0),
                    )
                    .on_hover_text(
                        "Additional records directories merged into the panel, e.g. a \
                         synced folder holding other machines' records. Records for the \
                         same agent merge by widest value — active days union by date, \
                         never summed — so one account synced from two machines is not \
                         double-counted. A leading ~/ expands to your home directory.",
                    )
                    .changed()
                {
                    agent_usage.agent_usage_extra_records_dirs = extra_dirs
                        .split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string)
                        .collect();
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });
        },
    );
}
