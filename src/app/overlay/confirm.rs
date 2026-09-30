//! The shared confirmation dialog (UX.md OV3, owner decision MD5).
//!
//! One component for every "are you sure?" dialog:
//!
//! - the **title names the object** ("Quit par-term?", "Close Pane?");
//! - the **body states the consequence**;
//! - the **primary button is the safe choice and the Enter default**;
//! - the **destructive button is red and needs its own key**, shown on the
//!   button (`Cmd+Backspace` on macOS, `Ctrl+Backspace` elsewhere) — never
//!   Enter, so a reflexive Enter cannot quit, close, or end a session;
//! - an optional **middle choice** (the par-mux last-tab dialog's Detach);
//! - **Escape cancels**;
//! - a **"Don't ask again" checkbox** where a setting exists.
//!
//! Key handling runs *after* the buttons: egui fake-clicks a focused
//! button on Enter, so a focused destructive button would otherwise take
//! its action on the same frame Enter is pressed. Resolving keys last lets
//! the safe choice win (B64).

use super::theme;
use egui::RichText;

/// What the user chose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConfirmChoice {
    /// The safe choice (Cancel / keep open). Enter and Escape land here.
    Safe,
    /// The middle choice, when the dialog has one (e.g. Detach).
    Alternate,
    /// The destructive choice (Quit, Close, End Session).
    Destructive,
}

/// What a dialog's answer carries: the choice plus the "don't ask again"
/// checkbox state (meaningful only for a non-safe choice).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ConfirmAnswer {
    pub(crate) choice: ConfirmChoice,
    pub(crate) dont_ask_again: bool,
}

/// The dialog's content. Built per frame by the owning dialog, so text can
/// follow live state.
pub(crate) struct ConfirmSpec<'a> {
    /// Names the object: "Quit par-term?", "Close Pane?".
    pub(crate) title: &'a str,
    /// Lines stating the consequence, most important first.
    pub(crate) body: &'a [String],
    /// An optional emphasized detail (the running command's name).
    pub(crate) detail: Option<&'a str>,
    /// Safe button label ("Cancel").
    pub(crate) safe_label: &'a str,
    /// Optional middle button label ("Detach (Keep Running)").
    pub(crate) alternate_label: Option<&'a str>,
    /// Destructive button label ("Quit", "Close Anyway", "End Session").
    pub(crate) destructive_label: &'a str,
    /// Offer "Don't ask again" — only where a setting backs it.
    pub(crate) offers_dont_ask_again: bool,
}

/// Per-dialog persistent state: the "don't ask again" checkbox.
#[derive(Debug, Default, Clone)]
pub(crate) struct ConfirmState {
    dont_ask_again: bool,
}

impl ConfirmState {
    /// Reset per open (the checkbox starts unticked every time).
    pub(crate) fn reset(&mut self) {
        self.dont_ask_again = false;
    }
}

/// The keys the dialog resolves, as read from one frame's input.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ConfirmKeys {
    pub(crate) enter: bool,
    pub(crate) escape: bool,
    /// The destructive action's own chord (Cmd/Ctrl+Backspace).
    pub(crate) destructive_chord: bool,
}

/// Resolve one frame: a button click (if any) and the keys pressed. Keys
/// win over a same-frame click (see the module docs), and among keys the
/// destructive chord needs its own modifier, so Enter and Escape always
/// resolve to the safe choice.
pub(crate) fn resolve(clicked: Option<ConfirmChoice>, keys: ConfirmKeys) -> Option<ConfirmChoice> {
    if keys.destructive_chord {
        return Some(ConfirmChoice::Destructive);
    }
    if keys.enter || keys.escape {
        return Some(ConfirmChoice::Safe);
    }
    clicked
}

/// The destructive chord's label, shown on the destructive button.
pub(crate) const DESTRUCTIVE_CHORD_LABEL: &str = if cfg!(target_os = "macos") {
    "Cmd+Backspace"
} else {
    "Ctrl+Backspace"
};

/// The modifier the destructive chord needs.
fn destructive_modifiers() -> egui::Modifiers {
    // egui's COMMAND is Cmd on macOS and Ctrl elsewhere.
    egui::Modifiers::COMMAND
}

/// Read this frame's confirm keys from egui input, consuming them so no
/// other widget (a focused button) acts on them too.
fn read_keys(ctx: &egui::Context) -> ConfirmKeys {
    ctx.input_mut(|i| ConfirmKeys {
        destructive_chord: i.consume_key(destructive_modifiers(), egui::Key::Backspace),
        escape: i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
        enter: i.key_pressed(egui::Key::Enter),
    })
}

/// Draw the dialog for one frame and return the answer, if any.
pub(crate) fn show(
    ctx: &egui::Context,
    spec: &ConfirmSpec<'_>,
    state: &mut ConfirmState,
) -> Option<ConfirmAnswer> {
    let mut clicked: Option<ConfirmChoice> = None;

    egui::Window::new(spec.title)
        .collapsible(false)
        .resizable(false)
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.set_max_width(theme::WIDTH_MEDIUM);
            ui.vertical_centered(|ui| {
                ui.add_space(8.0);
                for (i, line) in spec.body.iter().enumerate() {
                    let text = RichText::new(line);
                    ui.label(if i == 0 {
                        text
                    } else {
                        text.color(theme::TEXT_MUTED)
                    });
                    ui.add_space(4.0);
                }
                if let Some(detail) = spec.detail {
                    egui::Frame::new()
                        .fill(theme::DETAIL_FILL)
                        .inner_margin(egui::Margin::symmetric(12, 6))
                        .corner_radius(4.0)
                        .show(ui, |ui| {
                            ui.label(RichText::new(detail).color(theme::DETAIL_TEXT).monospace());
                        });
                    ui.add_space(4.0);
                }
                if spec.offers_dont_ask_again {
                    ui.checkbox(&mut state.dont_ask_again, "Don't ask again");
                }
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    let safe = egui::Button::new(
                        RichText::new(format!("{} (Enter)", spec.safe_label))
                            .color(theme::TEXT_ON_FILL),
                    )
                    .fill(theme::SAFE_FILL);
                    if ui.add(safe).clicked() {
                        clicked = Some(ConfirmChoice::Safe);
                    }
                    if let Some(label) = spec.alternate_label {
                        ui.add_space(8.0);
                        if ui.button(label).clicked() {
                            clicked = Some(ConfirmChoice::Alternate);
                        }
                    }
                    ui.add_space(8.0);
                    let destructive = egui::Button::new(
                        RichText::new(format!(
                            "{} ({DESTRUCTIVE_CHORD_LABEL})",
                            spec.destructive_label
                        ))
                        .color(theme::TEXT_ON_FILL),
                    )
                    .fill(theme::DESTRUCTIVE_FILL);
                    if ui.add(destructive).clicked() {
                        clicked = Some(ConfirmChoice::Destructive);
                    }
                });
                ui.add_space(6.0);
            });
        });

    let choice = resolve(clicked, read_keys(ctx))?;
    let answer = ConfirmAnswer {
        choice,
        dont_ask_again: state.dont_ask_again && choice != ConfirmChoice::Safe,
    };
    state.reset();
    Some(answer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_and_escape_resolve_to_the_safe_choice() {
        let enter = ConfirmKeys {
            enter: true,
            ..Default::default()
        };
        let escape = ConfirmKeys {
            escape: true,
            ..Default::default()
        };
        assert_eq!(resolve(None, enter), Some(ConfirmChoice::Safe));
        assert_eq!(resolve(None, escape), Some(ConfirmChoice::Safe));
    }

    #[test]
    fn enter_beats_a_same_frame_destructive_click() {
        // egui fake-clicks a focused button on Enter: a focused destructive
        // button would report a click on the frame Enter is pressed.
        let enter = ConfirmKeys {
            enter: true,
            ..Default::default()
        };
        assert_eq!(
            resolve(Some(ConfirmChoice::Destructive), enter),
            Some(ConfirmChoice::Safe)
        );
        assert_eq!(
            resolve(Some(ConfirmChoice::Alternate), enter),
            Some(ConfirmChoice::Safe)
        );
    }

    #[test]
    fn only_the_destructive_chord_or_its_button_is_destructive() {
        let chord = ConfirmKeys {
            destructive_chord: true,
            ..Default::default()
        };
        assert_eq!(resolve(None, chord), Some(ConfirmChoice::Destructive));
        assert_eq!(
            resolve(Some(ConfirmChoice::Destructive), ConfirmKeys::default()),
            Some(ConfirmChoice::Destructive)
        );
        assert_eq!(resolve(None, ConfirmKeys::default()), None);
    }

    fn frame(
        ctx: &egui::Context,
        state: &mut ConfirmState,
        events: Vec<egui::Event>,
    ) -> Option<ConfirmAnswer> {
        let spec = ConfirmSpec {
            title: "Close Pane?",
            body: &["The pane is running a command.".to_string()],
            detail: Some("sleep 100"),
            safe_label: "Cancel",
            alternate_label: None,
            destructive_label: "Close Anyway",
            offers_dont_ask_again: true,
        };
        let mut answer = None;
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1200.0, 800.0),
                )),
                events,
                ..Default::default()
            },
            |ui| answer = show(ui.ctx(), &spec, state),
        );
        out.textures_delta.clear();
        answer
    }

    fn key(key: egui::Key, modifiers: egui::Modifiers) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    #[test]
    fn a_focused_destructive_button_still_yields_safe_on_enter() {
        // Tab focus onto the destructive button, then Enter: egui's
        // fake-click would activate it, but the dialog resolves Safe.
        let ctx = egui::Context::default();
        let mut state = ConfirmState::default();
        let _ = frame(&ctx, &mut state, vec![]);
        // Tab through: checkbox, safe, destructive.
        for _ in 0..3 {
            let answer = frame(
                &ctx,
                &mut state,
                vec![key(egui::Key::Tab, egui::Modifiers::NONE)],
            );
            assert_eq!(answer, None, "Tab alone never answers");
        }
        let answer = frame(
            &ctx,
            &mut state,
            vec![key(egui::Key::Enter, egui::Modifiers::NONE)],
        );
        assert_eq!(
            answer.map(|a| a.choice),
            Some(ConfirmChoice::Safe),
            "Enter on a focused destructive button must stay safe"
        );
    }

    #[test]
    fn the_destructive_chord_answers_destructive_with_dont_ask_again() {
        let ctx = egui::Context::default();
        let mut state = ConfirmState {
            dont_ask_again: true,
        };
        let answer = frame(
            &ctx,
            &mut state,
            vec![key(egui::Key::Backspace, egui::Modifiers::COMMAND)],
        );
        assert_eq!(
            answer,
            Some(ConfirmAnswer {
                choice: ConfirmChoice::Destructive,
                dont_ask_again: true,
            })
        );
    }

    #[test]
    fn a_plain_backspace_is_not_destructive() {
        let ctx = egui::Context::default();
        let mut state = ConfirmState::default();
        let answer = frame(
            &ctx,
            &mut state,
            vec![key(egui::Key::Backspace, egui::Modifiers::NONE)],
        );
        assert_eq!(answer, None);
    }

    #[test]
    fn a_safe_answer_never_carries_dont_ask_again() {
        let ctx = egui::Context::default();
        let mut state = ConfirmState {
            dont_ask_again: true,
        };
        let answer = frame(
            &ctx,
            &mut state,
            vec![key(egui::Key::Escape, egui::Modifiers::NONE)],
        );
        assert_eq!(
            answer,
            Some(ConfirmAnswer {
                choice: ConfirmChoice::Safe,
                dont_ask_again: false,
            })
        );
    }
}
