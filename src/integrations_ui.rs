//! Combined integrations welcome dialog.
//!
//! Shows on first run when integrations are not installed,
//! offering to install shaders and/or shell integration.

use crate::config::ShellType;
use egui::{Align2, Color32, Context, Frame, RichText, Window, epaint::Shadow};

/// User's response to the integrations dialog
#[derive(Debug, Clone, Default)]
pub struct IntegrationsResponse {
    /// User wants to install shaders
    pub install_shaders: bool,
    /// User wants to install shell integration
    pub install_shell_integration: bool,
    /// User clicked Skip (dismiss for this session)
    pub skipped: bool,
    /// User clicked Never Ask
    pub never_ask: bool,
    /// Dialog was closed
    pub closed: bool,
    /// User responded to shader overwrite prompt
    pub shader_conflict_action: Option<ShaderConflictAction>,
}

/// Action chosen when modified shaders are detected
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaderConflictAction {
    /// Overwrite modified bundled shaders
    Overwrite,
    /// Keep user-modified shaders (skip overwrite)
    SkipModified,
    /// Cancel the installation flow
    Cancel,
}

/// Combined integrations welcome dialog
pub struct IntegrationsUI {
    /// Whether the dialog is visible
    pub visible: bool,
    /// Whether shaders checkbox is checked
    pub shaders_checked: bool,
    /// Whether shell integration checkbox is checked
    pub shell_integration_checked: bool,
    /// Detected shell type
    pub detected_shell: ShellType,
    /// Whether installation is in progress
    pub installing: bool,
    /// Installation progress message
    pub progress_message: Option<String>,
    /// Installation error message
    pub error_message: Option<String>,
    /// Installation success message
    pub success_message: Option<String>,
    /// Whether we're waiting for user decision on modified shaders
    pub awaiting_shader_overwrite: bool,
    /// List of modified bundled shader files detected
    pub shader_conflicts: Vec<String>,
    /// Pending install request flags preserved while waiting for confirmation
    pub pending_install_shaders: bool,
    pub pending_install_shell_integration: bool,
}

impl IntegrationsUI {
    /// Create a new integrations UI
    pub fn new() -> Self {
        Self {
            visible: false,
            shaders_checked: true,
            shell_integration_checked: true,
            detected_shell: ShellType::detect(),
            installing: false,
            progress_message: None,
            error_message: None,
            success_message: None,
            awaiting_shader_overwrite: false,
            shader_conflicts: Vec::new(),
            pending_install_shaders: false,
            pending_install_shell_integration: false,
        }
    }

    /// Show the dialog
    pub fn show_dialog(&mut self) {
        self.visible = true;
        self.installing = false;
        self.progress_message = None;
        self.error_message = None;
        self.success_message = None;
        // Re-detect shell when showing dialog
        self.detected_shell = ShellType::detect();
        self.awaiting_shader_overwrite = false;
        self.shader_conflicts.clear();
        self.pending_install_shaders = false;
        self.pending_install_shell_integration = false;
    }

    /// Hide the dialog
    pub fn hide(&mut self) {
        self.visible = false;
    }

    /// Render the integrations dialog
    /// Returns the user's response
    pub fn show(&mut self, ctx: &Context) -> IntegrationsResponse {
        if !self.visible {
            return IntegrationsResponse::default();
        }

        let mut response = IntegrationsResponse::default();

        // Ensure dialog is fully opaque
        let mut style = (*ctx.style()).clone();
        let solid_bg = Color32::from_rgba_unmultiplied(32, 32, 32, 255);
        style.visuals.window_fill = solid_bg;
        style.visuals.panel_fill = solid_bg;
        style.visuals.widgets.noninteractive.bg_fill = solid_bg;
        ctx.set_style(style);

        let viewport = ctx.input(|i| i.viewport_rect());

        Window::new("Welcome to par-term")
            .resizable(false)
            .collapsible(false)
            .default_width(500.0)
            .default_pos(viewport.center())
            .pivot(Align2::CENTER_CENTER)
            .frame(
                Frame::window(&ctx.style())
                    .fill(solid_bg)
                    .inner_margin(24.0)
                    .stroke(egui::Stroke::new(1.0, Color32::from_gray(80)))
                    .shadow(Shadow {
                        offset: [4, 4],
                        blur: 16,
                        spread: 4,
                        color: Color32::from_black_alpha(180),
                    }),
            )
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    // Header with version
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(format!("Welcome to par-term v{}", env!("CARGO_PKG_VERSION")))
                            .size(22.0)
                            .strong(),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("A GPU-accelerated terminal emulator")
                            .size(14.0)
                            .weak(),
                    );
                    ui.add_space(4.0);
                    ui.hyperlink_to(
                        RichText::new("View Changelog").size(12.0),
                        "https://github.com/paulrobello/par-term/blob/main/CHANGELOG.md",
                    );
                    ui.add_space(16.0);
                });

                // Description
                ui.label("par-term includes optional integrations to enhance your experience:");
                ui.add_space(16.0);

                // Show installation progress/error/success
                if self.installing {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(
                            self.progress_message
                                .as_deref()
                                .unwrap_or("Installing..."),
                        );
                    });
                    ui.add_space(16.0);
                } else if let Some(error) = &self.error_message {
                    ui.colored_label(Color32::from_rgb(255, 100, 100), error);
                    ui.add_space(8.0);
                } else if let Some(success) = &self.success_message {
                    ui.colored_label(Color32::from_rgb(100, 255, 100), success);
                    ui.add_space(8.0);
                    ui.label("You can configure these in Settings (F12).");
                    ui.add_space(16.0);
                }

                // Checkboxes for integrations (only show when not installing/succeeded)
                if !self.installing && self.success_message.is_none() {
                    if self.awaiting_shader_overwrite {
                        ui.group(|ui| {
                            ui.vertical(|ui| {
                                ui.label(RichText::new("Modified shaders detected").strong());
                                if self.shader_conflicts.is_empty() {
                                    ui.label(
                                        RichText::new(
                                            "Some bundled shaders were modified. Overwrite or keep your versions?",
                                        )
                                        .weak(),
                                    );
                                } else {
                                    ui.label(RichText::new(
                                        format!(
                                            "{} modified files found. Overwrite them or keep your changes?",
                                            self.shader_conflicts.len()
                                        ),
                                    ));
                                    let preview: Vec<_> = self
                                        .shader_conflicts
                                        .iter()
                                        .take(5)
                                        .cloned()
                                        .collect();
                                    ui.label(
                                        RichText::new(preview.join(", "))
                                            .small()
                                            .weak(),
                                    );
                                    if self.shader_conflicts.len() > 5 {
                                        ui.label(
                                            RichText::new("…and more")
                                                .small()
                                                .weak(),
                                        );
                                    }
                                }
                            });
                        });
                        ui.add_space(16.0);
                    } else {
                        // Shaders checkbox with description
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut self.shaders_checked, "");
                                ui.vertical(|ui| {
                                    ui.label(RichText::new("Custom Shaders").strong());
                                    ui.label(
                                        RichText::new(
                                            "49+ background shaders (CRT, Matrix, plasma) and \
                                             12 cursor effects (trails, glows)",
                                        )
                                        .weak()
                                        .small(),
                                    );
                                });
                            });
                        });

                        ui.add_space(8.0);

                        // Shell integration checkbox with description
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut self.shell_integration_checked, "");
                                ui.vertical(|ui| {
                                    let shell_name = self.detected_shell.display_name();
                                    let label = if self.detected_shell == ShellType::Unknown {
                                        "Shell Integration".to_string()
                                    } else {
                                        format!("Shell Integration ({})", shell_name)
                                    };
                                    ui.label(RichText::new(label).strong());
                                    ui.label(
                                        RichText::new(
                                            "Current directory tracking, command markers, \
                                             and semantic prompt zones",
                                        )
                                        .weak()
                                        .small(),
                                    );
                                    if self.detected_shell == ShellType::Unknown {
                                        ui.label(
                                            RichText::new(
                                                "Note: Could not detect shell. Manual setup may be required.",
                                            )
                                            .weak()
                                            .italics()
                                            .small(),
                                        );
                                    }
                                });
                            });
                        });

                        ui.add_space(20.0);
                    }
                }

                // Buttons (centered)
                ui.vertical_centered(|ui| {
                    if !self.installing && self.success_message.is_none() {
                        ui.horizontal(|ui| {
                            let button_width = 130.0;

                            if self.awaiting_shader_overwrite {
                                if ui
                                    .add_sized(
                                        [button_width + 20.0, 32.0],
                                        egui::Button::new("Overwrite modified"),
                                    )
                                    .clicked()
                                {
                                    response.shader_conflict_action =
                                        Some(ShaderConflictAction::Overwrite);
                                }

                                ui.add_space(8.0);

                                if ui
                                    .add_sized(
                                        [button_width + 10.0, 32.0],
                                        egui::Button::new("Skip modified"),
                                    )
                                    .clicked()
                                {
                                    response.shader_conflict_action =
                                        Some(ShaderConflictAction::SkipModified);
                                }

                                ui.add_space(8.0);

                                if ui
                                    .add_sized([button_width, 32.0], egui::Button::new("Cancel"))
                                    .clicked()
                                {
                                    response.shader_conflict_action =
                                        Some(ShaderConflictAction::Cancel);
                                }
                            } else {
                                // Install Selected button (only if something is checked)
                                let can_install =
                                    self.shaders_checked || self.shell_integration_checked;
                                ui.add_enabled_ui(can_install, |ui| {
                                    if ui
                                        .add_sized(
                                            [button_width, 32.0],
                                            egui::Button::new("Install Selected"),
                                        )
                                        .clicked()
                                    {
                                        response.install_shaders = self.shaders_checked;
                                        response.install_shell_integration =
                                            self.shell_integration_checked;
                                    }
                                });

                                ui.add_space(8.0);

                                if ui
                                    .add_sized([button_width, 32.0], egui::Button::new("Skip"))
                                    .on_hover_text("Dismiss for this session")
                                    .clicked()
                                {
                                    response.skipped = true;
                                }

                                ui.add_space(8.0);

                                if ui
                                    .add_sized([button_width, 32.0], egui::Button::new("Never Ask"))
                                    .on_hover_text("Don't ask again for these integrations")
                                    .clicked()
                                {
                                    response.never_ask = true;
                                }
                            }
                        });
                    } else if self.success_message.is_some() {
                        // Show OK button after successful install
                        if ui
                            .add_sized([120.0, 32.0], egui::Button::new("OK"))
                            .clicked()
                        {
                            self.visible = false;
                            response.closed = true;
                        }
                    }
                });

                ui.add_space(12.0);

                // Help text
                if !self.installing
                    && self.success_message.is_none()
                    && self.error_message.is_none()
                {
                    ui.vertical_centered(|ui| {
                        let msg = if self.awaiting_shader_overwrite {
                            "Choose how to handle modified shaders to continue installation"
                        } else {
                            "You can install these later via CLI or Settings (F12)"
                        };
                        ui.label(RichText::new(msg).weak().small());
                    });
                }
            });

        response
    }

    /// Set installation in progress
    pub fn set_installing(&mut self, message: &str) {
        self.installing = true;
        self.progress_message = Some(message.to_string());
        self.error_message = None;
    }

    /// Set installation error
    pub fn set_error(&mut self, error: &str) {
        self.installing = false;
        self.progress_message = None;
        self.error_message = Some(error.to_string());
    }

    /// Set installation success
    pub fn set_success(&mut self, message: &str) {
        self.installing = false;
        self.progress_message = None;
        self.error_message = None;
        self.success_message = Some(message.to_string());
    }
}

impl Default for IntegrationsUI {
    fn default() -> Self {
        Self::new()
    }
}
