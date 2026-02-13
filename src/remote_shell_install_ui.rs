//! Remote shell integration install confirmation dialog.
//!
//! Shows a confirmation dialog when the user selects "Install Shell Integration
//! on Remote Host" from the Shell menu. Displays the exact curl command that will
//! be sent to the active terminal and lets the user confirm or cancel.

/// The install command URL
const INSTALL_URL: &str = "https://paulrobello.github.io/par-term/install-shell-integration.sh";

/// Action returned by the remote shell install dialog
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteShellInstallAction {
    /// User confirmed - send the install command to the active terminal
    Install,
    /// User cancelled
    Cancel,
    /// No action yet (dialog still showing or not visible)
    None,
}

/// State for the remote shell integration install dialog
pub struct RemoteShellInstallUI {
    /// Whether the dialog is visible
    visible: bool,
}

impl Default for RemoteShellInstallUI {
    fn default() -> Self {
        Self::new()
    }
}

impl RemoteShellInstallUI {
    /// Create a new remote shell install UI
    pub fn new() -> Self {
        Self { visible: false }
    }

    /// Check if the dialog is currently visible
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Show the confirmation dialog
    pub fn show_dialog(&mut self) {
        self.visible = true;
    }

    /// Hide the dialog
    fn hide(&mut self) {
        self.visible = false;
    }

    /// Get the install command string
    pub fn install_command() -> String {
        format!("curl -sSL {} | sh", INSTALL_URL)
    }

    /// Render the dialog and return any action
    pub fn show(&mut self, ctx: &egui::Context) -> RemoteShellInstallAction {
        if !self.visible {
            return RemoteShellInstallAction::None;
        }

        let mut action = RemoteShellInstallAction::None;
        let command = Self::install_command();

        egui::Window::new("Install Shell Integration on Remote Host")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(10.0);

                    ui.label(
                        egui::RichText::new("Send Install Command to Terminal")
                            .size(16.0)
                            .strong(),
                    );
                    ui.add_space(8.0);

                    ui.label("This will send the following command to the active terminal:");
                    ui.add_space(8.0);

                    // Command preview in a highlighted code block
                    egui::Frame::new()
                        .fill(egui::Color32::from_rgba_unmultiplied(40, 40, 40, 220))
                        .inner_margin(egui::Margin::symmetric(12, 8))
                        .corner_radius(4.0)
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(&command)
                                    .color(egui::Color32::LIGHT_GREEN)
                                    .monospace()
                                    .size(13.0),
                            );
                        });

                    ui.add_space(10.0);

                    // Warning
                    ui.label(
                        egui::RichText::new(
                            "Only use this when SSH'd into a remote host that needs shell integration.",
                        )
                        .color(egui::Color32::YELLOW)
                        .size(12.0),
                    );

                    ui.add_space(15.0);

                    // Buttons
                    ui.horizontal(|ui| {
                        let install_button = egui::Button::new(
                            egui::RichText::new("Install").color(egui::Color32::WHITE),
                        )
                        .fill(egui::Color32::from_rgb(50, 120, 50));

                        if ui.add(install_button).clicked() {
                            action = RemoteShellInstallAction::Install;
                        }

                        ui.add_space(10.0);

                        if ui.button("Cancel").clicked() {
                            action = RemoteShellInstallAction::Cancel;
                        }
                    });
                    ui.add_space(10.0);
                });
            });

        // Handle escape key to cancel
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            action = RemoteShellInstallAction::Cancel;
        }

        // Hide dialog on any action
        if !matches!(action, RemoteShellInstallAction::None) {
            self.hide();
        }

        action
    }
}
