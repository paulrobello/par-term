//! The inline-edit contract (UX.md OV4, B31): Enter submits, Escape
//! cancels, and a click away **cancels** — never submits. Tab rename and
//! pane rename already follow it; this is the shared resolution for any
//! other inline field.

/// What an inline edit resolved to this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InlineEditOutcome {
    /// Still editing.
    Editing,
    /// Enter: apply the buffer.
    Submit,
    /// Escape or a click outside the field: discard the buffer.
    Cancel,
}

/// Resolve one frame of an inline text field. `response` is the field's
/// response; `first_frame` is true on the frame the edit opened (its
/// opening click must not count as a click away). Focus is requested only
/// on the first frame, so a click elsewhere can take it.
pub(crate) fn resolve(
    ui: &egui::Ui,
    response: &egui::Response,
    first_frame: bool,
) -> InlineEditOutcome {
    if first_frame {
        response.request_focus();
        return InlineEditOutcome::Editing;
    }
    let (enter, escape) = ui.input(|i| {
        (
            i.key_pressed(egui::Key::Enter),
            i.key_pressed(egui::Key::Escape),
        )
    });
    if escape {
        return InlineEditOutcome::Cancel;
    }
    if response.lost_focus() && enter {
        return InlineEditOutcome::Submit;
    }
    if response.clicked_elsewhere() || response.lost_focus() {
        return InlineEditOutcome::Cancel;
    }
    InlineEditOutcome::Editing
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run frames of a lone inline field; returns each frame's outcome.
    fn run(frames: Vec<Vec<egui::Event>>) -> Vec<InlineEditOutcome> {
        let ctx = egui::Context::default();
        let mut buffer = "name".to_string();
        let mut outcomes = Vec::new();
        for (n, events) in frames.into_iter().enumerate() {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(800.0, 600.0),
                    )),
                    events,
                    ..Default::default()
                },
                |ui| {
                    egui::CentralPanel::default().show(ui, |ui| {
                        let response = ui.add(egui::TextEdit::singleline(&mut buffer));
                        outcomes.push(resolve(ui, &response, n == 0));
                    });
                },
            );
            out.textures_delta.clear();
        }
        outcomes
    }

    fn key(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }
    }

    fn click(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        }
    }

    #[test]
    fn enter_submits() {
        let out = run(vec![vec![], vec![], vec![key(egui::Key::Enter)]]);
        assert_eq!(out.last(), Some(&InlineEditOutcome::Submit));
    }

    #[test]
    fn escape_cancels() {
        let out = run(vec![vec![], vec![], vec![key(egui::Key::Escape)]]);
        assert_eq!(out.last(), Some(&InlineEditOutcome::Cancel));
    }

    #[test]
    fn a_click_away_cancels_never_submits() {
        // B31: tab rename used to submit on click-away.
        let away = egui::pos2(700.0, 500.0);
        let out = run(vec![
            vec![],
            vec![],
            vec![egui::Event::PointerMoved(away)],
            vec![click(away, true)],
            vec![click(away, false)],
        ]);
        assert!(
            out.contains(&InlineEditOutcome::Cancel),
            "click-away must cancel: {out:?}"
        );
        assert!(!out.contains(&InlineEditOutcome::Submit), "never submit");
    }
}
