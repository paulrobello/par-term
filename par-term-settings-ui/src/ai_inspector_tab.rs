//! AI Inspector settings tab.
//!
//! Contains:
//! - Panel settings (enabled, width, scope, view mode)
//! - Agent settings (default agent, auto-launch, auto-context)
//! - Permission settings (auto-approve / yolo mode)

use super::SettingsUI;
use super::section::{collapsing_section, section_matches};
use par_term_config::CustomAcpAgentConfig;
use std::collections::{HashMap, HashSet};

/// Show the AI Inspector tab content.
pub fn show(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    let query = settings.search_query.trim().to_lowercase();

    // Panel section
    if section_matches(
        &query,
        "Panel",
        &[
            "enabled", "width", "scope", "view", "live", "update", "zones", "cards", "timeline",
            "tree",
        ],
    ) {
        show_panel_section(ui, settings, changes_this_frame, collapsed);
    }

    // Agent section
    if section_matches(
        &query,
        "Agent",
        &[
            "agent",
            "launch",
            "auto-launch",
            "context",
            "auto-context",
            "max lines",
        ],
    ) {
        show_agent_section(ui, settings, changes_this_frame, collapsed);
    }

    // Custom agents section
    if section_matches(
        &query,
        "Custom Agents",
        &[
            "custom",
            "acp",
            "agent",
            "identity",
            "run command",
            "env",
            "environment",
            "install command",
            "connector",
        ],
    ) {
        show_custom_agents_section(ui, settings, changes_this_frame, collapsed);
    }

    // Permissions section
    if section_matches(
        &query,
        "Permissions",
        &[
            "approve",
            "auto-approve",
            "yolo",
            "permission",
            "screenshot",
            "image",
        ],
    ) {
        show_permissions_section(ui, settings, changes_this_frame, collapsed);
    }
}

// ============================================================================
// Panel Section
// ============================================================================

fn show_panel_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "Panel", "ai_inspector_panel", true, collapsed, |ui| {
        if ui
            .checkbox(
                &mut settings.config.ai_inspector_enabled,
                "Enable Assistant Panel",
            )
            .on_hover_text("Show Assistant panel toggle keybinding (Cmd+I / Ctrl+Shift+I)")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if ui
            .checkbox(
                &mut settings.config.ai_inspector_open_on_startup,
                "Open on startup",
            )
            .on_hover_text("Automatically open the Assistant panel when a new window opens")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label("Default width:");
            if ui
                .add(
                    egui::Slider::new(&mut settings.config.ai_inspector_width, 200.0..=600.0)
                        .suffix("px"),
                )
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });

        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label("Default scope:");
            egui::ComboBox::from_id_salt("ai_scope")
                .selected_text(&settings.config.ai_inspector_default_scope)
                .show_ui(ui, |ui| {
                    for scope in &[
                        "visible",
                        "recent_5",
                        "recent_10",
                        "recent_25",
                        "recent_50",
                        "full",
                    ] {
                        if ui
                            .selectable_value(
                                &mut settings.config.ai_inspector_default_scope,
                                scope.to_string(),
                                *scope,
                            )
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    }
                });
        });

        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label("Default view:");
            egui::ComboBox::from_id_salt("ai_view")
                .selected_text(&settings.config.ai_inspector_view_mode)
                .show_ui(ui, |ui| {
                    for mode in &["cards", "timeline", "tree", "list_detail"] {
                        if ui
                            .selectable_value(
                                &mut settings.config.ai_inspector_view_mode,
                                mode.to_string(),
                                *mode,
                            )
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    }
                });
        });

        ui.add_space(4.0);

        if ui
            .checkbox(&mut settings.config.ai_inspector_live_update, "Live update")
            .on_hover_text("Automatically refresh panel content when terminal changes")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if ui
            .checkbox(
                &mut settings.config.ai_inspector_show_zones,
                "Show zone content",
            )
            .on_hover_text(
                "Display command zones in the panel (disable for compact agent-only mode)",
            )
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }
    });
}

// ============================================================================
// Agent Section
// ============================================================================

fn show_agent_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(ui, "Agent", "ai_inspector_agent", true, collapsed, |ui| {
        let all_agents = combined_available_agents(settings);

        ui.horizontal(|ui| {
            ui.label("Default agent:");
            // Find display name for the currently selected agent
            let selected_display = all_agents
                .iter()
                .find(|(id, _)| *id == settings.config.ai_inspector_agent)
                .map(|(_, name)| name.as_str())
                .unwrap_or(&settings.config.ai_inspector_agent);
            egui::ComboBox::from_id_salt("ai_agent")
                .selected_text(selected_display)
                .show_ui(ui, |ui| {
                    for (agent_id, agent_name) in &all_agents {
                        if ui
                            .selectable_value(
                                &mut settings.config.ai_inspector_agent,
                                agent_id.clone(),
                                agent_name,
                            )
                            .changed()
                        {
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    }
                });
        });

        ui.add_space(4.0);

        if ui
            .checkbox(
                &mut settings.config.ai_inspector_auto_launch,
                "Auto-launch agent",
            )
            .on_hover_text("Automatically connect to the configured agent when the panel opens")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        if ui
            .checkbox(
                &mut settings.config.ai_inspector_auto_context,
                "Auto-send context",
            )
            .on_hover_text("Automatically send command results to the agent when commands complete")
            .changed()
        {
            settings.has_changes = true;
            *changes_this_frame = true;
        }

        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label("Max context lines:");
            if ui
                .add(egui::Slider::new(
                    &mut settings.config.ai_inspector_context_max_lines,
                    50..=1000,
                ))
                .changed()
            {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        });
    });
}

fn combined_available_agents(settings: &SettingsUI) -> Vec<(String, String)> {
    let mut combined = settings.available_agent_ids.clone();
    for custom in &settings.config.ai_inspector_custom_agents {
        if custom.active == Some(false) {
            continue;
        }
        let label = if custom.name.trim().is_empty() {
            custom.identity.clone()
        } else {
            format!("{} (custom)", custom.name)
        };
        if let Some(existing) = combined.iter_mut().find(|(id, _)| *id == custom.identity) {
            existing.1 = label;
        } else {
            combined.push((custom.identity.clone(), label));
        }
    }
    combined
}

fn set_run_command(agent: &mut CustomAcpAgentConfig, key: &str, value: String) {
    let value = value.trim().to_string();
    if value.is_empty() {
        agent.run_command.remove(key);
    } else {
        agent.run_command.insert(key.to_string(), value);
    }
}

fn next_env_placeholder_key(env: &HashMap<String, String>) -> String {
    let base = "NEW_ENV_VAR";
    if !env.contains_key(base) {
        return base.to_string();
    }

    let mut idx = 2usize;
    loop {
        let candidate = format!("{base}_{idx}");
        if !env.contains_key(&candidate) {
            return candidate;
        }
        idx += 1;
    }
}

fn show_custom_agents_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Custom Agents",
        "ai_inspector_custom_agents",
        false,
        collapsed,
        |ui| {
            ui.label(
                "Define additional ACP agents directly in config. \
                 Entries override bundled/discovered agents with the same identity.",
            );
            ui.add_space(6.0);

            let mut remove_index: Option<usize> = None;
            for i in 0..settings.config.ai_inspector_custom_agents.len() {
                let mut changed = false;
                let mut request_remove = false;

                ui.group(|ui| {
                    ui.push_id(format!("custom_agent_{i}"), |ui| {
                        let agent = &mut settings.config.ai_inspector_custom_agents[i];

                        ui.horizontal(|ui| {
                            ui.strong(format!("Agent {}", i + 1));
                            if !agent.identity.trim().is_empty() {
                                ui.label(format!("({})", agent.identity));
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.button("Remove").clicked() {
                                        request_remove = true;
                                    }
                                },
                            );
                        });

                        changed |= ui
                            .text_edit_singleline(&mut agent.identity)
                            .on_hover_text(
                                "Unique agent ID (usually a domain-like string), \
                                 used for selection and overrides.",
                            )
                            .changed();
                        if agent.identity.trim().is_empty() {
                            ui.colored_label(
                                egui::Color32::from_rgb(255, 193, 7),
                                "Identity is required.",
                            );
                        } else {
                            ui.label("Identity");
                        }

                        changed |= ui
                            .text_edit_singleline(&mut agent.name)
                            .on_hover_text("Display name shown in agent selectors.")
                            .changed();
                        ui.label("Name");

                        changed |= ui
                            .text_edit_singleline(&mut agent.short_name)
                            .on_hover_text("Compact label used in tighter UI surfaces.")
                            .changed();
                        ui.label("Short name");

                        ui.horizontal(|ui| {
                            let active = agent.active.get_or_insert(true);
                            changed |= ui
                                .checkbox(active, "Active")
                                .on_hover_text("Inactive agents are hidden from the UI.")
                                .changed();

                            if agent.protocol != "acp" {
                                agent.protocol = "acp".to_string();
                                changed = true;
                            }

                            ui.label("Protocol")
                                .on_hover_text("ACP is currently the only supported protocol.");
                            ui.add_enabled(
                                false,
                                egui::TextEdit::singleline(&mut agent.protocol).desired_width(56.0),
                            )
                            .on_hover_text("Read-only: only `acp` is supported right now.");

                            ui.label("Type")
                                .on_hover_text("Agent category label (for organization/filtering).");
                            changed |= ui
                                .add(
                                    egui::TextEdit::singleline(&mut agent.r#type)
                                        .desired_width(100.0),
                                )
                                .on_hover_text("Typically `coding`.")
                                .changed();
                        });

                        ui.label("Install command (optional)");
                        let mut install_command = agent.install_command.clone().unwrap_or_default();
                        if ui
                            .text_edit_singleline(&mut install_command)
                            .on_hover_text(
                                "Shown when connector is missing. Example: npm/brew/pip install command.",
                            )
                            .changed()
                        {
                            let trimmed = install_command.trim().to_string();
                            agent.install_command = if trimmed.is_empty() {
                                None
                            } else {
                                Some(trimmed)
                            };
                            changed = true;
                        }

                        ui.add_space(4.0);
                        ui.strong("Run commands");

                        let mut wildcard = agent.run_command.get("*").cloned().unwrap_or_default();
                        ui.horizontal(|ui| {
                            ui.label("*")
                                .on_hover_text("Default command for all platforms.");
                            if ui
                                .text_edit_singleline(&mut wildcard)
                                .on_hover_text("Command used to launch the ACP connector.")
                                .changed()
                            {
                                set_run_command(agent, "*", wildcard.clone());
                                changed = true;
                            }
                        });

                        let mut macos = agent.run_command.get("macos").cloned().unwrap_or_default();
                        ui.horizontal(|ui| {
                            ui.label("macos")
                                .on_hover_text("Optional macOS-specific override command.");
                            if ui
                                .text_edit_singleline(&mut macos)
                                .on_hover_text("Leave empty to fall back to `*`.")
                                .changed()
                            {
                                set_run_command(agent, "macos", macos.clone());
                                changed = true;
                            }
                        });

                        let mut linux = agent.run_command.get("linux").cloned().unwrap_or_default();
                        ui.horizontal(|ui| {
                            ui.label("linux")
                                .on_hover_text("Optional Linux-specific override command.");
                            if ui
                                .text_edit_singleline(&mut linux)
                                .on_hover_text("Leave empty to fall back to `*`.")
                                .changed()
                            {
                                set_run_command(agent, "linux", linux.clone());
                                changed = true;
                            }
                        });

                        let mut windows = agent
                            .run_command
                            .get("windows")
                            .cloned()
                            .unwrap_or_default();
                        ui.horizontal(|ui| {
                            ui.label("windows")
                                .on_hover_text("Optional Windows-specific override command.");
                            if ui
                                .text_edit_singleline(&mut windows)
                                .on_hover_text("Leave empty to fall back to `*`.")
                                .changed()
                            {
                                set_run_command(agent, "windows", windows.clone());
                                changed = true;
                            }
                        });

                        if agent.run_command.is_empty() {
                            ui.colored_label(
                                egui::Color32::from_rgb(255, 152, 0),
                                "At least one run command is required.",
                            );
                        }

                        ui.add_space(4.0);
                        ui.strong("Environment variables");
                        ui.label("These key/value pairs are injected into the ACP subprocess.");

                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label("Ollama context")
                                .on_hover_text(
                                    "Optional helper for Ollama-backed agents. Sets \
                                     OLLAMA_CONTEXT_LENGTH on the ACP subprocess unless you \
                                     already define OLLAMA_CONTEXT_LENGTH in Env Vars.",
                                );
                            let mut ctx_text = agent
                                .ollama_context_length
                                .map(|v| v.to_string())
                                .unwrap_or_default();
                            let response = ui
                                .add(
                                    egui::TextEdit::singleline(&mut ctx_text)
                                        .desired_width(100.0)
                                        .hint_text("e.g. 32768"),
                                )
                                .on_hover_text(
                                    "Context window token limit to expose as \
                                     OLLAMA_CONTEXT_LENGTH. Leave blank to disable. \
                                     Note: if your Ollama server runs outside this ACP process \
                                     (for example a separate `ollama serve` / `ollama launch`), \
                                     set the same value in that server environment too.",
                                );
                            if response.changed() {
                                let trimmed = ctx_text.trim();
                                let parsed = if trimmed.is_empty() {
                                    Some(None)
                                } else {
                                    trimmed.parse::<u32>().ok().map(Some)
                                };
                                if let Some(value) = parsed {
                                    agent.ollama_context_length = value.filter(|v| *v > 0);
                                    changed = true;
                                }
                            }
                            if agent.env.contains_key("OLLAMA_CONTEXT_LENGTH") {
                                ui.label("(env override)")
                                    .on_hover_text(
                                        "Env Vars already defines OLLAMA_CONTEXT_LENGTH. \
                                         That value takes precedence over this helper field.",
                                    );
                            }
                        });

                        let mut env_rows: Vec<(String, String)> = agent
                            .env
                            .iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect();
                        env_rows.sort_by(|a, b| a.0.cmp(&b.0));

                        let mut remove_env_index: Option<usize> = None;
                        for (idx, (key, value)) in env_rows.iter_mut().enumerate() {
                            ui.horizontal(|ui| {
                                ui.label("KEY").on_hover_text(
                                    "Environment variable name injected into the ACP subprocess.",
                                );
                                if ui
                                    .text_edit_singleline(key)
                                    .on_hover_text("Example: ANTHROPIC_BASE_URL")
                                    .changed()
                                {
                                    changed = true;
                                }
                                ui.label("VALUE")
                                    .on_hover_text("Environment variable value for this key.");
                                if ui
                                    .text_edit_singleline(value)
                                    .on_hover_text("Example: http://127.0.0.1:11434")
                                    .changed()
                                {
                                    changed = true;
                                }
                                if ui.small_button("Remove").clicked() {
                                    remove_env_index = Some(idx);
                                    changed = true;
                                }
                            });
                        }

                        if let Some(idx) = remove_env_index {
                            env_rows.remove(idx);
                        }

                        if ui.small_button("Add Env Var").clicked() {
                            let key = next_env_placeholder_key(&agent.env);
                            env_rows.push((key, String::new()));
                            changed = true;
                        }

                        if changed {
                            agent.env = env_rows
                                .into_iter()
                                .filter_map(|(k, v)| {
                                    let key = k.trim().to_string();
                                    if key.is_empty() { None } else { Some((key, v)) }
                                })
                                .collect();
                        }
                    });
                });

                if changed {
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
                if request_remove {
                    remove_index = Some(i);
                }

                ui.add_space(6.0);
            }

            if let Some(idx) = remove_index {
                settings.config.ai_inspector_custom_agents.remove(idx);
                settings.has_changes = true;
                *changes_this_frame = true;
            }

            if settings.config.ai_inspector_custom_agents.is_empty() {
                ui.label("No custom agents defined.");
            }

            if ui.button("Add Custom Agent").clicked() {
                settings
                    .config
                    .ai_inspector_custom_agents
                    .push(CustomAcpAgentConfig {
                        identity: format!(
                            "custom.agent.{}",
                            settings.config.ai_inspector_custom_agents.len() + 1
                        ),
                        name: "Custom ACP Agent".to_string(),
                        short_name: "custom".to_string(),
                        protocol: "acp".to_string(),
                        r#type: "coding".to_string(),
                        active: Some(true),
                        run_command: std::collections::HashMap::from([(
                            "*".to_string(),
                            "your-agent-acp".to_string(),
                        )]),
                        env: std::collections::HashMap::new(),
                        ollama_context_length: None,
                        install_command: None,
                        actions: std::collections::HashMap::new(),
                    });
                settings.has_changes = true;
                *changes_this_frame = true;
            }
        },
    );
}

// ============================================================================
// Permissions Section
// ============================================================================

fn show_permissions_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    collapsing_section(
        ui,
        "Permissions",
        "ai_inspector_permissions",
        true,
        collapsed,
        |ui| {
            let yolo_response = ui
                .checkbox(&mut settings.config.ai_inspector_auto_approve, "Yolo Mode")
                .on_hover_text("Auto-approve all agent permission requests. Use with caution!");
            if yolo_response.changed() {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
            if settings.config.ai_inspector_auto_approve {
                ui.colored_label(
                    egui::Color32::from_rgb(255, 193, 7),
                    "Most agent permission requests will be auto-approved (screenshots still prompt)",
                );
            }

            ui.add_space(8.0);

            let terminal_access_response = ui
                .checkbox(
                    &mut settings.config.ai_inspector_agent_terminal_access,
                    "Allow Terminal Access",
                )
                .on_hover_text(
                    "Allow the AI agent to write input directly to the terminal. \
                     When enabled, command suggestions from the agent will be \
                     auto-executed in the active terminal.",
                );
            if terminal_access_response.changed() {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
            if settings.config.ai_inspector_agent_terminal_access {
                ui.colored_label(
                    egui::Color32::from_rgb(255, 152, 0),
                    "Agent can write commands to the terminal",
                );
            }

            ui.add_space(8.0);

            let screenshot_access_response = ui
                .checkbox(
                    &mut settings.config.ai_inspector_agent_screenshot_access,
                    "Allow Agent Screenshots",
                )
                .on_hover_text(
                    "Allow the agent to request terminal screenshots via the \
                     `terminal_screenshot` MCP tool for visual debugging. \
                     Screenshot captures remain permission-gated per request and \
                     are not auto-approved by Yolo Mode.",
                );
            if screenshot_access_response.changed() {
                settings.has_changes = true;
                *changes_this_frame = true;
            }
            if settings.config.ai_inspector_agent_screenshot_access {
                ui.colored_label(
                    egui::Color32::from_rgb(255, 193, 7),
                    "Screenshot requests still require per-request approval in chat",
                );
            } else {
                ui.colored_label(
                    egui::Color32::from_rgb(255, 152, 0),
                    "Agent screenshot requests will be denied",
                );
            }
        },
    );
}
