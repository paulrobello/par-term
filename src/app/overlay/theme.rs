//! Shared overlay look (UX.md OV1): widths come from one small set so
//! dialogs, pickers, and panels line up instead of each picking its own.

/// Narrow overlays: toasts, small choosers.
pub(crate) const WIDTH_SMALL: f32 = 360.0;
/// Dialogs and most pickers.
pub(crate) const WIDTH_MEDIUM: f32 = 520.0;
/// Wide pickers (the command palette with its chord column).
pub(crate) const WIDTH_LARGE: f32 = 640.0;

/// Make one overlay's contents opaque regardless of terminal opacity.
///
/// Scoped to the overlay's own `ui`: the per-panel `ctx.set_global_style`
/// calls this replaces changed the look of every later panel, so what a
/// panel looked like depended on which one opened first (UX.md OV1,
/// RT23).
pub(crate) fn solid_panel(ui: &mut egui::Ui, fill: egui::Color32) {
    let visuals = ui.visuals_mut();
    visuals.window_fill = fill;
    visuals.panel_fill = fill;
    visuals.widgets.noninteractive.bg_fill = fill;
}
