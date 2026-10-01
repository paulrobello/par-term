//! Sessions › par-mux: global par-mux settings (UX.md SX1).
//!
//! Only settings with a runtime reader get a control. The leader key (K7)
//! and a configurable last-tab close policy (M1) have no config field yet:
//! closing the last attached tab always asks (detach or end the session).

use crate::SettingsUI;
use crate::search::SearchTag;
use crate::section::{INPUT_WIDTH, keyword_section};
use std::collections::HashSet;

/// Install state of one agent's par-mux session hook, as reported by the
/// host (`SettingsUI::mux_hook_status_fn`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MuxHookStatus {
    /// Agent name shown in the list ("claude", "codex", ...).
    pub agent: String,
    /// Whether par-term's hook entry is present in the agent's config.
    pub installed: bool,
}

pub(crate) fn show_par_mux_section(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    keyword_section(
        ui,
        "par-mux",
        "sessions_par_mux",
        &["mux", "daemon", "attach", "hooks", "agent hooks"],
        true,
        collapsed,
        |ui| {
            ui.label(
                egui::RichText::new(
                    "par-mux keeps tabs and panes running in a background daemon, so a \
                     window can detach and reattach without losing work.",
                )
                .small()
                .weak(),
            );
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.label("Attach on launch:");
                let mut session = settings
                    .config
                    .tmux
                    .mux_auto_attach
                    .clone()
                    .unwrap_or_default();
                let response = ui
                    .add(
                        egui::TextEdit::singleline(&mut session)
                            .desired_width(INPUT_WIDTH)
                            .hint_text("session name (empty = off)"),
                    )
                    .search_tag(&["mux_auto_attach", "auto attach"])
                    .on_hover_text(
                        "The first window attaches to this par-mux session on launch, \
                         creating it when it does not exist. Empty: no auto-attach. \
                         The --attach command-line option takes precedence.",
                    );
                if response.changed() {
                    let trimmed = session.trim();
                    settings.config.tmux.mux_auto_attach =
                        (!trimmed.is_empty()).then(|| trimmed.to_string());
                    settings.has_changes = true;
                    *changes_this_frame = true;
                }
            });

            ui.add_space(8.0);
            show_hook_status(ui, settings);

            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(
                    "Closing the last tab of an attached session always asks whether to \
                     detach (the session keeps running) or end it. Per-profile par-mux \
                     sessions are set in each profile's Session page.",
                )
                .small()
                .weak(),
            );
        },
    );
}

fn show_hook_status(ui: &mut egui::Ui, settings: &mut SettingsUI) {
    ui.label(egui::RichText::new("Agent session hooks").strong());
    ui.label(
        egui::RichText::new(
            "Hooks let par-mux show which coding agent runs in each pane. Install or \
             remove them with `par-term install-mux-hooks` and `par-term \
             uninstall-mux-hooks`.",
        )
        .small()
        .weak(),
    );
    let Some(status_fn) = settings.mux_hook_status_fn else {
        ui.label(egui::RichText::new("Hook status is not available.").small());
        return;
    };
    // The status reads the agents' config files: read once, then on Refresh,
    // never every frame on the UI thread.
    if ui
        .small_button("Refresh")
        .on_hover_text("Read the agents' config files again")
        .clicked()
    {
        settings.mux_hook_status = None;
    }
    let statuses = settings
        .mux_hook_status
        .get_or_insert_with(status_fn)
        .clone();
    for status in statuses {
        ui.horizontal(|ui| {
            ui.label(format!("{}:", status.agent));
            if status.installed {
                ui.colored_label(egui::Color32::from_rgb(76, 175, 80), "installed");
            } else {
                ui.colored_label(egui::Color32::GRAY, "not installed");
            }
        });
    }
}
