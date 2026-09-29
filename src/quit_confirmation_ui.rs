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
    /// The attached par-mux session, if any — its survival changes what
    /// the dialog may truthfully claim (UX.md M9)
    mux_session: Option<String>,
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
            mux_session: None,
        }
    }

    /// Check if the dialog is currently visible
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Show the confirmation dialog with the number of active sessions
    /// and, when a par-mux session is attached, its name (UX.md M9: the
    /// dialog must state that the daemon session survives the quit).
    pub fn show_confirmation(&mut self, session_count: usize, mux_session: Option<&str>) {
        self.visible = true;
        self.session_count = session_count;
        self.mux_session = mux_session.map(str::to_string);
    }

    /// Hide the dialog and clear state
    pub(crate) fn hide(&mut self) {
        self.visible = false;
        self.session_count = 0;
        self.mux_session = None;
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

                    let (session_text, closing_text) =
                        summary_lines(self.session_count, self.mux_session.as_deref());
                    ui.label(&session_text);
                    ui.add_space(5.0);

                    ui.label(egui::RichText::new(closing_text).color(egui::Color32::GRAY));
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

        // MD5: Enter is the safe choice — it cancels like Escape. Quit
        // stays on its own button (B64 mapped Enter to Quit).
        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            action = QuitConfirmAction::Cancel;
        }

        // Hide dialog on any action
        if !matches!(action, QuitConfirmAction::None) {
            self.hide();
        }

        action
    }
}

impl crate::traits::OverlayComponent for QuitConfirmationUI {
    type Action = QuitConfirmAction;

    fn show(&mut self, ctx: &egui::Context) -> Self::Action {
        QuitConfirmationUI::show(self, ctx)
    }

    fn is_visible(&self) -> bool {
        self.is_visible()
    }

    fn set_visible(&mut self, visible: bool) {
        if !visible {
            self.hide();
        }
        // Note: setting visible=true requires session context.
        // Use show_confirmation(session_count, mux_session) to open this dialog.
    }
}

/// UX.md M9: the dialog's two body lines, matched to what quitting
/// actually does. A local-only quit terminates everything, but an
/// attached par-mux session survives the quit (the detach drops the
/// client, the daemon keeps running) — "All sessions will be
/// terminated" was false whenever one was attached.
fn summary_lines(session_count: usize, mux_session: Option<&str>) -> (String, String) {
    match mux_session {
        Some(name) => {
            let first = if session_count == 1 {
                "There is 1 open tab.".to_string()
            } else {
                format!("There are {session_count} open tabs.")
            };
            (
                first,
                format!("par-mux session '{name}' will keep running (it detaches)."),
            )
        }
        None => {
            let first = if session_count == 1 {
                "There is 1 active session.".to_string()
            } else {
                format!("There are {session_count} active sessions.")
            };
            (first, "All sessions will be terminated.".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{QuitConfirmAction, QuitConfirmationUI, summary_lines};

    /// Render one frame with an Enter key press under a headless egui
    /// context and return the action the dialog reported.
    fn show_with_enter(ui: &mut QuitConfirmationUI) -> QuitConfirmAction {
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

    /// MD5: Enter is the safe choice everywhere — in the quit dialog it
    /// must cancel, never quit. B64 mapped Enter to Quit.
    #[test]
    fn enter_cancels_never_quits() {
        let mut ui = QuitConfirmationUI::new();
        ui.show_confirmation(2, None);
        assert_eq!(show_with_enter(&mut ui), QuitConfirmAction::Cancel);
        assert!(!ui.is_visible());
    }

    /// UX.md M9: with a par-mux session attached, quitting detaches —
    /// the dialog must say the session survives, never "terminated".
    #[test]
    fn mux_attach_summary_claims_survival_not_termination() {
        let (count_line, closing_line) = summary_lines(3, Some("work"));
        assert_eq!(count_line, "There are 3 open tabs.");
        assert_eq!(
            closing_line,
            "par-mux session 'work' will keep running (it detaches)."
        );
    }

    /// The local-only quit keeps its original, truthful wording.
    #[test]
    fn local_only_summary_still_claims_termination() {
        let (count_line, closing_line) = summary_lines(1, None);
        assert_eq!(count_line, "There is 1 active session.");
        assert_eq!(closing_line, "All sessions will be terminated.");
        let (count_line, _) = summary_lines(4, None);
        assert_eq!(count_line, "There are 4 active sessions.");
    }
}
