//! Sub-tabs of the profile editor (UX.md 15.2: General · Session · Text &
//! Badge · Shader · SSH · Auto-Switch).
//!
//! Each sub-tab draws one group of the profile's fields; the header, the
//! validation message, and Done/Cancel stay visible on every sub-tab
//! (`edit_view.rs`).

use super::ProfileModalUI;
use std::collections::HashSet;

/// A sub-tab of the profile editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProfileEditTab {
    /// Name, icon, tags, inheritance, shortcut, shell, command, directory.
    #[default]
    General,
    /// tmux auto-connect and par-mux auto-attach.
    Session,
    /// Tab name, badge text and badge appearance.
    TextAndBadge,
    /// Background shader overrides.
    Shader,
    /// SSH connection.
    Ssh,
    /// Hostname, tmux-session, and directory auto-switch patterns.
    AutoSwitch,
}

impl ProfileEditTab {
    /// Every sub-tab, in display order.
    pub const ALL: [Self; 6] = [
        Self::General,
        Self::Session,
        Self::TextAndBadge,
        Self::Shader,
        Self::Ssh,
        Self::AutoSwitch,
    ];

    /// The sub-tab's label.
    pub fn label(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Session => "Session",
            Self::TextAndBadge => "Text & Badge",
            Self::Shader => "Shader",
            Self::Ssh => "SSH",
            Self::AutoSwitch => "Auto-Switch",
        }
    }
}

impl ProfileModalUI {
    /// The sub-tab selector.
    pub(super) fn render_edit_tab_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            for tab in ProfileEditTab::ALL {
                if ui
                    .selectable_label(self.edit_tab == tab, tab.label())
                    .clicked()
                {
                    self.edit_tab = tab;
                }
            }
        });
    }

    /// Draw the selected sub-tab's fields.
    pub(super) fn render_edit_tab(&mut self, ui: &mut egui::Ui, collapsed: &mut HashSet<String>) {
        match self.edit_tab {
            ProfileEditTab::General => self.render_general_tab(ui),
            ProfileEditTab::Session => {
                self.render_tmux_section(ui, collapsed);
                ui.add_space(8.0);
                self.render_mux_section(ui, collapsed);
            }
            ProfileEditTab::TextAndBadge => {
                form_grid(ui, "profile_form_text", |ui| {
                    text_row(ui, "Tab Name:", &mut self.temp_tab_name, "(optional)");
                    text_row(
                        ui,
                        "Badge Text:",
                        &mut self.temp_badge_text,
                        "(overrides global)",
                    );
                });
                ui.add_space(8.0);
                self.render_badge_section(ui, collapsed);
            }
            ProfileEditTab::Shader => self.render_shader_section(ui, collapsed),
            ProfileEditTab::Ssh => self.render_ssh_section(ui, collapsed),
            ProfileEditTab::AutoSwitch => {
                ui.label(
                    egui::RichText::new(
                        "This profile is applied automatically when one of these patterns \
                         matches. Separate patterns with commas.",
                    )
                    .small()
                    .weak(),
                );
                form_grid(ui, "profile_form_auto_switch", |ui| {
                    text_row(
                        ui,
                        "Hosts:",
                        &mut self.temp_hostname_patterns,
                        "(*.example.com)",
                    );
                    text_row(
                        ui,
                        "tmux sessions:",
                        &mut self.temp_tmux_session_patterns,
                        "(work-*, *-dev)",
                    );
                    text_row(
                        ui,
                        "Directories:",
                        &mut self.temp_directory_patterns,
                        "(~/projects/work-*)",
                    );
                });
            }
        }
    }

    /// General: identity, inheritance, and what the profile runs.
    fn render_general_tab(&mut self, ui: &mut egui::Ui) {
        form_grid(ui, "profile_form", |ui| {
            ui.label("Name:");
            ui.text_edit_singleline(&mut self.temp_name);
            ui.end_row();

            ui.label("Icon:");
            self.render_icon_picker(ui);
            ui.end_row();

            text_row(ui, "Tags:", &mut self.temp_tags, "(comma-separated)");

            ui.label("Inherit From:");
            self.render_parent_selector(ui);
            ui.end_row();

            self.render_shortcut_row(ui);

            ui.label("Working Directory:");
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut self.temp_working_dir);
                if ui.small_button("Browse...").clicked()
                    && let Some(path) = rfd::FileDialog::new().pick_folder()
                {
                    self.temp_working_dir = path.display().to_string();
                }
            });
            ui.end_row();

            ui.label("Shell:");
            self.render_shell_selector(ui);
            ui.end_row();

            ui.label("Login Shell:");
            ui.horizontal(|ui| {
                let mut use_custom = self.temp_login_shell.is_some();
                if ui.checkbox(&mut use_custom, "").changed() {
                    self.temp_login_shell = use_custom.then_some(true);
                }
                if let Some(ref mut login) = self.temp_login_shell {
                    ui.checkbox(login, "Use login shell (-l)");
                } else {
                    ui.label(
                        egui::RichText::new("(inherit global)")
                            .small()
                            .color(egui::Color32::GRAY),
                    );
                }
            });
            ui.end_row();

            text_row(ui, "Command:", &mut self.temp_command, "(overrides shell)");
            text_row(ui, "Arguments:", &mut self.temp_args, "(space-separated)");
        });
    }
}

/// A two-column label/field grid.
fn form_grid(ui: &mut egui::Ui, id: &str, add_rows: impl FnOnce(&mut egui::Ui)) {
    egui::Grid::new(id)
        .num_columns(2)
        .spacing([10.0, 8.0])
        .show(ui, add_rows);
}

/// One grid row: label, text field, and a gray hint.
fn text_row(ui: &mut egui::Ui, label: &str, value: &mut String, hint: &str) {
    ui.label(label);
    ui.horizontal(|ui| {
        ui.text_edit_singleline(value);
        ui.label(egui::RichText::new(hint).small().color(egui::Color32::GRAY));
    });
    ui.end_row();
}
