//! egui overlay rendering for active file transfers.
//!
//! Extracted from `file_transfers.rs` (R-42). This free function renders the
//! semi-transparent progress window anchored to the bottom-right of the terminal.

use crate::ui_constants::{FILE_TRANSFERS_ANCHOR_OFFSET, FILE_TRANSFERS_MIN_WIDTH};
use par_term_emu_core_rust::terminal::file_transfer::TransferDirection;

use super::format_bytes;
use super::types::FileTransferState;

/// Render the file transfer progress overlay using egui.
///
/// This is a free function (not a method on WindowState) so it can be called
/// from inside the `egui_ctx.run()` closure where `self` is already borrowed.
///
/// Shows a semi-transparent window anchored at the bottom-right with
/// progress bars for each active transfer. Auto-hides after transfers complete.
pub(crate) fn render_file_transfer_overlay(state: &FileTransferState, ctx: &egui::Context) {
    let has_active = !state.active_transfers.is_empty();
    let has_pending = !state.pending_saves.is_empty() || !state.pending_uploads.is_empty();
    let has_recent = !state.recent_transfers.is_empty();

    if !has_active && !has_pending && !has_recent {
        return;
    }

    egui::Window::new("File Transfers")
        .id(egui::Id::new("file_transfer_overlay_window"))
        .anchor(
            egui::Align2::RIGHT_BOTTOM,
            egui::vec2(-FILE_TRANSFERS_ANCHOR_OFFSET, -FILE_TRANSFERS_ANCHOR_OFFSET),
        )
        .order(egui::Order::Foreground)
        .resizable(false)
        .collapsible(false)
        .title_bar(true)
        .frame(
            egui::Frame::window(&ctx.style())
                .fill(egui::Color32::from_rgba_unmultiplied(40, 40, 80, 240))
                .stroke(egui::Stroke::new(
                    2.0,
                    egui::Color32::from_rgb(100, 200, 255),
                )),
        )
        .show(ctx, |ui| {
            ui.set_min_width(FILE_TRANSFERS_MIN_WIDTH);

            // Show recently-completed transfers with full progress bar
            for t in &state.recent_transfers {
                let direction_icon = match t.direction {
                    TransferDirection::Download => "\u{2B07}", // down arrow
                    TransferDirection::Upload => "\u{2B06}",   // up arrow
                };

                ui.horizontal(|ui| {
                    ui.label(direction_icon);
                    ui.label(&t.filename);
                });
                let size_text = format!("{} \u{2714}", format_bytes(t.size)); // checkmark
                ui.add(egui::ProgressBar::new(1.0).text(size_text).animate(false));
                ui.add_space(4.0);
            }

            // Show active in-progress transfers
            for info in &state.active_transfers {
                let direction_icon = match info.direction {
                    TransferDirection::Download => "\u{2B07}", // down arrow
                    TransferDirection::Upload => "\u{2B06}",   // up arrow
                };

                ui.horizontal(|ui| {
                    ui.label(direction_icon);
                    ui.label(&info.filename);
                });

                if let Some(total) = info.total_bytes {
                    if total > 0 {
                        let fraction = info.bytes_transferred as f32 / total as f32;
                        let text = format!(
                            "{} / {}",
                            format_bytes(info.bytes_transferred),
                            format_bytes(total)
                        );
                        ui.add(egui::ProgressBar::new(fraction).text(text).animate(false));
                    }
                } else {
                    // Indeterminate progress
                    let text = format_bytes(info.bytes_transferred);
                    ui.add(egui::ProgressBar::new(0.0).text(text).animate(true));
                }

                ui.add_space(4.0);
            }

            if !state.pending_saves.is_empty() {
                ui.separator();
                let count = state.pending_saves.len();
                ui.label(format!(
                    "{} download{} waiting to save",
                    count,
                    if count == 1 { "" } else { "s" }
                ));
            }

            if !state.pending_uploads.is_empty() {
                ui.separator();
                let count = state.pending_uploads.len();
                ui.label(format!(
                    "{} upload request{} pending",
                    count,
                    if count == 1 { "" } else { "s" }
                ));
            }
        });

    // Request redraw while overlay is visible so it updates and auto-hides
    if has_active || has_recent {
        ctx.request_repaint();
    }
}
