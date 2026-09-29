//! Exited-pane chrome for par-mux panes the daemon HOLDS after their
//! process exited (`%pane-exited`): a banner over — never instead of —
//! the frozen screen, reading "Process exited (code N)" with a Restart
//! button. `%pane-respawned` removes the state and the banner with it.

use par_term_tmux::TmuxPaneId;

/// One held pane's banner, in egui logical points.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ExitedPaneBanner {
    pub(crate) tmux_pane: TmuxPaneId,
    pub(crate) code: Option<i32>,
    /// `(x, y, width, height)` of the pane in logical points.
    pub(crate) rect: (f32, f32, f32, f32),
}

/// The banner text for an exit code; `None` is a signal death or a code
/// the daemon could not read.
pub(crate) fn exited_label(code: Option<i32>) -> String {
    match code {
        Some(code) => format!("Process exited (code {code})"),
        None => "Process exited".to_string(),
    }
}

/// Banners for the active tab's held panes. Pane bounds are physical
/// pixels; egui draws in logical points, hence the `scale` divide.
pub(crate) fn gather_exited_pane_banners(
    tmux_state: &crate::app::tmux_handler::tmux_state::TmuxState,
    tab: Option<&crate::tab::Tab>,
    scale: f32,
) -> Vec<ExitedPaneBanner> {
    if tmux_state.mux_exited_panes.is_empty() {
        return Vec::new();
    }
    let Some(tab) = tab else {
        return Vec::new();
    };
    let Some(pm) = tab.pane_manager() else {
        return Vec::new();
    };
    let scale = if scale > 0.0 { scale } else { 1.0 };
    let mut banners: Vec<ExitedPaneBanner> = tmux_state
        .mux_exited_panes
        .iter()
        .filter_map(|(&tmux_pane, &code)| {
            let (owner, native) = tmux_state.tmux_pane_owner(tmux_pane)?;
            if owner != tab.id {
                return None;
            }
            let bounds = pm.get_pane(native)?.bounds;
            Some(ExitedPaneBanner {
                tmux_pane,
                code,
                rect: (
                    bounds.x / scale,
                    bounds.y / scale,
                    bounds.width / scale,
                    bounds.height / scale,
                ),
            })
        })
        .collect();
    banners.sort_by_key(|b| b.tmux_pane);
    banners
}

/// Draw the banners; returns the pane whose Restart button was clicked.
pub(super) fn render_exited_pane_banners(
    ctx: &egui::Context,
    banners: &[ExitedPaneBanner],
) -> Option<TmuxPaneId> {
    let mut restart = None;
    for banner in banners {
        let (x, y, width, height) = banner.rect;
        let area_width = width.clamp(120.0, 420.0);
        let pos = egui::pos2(x + (width - area_width) / 2.0, y + height - 56.0);
        egui::Area::new(egui::Id::new(("mux_exited_pane", banner.tmux_pane)))
            .fixed_pos(pos)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.set_width(area_width);
                egui::Frame::NONE
                    .fill(egui::Color32::from_rgba_unmultiplied(20, 20, 20, 225))
                    .inner_margin(egui::Margin::symmetric(12, 8))
                    .corner_radius(6.0)
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(200, 90, 60)))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(exited_label(banner.code))
                                    .strong()
                                    .color(egui::Color32::from_rgb(235, 235, 235)),
                            );
                            if ui.button("Restart").clicked() {
                                restart = Some(banner.tmux_pane);
                            }
                            ui.label(egui::RichText::new("Enter: restart").weak().small());
                        });
                    });
            });
    }
    restart
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_carries_the_daemon_code() {
        assert_eq!(exited_label(Some(7)), "Process exited (code 7)");
        assert_eq!(exited_label(Some(0)), "Process exited (code 0)");
        assert_eq!(exited_label(None), "Process exited");
    }

    #[test]
    fn banners_render_headless_without_panicking() {
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(800.0, 600.0),
            )),
            ..Default::default()
        };
        let banners = [ExitedPaneBanner {
            tmux_pane: 3,
            code: Some(1),
            rect: (0.0, 0.0, 400.0, 300.0),
        }];
        let mut clicked = Some(0);
        let mut output = ctx.run_ui(input, |ctx| {
            clicked = render_exited_pane_banners(ctx, &banners);
        });
        output.textures_delta.clear();
        assert_eq!(clicked, None, "no click was simulated");
    }
}
