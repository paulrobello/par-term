//! Right-click menu in a pane body (UX.md V10): split, zoom, rename, move,
//! broadcast, restart, and close, when the program in the pane is not
//! capturing the mouse. Each row runs a registry action on the pane it was
//! opened on and shows that action's live chord.
//!
//! Dismissal follows the pane rename popup's frame guards: the opening
//! click is still in egui's input on the first frame, and its release
//! completes a frame later, so click-away waits for both.

use crate::pane::PaneId;

/// The rows, in display order: `(action id, label)`; `None` is a separator.
pub(crate) const PANE_MENU_ROWS: &[Option<(&str, &str)>] = &[
    Some(("split_right", "Split Right")),
    Some(("split_down", "Split Down")),
    None,
    Some(("toggle_pane_zoom", "Zoom Pane")),
    Some(("equalize_panes", "Equalize Panes")),
    None,
    Some(("rename_pane", "Rename Pane...")),
    Some(("promote_pane_to_tab", "Move Pane to New Tab")),
    Some(("toggle_pane_broadcast", "Toggle Broadcast for This Pane")),
    Some(("restart_pane", "Restart Pane Process")),
    None,
    Some(("close_pane", "Close Pane")),
];

/// The pane context menu's state.
#[derive(Default)]
pub(crate) struct PaneContextMenuUI {
    pane: Option<PaneId>,
    pos: egui::Pos2,
    opened_frame: Option<u64>,
    awaiting_open_release: bool,
}

impl PaneContextMenuUI {
    pub(crate) fn open(&mut self, pane: PaneId, pos: egui::Pos2) {
        self.pane = Some(pane);
        self.pos = pos;
        self.opened_frame = None;
        self.awaiting_open_release = true;
    }

    pub(crate) fn close(&mut self) {
        self.pane = None;
    }

    pub(crate) fn is_open(&self) -> bool {
        self.pane.is_some()
    }

    /// Draw the menu; returns `(pane, action id)` for the chosen row.
    /// `chord_for` gives each action's live chord text.
    pub(crate) fn render(
        &mut self,
        ctx: &egui::Context,
        chord_for: &dyn Fn(&str) -> Option<String>,
    ) -> Option<(PaneId, &'static str)> {
        let pane = self.pane?;
        let frame = ctx.cumulative_frame_nr();
        let opened = *self.opened_frame.get_or_insert(frame);
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.close();
            return None;
        }
        let mut chosen = None;
        let response = egui::Area::new(egui::Id::new("pane_context_menu"))
            .fixed_pos(self.pos)
            .constrain(true)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_min_width(220.0);
                    for row in PANE_MENU_ROWS {
                        let Some((id, label)) = row else {
                            ui.separator();
                            continue;
                        };
                        ui.horizontal(|ui| {
                            if ui.add(egui::Button::new(*label).frame(false)).clicked() {
                                chosen = Some((pane, *id));
                            }
                            if let Some(chord) = chord_for(id) {
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| ui.weak(chord),
                                );
                            }
                        });
                    }
                });
            })
            .response;

        let pointer_down = ctx.input(|i| i.pointer.any_down());
        if self.awaiting_open_release && !pointer_down && frame > opened {
            self.awaiting_open_release = false;
        }
        let clicked_away = frame > opened
            && !self.awaiting_open_release
            && ctx.input(|i| i.pointer.any_click())
            && !response.hovered();
        if chosen.is_some() || clicked_away {
            self.close();
        }
        chosen
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every row's action is dispatchable: the menu never offers a dead item.
    #[test]
    fn every_row_runs_a_dispatchable_action() {
        use crate::app::input_events::keybinding_actions::ACTION_HANDLERS;
        for (id, _) in PANE_MENU_ROWS.iter().flatten() {
            assert!(
                ACTION_HANDLERS.iter().any(|(name, _)| name == id),
                "{id} has no handler"
            );
        }
    }

    fn frame(
        ctx: &egui::Context,
        menu: &mut PaneContextMenuUI,
        events: Vec<egui::Event>,
    ) -> Option<(PaneId, &'static str)> {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200.0, 800.0),
            )),
            events,
            ..Default::default()
        };
        let mut chosen = None;
        let mut output = ctx.run_ui(input, |ui| {
            chosen = menu.render(ui.ctx(), &|_| None);
        });
        output.textures_delta.clear();
        chosen
    }

    #[test]
    fn escape_closes_the_menu() {
        let ctx = egui::Context::default();
        let mut menu = PaneContextMenuUI::default();
        menu.open(2, egui::pos2(100.0, 100.0));
        frame(&ctx, &mut menu, vec![]);
        frame(
            &ctx,
            &mut menu,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        assert!(!menu.is_open());
    }

    /// Clicking the first row reports its action for the menu's pane.
    #[test]
    fn a_row_click_reports_the_action_for_its_pane() {
        let ctx = egui::Context::default();
        let mut menu = PaneContextMenuUI::default();
        menu.open(5, egui::pos2(100.0, 100.0));
        frame(&ctx, &mut menu, vec![]);
        // The first row sits just inside the popup's top-left corner.
        let at = egui::pos2(130.0, 112.0);
        let press = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        frame(&ctx, &mut menu, vec![egui::Event::PointerMoved(at)]);
        frame(&ctx, &mut menu, vec![egui::Event::PointerMoved(at)]);
        let mut chosen = frame(&ctx, &mut menu, vec![press(true)]);
        if chosen.is_none() {
            chosen = frame(&ctx, &mut menu, vec![press(false)]);
        }
        assert_eq!(chosen, Some((5, "split_right")));
        assert!(!menu.is_open(), "a choice closes the menu");
    }

    #[test]
    fn open_and_close() {
        let mut menu = PaneContextMenuUI::default();
        assert!(!menu.is_open());
        menu.open(3, egui::pos2(10.0, 10.0));
        assert!(menu.is_open());
        menu.close();
        assert!(!menu.is_open());
    }
}
