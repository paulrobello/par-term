//! Two-click confirmation for destructive buttons (UX bug B52).
//!
//! The first click arms the row, which then shows a confirm button and a
//! Cancel button in place of the original; only the confirm click acts. This
//! is the pattern the agent-launch and agent-command lists already used.
//!
//! The armed row lives in `SettingsUI::pending_list_delete` as
//! `(list, item key)`. Keying by the item's identity rather than its index
//! means a list that shifts under an armed row cannot delete the wrong item.

/// Armed-row slot: `(list name, item key)`.
pub type PendingDelete = Option<(&'static str, String)>;

const DANGER: egui::Color32 = egui::Color32::from_rgb(200, 80, 80);

/// Small red "Delete"-style row button. Returns `true` on the confirming
/// second click.
pub fn confirm_delete_button(
    ui: &mut egui::Ui,
    pending: &mut PendingDelete,
    list: &'static str,
    key: &str,
    label: &str,
) -> bool {
    confirm_button(ui, pending, list, key, label, true)
}

/// Full-size button for a destructive whole-list action (uninstall,
/// replace). Returns `true` on the confirming second click.
pub fn confirm_action_button(
    ui: &mut egui::Ui,
    pending: &mut PendingDelete,
    list: &'static str,
    key: &str,
    label: &str,
) -> bool {
    confirm_button(ui, pending, list, key, label, false)
}

fn confirm_button(
    ui: &mut egui::Ui,
    pending: &mut PendingDelete,
    list: &'static str,
    key: &str,
    label: &str,
    small: bool,
) -> bool {
    let armed = pending
        .as_ref()
        .is_some_and(|(l, k)| *l == list && k == key);
    let button = |ui: &mut egui::Ui, text: egui::RichText| {
        if small {
            ui.small_button(text)
        } else {
            ui.button(text)
        }
    };

    if !armed {
        if button(ui, egui::RichText::new(label).color(DANGER)).clicked() {
            *pending = Some((list, key.to_string()));
        }
        return false;
    }

    // Cancel is drawn first, so it lands where the original button was: an
    // accidental double click disarms the row instead of deleting.
    let mut confirmed = false;
    let cancel = button(ui, egui::RichText::new("Cancel")).clicked();
    if button(
        ui,
        egui::RichText::new(format!("Confirm {}", label.to_lowercase()))
            .color(DANGER)
            .strong(),
    )
    .clicked()
    {
        confirmed = true;
    }
    if cancel || confirmed {
        *pending = None;
    }
    confirmed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(pending: &mut PendingDelete, click: Option<egui::Pos2>, key: &str) -> bool {
        let ctx = egui::Context::default();
        let mut confirmed = false;
        let events = |pressed| {
            click
                .map(|pos| {
                    vec![
                        egui::Event::PointerMoved(pos),
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::default(),
                        },
                    ]
                })
                .unwrap_or_default()
        };
        let hover = || {
            click
                .map(|pos| vec![egui::Event::PointerMoved(pos)])
                .unwrap_or_default()
        };
        for evs in [hover(), hover(), events(true), events(false)] {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(400.0, 200.0),
                )),
                events: evs,
                ..Default::default()
            };
            let mut output = ctx.run_ui(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        confirmed |= confirm_delete_button(ui, pending, "rows", key, "Delete");
                    });
                });
            });
            output.textures_delta.clear();
        }
        confirmed
    }

    #[test]
    fn first_click_arms_second_click_confirms() {
        let mut pending = None;
        let first_button = egui::pos2(20.0, 14.0);

        assert!(!frame(&mut pending, Some(first_button), "a"));
        assert_eq!(pending, Some(("rows", "a".to_string())));

        // Armed layout: [Cancel][Confirm delete]. Clicking Cancel disarms.
        assert!(!frame(&mut pending, Some(first_button), "a"));
        assert_eq!(pending, None);
    }

    #[test]
    fn armed_row_confirms_on_the_confirm_button() {
        let mut pending = Some(("rows", "a".to_string()));
        // Cancel is ~50px wide at the left; the confirm button follows it.
        assert!(frame(&mut pending, Some(egui::pos2(100.0, 14.0)), "a"));
        assert_eq!(pending, None);
    }

    #[test]
    fn another_row_is_not_armed_by_a_different_key() {
        let mut pending = Some(("rows", "a".to_string()));
        assert!(!frame(&mut pending, None, "b"));
        assert_eq!(pending, Some(("rows", "a".to_string())));
    }
}
