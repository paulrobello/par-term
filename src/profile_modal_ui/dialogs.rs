//! Modal dialog and inline embedding rendering for the profile UI.
//!
//! Contains the top-level `show()` method for the floating modal window,
//! `show_inline()` for embedding within the settings panel, and the
//! delete confirmation dialog.

use crate::ui_constants::{PROFILE_MODAL_HEIGHT, PROFILE_MODAL_WIDTH};

use super::state::{ModalMode, ProfileModalAction, ProfileModalUI};

impl ProfileModalUI {
    /// Render the profile list/edit UI inline (no egui::Window wrapper).
    ///
    /// Used inside the settings window's Profiles tab to embed the profile
    /// management UI directly. Returns `ProfileModalAction` to communicate
    /// save/cancel/open-profile requests to the caller.
    pub fn show_inline(&mut self, ui: &mut egui::Ui) -> ProfileModalAction {
        let action = match &self.mode.clone() {
            ModalMode::List => self.render_list_view(ui),
            ModalMode::Edit(_) | ModalMode::Create => {
                self.render_edit_view(ui);
                ProfileModalAction::None
            }
        };

        // Render delete confirmation dialog on top
        if self.pending_delete.is_some() {
            self.render_delete_confirmation(ui.ctx());
        }

        action
    }

    /// Render the modal and return any action triggered
    pub fn show(&mut self, ctx: &egui::Context) -> ProfileModalAction {
        if !self.visible {
            return ProfileModalAction::None;
        }

        let mut action = ProfileModalAction::None;

        // Handle Escape key
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            match &self.mode {
                ModalMode::Edit(_) | ModalMode::Create => {
                    self.cancel_edit();
                }
                ModalMode::List => {
                    self.close();
                    return ProfileModalAction::Cancel;
                }
            }
        }

        let modal_size = egui::vec2(PROFILE_MODAL_WIDTH, PROFILE_MODAL_HEIGHT);

        egui::Window::new("Manage Profiles")
            .collapsible(false)
            .resizable(false)
            .order(egui::Order::Foreground)
            .default_size(modal_size)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .frame(
                egui::Frame::window(&ctx.style())
                    .fill(egui::Color32::from_rgba_unmultiplied(30, 30, 30, 250))
                    .inner_margin(egui::Margin::same(16)),
            )
            .show(ctx, |ui| match &self.mode.clone() {
                ModalMode::List => {
                    action = self.render_list_view(ui);
                }
                ModalMode::Edit(_) | ModalMode::Create => {
                    self.render_edit_view(ui);
                }
            });

        // Render delete confirmation dialog on top
        if self.pending_delete.is_some() {
            self.render_delete_confirmation(ctx);
        }

        action
    }

    /// Render delete confirmation dialog
    pub(super) fn render_delete_confirmation(&mut self, ctx: &egui::Context) {
        let (_, profile_name) = self
            .pending_delete
            .as_ref()
            .expect("render_delete_confirmation is only called when pending_delete is Some");
        let name = profile_name.clone();

        egui::Window::new("Confirm Delete")
            .collapsible(false)
            .resizable(false)
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .frame(
                egui::Frame::window(&ctx.style())
                    .fill(egui::Color32::from_rgba_unmultiplied(40, 40, 40, 255))
                    .inner_margin(egui::Margin::same(20)),
            )
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(format!("Delete profile \"{}\"?", name));
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("This action cannot be undone.")
                            .small()
                            .color(egui::Color32::GRAY),
                    );
                    ui.add_space(16.0);
                    ui.horizontal(|ui| {
                        if ui.button("Delete").clicked() {
                            self.confirm_delete();
                        }
                        if ui.button("Cancel").clicked() {
                            self.cancel_delete();
                        }
                    });
                });
            });
    }
}
