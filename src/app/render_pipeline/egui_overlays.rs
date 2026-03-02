//! Standalone egui overlay renderers used inside `submit_gpu_frame`.
//!
//! Each function in this module is a pure free function that takes only the data
//! it needs and an `egui::Context`.  They contain no borrow of `self`, which lets
//! them be called freely from inside the `egui_ctx.run(|ctx| { ... })` closure
//! while `self.renderer` is mutably borrowed.

use crate::copy_mode::VisualMode;
use crate::scrollback_metadata::ScrollbackMark;

/// Render the FPS / frame-time debug overlay in the top-right corner.
///
/// Only renders when `show_fps` is `true`.
pub(super) fn render_fps_overlay(
    ctx: &egui::Context,
    show_fps: bool,
    fps_value: f64,
    frame_time_ms: f64,
) {
    if !show_fps {
        return;
    }
    egui::Area::new(egui::Id::new("fps_overlay"))
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-30.0, 10.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(egui::Color32::from_rgba_unmultiplied(0, 0, 0, 200))
                .inner_margin(egui::Margin::same(8))
                .corner_radius(4.0)
                .show(ui, |ui| {
                    ui.style_mut().visuals.override_text_color =
                        Some(egui::Color32::from_rgb(0, 255, 0));
                    ui.label(
                        egui::RichText::new(format!(
                            "FPS: {:.1}\nFrame: {:.2}ms",
                            fps_value, frame_time_ms
                        ))
                        .monospace()
                        .size(14.0),
                    );
                });
        });
}

/// Render the resize overlay (centered) showing current grid/pixel dimensions.
///
/// Only renders when `resize_overlay_visible` is `true` and `dimensions` is `Some`.
pub(super) fn render_resize_overlay(
    ctx: &egui::Context,
    resize_overlay_visible: bool,
    dimensions: Option<(u32, u32, usize, usize)>,
) {
    if !resize_overlay_visible {
        return;
    }
    let Some((width_px, height_px, cols, rows)) = dimensions else {
        return;
    };
    egui::Area::new(egui::Id::new("resize_overlay"))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(egui::Color32::from_rgba_unmultiplied(0, 0, 0, 220))
                .inner_margin(egui::Margin::same(16))
                .corner_radius(8.0)
                .show(ui, |ui| {
                    ui.style_mut().visuals.override_text_color =
                        Some(egui::Color32::from_rgb(255, 255, 255));
                    ui.label(
                        egui::RichText::new(format!(
                            "{}×{}\n{}×{} px",
                            cols, rows, width_px, height_px
                        ))
                        .monospace()
                        .size(24.0),
                    );
                });
        });
}

/// Render the toast notification overlay (top-center) for transient status messages.
///
/// Only renders when `message` is `Some`.
pub(super) fn render_toast_overlay(ctx: &egui::Context, message: Option<&str>) {
    let Some(message) = message else {
        return;
    };
    egui::Area::new(egui::Id::new("toast_notification"))
        .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 60.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(egui::Color32::from_rgba_unmultiplied(30, 30, 30, 240))
                .inner_margin(egui::Margin::symmetric(20, 12))
                .corner_radius(8.0)
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 80, 80)))
                .show(ui, |ui| {
                    ui.style_mut().visuals.override_text_color =
                        Some(egui::Color32::from_rgb(255, 255, 255));
                    ui.label(egui::RichText::new(message).size(16.0));
                });
        });
}

/// Render the scrollbar mark tooltip near the mouse pointer.
///
/// The tooltip shows command, time, duration, and exit code from a `ScrollbackMark`.
/// It is shown when the user hovers over a scrollbar mark; pass `None` to skip.
pub(super) fn render_scrollbar_mark_tooltip(ctx: &egui::Context, mark: Option<&ScrollbackMark>) {
    let Some(mark) = mark else {
        return;
    };

    let mut lines = Vec::new();

    if let Some(ref cmd) = mark.command {
        let truncated = if cmd.len() > 50 {
            format!("{}...", &cmd[..47])
        } else {
            cmd.clone()
        };
        lines.push(format!("Command: {}", truncated));
    }

    if let Some(start_time) = mark.start_time {
        use chrono::{DateTime, Local, Utc};
        let dt = DateTime::<Utc>::from_timestamp_millis(start_time as i64)
            .expect("window_state: start_time millis out of valid timestamp range");
        let local: DateTime<Local> = dt.into();
        lines.push(format!("Time: {}", local.format("%H:%M:%S")));
    }

    if let Some(duration_ms) = mark.duration_ms {
        if duration_ms < 1000 {
            lines.push(format!("Duration: {}ms", duration_ms));
        } else if duration_ms < 60000 {
            lines.push(format!("Duration: {:.1}s", duration_ms as f64 / 1000.0));
        } else {
            let mins = duration_ms / 60000;
            let secs = (duration_ms % 60000) / 1000;
            lines.push(format!("Duration: {}m {}s", mins, secs));
        }
    }

    if let Some(exit_code) = mark.exit_code {
        lines.push(format!("Exit: {}", exit_code));
    }

    let tooltip_text = lines.join("\n");

    let mouse_pos = ctx.pointer_hover_pos().unwrap_or(egui::pos2(100.0, 100.0));
    let tooltip_x = (mouse_pos.x - 180.0).max(10.0);
    let tooltip_y = (mouse_pos.y - 20.0).max(10.0);

    egui::Area::new(egui::Id::new("scrollbar_mark_tooltip"))
        .order(egui::Order::Tooltip)
        .fixed_pos(egui::pos2(tooltip_x, tooltip_y))
        .show(ctx, |ui| {
            ui.set_min_width(150.0);
            egui::Frame::NONE
                .fill(egui::Color32::from_rgba_unmultiplied(30, 30, 30, 240))
                .inner_margin(egui::Margin::same(8))
                .corner_radius(4.0)
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 80, 80)))
                .show(ui, |ui| {
                    ui.set_min_width(140.0);
                    ui.style_mut().visuals.override_text_color =
                        Some(egui::Color32::from_rgb(220, 220, 220));
                    ui.label(egui::RichText::new(&tooltip_text).monospace().size(12.0));
                });
        });
}

/// Render the copy-mode status bar overlay pinned to the bottom-left of the window.
///
/// Shows the current copy-mode type (COPY / VISUAL / V-LINE / V-BLOCK / SEARCH) and
/// status text.  Only renders when `active` and `show_status` are both `true`.
pub(super) fn render_copy_mode_status_bar(
    ctx: &egui::Context,
    active: bool,
    show_status: bool,
    is_searching: bool,
    visual_mode: VisualMode,
    mode_text_str: &str,
    status: &str,
) {
    if !active || !show_status {
        return;
    }
    let color = if is_searching {
        egui::Color32::from_rgb(255, 165, 0)
    } else {
        match visual_mode {
            VisualMode::None => egui::Color32::from_rgb(100, 200, 100),
            VisualMode::Char | VisualMode::Line | VisualMode::Block => {
                egui::Color32::from_rgb(100, 150, 255)
            }
        }
    };
    egui::Area::new(egui::Id::new("copy_mode_status_bar"))
        .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(0.0, 0.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            let available_width = ui.available_width();
            egui::Frame::NONE
                .fill(egui::Color32::from_rgba_unmultiplied(40, 40, 40, 230))
                .inner_margin(egui::Margin::symmetric(12, 6))
                .show(ui, |ui| {
                    ui.set_min_width(available_width);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(mode_text_str)
                                .monospace()
                                .size(13.0)
                                .color(color)
                                .strong(),
                        );
                        ui.separator();
                        ui.label(
                            egui::RichText::new(status)
                                .monospace()
                                .size(12.0)
                                .color(egui::Color32::from_rgb(200, 200, 200)),
                        );
                    });
                });
        });
}

/// Render large pane index labels centered on each pane (used by the "identify panes" feature).
///
/// Each entry in `pane_bounds` is `(pane_index, PaneBounds)`.
/// Renders nothing when `pane_bounds` is empty.
pub(super) fn render_pane_identify_overlay(
    ctx: &egui::Context,
    pane_bounds: &[(usize, crate::pane::PaneBounds)],
) {
    for (index, bounds) in pane_bounds {
        let center_x = bounds.x + bounds.width / 2.0;
        let center_y = bounds.y + bounds.height / 2.0;
        egui::Area::new(egui::Id::new(format!("pane_identify_{}", index)))
            .fixed_pos(egui::pos2(center_x - 30.0, center_y - 30.0))
            .order(egui::Order::Foreground)
            .interactable(false)
            .show(ctx, |ui| {
                egui::Frame::NONE
                    .fill(egui::Color32::from_rgba_unmultiplied(0, 0, 0, 200))
                    .inner_margin(egui::Margin::symmetric(16, 8))
                    .corner_radius(8.0)
                    .stroke(egui::Stroke::new(
                        2.0,
                        egui::Color32::from_rgb(100, 200, 255),
                    ))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(format!("Pane {}", index))
                                .monospace()
                                .size(28.0)
                                .color(egui::Color32::from_rgb(100, 200, 255)),
                        );
                    });
            });
    }
}
