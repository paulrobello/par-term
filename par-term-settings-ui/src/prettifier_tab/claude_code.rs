//! Claude Code integration section for the Content Prettifier tab.

use crate::SettingsUI;
use crate::section::collapsing_section;
use std::collections::HashSet;

pub fn show_claude_code_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Claude Code Integration",
        "prettifier_claude_code",
        true,
        collapsed,
        |ui| {
            let cc = &mut settings.config.content_prettifier.claude_code_integration;

            if ui
                .checkbox(&mut cc.auto_detect, "Auto-detect Claude Code sessions")
                .on_hover_text("Automatically detect when Claude Code is running")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if ui
                .checkbox(&mut cc.render_markdown, "Render Markdown")
                .on_hover_text("Render markdown content in Claude Code output")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if ui
                .checkbox(&mut cc.render_diffs, "Render Diffs")
                .on_hover_text("Render diff content in Claude Code output")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if ui
                .checkbox(
                    &mut cc.auto_render_on_expand,
                    "Auto-render on expand (Ctrl+O)",
                )
                .on_hover_text("Automatically render content when a collapsed block is expanded")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if ui
                .checkbox(&mut cc.show_format_badges, "Show format badges")
                .on_hover_text("Show format badges (e.g., 'MD', 'JSON') on collapsed blocks")
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        },
    );
}
