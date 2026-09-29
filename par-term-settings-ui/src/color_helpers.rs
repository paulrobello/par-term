//! Color pickers, one per storage type (UX rule SC3).
//!
//! - [`rgb_color_button`] edits an opaque `[u8; 3]` and shows no alpha control.
//! - [`rgba_color_button`] edits an unmultiplied `[u8; 4]` and preserves alpha.
//! - [`rgba_f32_color_button`] edits an unmultiplied `[f32; 4]` (0.0-1.0).
//!
//! egui's `Ui::color_edit_button_srgba` defaults to an additive-capable alpha
//! control, which shows alpha on fields that cannot store it, and
//! `Alpha::Opaque` forces alpha to 1.0 as soon as the popup opens. Neither
//! may be used on config fields; go through these helpers instead.

/// Opaque RGB picker for a `[u8; 3]` field.
pub fn rgb_color_button(ui: &mut egui::Ui, rgb: &mut [u8; 3]) -> egui::Response {
    ui.color_edit_button_srgb(rgb)
}

/// Alpha-preserving picker for an unmultiplied `[u8; 4]` field.
///
/// The field is written only when the user changes the color, so opening
/// and closing the picker never alters the stored value.
pub fn rgba_color_button(ui: &mut egui::Ui, rgba: &mut [u8; 4]) -> egui::Response {
    let mut edit = *rgba;
    let response = ui.color_edit_button_srgba_unmultiplied(&mut edit);
    if response.changed() {
        *rgba = edit;
    }
    response
}

/// Alpha-preserving picker for an unmultiplied `[f32; 4]` field in 0.0-1.0.
///
/// Written back only on change, so an untouched value keeps its full
/// precision instead of being quantized to 8 bits.
pub fn rgba_f32_color_button(ui: &mut egui::Ui, rgba: &mut [f32; 4]) -> egui::Response {
    let mut bytes = rgba.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
    let response = rgba_color_button(ui, &mut bytes);
    if response.changed() {
        *rgba = par_term_config::color_u8x4_to_f32(bytes);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pass(ctx: &egui::Context, events: Vec<egui::Event>, show: &mut dyn FnMut(&mut egui::Ui)) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(800.0, 600.0),
            )),
            events,
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| show(ui));
        });
        output.textures_delta.clear();
    }

    /// Render `show` (whose first widget is a color button at the panel's
    /// top-left), click that button, and render a few more frames with the
    /// picker popup open.
    fn click_open_and_idle(show: &mut dyn FnMut(&mut egui::Ui)) {
        let ctx = egui::Context::default();
        let at = egui::pos2(20.0, 18.0);
        let hover = || vec![egui::Event::PointerMoved(at)];
        let button = |pressed| {
            vec![
                egui::Event::PointerMoved(at),
                egui::Event::PointerButton {
                    pos: at,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::default(),
                },
            ]
        };
        pass(&ctx, hover(), show);
        pass(&ctx, hover(), show);
        pass(&ctx, button(true), show);
        pass(&ctx, button(false), show);
        for _ in 0..3 {
            pass(&ctx, hover(), show);
        }
    }

    /// Positive control: the click sequence really opens the popup, which is
    /// the moment `Alpha::Opaque` overwrites alpha (UX bug B44).
    #[test]
    fn click_sequence_opens_the_picker() {
        let mut color = egui::Color32::from_rgba_unmultiplied(10, 20, 30, 100);
        click_open_and_idle(&mut |ui| {
            egui::color_picker::color_edit_button_srgba(
                ui,
                &mut color,
                egui::color_picker::Alpha::Opaque,
            );
        });
        assert_eq!(
            color.a(),
            255,
            "opening an Opaque picker forces alpha to 1.0"
        );
    }

    #[test]
    fn rgba_f32_picker_keeps_alpha_when_opened() {
        let original = [0.25, 0.5, 0.75, 0.4];
        let mut value = original;
        click_open_and_idle(&mut |ui| {
            rgba_f32_color_button(ui, &mut value);
        });
        assert_eq!(
            value, original,
            "opening the picker must not change the field"
        );
    }

    #[test]
    fn rgba_picker_keeps_alpha_when_opened() {
        let original = [255, 255, 0, 80];
        let mut value = original;
        click_open_and_idle(&mut |ui| {
            rgba_color_button(ui, &mut value);
        });
        assert_eq!(value, original);
    }

    #[test]
    fn rgb_picker_round_trips() {
        let original = [12, 34, 56];
        let mut value = original;
        click_open_and_idle(&mut |ui| {
            rgb_color_button(ui, &mut value);
        });
        assert_eq!(value, original);
    }

    /// SC3 gate: the method form `ui.color_edit_button_srgba(..)` shows an
    /// additive alpha control and must not be used on config fields.
    #[test]
    fn no_default_alpha_color_buttons_remain() {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut offenders = Vec::new();
        let mut stack = vec![src];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir).expect("read src dir").flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|e| e == "rs")
                    && !path.ends_with("color_helpers.rs")
                {
                    let text = std::fs::read_to_string(&path).expect("read source");
                    for (n, line) in text.lines().enumerate() {
                        if line.contains(".color_edit_button_srgba(")
                            || line.contains("Alpha::BlendOrAdditive")
                        {
                            offenders.push(format!("{}:{}", path.display(), n + 1));
                        }
                    }
                }
            }
        }
        assert!(
            offenders.is_empty(),
            "use crate::color_helpers instead: {offenders:#?}"
        );
    }
}
