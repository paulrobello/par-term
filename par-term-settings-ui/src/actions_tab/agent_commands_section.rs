//! Agent Commands section: browse and delete the per-command YAML files under
//! `<config_dir>/commands/` (docs/features/AGENT_COMMANDS.md).
//!
//! The list is a snapshot read on first display and on Refresh — the running
//! app's directory watcher owns hot-reload for the palette, so this section
//! only needs to be current when the user looks at it. Deletion acts with the
//! user's authority (`delete_command_file_as_user`): unlike the MCP path it
//! removes user-authored files too, behind a two-step confirm.

use super::ActionsTabState;
use crate::SettingsUI;
use par_term_config::agent_commands::{
    CommandAuthor, commands_dir, delete_command_file_as_user, load_all_commands,
};

/// Show the Agent Commands section.
pub fn show_agent_commands_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    collapsed: &mut std::collections::HashSet<String>,
) {
    crate::section::collapsing_section(
        ui,
        "Agent Commands",
        "agent_commands_list",
        true,
        collapsed,
        |ui| show_commands_body(ui, &mut settings.actions_tab, &commands_dir()),
    );
}

/// The section body, against an explicit directory (tests point it at a
/// tempdir rather than the real config).
fn show_commands_body(ui: &mut egui::Ui, state: &mut ActionsTabState, dir: &std::path::Path) {
    ui.label(format!(
        "Command files in {} — run them from the command palette \
         (agent-cmd:<id>), a keybinding, or `par-term <id>`.",
        dir.display()
    ));
    ui.add_space(4.0);

    let refresh = ui.button("Refresh").clicked();
    if refresh || state.agent_commands.is_none() {
        state.agent_commands = Some(load_all_commands(dir));
        state.agent_command_pending_delete = None;
    }
    if let Some(err) = &state.agent_command_error {
        ui.colored_label(egui::Color32::from_rgb(255, 100, 100), err);
    }

    let commands = state.agent_commands.as_deref().unwrap_or_default();
    if commands.is_empty() {
        ui.label(egui::RichText::new("No commands.").italics());
        return;
    }

    let mut delete_now: Option<String> = None;
    egui::Grid::new("agent_commands_grid")
        .num_columns(5)
        .striped(true)
        .show(ui, |ui| {
            for header in ["Id", "Title", "Kind", "Author", ""] {
                ui.label(egui::RichText::new(header).strong());
            }
            ui.end_row();
            for cmd in commands {
                let id = cmd.file.id();
                ui.monospace(id);
                ui.label(cmd.file.title());
                ui.label(if cmd.file.is_script() {
                    "script"
                } else {
                    "macro"
                });
                ui.label(match cmd.file.created_by {
                    CommandAuthor::Agent => format!(
                        "agent ({})",
                        cmd.file.source_agent.as_deref().unwrap_or("?")
                    ),
                    CommandAuthor::User => "user".to_string(),
                });
                if state.agent_command_pending_delete.as_deref() == Some(id) {
                    ui.horizontal(|ui| {
                        if ui.button("Confirm delete").clicked() {
                            delete_now = Some(id.to_string());
                        }
                        if ui.button("Cancel").clicked() {
                            state.agent_command_pending_delete = None;
                        }
                    });
                } else if ui.button("Delete").clicked() {
                    state.agent_command_pending_delete = Some(id.to_string());
                }
                ui.end_row();
            }
        });

    if let Some(id) = delete_now {
        confirm_delete(state, dir, &id);
    }
}

/// The second click of a delete: remove the file, record any error, and
/// reload so the list reflects the directory either way.
fn confirm_delete(state: &mut ActionsTabState, dir: &std::path::Path, id: &str) {
    state.agent_command_error = delete_command_file_as_user(id, dir)
        .err()
        .map(|e| format!("Delete failed: {e:#}"));
    state.agent_command_pending_delete = None;
    state.agent_commands = Some(load_all_commands(dir));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(state: &mut ActionsTabState, dir: &std::path::Path) {
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| show_commands_body(ui, state, dir));
        });
        output.textures_delta.clear();
    }

    #[test]
    fn renders_empty_list_and_each_row_state() {
        let dir = tempfile::tempdir().unwrap();
        let mut state = ActionsTabState::default();
        render(&mut state, dir.path());
        assert_eq!(state.agent_commands.as_deref().map(<[_]>::len), Some(0));

        std::fs::write(
            dir.path().join("deploy.yaml"),
            "created_by: agent\nsource_agent: claude-code\naction:\n  type: shell_command\n  \
             id: deploy\n  title: Deploy\n  command: echo\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("greet.yaml"),
            "action:\n  type: insert_text\n  id: greet\n  title: Greet\n  text: hi\n",
        )
        .unwrap();
        state.agent_commands = None;
        render(&mut state, dir.path());
        let ids: Vec<&str> = state
            .agent_commands
            .as_deref()
            .unwrap()
            .iter()
            .map(|c| c.file.id())
            .collect();
        assert_eq!(ids, ["deploy", "greet"]);

        state.agent_command_pending_delete = Some("greet".to_string());
        state.agent_command_error = Some("Delete failed: example".to_string());
        render(&mut state, dir.path());
        assert_eq!(state.agent_command_pending_delete.as_deref(), Some("greet"));
    }

    #[test]
    fn confirm_delete_removes_user_file_and_reports_failure() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("greet.yaml"),
            "action:\n  type: insert_text\n  id: greet\n  title: Greet\n  text: hi\n",
        )
        .unwrap();
        std::fs::write(dir.path().join(".confirmations.json"), r#"{"greet":"h"}"#).unwrap();
        let mut state = ActionsTabState {
            agent_commands: Some(load_all_commands(dir.path())),
            agent_command_pending_delete: Some("greet".to_string()),
            ..ActionsTabState::default()
        };

        confirm_delete(&mut state, dir.path(), "greet");
        assert!(!dir.path().join("greet.yaml").exists());
        assert!(
            !par_term_config::agent_commands::load_confirmation_ledger(dir.path())
                .contains_key("greet")
        );
        assert_eq!(state.agent_command_error, None);
        assert_eq!(state.agent_command_pending_delete, None);
        assert_eq!(state.agent_commands.as_deref().map(<[_]>::len), Some(0));

        confirm_delete(&mut state, dir.path(), "greet");
        let err = state.agent_command_error.as_deref().unwrap();
        assert!(err.contains("no such command"), "{err}");
        assert_eq!(state.agent_commands.as_deref().map(<[_]>::len), Some(0));
    }
}
