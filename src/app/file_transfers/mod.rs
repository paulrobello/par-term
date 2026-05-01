//! File transfer handling for downloads and uploads.
//!
//! This module manages file transfer state, processes completed downloads via
//! native save dialogs, handles upload requests via native file pickers, and
//! renders an egui progress overlay for active transfers.
//!
//! Organized into four sub-layers:
//! - `types`   — data structures (`FileTransferState`, `TransferInfo`, etc.)
//! - `overlay` — egui overlay rendering (`render_file_transfer_overlay`)
//! - `upload`  — upload dialog, background upload thread, tar.gz helper
//! - `mod`     — `WindowState` impl methods (poll, save dialog, download)

mod overlay;
mod types;
mod upload;

pub(crate) use overlay::render_file_transfer_overlay;
pub(crate) use types::{
    FileTransferState, PendingSave, PendingUpload, RecentTransfer, TransferInfo,
};

use std::path::PathBuf;
use std::sync::atomic::Ordering;

use par_term_emu_core_rust::terminal::file_transfer::{
    FileTransfer, TransferDirection, TransferStatus,
};

use super::window_state::WindowState;
use crate::config::DownloadSaveLocation;

/// Chunk size for writing upload data to the PTY.
/// Matches typical macOS PTY buffer size for efficient writes.
const UPLOAD_CHUNK_SIZE: usize = 65536;

/// How long to show the overlay before opening the save dialog (ms).
/// Gives the egui overlay time to render before the blocking dialog steals focus.
const SAVE_DIALOG_DELAY_MS: u64 = 750;

/// How long to show completed transfers in the overlay (seconds).
const RECENT_TRANSFER_DISPLAY_SECS: u64 = 3;

/// Format a byte count as a human-readable string
pub(super) fn format_bytes(bytes: usize) -> String {
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
        filename: if ft.filename.is_empty() {
            format!("transfer-{}", ft.id)
        } else {
            ft.filename.clone()
        },
        direction: ft.direction,
        bytes_transferred,
        total_bytes,
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
    /// - Track background upload progress
    /// - Process pending save/upload dialogs
    pub(crate) fn check_file_transfers(&mut self) {
        let tab = if let Some(t) = self.tab_manager.active_tab() {
            t
        } else {
            return;
        };

        // Always clear active_transfers before rebuilding — this must happen outside
        // the try_lock block so it's reset even when the terminal is locked by the
        // upload thread. poll_active_uploads will re-add upload entries below.
        self.file_transfer_state.active_transfers.clear();

        // try_lock: intentional — file transfer polling in about_to_wait (sync event loop).
        // On miss: active_transfers stays cleared (cleared above) and no transfer progress
        // is shown for this frame. The overlay will be repopulated on the next poll.
        if let Ok(term) = tab.terminal.try_write() {
            // 1. Update active transfers for overlay (terminal-side transfers like downloads)
            let active = term.get_active_transfers();
            self.file_transfer_state
                .active_transfers
                .extend(active.iter().map(transfer_to_info));

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
                // try_lock: intentional — taking a completed download from the terminal in
                // a spawned async task using sync try_lock. On miss: the completed transfer
                // is not taken this iteration; it will be picked up on the next poll.
                if let Ok(term) = terminal_arc.try_write()
                    && let Some(ft) = term.take_completed_transfer(id)
                {
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
                    let size = ft.data.len();
                    self.file_transfer_state
                        .pending_saves
                        .push_back(PendingSave {
                            filename: filename.clone(),
                            data: ft.data,
                        });
                    self.file_transfer_state
                        .recent_transfers
                        .push(RecentTransfer {
                            filename: filename.clone(),
                            size,
                            direction: TransferDirection::Download,
                            completed_at: std::time::Instant::now(),
                        });
                    self.file_transfer_state.last_completion_time = Some(std::time::Instant::now());
                    self.deliver_notification(
                        "Download Received",
                        &format!("Received {} ({})", filename, format_bytes(size)),
                    );
                }
            }

            // 3. Check for failed transfers and notify
            // try_lock: intentional — checking for failed transfers in a spawned async task.
            // On miss: failure detection is deferred to the next poll iteration. No data lost.
            if let Ok(term) = terminal_arc.try_write() {
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
                    if let Ok(term) = terminal_arc.try_write() {
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
            // try_lock: intentional — upload request polling in spawned async task.
            // On miss: upload requests are deferred to the next poll. No user data is lost.
            if let Ok(term) = terminal_arc.try_write() {
                let upload_requests = term.poll_upload_requests();
                for _format in upload_requests {
                    self.file_transfer_state
                        .pending_uploads
                        .push_back(PendingUpload {});
                    self.deliver_notification(
                        "Upload Requested",
                        "Remote application is requesting a file upload",
                    );
                }
            }
        }

        // 5. Expire old recent transfers
        self.file_transfer_state.recent_transfers.retain(|t| {
            t.completed_at.elapsed() < std::time::Duration::from_secs(RECENT_TRANSFER_DISPLAY_SECS)
        });
        if !self.file_transfer_state.recent_transfers.is_empty() {
            self.request_redraw();
        }

        // 6. Track background upload progress
        self.poll_active_uploads();

        // 7. Process pending save/upload dialogs (outside the terminal lock)
        // Delay save dialogs briefly so the transfer overlay has time to render
        // before the blocking native dialog steals focus.
        if !self.file_transfer_state.dialog_open {
            let save_ready = self.file_transfer_state.pending_saves.front().is_some()
                && self
                    .file_transfer_state
                    .last_completion_time
                    .is_some_and(|t| {
                        t.elapsed() >= std::time::Duration::from_millis(SAVE_DIALOG_DELAY_MS)
                    });

            if save_ready {
                if let Some(pending) = self.file_transfer_state.pending_saves.pop_front() {
                    self.process_save_dialog(pending);
                    // Refresh recent transfer timers so overlay stays visible
                    // after the blocking dialog returns
                    let now = std::time::Instant::now();
                    for t in &mut self.file_transfer_state.recent_transfers {
                        t.completed_at = now;
                    }
                    self.file_transfer_state.last_completion_time = Some(now);
                }
            } else if self.file_transfer_state.pending_saves.front().is_some() {
                // Keep redrawing while waiting for the delay
                self.request_redraw();
            } else if let Some(pending) = self.file_transfer_state.pending_uploads.pop_front() {
                self.process_upload_dialog(pending);
            }
        }
    }

    /// Check background upload threads for progress and completion.
    fn poll_active_uploads(&mut self) {
        // Collect completed uploads and their results
        let mut completed_info: Vec<(String, usize, Option<String>)> = Vec::new();
        self.file_transfer_state.active_uploads.retain(|upload| {
            if upload.completed.load(Ordering::Relaxed) {
                let error = upload.error.lock().take();
                completed_info.push((upload.filename.clone(), upload.file_size, error));
                false
            } else {
                true
            }
        });

        // Notify for completed uploads and add to recent transfers
        for (filename, file_size, error) in completed_info {
            if let Some(e) = error {
                self.deliver_notification("Upload Failed", &e);
            } else {
                self.file_transfer_state
                    .recent_transfers
                    .push(RecentTransfer {
                        filename: filename.clone(),
                        size: file_size,
                        direction: TransferDirection::Upload,
                        completed_at: std::time::Instant::now(),
                    });
                self.deliver_notification(
                    "Upload Complete",
                    &format!("Uploaded {} ({})", filename, format_bytes(file_size)),
                );
            }
            self.file_transfer_state.last_completion_time = Some(std::time::Instant::now());
        }

        // Add active upload progress to the transfer overlay
        for upload in &self.file_transfer_state.active_uploads {
            let wire_written = upload.bytes_written.load(Ordering::Relaxed);
            // Map wire bytes back to file-size proportion for display
            let bytes_transferred = if upload.total_wire_bytes > 0 {
                ((wire_written as f64 / upload.total_wire_bytes as f64) * upload.file_size as f64)
                    as usize
            } else {
                0
            };

            self.file_transfer_state
                .active_transfers
                .push(TransferInfo {
                    filename: upload.filename.clone(),
                    direction: TransferDirection::Upload,
                    bytes_transferred,
                    total_bytes: Some(upload.file_size),
                });
        }

        // Redraws during active uploads are managed by about_to_wait's
        // file transfer progress section (section 8) for proper scheduling.
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
                        self.config.rcu(|old| {
                            let mut new = (**old).clone();
                            new.last_download_directory =
                                Some(parent.to_string_lossy().to_string());
                            std::sync::Arc::new(new)
                        });
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

    /// Resolve the default download directory based on config settings.
    fn resolve_download_directory(&self) -> Option<PathBuf> {
        match &self.config.load().download_save_location {
            DownloadSaveLocation::Downloads => dirs::download_dir(),
            DownloadSaveLocation::LastUsed => self
                .config
                .load()
                .last_download_directory
                .as_ref()
                .map(PathBuf::from)
                .or_else(dirs::download_dir),
            DownloadSaveLocation::Cwd => {
                // Try to get CWD from shell integration
                // try_lock: intentional — getting download save path in sync event loop.
                // On miss: falls through to the Downloads fallback below. Acceptable UX.
                if let Some(tab) = self.tab_manager.active_tab()
                    && let Ok(term) = tab.terminal.try_write()
                    && let Some(cwd) = term.shell_integration_cwd()
                {
                    return Some(PathBuf::from(cwd));
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
