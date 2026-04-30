//! Assistant prompt-library settings section.

use crate::SettingsUI;
use crate::section::{collapsing_section, section_matches};
use par_term_config::{AssistantPrompt, AssistantPromptDraft};
use std::collections::HashSet;

const PROMPT_LIBRARY_KEYWORDS: &[&str] = &[
    "prompt",
    "library",
    "saved prompt",
    "auto submit",
    "auto-submit",
    "markdown",
    "frontmatter",
];

/// Show the Assistant Prompt Library settings section.
pub(super) fn show_prompt_library_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    collapsed: &mut HashSet<String>,
) {
    if !section_matches(
        &settings.search_query.trim().to_lowercase(),
        "Prompt Library",
        PROMPT_LIBRARY_KEYWORDS,
    ) {
        return;
    }

    collapsing_section(
        ui,
        "Prompt Library",
        "ai_inspector_prompt_library",
        true,
        collapsed,
        |ui| {
            ui.label("Saved Assistant prompts are stored as Markdown files with YAML frontmatter.");
            ui.add_space(4.0);

            if let Some(error) = &settings.assistant_prompt_error {
                ui.colored_label(egui::Color32::RED, error);
                ui.add_space(4.0);
            }

            let mut edit_index: Option<usize> = None;
            let mut delete_index: Option<usize> = None;

            if settings.assistant_prompts.is_empty() {
                ui.label(egui::RichText::new("No prompts saved.").italics());
            } else {
                for (index, prompt) in settings.assistant_prompts.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(&prompt.title).strong());
                        ui.label(if prompt.auto_submit {
                            "Auto-submit: on"
                        } else {
                            "Auto-submit: off"
                        });
                        if ui.button("Edit").clicked() {
                            edit_index = Some(index);
                        }
                        if ui.button("Delete").clicked() {
                            delete_index = Some(index);
                        }
                    });
                }
            }

            if let Some(index) = edit_index {
                if let Some(prompt) = settings.assistant_prompts.get(index).cloned() {
                    settings.editing_assistant_prompt_index = Some(index);
                    settings.adding_new_assistant_prompt = false;
                    settings.assistant_prompt_error = None;
                    populate_assistant_prompt_editor(settings, &prompt);
                }
            }

            if let Some(index) = delete_index {
                if let Some(prompt) = settings.assistant_prompts.get(index).cloned() {
                    delete_assistant_prompt(settings, &prompt);
                }
            }

            ui.add_space(8.0);
            if ui.button("+ Add Prompt").clicked() {
                settings.editing_assistant_prompt_index = None;
                settings.adding_new_assistant_prompt = true;
                settings.assistant_prompt_error = None;
                reset_assistant_prompt_editor(settings);
            }

            if settings.adding_new_assistant_prompt
                || settings.editing_assistant_prompt_index.is_some()
            {
                ui.separator();
                show_prompt_editor(ui, settings);
            }
        },
    );
}

fn show_prompt_editor(ui: &mut egui::Ui, settings: &mut SettingsUI) {
    let heading = if settings.adding_new_assistant_prompt {
        "Add Prompt"
    } else {
        "Edit Prompt"
    };
    ui.strong(heading);
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        ui.label("Title:");
        ui.add(
            egui::TextEdit::singleline(&mut settings.temp_assistant_prompt_title)
                .desired_width(320.0)
                .hint_text("Debug build"),
        );
    });

    ui.label("Prompt:");
    ui.add(
        egui::TextEdit::multiline(&mut settings.temp_assistant_prompt_body)
            .desired_width(f32::INFINITY)
            .desired_rows(8)
            .hint_text("Write the Assistant prompt here..."),
    );

    ui.checkbox(
        &mut settings.temp_assistant_prompt_auto_submit,
        "Auto-submit when selected",
    )
    .on_hover_text(
        "When enabled, selecting this prompt in the Assistant panel sends it immediately.",
    );

    ui.horizontal(|ui| {
        if ui.button("Save").clicked() {
            save_assistant_prompt(settings);
        }
        if ui.button("Cancel").clicked() {
            settings.adding_new_assistant_prompt = false;
            settings.editing_assistant_prompt_index = None;
            settings.assistant_prompt_error = None;
            reset_assistant_prompt_editor(settings);
        }
    });
}

fn save_assistant_prompt(settings: &mut SettingsUI) {
    let draft = match assistant_prompt_draft(
        &settings.temp_assistant_prompt_title,
        &settings.temp_assistant_prompt_body,
        settings.temp_assistant_prompt_auto_submit,
    ) {
        Ok(draft) => draft,
        Err(error) => {
            settings.assistant_prompt_error = Some(error);
            return;
        }
    };

    let existing_path = settings
        .editing_assistant_prompt_index
        .and_then(|index| settings.assistant_prompts.get(index))
        .map(|prompt| prompt.path.as_path());

    match par_term_config::save_prompt(existing_path, &draft) {
        Ok(_) => {
            settings.adding_new_assistant_prompt = false;
            settings.editing_assistant_prompt_index = None;
            reset_assistant_prompt_editor(settings);
            refresh_assistant_prompts(settings);
        }
        Err(error) => settings.assistant_prompt_error = Some(error),
    }
}

fn delete_assistant_prompt(settings: &mut SettingsUI, prompt: &AssistantPrompt) {
    match par_term_config::delete_prompt(&prompt.path) {
        Ok(()) => {
            settings.adding_new_assistant_prompt = false;
            settings.editing_assistant_prompt_index = None;
            reset_assistant_prompt_editor(settings);
            refresh_assistant_prompts(settings);
        }
        Err(error) => settings.assistant_prompt_error = Some(error),
    }
}

fn refresh_assistant_prompts(settings: &mut SettingsUI) {
    match par_term_config::list_prompts() {
        Ok(prompts) => {
            settings.assistant_prompts = prompts;
            settings.assistant_prompt_error = None;
        }
        Err(error) => settings.assistant_prompt_error = Some(error),
    }
}

fn assistant_prompt_draft(
    title: &str,
    body: &str,
    auto_submit: bool,
) -> Result<AssistantPromptDraft, String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("prompt title is required".to_string());
    }
    if body.trim().is_empty() {
        return Err("prompt body is required".to_string());
    }

    Ok(AssistantPromptDraft {
        title: title.to_string(),
        auto_submit,
        prompt: body.to_string(),
    })
}

fn populate_assistant_prompt_editor(settings: &mut SettingsUI, prompt: &AssistantPrompt) {
    let (title, body, auto_submit) = assistant_prompt_editor_values(prompt);
    settings.temp_assistant_prompt_title = title;
    settings.temp_assistant_prompt_body = body;
    settings.temp_assistant_prompt_auto_submit = auto_submit;
}

fn assistant_prompt_editor_values(prompt: &AssistantPrompt) -> (String, String, bool) {
    (
        prompt.title.clone(),
        prompt.prompt.clone(),
        prompt.auto_submit,
    )
}

fn reset_assistant_prompt_editor(settings: &mut SettingsUI) {
    settings.temp_assistant_prompt_title.clear();
    settings.temp_assistant_prompt_body.clear();
    settings.temp_assistant_prompt_auto_submit = false;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn assistant_prompt_draft_rejects_empty_title_or_body() {
        assert!(assistant_prompt_draft("", "body", false).is_err());
        assert!(assistant_prompt_draft("title", "   ", false).is_err());
    }

    #[test]
    fn assistant_prompt_draft_trims_title_and_preserves_body() {
        let draft = assistant_prompt_draft("  Debug  ", "  keep body spacing  ", true)
            .expect("valid draft");

        assert_eq!(draft.title, "Debug");
        assert_eq!(draft.prompt, "  keep body spacing  ");
        assert!(draft.auto_submit);
    }

    #[test]
    fn assistant_prompt_editor_values_copy_prompt_fields() {
        let prompt = AssistantPrompt {
            path: PathBuf::from("prompt.md"),
            title: "Debug build".to_string(),
            auto_submit: true,
            prompt: "Fix it".to_string(),
        };

        let (title, body, auto_submit) = assistant_prompt_editor_values(&prompt);

        assert_eq!(title, "Debug build");
        assert_eq!(body, "Fix it");
        assert!(auto_submit);
    }
}
