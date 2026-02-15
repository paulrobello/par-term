//! File transfer handling for downloads and uploads.
//!
//! This module manages file transfer state, processes completed downloads via
//! native save dialogs, handles upload requests via native file pickers, and
//! renders an egui progress overlay for active transfers.

use std::collections::VecDeque;
use std::path::PathBuf;

use par_term_emu_core_rust::terminal::file_transfer::{
    FileTransfer, TransferDirection, TransferStatus,
};

use super::window_state::WindowState;
use crate::config::DownloadSaveLocation;

/// UI-friendly information about an active file transfer
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct TransferInfo {
    /// Unique transfer identifier
    pub id: u64,
    /// Display filename
    pub filename: String,
    /// Transfer direction
    pub direction: TransferDirection,
    /// Bytes transferred so far
    pub bytes_transferred: usize,
    /// Total expected bytes (None if unknown)
    pub total_bytes: Option<usize>,
    /// When the transfer started (unix millis)
    pub started_at: u64,
}

/// A completed download awaiting the save dialog
#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct PendingSave {
    /// Transfer ID
    pub id: u64,
    /// Suggested filename
    pub filename: String,
    /// The downloaded data
    pub data: Vec<u8>,
}

/// An upload request awaiting the file picker
#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct PendingUpload {
    /// Upload format (e.g., "base64")
    pub format: String,
}

/// Tracks all file transfer UI state
#[derive(Debug, Default)]
pub(crate) struct FileTransferState {
    /// Currently active transfers (for overlay display)
    pub active_transfers: Vec<TransferInfo>,
    /// Completed downloads waiting to be saved
    pub pending_saves: VecDeque<PendingSave>,
    /// Upload requests waiting for the file picker
    pub pending_uploads: VecDeque<PendingUpload>,
    /// Whether a modal dialog (save/open) is currently showing
    pub dialog_open: bool,
    /// When the last transfer completed (for auto-hiding the overlay)
    pub last_completion_time: Option<std::time::Instant>,
}

/// Format a byte count as a human-readable string
fn format_bytes(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// Extract UI transfer info from a core `FileTransfer`
fn transfer_to_info(ft: &FileTransfer) -> TransferInfo {
    let (bytes_transferred, total_bytes) = match &ft.status {
        TransferStatus::InProgress {
            bytes_transferred,
            total_bytes,
        } => (*bytes_transferred, *total_bytes),
        TransferStatus::Completed => (ft.data.len(), Some(ft.data.len())),
        _ => (0, None),
    };

    TransferInfo {
        id: ft.id,
        filename: if ft.filename.is_empty() {
            format!("transfer-{}", ft.id)
        } else {
            ft.filename.clone()
        },
        direction: ft.direction,
        bytes_transferred,
        total_bytes,
        started_at: ft.started_at,
    }
}

impl WindowState {
    /// Poll terminal for file transfer events each frame.
    ///
    /// Called from `about_to_wait()` to:
    /// - Update active transfer list for the progress overlay
    /// - Collect completed downloads for save dialogs
    /// - Collect upload requests for file pickers
    /// - Notify on failures
    /// - Process pending save/upload dialogs
    pub(crate) fn check_file_transfers(&mut self) {
        let tab = if let Some(t) = self.tab_manager.active_tab() {
            t
        } else {
            return;
        };

        if let Ok(term) = tab.terminal.try_lock() {
            // 1. Update active transfers for overlay
            let active = term.get_active_transfers();
            self.file_transfer_state.active_transfers =
                active.iter().map(transfer_to_info).collect();

            // 2. Check for completed downloads
            let completed = term.get_completed_transfers();
            let completed_ids: Vec<u64> = completed
                .iter()
                .filter(|ft| {
                    ft.direction == TransferDirection::Download
                        && ft.status == TransferStatus::Completed
                })
                .map(|ft| ft.id)
                .collect();

            // Drop the lock before taking completed transfers (needs &self not &mut)
            drop(term);

            // Take each completed download and queue for save dialog
            let terminal_arc = std::sync::Arc::clone(&tab.terminal);
            for id in completed_ids {
                if let Ok(term) = terminal_arc.try_lock() {
                    if let Some(ft) = term.take_completed_transfer(id) {
                        let filename = if ft.filename.is_empty() {
                            format!("download-{}", ft.id)
                        } else {
                            ft.filename.clone()
                        };
                        crate::debug_info!(
                            "FILE_TRANSFER",
                            "Download completed: {} ({} bytes)",
                            filename,
                            ft.data.len()
                        );
                        self.file_transfer_state
                            .pending_saves
                            .push_back(PendingSave {
                                id: ft.id,
                                filename,
                                data: ft.data,
                            });
                        self.file_transfer_state.last_completion_time =
                            Some(std::time::Instant::now());
                    }
                }
            }

            // 3. Check for failed transfers and notify
            if let Ok(term) = terminal_arc.try_lock() {
                let failed: Vec<(u64, String)> = term
                    .get_completed_transfers()
                    .iter()
                    .filter_map(|ft| {
                        if let TransferStatus::Failed(reason) = &ft.status {
                            Some((ft.id, reason.clone()))
                        } else {
                            None
                        }
                    })
                    .collect();

                // Take failed transfers to consume them
                drop(term);
                for (id, reason) in &failed {
                    if let Ok(term) = terminal_arc.try_lock() {
                        let _ = term.take_completed_transfer(*id);
                    }
                    self.deliver_notification(
                        "File Transfer Failed",
                        &format!("Transfer failed: {}", reason),
                    );
                    self.file_transfer_state.last_completion_time = Some(std::time::Instant::now());
                }
            }

            // 4. Poll for upload requests
            if let Ok(term) = terminal_arc.try_lock() {
                let upload_requests = term.poll_upload_requests();
                for format in upload_requests {
                    self.file_transfer_state
                        .pending_uploads
                        .push_back(PendingUpload { format });
                    self.deliver_notification(
                        "Upload Requested",
                        "Remote application is requesting a file upload",
                    );
                }
            }
        }

        // 5. Process pending save dialogs (outside the terminal lock)
        if !self.file_transfer_state.dialog_open {
            if let Some(pending) = self.file_transfer_state.pending_saves.pop_front() {
                self.process_save_dialog(pending);
            } else if let Some(pending) = self.file_transfer_state.pending_uploads.pop_front() {
                self.process_upload_dialog(pending);
            }
        }
    }

    /// Show a native save dialog for a completed download and write the file.
    fn process_save_dialog(&mut self, pending: PendingSave) {
        self.file_transfer_state.dialog_open = true;

        let default_dir = self.resolve_download_directory();

        let mut dialog = rfd::FileDialog::new().set_file_name(&pending.filename);

        if let Some(dir) = &default_dir {
            dialog = dialog.set_directory(dir);
        }

        let result = dialog.save_file();

        self.file_transfer_state.dialog_open = false;

        if let Some(path) = result {
            match std::fs::write(&path, &pending.data) {
                Ok(()) => {
                    let size_str = format_bytes(pending.data.len());
                    crate::debug_info!(
                        "FILE_TRANSFER",
                        "Saved download to: {} ({})",
                        path.display(),
                        size_str
                    );
                    self.deliver_notification(
                        "Download Saved",
                        &format!(
                            "Saved {} to {} ({})",
                            pending.filename,
                            path.display(),
                            size_str
                        ),
                    );

                    // Update last_download_directory for LastUsed config option
                    if let Some(parent) = path.parent() {
                        self.config.last_download_directory =
                            Some(parent.to_string_lossy().to_string());
                    }
                }
                Err(e) => {
                    crate::debug_info!("FILE_TRANSFER", "Failed to save download: {}", e);
                    self.deliver_notification(
                        "Download Save Failed",
                        &format!("Failed to save {}: {}", pending.filename, e),
                    );
                }
            }
        } else {
            crate::debug_info!(
                "FILE_TRANSFER",
                "Save dialog cancelled for {}",
                pending.filename
            );
        }
    }

    /// Show a native file picker for an upload request, read the file, and send data.
    fn process_upload_dialog(&mut self, _pending: PendingUpload) {
        self.file_transfer_state.dialog_open = true;

        let result = rfd::FileDialog::new().pick_file();

        self.file_transfer_state.dialog_open = false;

        if let Some(path) = result {
            match std::fs::read(&path) {
                Ok(data) => {
                    let size_str = format_bytes(data.len());
                    let filename = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "unknown".to_string());

                    crate::debug_info!(
                        "FILE_TRANSFER",
                        "Uploading file: {} ({})",
                        filename,
                        size_str
                    );

                    // Send the upload data to the terminal
                    if let Some(tab) = self.tab_manager.active_tab() {
                        if let Ok(term) = tab.terminal.try_lock() {
                            term.send_upload_data(&data);
                            self.deliver_notification(
                                "Upload Sent",
                                &format!("Uploaded {} ({})", filename, size_str),
                            );
                        } else {
                            self.deliver_notification(
                                "Upload Failed",
                                "Could not lock terminal to send upload data",
                            );
                        }
                    }
                }
                Err(e) => {
                    crate::debug_info!("FILE_TRANSFER", "Failed to read upload file: {}", e);
                    self.deliver_notification(
                        "Upload Failed",
                        &format!("Failed to read file: {}", e),
                    );
                    // Cancel the upload since we can't send data
                    if let Some(tab) = self.tab_manager.active_tab() {
                        if let Ok(term) = tab.terminal.try_lock() {
                            term.cancel_upload();
                        }
                    }
                }
            }
        } else {
            crate::debug_info!("FILE_TRANSFER", "Upload file picker cancelled");
            // Cancel the upload since user cancelled the picker
            if let Some(tab) = self.tab_manager.active_tab() {
                if let Ok(term) = tab.terminal.try_lock() {
                    term.cancel_upload();
                }
            }
        }
    }

    /// Resolve the default download directory based on config settings.
    fn resolve_download_directory(&self) -> Option<PathBuf> {
        match &self.config.download_save_location {
            DownloadSaveLocation::Downloads => dirs::download_dir(),
            DownloadSaveLocation::LastUsed => self
                .config
                .last_download_directory
                .as_ref()
                .map(PathBuf::from)
                .or_else(dirs::download_dir),
            DownloadSaveLocation::Cwd => {
                // Try to get CWD from shell integration
                if let Some(tab) = self.tab_manager.active_tab() {
                    if let Ok(term) = tab.terminal.try_lock() {
                        if let Some(cwd) = term.shell_integration_cwd() {
                            return Some(PathBuf::from(cwd));
                        }
                    }
                }
                // Fall back to Downloads if CWD not available
                dirs::download_dir()
            }
            DownloadSaveLocation::Custom(path) => {
                let p = PathBuf::from(path);
                if p.is_dir() {
                    Some(p)
                } else {
                    dirs::download_dir()
                }
            }
        }
    }

}

/// Render the file transfer progress overlay using egui.
///
/// This is a free function (not a method on WindowState) so it can be called
/// from inside the `egui_ctx.run()` closure where `self` is already borrowed.
///
/// Shows a semi-transparent window anchored at the bottom-right with
/// progress bars for each active transfer. Auto-hides 2 seconds after
/// the last transfer completes.
pub(crate) fn render_file_transfer_overlay(state: &FileTransferState, ctx: &egui::Context) {
    let has_active = !state.active_transfers.is_empty();
    let has_pending = !state.pending_saves.is_empty() || !state.pending_uploads.is_empty();

    // Check if we should still show the overlay (2s after last completion)
    let show_completion = state
        .last_completion_time
        .is_some_and(|last| last.elapsed() < std::time::Duration::from_secs(2));

    if !has_active && !has_pending && !show_completion {
        return;
    }

    egui::Window::new("File Transfers")
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-10.0, -10.0))
        .resizable(false)
        .collapsible(false)
        .title_bar(true)
        .frame(
            egui::Frame::window(&ctx.style())
                .fill(egui::Color32::from_rgba_unmultiplied(30, 30, 30, 220)),
        )
        .show(ctx, |ui| {
            ui.set_min_width(250.0);

            if state.active_transfers.is_empty()
                && state.pending_saves.is_empty()
                && state.pending_uploads.is_empty()
            {
                ui.label("All transfers complete");
                return;
            }

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

    // Request redraw while overlay is visible so animations work
    if has_active {
        ctx.request_repaint();
    }
}
