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
    /// Shared ConfirmDialog state (UX.md OV3)
    confirm: crate::app::overlay::confirm::ConfirmState,
    /// "Don't ask again" was ticked on the last Close answer
    dont_ask_again_chosen: bool,
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
            confirm: Default::default(),
            dont_ask_again_chosen: false,
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
        self.confirm.reset();
    }

    /// Render the dialog and return any action.
    ///
    /// Drawn by the shared `ConfirmDialog` (UX.md OV3): the title names the
    /// object being closed ("Close Pane?" — the old title said "Close Tab?"
    /// for panes too), Enter and Escape cancel (MD5), and Close needs its
    /// own chord or its button. "Don't ask again" turns off
    /// `confirm_close_running_jobs`.
    pub fn show(&mut self, ctx: &egui::Context) -> CloseConfirmAction {
        use crate::app::overlay::confirm::{self, ConfirmChoice, ConfirmSpec};

        if !self.visible {
            return CloseConfirmAction::None;
        }

        let (title, target) = if self.pending_pane_id.is_some() {
            ("Close Pane?", "pane")
        } else {
            ("Close Tab?", "tab")
        };
        let body = [
            format!("The {target} \"{}\" has a running command:", self.tab_title),
            "Closing will terminate this process.".to_string(),
        ];
        let spec = ConfirmSpec {
            title,
            body: &body,
            detail: Some(&self.command_name),
            safe_label: "Cancel",
            alternate_label: None,
            destructive_label: "Close Anyway",
            offers_dont_ask_again: true,
        };
        let Some(answer) = confirm::show(ctx, &spec, &mut self.confirm) else {
            return CloseConfirmAction::None;
        };
        self.dont_ask_again_chosen = answer.dont_ask_again;
        let action = match (answer.choice, self.pending_tab_id) {
            (ConfirmChoice::Destructive, Some(tab_id)) => CloseConfirmAction::Close {
                tab_id,
                pane_id: self.pending_pane_id,
            },
            _ => CloseConfirmAction::Cancel,
        };
        self.hide();
        action
    }

    /// Whether the last answer ticked "Don't ask again" (read once by the
    /// caller after a Close, then cleared).
    pub(crate) fn take_dont_ask_again(&mut self) -> bool {
        std::mem::take(&mut self.dont_ask_again_chosen)
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
