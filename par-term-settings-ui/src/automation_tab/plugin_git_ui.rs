//! Git-distribution UI for the Plugins section: install from a URL,
//! update review, and remove — the Settings surface of the same
//! operations the `par-term plugin` subcommands offer. All git I/O runs
//! through the async_ops background-thread pattern; the git layer
//! enforces the prompt-free environment and per-invocation timeouts.

use crate::PluginGitOutcome;
use crate::SettingsUI;
use par_term_scripting::plugin_git;

/// Section-level git bar: install a plugin from a URL, the last
/// operation's status line, and a fetched update's diff awaiting Apply.
/// A clone or fetch must never block the render loop.
pub(super) fn show_plugin_git_bar(ui: &mut egui::Ui, settings: &mut SettingsUI) {
    let busy = settings.automation_tab.plugin_git_busy;
    ui.horizontal(|ui| {
        ui.label("Add from git URL:");
        ui.add_enabled(
            !busy,
            egui::TextEdit::singleline(&mut settings.automation_tab.plugin_git_url)
                .hint_text("https://host/owner/plugin.git")
                .desired_width(ui.available_width() * 0.6),
        );
        let url_ready = !settings.automation_tab.plugin_git_url.trim().is_empty();
        if ui
            .add_enabled(!busy && url_ready, egui::Button::new("Add"))
            .clicked()
        {
            let url = settings.automation_tab.plugin_git_url.trim().to_string();
            settings.automation_tab.plugin_git_url.clear();
            let root = par_term_config::Config::config_dir().join("plugins");
            settings.start_plugin_git_op(
                "Cloning plugin — it will land DISABLED…",
                move || {
                    plugin_git::add(&root, &url).map(|plugin| {
                        PluginGitOutcome::Done(format!(
                            "Installed {} {} ({}). DISABLED until enabled below — read its code first.",
                            plugin.manifest.name, plugin.manifest.version, plugin.manifest.id
                        ))
                    })
                },
            );
        }
    });
    if let Some(message) = settings.automation_tab.plugin_git_message.clone() {
        let error = message.starts_with("Error");
        ui.label(egui::RichText::new(message).small().color(if error {
            egui::Color32::from_rgb(220, 130, 130)
        } else {
            egui::Color32::GRAY
        }));
    }
    if let Some((id, preview)) = settings.automation_tab.plugin_update_preview.clone() {
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(format!(
                "Update ready for {id}: {} → {}",
                preview.current, preview.incoming
            ))
            .strong(),
        );
        let diff = preview.diff.trim();
        if !diff.is_empty() {
            ui.label(egui::RichText::new(diff).monospace().small());
        }
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!busy, egui::Button::new("Apply update"))
                .clicked()
            {
                settings.automation_tab.plugin_update_preview = None;
                let dir = par_term_config::Config::config_dir()
                    .join("plugins")
                    .join(&id);
                settings.start_plugin_git_op("Applying plugin update…", move || {
                    plugin_git::update_apply(&dir)
                        .map(|head| PluginGitOutcome::Done(format!("{id} updated to {head}.")))
                });
            }
            if ui.button("Discard").clicked() {
                settings.automation_tab.plugin_update_preview = None;
            }
            ui.label(
                egui::RichText::new(
                    "Fast-forward only — a rewritten upstream is refused; remove and re-add to take it.",
                )
                .small()
                .color(egui::Color32::GRAY),
            );
        });
    }
    ui.add_space(4.0);
}

/// Per-row git lifecycle controls for plugins installed by
/// `par-term plugin add`: Check for updates fetches (the diff and Apply
/// appear at the section level), and Remove arms on the first click and
/// acts on the second.
pub(super) fn show_plugin_git_actions(ui: &mut egui::Ui, settings: &mut SettingsUI, id: &str) {
    let busy = settings.automation_tab.plugin_git_busy;
    ui.horizontal(|ui| {
        if ui
            .add_enabled(!busy, egui::Button::new("Check for updates"))
            .clicked()
        {
            let dir = par_term_config::Config::config_dir()
                .join("plugins")
                .join(id);
            let id = id.to_string();
            settings.start_plugin_git_op("Fetching plugin updates…", move || {
                plugin_git::update_fetch(&dir).map(|preview| {
                    if preview.up_to_date {
                        PluginGitOutcome::Done(format!(
                            "{id} is already up to date ({}).",
                            preview.current
                        ))
                    } else {
                        PluginGitOutcome::UpdateReady { id, preview }
                    }
                })
            });
        }
        // Destructive: arm on the first click, act on the second.
        let armed = settings.automation_tab.plugin_remove_pending.as_deref() == Some(id);
        let label = if armed { "Really remove?" } else { "Remove…" };
        if ui.add_enabled(!busy, egui::Button::new(label)).clicked() {
            if armed {
                settings.automation_tab.plugin_remove_pending = None;
                let root = par_term_config::Config::config_dir().join("plugins");
                let id = id.to_string();
                settings.start_plugin_git_op("Removing plugin…", move || {
                    plugin_git::remove(&root, &id)
                        .map(|()| PluginGitOutcome::Done(format!("Removed {id}.")))
                });
            } else {
                settings.automation_tab.plugin_remove_pending = Some(id.to_string());
            }
        }
        if armed && ui.button("Keep").clicked() {
            settings.automation_tab.plugin_remove_pending = None;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use par_term_config::Config;

    /// The git bar and per-row controls must lay out without panicking in
    /// every state they can be in. Rendered through a throwaway
    /// `egui::Context` — the same CPU-side layout pass the real settings
    /// window runs each frame; no GPU or window is involved.
    #[test]
    fn git_controls_render_in_every_state_without_panicking() {
        let mut settings = SettingsUI::new_for_tests(Config::default());

        let render = |settings: &mut SettingsUI| {
            let ctx = egui::Context::default();
            let output = ctx.run_ui(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    show_plugin_git_bar(ui, settings);
                    // The row scope show_plugin_row wraps each plugin's
                    // controls in — replicated so id-salt behaviour is
                    // exercised exactly as production uses it.
                    ui.push_id("plugin_git_com.test.panel", |ui| {
                        show_plugin_git_actions(ui, settings, "com.test.panel");
                    });
                });
            });
            // Nothing applies the font-texture delta headless; clear it so
            // epaint's unapplied-delta assert does not fire on drop.
            let mut output = output;
            output.textures_delta.clear();
        };

        // Idle: no URL typed, nothing in flight, nothing fetched.
        render(&mut settings);

        // Busy cloning with a URL half-typed and a status line showing.
        settings.automation_tab.plugin_git_busy = true;
        settings.automation_tab.plugin_git_message = Some("Cloning…".to_string());
        settings.automation_tab.plugin_git_url = "https://host/owner/plugin.git".to_string();
        render(&mut settings);

        // Idle again, an error message from a failed op.
        settings.automation_tab.plugin_git_busy = false;
        settings.automation_tab.plugin_git_message = Some("Error: boom".to_string());
        render(&mut settings);

        // A fetched update awaiting review, with a diff to show.
        settings.automation_tab.plugin_git_message = None;
        settings.automation_tab.plugin_update_preview = Some((
            "com.test.panel".to_string(),
            plugin_git::UpdatePreview {
                up_to_date: false,
                current: "1111111".to_string(),
                incoming: "2222222".to_string(),
                diff: " manifest.json | 2 +-\n 1 file changed, 1 insertion(+), 1 deletion(-)"
                    .to_string(),
            },
        ));
        render(&mut settings);

        // Remove armed for the row (two-click confirm mid-flight).
        settings.automation_tab.plugin_update_preview = None;
        settings.automation_tab.plugin_remove_pending = Some("com.test.panel".to_string());
        render(&mut settings);
    }
}
