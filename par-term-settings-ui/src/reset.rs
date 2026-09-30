//! Per-control "reset to default" (UX.md SS4).
//!
//! A small ↺ after a control appears only while its value differs from the
//! default; clicking it restores the default as an ordinary edit, so Save
//! persists it and Revert undoes it.

use std::cell::RefCell;

use par_term_config::Config;

use crate::SettingsUI;

thread_local! {
    /// `Config::default()`, built once per UI thread: it is large and every
    /// button reads it every frame. Mutable only because field accessors
    /// take `&mut Config`; nothing writes to it.
    static DEFAULT: RefCell<Config> = RefCell::new(Config::default());
}

/// Draw ↺ after a control when `field(config)` differs from its default;
/// a click writes the default back and marks Settings changed.
///
/// `field` selects the value from a config, so the same accessor reads the
/// working config and the default one.
pub fn reset_button<T: PartialEq + Clone>(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    changes_this_frame: &mut bool,
    field: impl Fn(&mut Config) -> &mut T,
) {
    let default = DEFAULT.with(|d| field(&mut d.borrow_mut()).clone());
    let differs = *field(&mut settings.config) != default;
    #[cfg(test)]
    record(differs);
    if differs
        && ui
            .small_button("\u{21BA}")
            .on_hover_text("Reset to default")
            .clicked()
    {
        *field(&mut settings.config) = default;
        settings.has_changes = true;
        *changes_this_frame = true;
    }
}

#[cfg(test)]
thread_local! {
    static SHOWN: std::cell::RefCell<Vec<bool>> = const { std::cell::RefCell::new(Vec::new()) };
}

#[cfg(test)]
fn record(differs: bool) {
    SHOWN.with(|s| s.borrow_mut().push(differs));
}

#[cfg(test)]
pub(crate) fn take_shown() -> Vec<bool> {
    SHOWN.with(|s| std::mem::take(&mut *s.borrow_mut()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(settings: &mut SettingsUI, changes: &mut bool) {
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                reset_button(ui, settings, changes, |c| &mut c.window.window_opacity);
            });
        });
        output.textures_delta.clear();
    }

    #[test]
    fn reset_shows_only_when_the_value_differs_from_default() {
        let mut settings = SettingsUI::new_for_tests(Config::default());
        let mut changes = false;
        take_shown();

        frame(&mut settings, &mut changes);
        assert_eq!(take_shown(), [false], "default value: no ↺");

        settings.config.window.window_opacity = 0.42;
        frame(&mut settings, &mut changes);
        assert_eq!(take_shown(), [true], "changed value: ↺ shown");
    }

    #[test]
    fn clicking_reset_restores_the_default_as_an_edit() {
        let mut settings = SettingsUI::new_for_tests(Config::default());
        settings.config.window.window_opacity = 0.42;
        let mut changes = false;
        // Lay out once, then click where the ↺ was drawn.
        let ctx = egui::Context::default();
        fn run(
            ctx: &egui::Context,
            input: egui::RawInput,
            settings: &mut SettingsUI,
            changes: &mut bool,
        ) -> egui::Rect {
            let mut rect = egui::Rect::NOTHING;
            let mut output = ctx.run_ui(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    reset_button(ui, settings, changes, |c| &mut c.window.window_opacity);
                    rect = ui.min_rect();
                });
            });
            output.textures_delta.clear();
            rect
        }
        let rect = run(&ctx, egui::RawInput::default(), &mut settings, &mut changes);
        let at = rect.left_top() + egui::vec2(6.0, 6.0);
        for event in [
            egui::Event::PointerMoved(at),
            egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ] {
            let input = egui::RawInput {
                events: vec![event],
                ..Default::default()
            };
            run(&ctx, input, &mut settings, &mut changes);
        }

        assert_eq!(
            settings.config.window.window_opacity,
            Config::default().window.window_opacity
        );
        assert!(changes && settings.has_changes);
    }
}
