//! Last-attached-tab close dialog for par-mux sessions (UX.md M1).
//!
//! Closing the last attached tab sends `kill-window` for the session's
//! only window, and the daemon deletes an emptied session — the close
//! would silently end work that lives in the daemon. The close is
//! therefore gated behind an explicit choice: Detach (the default — the
//! session keeps running), End session, or Cancel.

/// Action returned by the last-tab close dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MuxLastTabAction {
    /// Drop the attach connection; the session keeps running in the daemon.
    Detach,
    /// Kill every window and pane in the session (`kill-session`).
    EndSession,
    /// Keep the tab open.
    Cancel,
    /// No action yet (dialog still showing).
    None,
}

/// State for the par-mux last-tab close dialog.
pub struct MuxLastTabUI {
    /// Whether the dialog is visible
    visible: bool,
    /// The attached session's name, for the message
    session_name: String,
    /// Shared ConfirmDialog state (UX.md OV3)
    confirm: crate::app::overlay::confirm::ConfirmState,
}

impl Default for MuxLastTabUI {
    fn default() -> Self {
        Self::new()
    }
}

impl MuxLastTabUI {
    /// Create a new (hidden) last-tab close dialog
    pub fn new() -> Self {
        Self {
            visible: false,
            session_name: String::new(),
            confirm: Default::default(),
        }
    }

    /// Check if the dialog is currently visible
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Show the dialog for the last attached tab of `session_name`
    pub fn show_for_session(&mut self, session_name: &str) {
        self.visible = true;
        self.session_name = session_name.to_string();
    }

    /// Hide the dialog and clear state
    pub(crate) fn hide(&mut self) {
        self.visible = false;
        self.session_name.clear();
        self.confirm.reset();
    }

    /// Render the dialog and return any action.
    ///
    /// Drawn by the shared `ConfirmDialog` (UX.md OV3): Enter and Escape
    /// cancel (MD5 — the tab stays open, nothing ends), Detach is the
    /// middle choice, and End Session needs its own chord or its button.
    pub fn show(&mut self, ctx: &egui::Context) -> MuxLastTabAction {
        use crate::app::overlay::confirm::{self, ConfirmChoice, ConfirmSpec};

        if !self.visible {
            return MuxLastTabAction::None;
        }

        let body = [
            format!(
                "Closing it would end the par-mux session \"{}\" and every pane in it.",
                self.session_name
            ),
            "Detach keeps the session running in the daemon for later reattach.".to_string(),
        ];
        let spec = ConfirmSpec {
            title: "Close the Last Tab of the par-mux Session?",
            body: &body,
            detail: None,
            safe_label: "Cancel",
            alternate_label: Some("Detach (Keep Running)"),
            destructive_label: "End Session",
            offers_dont_ask_again: false,
        };
        let Some(answer) = confirm::show(ctx, &spec, &mut self.confirm) else {
            return MuxLastTabAction::None;
        };
        self.hide();
        match answer.choice {
            ConfirmChoice::Safe => MuxLastTabAction::Cancel,
            ConfirmChoice::Alternate => MuxLastTabAction::Detach,
            ConfirmChoice::Destructive => MuxLastTabAction::EndSession,
        }
    }
}

impl crate::traits::OverlayComponent for MuxLastTabUI {
    type Action = MuxLastTabAction;

    fn show(&mut self, ctx: &egui::Context) -> Self::Action {
        MuxLastTabUI::show(self, ctx)
    }

    fn is_visible(&self) -> bool {
        self.is_visible()
    }

    fn set_visible(&mut self, visible: bool) {
        if !visible {
            self.hide();
        }
        // Note: setting visible=true requires the session name — use
        // show_for_session(&name) to open this dialog.
    }
}

#[cfg(test)]
mod tests {
    use super::{MuxLastTabAction, MuxLastTabUI};

    /// Render one frame with an Enter key press under a headless egui
    /// context and return the action the dialog reported.
    fn show_with_enter(ui: &mut MuxLastTabUI) -> MuxLastTabAction {
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
    /// EndSession. egui also fake-clicks a focused button on Enter, so
    /// this must win over any same-frame button activation.
    #[test]
    fn enter_cancels_never_ends_session() {
        let mut ui = MuxLastTabUI::new();
        ui.show_for_session("work");
        assert_eq!(show_with_enter(&mut ui), MuxLastTabAction::Cancel);
        assert!(!ui.is_visible());
    }
}
