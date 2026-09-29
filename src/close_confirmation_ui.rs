//! Close confirmation dialog for tabs with running jobs.
//!
//! Shows a confirmation dialog when the user attempts to close a terminal tab
//! that has a running command (detected via shell integration). Allows the user
//! to either force close the tab or cancel the close operation.

use crate::tab::TabId;

/// Action returned by the close confirmation dialog
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloseConfirmAction {
    /// User confirmed - close the tab/pane with the given IDs
    Close {
        tab_id: TabId,
        pane_id: Option<crate::pane::PaneId>,
    },
    /// User cancelled - keep the tab open
    Cancel,
    /// No action yet (dialog still showing)
    None,
}

/// State for the close confirmation dialog
pub struct CloseConfirmationUI {
    /// Whether the dialog is visible
    visible: bool,
    /// The tab ID pending close
    pending_tab_id: Option<TabId>,
    /// The pane ID pending close (None means close entire tab)
    pending_pane_id: Option<crate::pane::PaneId>,
    /// The name of the running command
    command_name: String,
    /// The tab title for display
    tab_title: String,
}

impl Default for CloseConfirmationUI {
    fn default() -> Self {
        Self::new()
    }
}

impl CloseConfirmationUI {
    /// Create a new close confirmation UI
    pub fn new() -> Self {
        Self {
            visible: false,
            pending_tab_id: None,
            pending_pane_id: None,
            command_name: String::new(),
            tab_title: String::new(),
        }
    }

    /// Check if the dialog is currently visible
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Show the confirmation dialog for a tab with a running command
    pub fn show_for_tab(&mut self, tab_id: TabId, tab_title: &str, command_name: &str) {
        self.visible = true;
        self.pending_tab_id = Some(tab_id);
        self.pending_pane_id = None;
        self.command_name = command_name.to_string();
        self.tab_title = tab_title.to_string();
    }

    /// Show the confirmation dialog for a pane with a running command
    pub fn show_for_pane(
        &mut self,
        tab_id: TabId,
        pane_id: crate::pane::PaneId,
        tab_title: &str,
        command_name: &str,
    ) {
        self.visible = true;
        self.pending_tab_id = Some(tab_id);
        self.pending_pane_id = Some(pane_id);
        self.command_name = command_name.to_string();
        self.tab_title = tab_title.to_string();
    }

    /// Hide the dialog and clear state
    pub(crate) fn hide(&mut self) {
        self.visible = false;
        self.pending_tab_id = None;
        self.pending_pane_id = None;
        self.command_name.clear();
        self.tab_title.clear();
    }

    /// Render the dialog and return any action
    pub fn show(&mut self, ctx: &egui::Context) -> CloseConfirmAction {
        if !self.visible {
            return CloseConfirmAction::None;
        }

        let mut action = CloseConfirmAction::None;

        egui::Window::new("Close Tab?")
            .collapsible(false)
            .resizable(false)
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(10.0);

                    // Warning icon and title
                    ui.label(
                        egui::RichText::new("⚠ Running Job Detected")
                            .color(egui::Color32::YELLOW)
                            .size(18.0)
                            .strong(),
                    );
                    ui.add_space(10.0);

                    // Tab/pane info
                    let target = if self.pending_pane_id.is_some() {
                        "pane"
                    } else {
                        "tab"
                    };
                    ui.label(format!(
                        "The {} \"{}\" has a running command:",
                        target, self.tab_title
                    ));
                    ui.add_space(5.0);

                    // Command name in a highlighted box
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);
                        egui::Frame::new()
                            .fill(egui::Color32::from_rgba_unmultiplied(60, 60, 60, 200))
                            .inner_margin(egui::Margin::symmetric(12, 6))
                            .corner_radius(4.0)
                            .show(ui, |ui| {
                                ui.label(
                                    egui::RichText::new(&self.command_name)
                                        .color(egui::Color32::LIGHT_GREEN)
                                        .monospace()
                                        .size(14.0),
                                );
                            });
                    });

                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new("Closing will terminate this process.")
                            .color(egui::Color32::GRAY),
                    );
                    ui.add_space(15.0);

                    // Buttons
                    ui.horizontal(|ui| {
                        // Close button with danger styling
                        let close_button = egui::Button::new(
                            egui::RichText::new("Close Anyway").color(egui::Color32::WHITE),
                        )
                        .fill(egui::Color32::from_rgb(180, 50, 50));

                        if ui.add(close_button).clicked() {
                            // Capture IDs before we hide
                            if let Some(tab_id) = self.pending_tab_id {
                                action = CloseConfirmAction::Close {
                                    tab_id,
                                    pane_id: self.pending_pane_id,
                                };
                            }
                        }

                        ui.add_space(10.0);

                        if ui.button("Cancel").clicked() {
                            action = CloseConfirmAction::Cancel;
                        }
                    });
                    ui.add_space(10.0);
                });
            });

        // Handle escape key to cancel
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            action = CloseConfirmAction::Cancel;
        }

        // MD5: Enter is the safe choice, never the destructive Close. It
        // runs after the buttons so it also wins over a focused button's
        // same-frame Enter activation (egui fake-clicks focused widgets).
        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            action = CloseConfirmAction::Cancel;
        }

        // Hide dialog on any action
        if !matches!(action, CloseConfirmAction::None) {
            self.hide();
        }

        action
    }
}

impl crate::traits::OverlayComponent for CloseConfirmationUI {
    type Action = CloseConfirmAction;

    fn show(&mut self, ctx: &egui::Context) -> Self::Action {
        CloseConfirmationUI::show(self, ctx)
    }

    fn is_visible(&self) -> bool {
        self.is_visible()
    }

    fn set_visible(&mut self, visible: bool) {
        if !visible {
            self.hide();
        }
        // Note: setting visible=true requires additional state (tab_id, command_name, etc.).
        // Use show_for_tab() or show_for_pane() to open this dialog.
    }
}

#[cfg(test)]
mod tests {
    use super::{CloseConfirmAction, CloseConfirmationUI};

    /// Render one frame with an Enter key press under a headless egui
    /// context and return the action the dialog reported.
    fn show_with_enter(ui: &mut CloseConfirmationUI) -> CloseConfirmAction {
        let ctx = egui::Context::default();
        ctx.begin_pass(egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200.0, 800.0),
            )),
            events: vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
            ..Default::default()
        });
        let action = ui.show(&ctx);
        ctx.end_pass().textures_delta.clear();
        action
    }

    /// MD5: Enter is the safe choice everywhere — never the destructive
    /// Close. egui also fake-clicks a focused button on Enter, so this
    /// must win over any same-frame button activation.
    #[test]
    fn enter_cancels_never_closes_tab() {
        let mut ui = CloseConfirmationUI::new();
        ui.show_for_tab(7, "work", "sleep 100");
        assert_eq!(show_with_enter(&mut ui), CloseConfirmAction::Cancel);
        assert!(!ui.is_visible());
    }

    /// The pane flavor of the same dialog (close-job confirm) shares the
    /// Enter rule.
    #[test]
    fn enter_cancels_never_closes_pane() {
        let mut ui = CloseConfirmationUI::new();
        ui.show_for_pane(7, 2, "work", "cargo build");
        assert_eq!(show_with_enter(&mut ui), CloseConfirmAction::Cancel);
        assert!(!ui.is_visible());
    }
}
