//! Shared overlay look (UX.md OV1): widths come from one small set and
//! colours from one set of tokens, so dialogs, pickers, toasts, and panels
//! line up instead of each picking its own.
//!
//! Surfaces read these tokens instead of spelling colour literals. The
//! `overlay_surfaces_take_colours_from_the_theme` test pins the migrated
//! files to that rule; content colours with their own meaning (command
//! history's exit-status dots, fuzzy-match highlights) stay local.

use egui::Color32;

/// Narrow overlays: toasts, small choosers.
pub(crate) const WIDTH_SMALL: f32 = 360.0;
/// Dialogs and most pickers.
pub(crate) const WIDTH_MEDIUM: f32 = 520.0;
/// Wide pickers (the command palette with its chord column).
pub(crate) const WIDTH_LARGE: f32 = 640.0;

/// Opaque fill behind panels that must stay readable over any terminal
/// opacity (help, search, the tmux picker, shader install, integrations).
pub(crate) const PANEL_FILL: Color32 = Color32::from_rgb(30, 30, 30);
/// A panel frame's border.
pub(crate) const PANEL_BORDER: Color32 = Color32::from_gray(80);
/// Toast background: the panel fill, slightly translucent.
pub(crate) const TOAST_FILL: Color32 = Color32::from_rgba_unmultiplied_const(30, 30, 30, 240);
/// Mode banner background (action prefix, resize mode, demote pick).
pub(crate) const BANNER_FILL: Color32 = Color32::from_rgba_unmultiplied_const(20, 40, 70, 240);
/// Inset block for an emphasized detail (a dialog's running command).
pub(crate) const DETAIL_FILL: Color32 = Color32::from_rgba_unmultiplied_const(60, 60, 60, 200);

/// Primary text on the dark overlay fills.
pub(crate) const TEXT: Color32 = Color32::from_rgb(235, 235, 235);
/// Secondary text: a dialog's supporting lines.
pub(crate) const TEXT_MUTED: Color32 = Color32::GRAY;
/// Text on a filled (safe or destructive) button.
pub(crate) const TEXT_ON_FILL: Color32 = Color32::WHITE;
/// An emphasized detail's text (the running command's name).
pub(crate) const DETAIL_TEXT: Color32 = Color32::LIGHT_GREEN;

/// Informational accent: info toasts, the mode banner's border.
pub(crate) const ACCENT: Color32 = Color32::from_rgb(90, 150, 230);
/// Success: success toasts, "this window" markers, install succeeded.
pub(crate) const SUCCESS: Color32 = Color32::from_rgb(80, 180, 110);
/// Warning toasts.
pub(crate) const WARNING: Color32 = Color32::from_rgb(230, 180, 60);
/// Errors and destructive text: error toasts and labels, End Session.
pub(crate) const DANGER: Color32 = Color32::from_rgb(230, 90, 90);

/// A dialog's safe (Enter-default) button.
pub(crate) const SAFE_FILL: Color32 = Color32::from_rgb(60, 120, 180);
/// A dialog's destructive button.
pub(crate) const DESTRUCTIVE_FILL: Color32 = Color32::from_rgb(180, 50, 50);

/// Make one overlay's contents opaque regardless of terminal opacity.
///
/// Scoped to the overlay's own `ui`: the per-panel `ctx.set_global_style`
/// calls this replaces changed the look of every later panel, so what a
/// panel looked like depended on which one opened first (UX.md OV1,
/// RT23).
pub(crate) fn solid_panel(ui: &mut egui::Ui) {
    let visuals = ui.visuals_mut();
    visuals.window_fill = PANEL_FILL;
    visuals.panel_fill = PANEL_FILL;
    visuals.widgets.noninteractive.bg_fill = PANEL_FILL;
}

#[cfg(test)]
mod tests {
    #[test]
    fn overlay_surfaces_take_colours_from_the_theme() {
        // OV1: the surfaces migrated onto the tokens must not grow their
        // own colour literals back. Needles are assembled so this file,
        // which defines the tokens, is not what the scan finds.
        let needles = [
            ["Color32::from", "_rgb("].join(""),
            ["Color32::from", "_rgba_unmultiplied("].join(""),
            ["Color32::from", "_gray("].join(""),
        ];
        for (path, source) in [
            ("overlay/confirm.rs", include_str!("confirm.rs")),
            ("overlay/toast.rs", include_str!("toast.rs")),
            (
                "session_picker_mux.rs",
                include_str!("../../session_picker_mux.rs"),
            ),
            (
                "tmux_session_picker_ui.rs",
                include_str!("../../tmux_session_picker_ui.rs"),
            ),
            ("help_ui.rs", include_str!("../../help_ui.rs")),
            ("search/mod.rs", include_str!("../../search/mod.rs")),
            (
                "shader_install_ui.rs",
                include_str!("../../shader_install_ui.rs"),
            ),
            (
                "integrations_ui.rs",
                include_str!("../../integrations_ui.rs"),
            ),
        ] {
            for needle in &needles {
                assert!(
                    !source.contains(needle.as_str()),
                    "{path} spells a colour literal ({needle}); use an overlay::theme token"
                );
            }
        }
    }
}
