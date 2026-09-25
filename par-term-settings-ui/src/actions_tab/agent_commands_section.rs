//! Agent Commands section: browse, edit, and delete the per-command YAML files
//! under `<config_dir>/commands/` (docs/features/AGENT_COMMANDS.md).
//!
//! The list is a snapshot read on first display and on Refresh — the running
//! app's directory watcher owns hot-reload for the palette, so this section
//! only needs to be current when the user looks at it. Edit and delete act
//! with the user's authority (`update_command_yaml_as_user`,
//! `delete_command_file_as_user`): unlike the MCP path they touch
//! user-authored files too. An edit keeps the file's provenance (D2).

use super::ActionsTabState;
use crate::SettingsUI;
use par_term_config::agent_commands::{
    CommandAuthor, command_to_yaml, commands_dir, delete_command_file_as_user, load_all_commands,
    update_command_yaml_as_user,
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

    if let Some((id, text)) = &mut state.agent_command_editing {
        let id = id.clone();
        ui.add_space(4.0);
        ui.label(egui::RichText::new(format!("Editing {id}")).strong());
        ui.label("The id and the author fields are kept as they are on disk.");
        ui.add(
            egui::TextEdit::multiline(text)
                .code_editor()
                .desired_rows(10)
                .desired_width(f32::INFINITY),
        );
        let (mut save, mut cancel) = (false, false);
        ui.horizontal(|ui| {
            save = ui.button("Save").clicked();
            cancel = ui.button("Cancel").clicked();
        });
        if save {
            save_edit(state, dir);
        } else if cancel {
            state.agent_command_editing = None;
            state.agent_command_error = None;
        }
        return;
    }

    let commands = state.agent_commands.as_deref().unwrap_or_default();
    if commands.is_empty() {
        ui.label(egui::RichText::new("No commands.").italics());
        return;
    }

    let mut delete_now: Option<String> = None;
    let mut edit_now: Option<(String, String)> = None;
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
                } else {
                    ui.horizontal(|ui| {
                        if ui.button("Edit").clicked() {
                            let text = command_to_yaml(&cmd.file)
                                .map(|b| String::from_utf8_lossy(&b).into_owned())
                                .unwrap_or_default();
                            edit_now = Some((id.to_string(), text));
                        }
                        if ui.button("Delete").clicked() {
                            state.agent_command_pending_delete = Some(id.to_string());
                        }
                    });
                }
                ui.end_row();
            }
        });

    if let Some(id) = delete_now {
        confirm_delete(state, dir, &id);
    } else if edit_now.is_some() {
        state.agent_command_editing = edit_now;
        state.agent_command_error = None;
    }
}

/// Save the open editor: write through the user-authority primitive, then
/// close the editor and reload on success, or keep it open with the error.
fn save_edit(state: &mut ActionsTabState, dir: &std::path::Path) {
    let Some((id, text)) = &state.agent_command_editing else {
        return;
    };
    match update_command_yaml_as_user(id, text, dir) {
        Ok(_) => {
            state.agent_command_editing = None;
            state.agent_command_error = None;
            state.agent_commands = Some(load_all_commands(dir));
        }
        Err(e) => state.agent_command_error = Some(format!("Save failed: {e:#}")),
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

    #[test]
    fn save_edit_writes_through_and_keeps_editor_open_on_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("greet.yaml"),
            "action:\n  type: insert_text\n  id: greet\n  title: Greet\n  text: hi\n",
        )
        .unwrap();
        let bad = "action:\n  type: insert_text\n  id: renamed\n  title: X\n  text: y\n";
        let mut state = ActionsTabState {
            agent_command_editing: Some(("greet".to_string(), bad.to_string())),
            ..ActionsTabState::default()
        };
        render(&mut state, dir.path());

        save_edit(&mut state, dir.path());
        assert!(state.agent_command_editing.is_some(), "editor stays open");
        let err = state.agent_command_error.as_deref().unwrap();
        assert!(err.contains("cannot change"), "{err}");
        render(&mut state, dir.path());

        state.agent_command_editing = Some((
            "greet".to_string(),
            "action:\n  type: insert_text\n  id: greet\n  title: Hello\n  text: hi\n".to_string(),
        ));
        save_edit(&mut state, dir.path());
        assert_eq!(state.agent_command_editing, None);
        assert_eq!(state.agent_command_error, None);
        let titles: Vec<&str> = state
            .agent_commands
            .as_deref()
            .unwrap()
            .iter()
            .map(|c| c.file.title())
            .collect();
        assert_eq!(titles, ["Hello"]);
    }
}
