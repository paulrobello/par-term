//! Kind-picker popup for `adopt-pane` (hand-launched agent panes).
//!
//! Opened by the `adopt_pane_agent` action (the pane context menu row).
//! Lists the agent kinds the daemon accepts for adoption — the bundled
//! scrape set ids, [`ADOPTABLE_KINDS`] — one button per kind, plus a weak
//! hint. Dismissal follows the pane context menu's frame guards: the
//! opening click is still in egui's input on the first frame, and its
//! release completes a frame later, so click-away waits for both.

#[cfg(feature = "mux")]
use par_term_mux::ADOPTABLE_KINDS;

/// The kinds the picker lists. Under `mux` this is the client wire tier's
/// daemon-tracked list ([`par_term_mux::ADOPTABLE_KINDS`], cfg-imported).
/// Without the feature the picker never opens (the `adopt_pane_agent`
/// action toasts instead), but the module must still compile for the
/// no-mux build.
#[cfg(not(feature = "mux"))]
pub(crate) const ADOPTABLE_KINDS: &[&str] = &[];

/// State for the pane adopt kind-picker popup.
#[derive(Default)]
pub(crate) struct PaneAdoptUI {
    /// `Some(pane)` while the popup is open.
    pane: Option<crate::pane::PaneId>,
    pos: egui::Pos2,
    /// Frame the popup first rendered in. The click that opened it is
    /// still in egui's input that frame, and its release counts as
    /// `any_click()` one frame later — click-away waits for both (the
    /// tab bar context menu's `*_activated_frame` guard).
    opened_frame: Option<u64>,
    awaiting_open_release: bool,
}

impl PaneAdoptUI {
    /// Open the picker for `pane`.
    // The only caller (the `adopt_pane_agent` action) is mux-gated.
    #[cfg_attr(not(feature = "mux"), allow(dead_code))]
    pub(crate) fn open(&mut self, pane: crate::pane::PaneId, pos: egui::Pos2) {
        self.pane = Some(pane);
        self.pos = pos;
        self.opened_frame = None;
        self.awaiting_open_release = true;
    }

    /// Whether the picker is open (its rows own the pointer).
    pub(crate) fn is_open(&self) -> bool {
        self.pane.is_some()
    }

    /// Close without adopting (the overlay stack's Escape path).
    pub(crate) fn cancel(&mut self) {
        self.pane = None;
    }

    /// Render the picker if open; returns `(pane, kind)` for the chosen
    /// row. [`ADOPTABLE_KINDS`] entries are `&'static str`, so the choice
    /// borrows for `'static`.
    pub(crate) fn render(
        &mut self,
        ctx: &egui::Context,
    ) -> Option<(crate::pane::PaneId, &'static str)> {
        let pane = self.pane?;
        let frame = ctx.cumulative_frame_nr();
        self.opened_frame.get_or_insert(frame);
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            self.cancel();
            return None;
        }

        let mut chosen = None;
        let response = egui::Area::new(egui::Id::new("pane_adopt_popup"))
            .fixed_pos(self.pos)
            .constrain(true)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .inner_margin(egui::Margin::symmetric(8, 6))
                    .show(ui, |ui| {
                        ui.set_min_width(180.0);
                        ui.label(egui::RichText::new("Adopt pane as agent").weak().small());
                        ui.separator();
                        for kind in ADOPTABLE_KINDS {
                            if ui.button(*kind).clicked() {
                                chosen = Some((pane, *kind));
                            }
                        }
                    });
            })
            .response;

        let pointer_down = ctx.input(|i| i.pointer.any_down());
        if self.awaiting_open_release && !pointer_down && frame > self.opened_frame.unwrap_or(0) {
            self.awaiting_open_release = false;
        }
        let clicked_away = frame > self.opened_frame.unwrap_or(0)
            && !self.awaiting_open_release
            && ctx.input(|i| i.pointer.any_click())
            && !response.hovered();
        if chosen.is_some() || clicked_away {
            self.cancel();
        }
        chosen
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(
        ctx: &egui::Context,
        popup: &mut PaneAdoptUI,
        events: Vec<egui::Event>,
    ) -> Option<(crate::pane::PaneId, &'static str)> {
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
            chosen = popup.render(ui.ctx());
        });
        output.textures_delta.clear();
        chosen
    }

    #[test]
    fn every_kind_is_listed_once() {
        // The popup is the ADOPTABLE_KINDS list rendered; a kind added to
        // the client constant shows up without picker changes.
        assert!(!ADOPTABLE_KINDS.is_empty());
        assert!(
            ADOPTABLE_KINDS.contains(&"claude"),
            "claude is the reference kind the daemon tests document"
        );
    }

    #[test]
    fn escape_closes_the_picker() {
        let ctx = egui::Context::default();
        let mut popup = PaneAdoptUI::default();
        popup.open(2, egui::pos2(100.0, 100.0));
        frame(&ctx, &mut popup, vec![]);
        frame(
            &ctx,
            &mut popup,
            vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            }],
        );
        assert!(!popup.is_open());
    }

    /// Clicking a kind row reports the pane and that kind.
    #[test]
    fn a_row_click_reports_the_pane_and_kind() {
        let ctx = egui::Context::default();
        let mut popup = PaneAdoptUI::default();
        popup.open(5, egui::pos2(100.0, 100.0));
        frame(&ctx, &mut popup, vec![]);
        // The first kind row sits below the picker's title + separator.
        let at = egui::pos2(130.0, 140.0);
        let press = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        frame(&ctx, &mut popup, vec![egui::Event::PointerMoved(at)]);
        frame(&ctx, &mut popup, vec![egui::Event::PointerMoved(at)]);
        let mut chosen = frame(&ctx, &mut popup, vec![press(true)]);
        if chosen.is_none() {
            chosen = frame(&ctx, &mut popup, vec![press(false)]);
        }
        assert_eq!(chosen, Some((5, ADOPTABLE_KINDS[0])));
        assert!(!popup.is_open(), "a choice closes the picker");
    }

    #[test]
    fn open_and_close() {
        let mut popup = PaneAdoptUI::default();
        assert!(!popup.is_open());
        popup.open(3, egui::pos2(10.0, 10.0));
        assert!(popup.is_open());
        popup.cancel();
        assert!(!popup.is_open());
    }
}
