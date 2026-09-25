//! Inline rename popup for pane titles.
//!
//! Opened by right-clicking a pane title bar or by the `rename_pane`
//! action on the focused pane. Mirrors the tab bar's inline rename row
//! (tab_bar_ui/context_menu.rs): single-line TextEdit, Enter submits,
//! Escape cancels, a click outside closes, and leaving the field blank
//! reverts the pane to automatic titles.

use crate::pane::PaneId;

/// What the popup produced this frame.
#[derive(Debug, PartialEq)]
pub(crate) enum PaneRenameOutcome {
    /// Enter pressed: apply this name (possibly empty — empty reverts).
    Submit {
        pane_id: PaneId,
        name: String,
    },
    None,
}

/// State for the pane-title rename popup.
pub(crate) struct PaneRenameUI {
    /// `Some(pane)` while the popup is open.
    pane_id: Option<PaneId>,
    buffer: String,
    /// Popup origin in logical (egui) coordinates.
    pos: egui::Pos2,
    /// Frame the popup first rendered in. The click that opened it is still
    /// in egui's input that frame, so click-away dismissal must wait for a
    /// later frame — the tab bar context menu's `*_activated_frame` guard.
    opened_frame: Option<u64>,
    /// The opening click's RELEASE still completes one frame later and
    /// counts as `any_click()`; click-away waits for the pointer to come
    /// up once first, or the popup would close the instant the user
    /// releases the button that opened it.
    awaiting_open_release: bool,
}

impl Default for PaneRenameUI {
    fn default() -> Self {
        Self {
            pane_id: None,
            buffer: String::new(),
            pos: egui::Pos2::ZERO,
            opened_frame: None,
            awaiting_open_release: false,
        }
    }
}

impl PaneRenameUI {
    /// Open the popup for `pane_id`, seeding the buffer with its current
    /// title so a rename starts from what is shown.
    pub(crate) fn open(&mut self, pane_id: PaneId, current_title: &str, pos: egui::Pos2) {
        self.pane_id = Some(pane_id);
        self.buffer = current_title.to_string();
        self.pos = pos;
        self.opened_frame = None;
        self.awaiting_open_release = true;
    }

    fn close(&mut self) {
        self.pane_id = None;
        self.opened_frame = None;
    }

    /// Render the popup if open. Runs inside the egui pass; the returned
    /// outcome is applied by the caller after the egui borrow ends.
    pub(crate) fn render(&mut self, ctx: &egui::Context) -> PaneRenameOutcome {
        let Some(pane_id) = self.pane_id else {
            return PaneRenameOutcome::None;
        };

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.close();
            return PaneRenameOutcome::None;
        }

        let mut outcome = PaneRenameOutcome::None;
        let pos = self.pos;
        let buffer = &mut self.buffer;

        let area = egui::Area::new(egui::Id::new("pane_rename_popup"))
            .fixed_pos(pos)
            .constrain(true)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .inner_margin(egui::Margin::symmetric(8, 6))
                    .show(ui, |ui| {
                        ui.set_min_width(200.0);
                        ui.horizontal(|ui| {
                            let response =
                                ui.add(egui::TextEdit::singleline(buffer).hint_text("Pane name"));
                            if !response.has_focus() {
                                response.request_focus();
                            }
                            if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                outcome = PaneRenameOutcome::Submit {
                                    pane_id,
                                    name: buffer.trim().to_string(),
                                };
                            }
                        });
                        ui.label(
                            egui::RichText::new("Leave blank to use auto title")
                                .weak()
                                .small(),
                        );
                    })
            });

        // Click-away dismissal, guarded against the opening click: its
        // press shares the popup's first rendered frame, and its release
        // completes a frame later as any_click() — both must pass before a
        // real click can close the popup.
        let frame = ctx.cumulative_frame_nr();
        if self.opened_frame.is_none() {
            self.opened_frame = Some(frame);
        }
        if self.awaiting_open_release {
            if !ctx.input(|i| i.pointer.any_down()) {
                self.awaiting_open_release = false;
            }
        } else if frame > self.opened_frame.unwrap_or(u64::MAX)
            && ctx.input(|i| i.pointer.any_click())
            && !area.response.clicked()
        {
            self.close();
        }

        if outcome != PaneRenameOutcome::None {
            self.close();
        }
        outcome
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_seeds_the_buffer() {
        let mut ui = PaneRenameUI::default();
        ui.open(7, "build box", egui::Pos2::ZERO);
        assert_eq!(ui.pane_id, Some(7));
        assert_eq!(ui.buffer, "build box");
    }
}
