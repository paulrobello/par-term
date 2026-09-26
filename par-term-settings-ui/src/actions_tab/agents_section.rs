//! Agents (Launcher) section: edit the top-level `agents:` config list
//! that feeds the agent-launcher palette — the "Launch <name>" rows, the
//! labelled "(autonomous)" variants, and "Launch Default Agent".
//!
//! Edits go straight into `settings.config.agents` and mark
//! `has_changes`, so persistence rides the settings window's normal
//! Save path — the same shape as the custom-actions list.

use super::ActionsTabState;
use crate::SettingsUI;
use par_term_config::agent_launcher::AgentLaunchConfig;

/// Show the Agents (Launcher) section.
pub fn show_agents_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut std::collections::HashSet<String>,
) {
    crate::section::collapsing_section(ui, "Agents", "agents_list", true, collapsed, |ui| {
        let mut changed = false;
        show_agents_body(
            ui,
            &mut settings.actions_tab,
            &mut settings.config.agents,
            &mut changed,
        );
        if changed {
            settings.has_changes = true;
            *changes_this_frame = true;
        }
    });
}

/// The section body, against an explicit agents list (tests pass a plain
/// `Vec` rather than a `SettingsUI`).
fn show_agents_body(
    ui: &mut egui::Ui,
    state: &mut ActionsTabState,
    agents: &mut Vec<AgentLaunchConfig>,
    changes_this_frame: &mut bool,
) {
    ui.label(
        "Launchable CLI agents — each gets a \"Launch <name>\" palette row; \
         autonomy_args arms a labelled \"(autonomous)\" row. Bind \
         launch-agent:<id> to launch from a keybinding.",
    );
    ui.add_space(4.0);
    if let Some(err) = &state.agent_launch_error {
        ui.colored_label(egui::Color32::from_rgb(255, 100, 100), err);
    }

    if state.agent_launch_adding || state.agent_launch_editing.is_some() {
        show_agent_form(ui, state, agents, changes_this_frame);
        return;
    }

    if agents.is_empty() {
        ui.label(egui::RichText::new("No agents configured.").italics());
    }

    // Clicks are collected during the row loop and applied after it —
    // acting mid-loop would mutate `agents` while it is being iterated.
    let mut edit_clicked: Option<usize> = None;
    let mut remove_clicked: Option<String> = None;
    for (i, agent) in agents.iter().enumerate() {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("{} ({})", agent.name, agent.id)).strong());
            if agent.default {
                ui.label("[default]");
            }
            ui.label(agent.command.clone());
            if !agent.autonomy_args.is_empty() {
                ui.label(format!("+ {}", agent.autonomy_args));
            }
            if ui.button("Edit").clicked() {
                edit_clicked = Some(i);
            }
            let pending = state.agent_launch_pending_delete.as_deref() == Some(agent.id.as_str());
            let remove_label = if pending { "Confirm Remove" } else { "Remove" };
            if ui.button(remove_label).clicked() {
                if pending {
                    remove_clicked = Some(agent.id.clone());
                } else {
                    state.agent_launch_pending_delete = Some(agent.id.clone());
                }
            }
            if pending && ui.button("Keep").clicked() {
                state.agent_launch_pending_delete = None;
            }
        });
    }
    if let Some(i) = edit_clicked {
        state.agent_launch_editing = Some(i);
        state.agent_launch_adding = false;
        let agent = &agents[i];
        state.temp_agent_launch_id = agent.id.clone();
        state.temp_agent_launch_name = agent.name.clone();
        state.temp_agent_launch_command = agent.command.clone();
        state.temp_agent_launch_autonomy_args = agent.autonomy_args.clone();
        state.temp_agent_launch_default = agent.default;
        state.agent_launch_error = None;
    }
    if let Some(id) = remove_clicked {
        confirm_agent_removal(state, agents, &id, changes_this_frame);
    }

    ui.add_space(4.0);
    if ui.button("Add Agent").clicked() {
        state.agent_launch_adding = true;
        state.agent_launch_editing = None;
        state.temp_agent_launch_id = String::new();
        state.temp_agent_launch_name = String::new();
        state.temp_agent_launch_command = String::new();
        state.temp_agent_launch_autonomy_args = String::new();
        state.temp_agent_launch_default = false;
        state.agent_launch_error = None;
    }
}

/// The add/edit form over the temp fields on `ActionsTabState`.
fn show_agent_form(
    ui: &mut egui::Ui,
    state: &mut ActionsTabState,
    agents: &mut Vec<AgentLaunchConfig>,
    changes_this_frame: &mut bool,
) {
    let adding = state.agent_launch_adding;
    ui.label(egui::RichText::new(if adding { "Add Agent" } else { "Edit Agent" }).strong());
    egui::Grid::new("agent_launch_form")
        .num_columns(2)
        .spacing([10.0, 4.0])
        .show(ui, |ui| {
            ui.label("Id:");
            ui.text_edit_singleline(&mut state.temp_agent_launch_id);
            ui.end_row();

            ui.label("Name:");
            ui.text_edit_singleline(&mut state.temp_agent_launch_name);
            ui.end_row();

            ui.label("Command:");
            ui.text_edit_singleline(&mut state.temp_agent_launch_command);
            ui.end_row();

            ui.label("Autonomy args:");
            ui.text_edit_singleline(&mut state.temp_agent_launch_autonomy_args);
            ui.end_row();

            ui.label("Default:");
            ui.checkbox(&mut state.temp_agent_launch_default, "");
            ui.end_row();
        });
    ui.label(
        "Multiple default entries are legal — the first wins. Non-empty \
         autonomy_args adds the \"(autonomous)\" palette row.",
    );
    let (mut save, mut cancel) = (false, false);
    ui.horizontal(|ui| {
        save = ui.button("Save").clicked();
        cancel = ui.button("Cancel").clicked();
    });
    if save {
        save_agent(state, agents, changes_this_frame);
    } else if cancel {
        state.agent_launch_adding = false;
        state.agent_launch_editing = None;
        state.agent_launch_error = None;
    }
}

/// Remove the agent whose id is awaiting its confirm click. An id that no
/// longer exists (already removed through another path) is a no-op.
fn confirm_agent_removal(
    state: &mut ActionsTabState,
    agents: &mut Vec<AgentLaunchConfig>,
    id: &str,
    changes_this_frame: &mut bool,
) {
    let before = agents.len();
    agents.retain(|agent| agent.id != id);
    state.agent_launch_pending_delete = None;
    state.agent_launch_error = None;
    if agents.len() != before {
        *changes_this_frame = true;
    }
}

/// Validate the form and write it into the list (push on add, replace on
/// edit). Invalid input keeps the form open with `agent_launch_error` set.
fn save_agent(
    state: &mut ActionsTabState,
    agents: &mut Vec<AgentLaunchConfig>,
    changes_this_frame: &mut bool,
) {
    let id = state.temp_agent_launch_id.trim().to_string();
    let name = state.temp_agent_launch_name.trim().to_string();
    let command = state.temp_agent_launch_command.trim().to_string();
    if id.is_empty() || name.is_empty() || command.is_empty() {
        state.agent_launch_error = Some("Id, name, and command are all required.".to_string());
        return;
    }
    let duplicate = agents
        .iter()
        .enumerate()
        .any(|(i, agent)| agent.id == id && state.agent_launch_editing != Some(i));
    if duplicate {
        state.agent_launch_error = Some(format!(
            "Id '{id}' is already used by another agent — action names like \
             launch-agent:{id} must stay unique."
        ));
        return;
    }
    let entry = AgentLaunchConfig {
        id,
        name,
        command,
        autonomy_args: state.temp_agent_launch_autonomy_args.trim().to_string(),
        default: state.temp_agent_launch_default,
    };
    if state.agent_launch_adding {
        agents.push(entry);
        state.agent_launch_adding = false;
    } else if let Some(i) = state.agent_launch_editing {
        agents[i] = entry;
        state.agent_launch_editing = None;
    }
    state.agent_launch_error = None;
    state.agent_launch_pending_delete = None;
    *changes_this_frame = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(state: &mut ActionsTabState, agents: &mut Vec<AgentLaunchConfig>) -> bool {
        let ctx = egui::Context::default();
        let mut changed = false;
        let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default()
                .show(ctx, |ui| show_agents_body(ui, state, agents, &mut changed));
        });
        output.textures_delta.clear();
        changed
    }

    fn agent(id: &str, default: bool) -> AgentLaunchConfig {
        AgentLaunchConfig {
            id: id.to_string(),
            name: format!("{id}-name"),
            command: format!("{id} --flag"),
            autonomy_args: if default {
                "--auto".to_string()
            } else {
                String::new()
            },
            default,
        }
    }

    #[test]
    fn renders_rows_and_keeps_list_untouched() {
        let mut state = ActionsTabState::default();
        let mut agents = vec![agent("claude", true), agent("codex", false)];
        assert!(!render(&mut state, &mut agents));
        assert_eq!(
            state.agent_launch_pending_delete, None,
            "no delete is pending without clicks"
        );
        assert_eq!(agents.len(), 2);

        // The edit form renders against the same list without touching it.
        state.agent_launch_editing = Some(0);
        render(&mut state, &mut agents);
        assert_eq!(agents[0].id, "claude", "rendering the form must not save");
    }

    #[test]
    fn save_rejects_missing_fields_and_duplicate_id() {
        let mut state = ActionsTabState::default();
        let mut agents = vec![agent("claude", false)];

        state.agent_launch_adding = true;
        state.temp_agent_launch_id = "codex".to_string();
        state.temp_agent_launch_name = String::new();
        state.temp_agent_launch_command = "codex".to_string();
        save_agent(&mut state, &mut agents, &mut bool::default());
        assert!(state.agent_launch_error.is_some(), "empty name rejected");
        assert_eq!(agents.len(), 1);

        state.temp_agent_launch_name = "Codex".to_string();
        state.temp_agent_launch_id = "claude".to_string();
        save_agent(&mut state, &mut agents, &mut bool::default());
        let err = state.agent_launch_error.as_deref().unwrap();
        assert!(err.contains("already used"), "{err}");
        assert_eq!(agents.len(), 1);
        assert!(
            state.agent_launch_adding,
            "invalid save keeps the form open"
        );
    }

    #[test]
    fn save_add_pushes_and_save_edit_replaces() {
        let mut changed = false;
        let mut state = ActionsTabState::default();
        let mut agents = vec![agent("claude", false)];

        state.agent_launch_adding = true;
        state.temp_agent_launch_id = "codex".to_string();
        state.temp_agent_launch_name = "Codex".to_string();
        state.temp_agent_launch_command = "codex".to_string();
        state.temp_agent_launch_autonomy_args = " --yolo ".to_string();
        state.temp_agent_launch_default = true;
        save_agent(&mut state, &mut agents, &mut changed);
        assert!(changed, "a save must mark changes");
        assert!(!state.agent_launch_adding);
        assert_eq!(agents.len(), 2);
        assert_eq!(agents[1].autonomy_args, "--yolo", "fields are trimmed");
        assert!(agents[1].default);

        changed = false;
        state.agent_launch_editing = Some(0);
        state.temp_agent_launch_id = "claude".to_string();
        state.temp_agent_launch_name = "Claude".to_string();
        state.temp_agent_launch_command = "claude".to_string();
        state.temp_agent_launch_autonomy_args = String::new();
        state.temp_agent_launch_default = false;
        save_agent(&mut state, &mut agents, &mut changed);
        assert!(changed);
        assert_eq!(agents.len(), 2, "edit replaces in place");
        assert_eq!(agents[0].name, "Claude");
        assert_eq!(agents[0].autonomy_args, "");
    }

    #[test]
    fn confirm_remove_deletes_only_the_named_agent() {
        let mut changed = false;
        let mut state = ActionsTabState::default();
        let mut agents = vec![agent("claude", true), agent("codex", false)];

        state.agent_launch_pending_delete = Some("codex".to_string());
        confirm_agent_removal(&mut state, &mut agents, "codex", &mut changed);
        assert!(changed, "a removal must mark changes");
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].id, "claude");
        assert_eq!(state.agent_launch_pending_delete, None);

        // An id that vanished through another path is a no-op.
        let mut changed_again = false;
        confirm_agent_removal(&mut state, &mut agents, "ghost", &mut changed_again);
        assert!(!changed_again);
        assert_eq!(agents.len(), 1);
    }
}
