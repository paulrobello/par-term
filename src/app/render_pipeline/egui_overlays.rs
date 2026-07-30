//! Standalone egui overlay renderers used inside `submit_gpu_frame`.
//!
//! Each function in this module is a pure free function that takes only the data
//! it needs and an `egui::Context`.  They contain no borrow of `self`, which lets
//! them be called freely from inside the `egui_ctx.run_ui(|ctx| { ... })` closure
//! while `self.renderer` is mutably borrowed.

use crate::config::ScrollbackMark;
use crate::copy_mode::VisualMode;
use par_term_config::text::truncate_chars;

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
        let truncated = if cmd.chars().count() > 50 {
            format!("{}...", truncate_chars(cmd, 47))
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

/// Render the automation confirmation dialog (center modal).
///
/// Serves every producer that queues onto `pending_trigger_actions`: output
/// triggers, profile auto-switch commands, and script `WriteText`. A producer
/// that is not an output trigger registers a `TriggerState::automation_action_notes`
/// entry so the dialog does not claim terminal output caused the action.
///
/// Shows the first pending action and presents Allow Once / Always Allow / Deny buttons.
/// On approval: moves the action to whichever approved queue matches its target —
/// `approved_pending_actions` for the active tab, `approved_targeted_actions` when the
/// action names a tab of its own. On deny: discards the action.
///
/// `target_note` is the caller-resolved sentence naming that tab (see
/// `WindowState::pending_action_target_note`). It must be present whenever the head
/// action carries a target: approving a write into a tab the user cannot see is only
/// legitimate if the dialog says which tab that is.
///
/// Uses `trigger_prompt_activated_frame` as a flicker guard to prevent the click that opens
/// the dialog from immediately dismissing it.
pub(super) fn render_trigger_prompt_dialog(
    ctx: &egui::Context,
    trigger_state: &mut crate::app::window_state::TriggerState,
    target_note: Option<&str>,
) {
    if trigger_state.pending_trigger_actions.is_empty() {
        trigger_state.trigger_prompt_dialog_open = false;
        trigger_state.trigger_prompt_activated_frame = None;
        return;
    }

    // A targeted action the caller could not name is one whose tab closed
    // between `prune_orphaned_pending_actions` and this frame. Withholding the
    // dialog makes "a targeted prompt always names its tab" true by
    // construction rather than by convention; the next prune withdraws it.
    if trigger_state.pending_trigger_actions[0].target.is_some() && target_note.is_none() {
        trigger_state.trigger_prompt_dialog_open = false;
        trigger_state.trigger_prompt_activated_frame = None;
        return;
    }

    // Record activation frame on first open (flicker guard)
    if !trigger_state.trigger_prompt_dialog_open {
        trigger_state.trigger_prompt_dialog_open = true;
        trigger_state.trigger_prompt_activated_frame = Some(ctx.cumulative_frame_nr());
    }

    let activated_frame = trigger_state.trigger_prompt_activated_frame.unwrap_or(0);
    let current_frame = ctx.cumulative_frame_nr();

    // Extract display info before the egui closure to avoid re-borrowing trigger_state inside it
    let trigger_name = trigger_state.pending_trigger_actions[0]
        .trigger_name
        .clone();
    let description = trigger_state.pending_trigger_actions[0].description.clone();
    let pending_count = trigger_state.pending_trigger_actions.len();

    // Actions that no output trigger produced carry their own source sentence;
    // without one, this is a trigger and the default sentence is accurate.
    let action_id = trigger_state.pending_trigger_actions[0].trigger_id;
    let source_note = trigger_state
        .automation_action_notes
        .get(&action_id)
        .cloned();
    let source_label = if source_note.is_some() {
        format!("Source: {}", trigger_name)
    } else {
        format!("Trigger: {}", trigger_name)
    };
    let source_note = source_note.unwrap_or_else(|| {
        "A trigger matched terminal output and wants to run this action.".to_string()
    });

    let mut approved = false;
    let mut always_approve = false;
    let mut denied = false;

    egui::Window::new("Automation Action Confirmation")
        .id(egui::Id::new("trigger_prompt_dialog"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.set_min_width(380.0);
            ui.set_max_width(500.0);

            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Automation Action Requires Confirmation")
                    .strong()
                    .size(15.0),
            );
            ui.add_space(8.0);
            ui.label(source_label.as_str());
            ui.add_space(4.0);

            egui::Frame::NONE
                .fill(egui::Color32::from_rgba_unmultiplied(0, 0, 0, 40))
                .inner_margin(egui::Margin::same(8))
                .corner_radius(4.0)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new(&description).monospace());
                });

            ui.add_space(4.0);
            ui.label(egui::RichText::new(source_note.as_str()).weak().small());

            // Where the write lands. Rendered at full strength rather than as
            // weak small print like the source note above: for a background tab
            // this is the part of the dialog that changes the decision.
            if let Some(note) = target_note {
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(note)
                        .strong()
                        .color(egui::Color32::from_rgb(230, 170, 60)),
                );
            }

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                if ui
                    .button(egui::RichText::new("Deny").color(egui::Color32::from_rgb(220, 60, 60)))
                    .clicked()
                    && current_frame > activated_frame
                {
                    denied = true;
                }
                ui.add_space(4.0);
                if ui.button("Allow Once").clicked() && current_frame > activated_frame {
                    approved = true;
                }
                ui.add_space(4.0);
                if ui
                    .button(
                        egui::RichText::new("Always Allow")
                            .color(egui::Color32::from_rgb(80, 180, 80)),
                    )
                    .clicked()
                    && current_frame > activated_frame
                {
                    always_approve = true;
                    approved = true;
                }
            });

            if pending_count > 1 {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(format!("({} more pending actions)", pending_count - 1))
                        .weak()
                        .small(),
                );
            }
        });

    if denied || approved {
        let pending = trigger_state.pending_trigger_actions.remove(0);
        trigger_state
            .automation_action_notes
            .remove(&pending.trigger_id);
        if approved {
            if always_approve {
                trigger_state
                    .always_allow_trigger_ids
                    .insert(pending.trigger_id);
            }
            match pending.target {
                Some(target) => trigger_state
                    .approved_targeted_actions
                    .push((target, pending.action)),
                None => trigger_state.approved_pending_actions.push(pending.action),
            }
        }
        if trigger_state.pending_trigger_actions.is_empty() {
            trigger_state.trigger_prompt_dialog_open = false;
            trigger_state.trigger_prompt_activated_frame = None;
        } else {
            // More actions queued — reset activated frame for the next one
            trigger_state.trigger_prompt_activated_frame = Some(ctx.cumulative_frame_nr());
        }
    }
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
