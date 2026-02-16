//! Quit confirmation dialog for the application.
//!
//! Shows a confirmation dialog when the user attempts to close the window
//! while there are active terminal sessions. Allows the user to either
//! quit the application or cancel the close operation.

/// Action returned by the quit confirmation dialog
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuitConfirmAction {
    /// User confirmed - quit the application
    Quit,
    /// User cancelled - keep the window open
    Cancel,
    /// No action yet (dialog not showing or still showing)
    None,
}

/// State for the quit confirmation dialog
pub struct QuitConfirmationUI {
    /// Whether the dialog is visible
    visible: bool,
    /// Number of active sessions to display
    session_count: usize,
}

impl Default for QuitConfirmationUI {
    fn default() -> Self {
        Self::new()
    }
}

impl QuitConfirmationUI {
    /// Create a new quit confirmation UI
    pub fn new() -> Self {
        Self {
            visible: false,
            session_count: 0,
        }
    }

    /// Check if the dialog is currently visible
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Show the confirmation dialog with the number of active sessions
    pub fn show_confirmation(&mut self, session_count: usize) {
        self.visible = true;
        self.session_count = session_count;
    }

    /// Hide the dialog and clear state
    fn hide(&mut self) {
        self.visible = false;
        self.session_count = 0;
    }

    /// Render the dialog and return any action
    pub fn show(&mut self, ctx: &egui::Context) -> QuitConfirmAction {
        if !self.visible {
            return QuitConfirmAction::None;
        }

        let mut action = QuitConfirmAction::None;

        egui::Window::new("Quit par-term?")
            .collapsible(false)
            .resizable(false)
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(10.0);

                    ui.label(
                        egui::RichText::new("⚠ Quit Application?")
                            .color(egui::Color32::YELLOW)
                            .size(18.0)
                            .strong(),
                    );
                    ui.add_space(10.0);

                    let session_text = if self.session_count == 1 {
                        "There is 1 active session.".to_string()
                    } else {
                        format!("There are {} active sessions.", self.session_count)
                    };
                    ui.label(&session_text);
                    ui.add_space(5.0);

                    ui.label(
                        egui::RichText::new("All sessions will be terminated.")
                            .color(egui::Color32::GRAY),
                    );
                    ui.add_space(15.0);

                    // Buttons
                    ui.horizontal(|ui| {
                        let quit_button = egui::Button::new(
                            egui::RichText::new("Quit").color(egui::Color32::WHITE),
                        )
                        .fill(egui::Color32::from_rgb(180, 50, 50));

                        if ui.add(quit_button).clicked() {
                            action = QuitConfirmAction::Quit;
                        }

                        ui.add_space(10.0);

                        if ui.button("Cancel").clicked() {
                            action = QuitConfirmAction::Cancel;
                        }
                    });
                    ui.add_space(10.0);
                });
            });

        // Handle escape key to cancel
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            action = QuitConfirmAction::Cancel;
        }

        // Handle enter key to confirm quit
        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            action = QuitConfirmAction::Quit;
        }

        // Hide dialog on any action
        if !matches!(action, QuitConfirmAction::None) {
            self.hide();
        }

        action
    }
}
