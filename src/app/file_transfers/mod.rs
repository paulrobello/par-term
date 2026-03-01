//! File transfer handling for downloads and uploads.
//!
//! This module manages file transfer state, processes completed downloads via
//! native save dialogs, handles upload requests via native file pickers, and
//! renders an egui progress overlay for active transfers.
//!
//! Organized into three sub-layers (R-42):
//! - `types`   — data structures (`FileTransferState`, `TransferInfo`, etc.)
//! - `overlay` — egui overlay rendering (`render_file_transfer_overlay`)
//! - `mod`     — `WindowState` impl methods (poll, save dialog, upload dialog)

mod overlay;
mod types;

pub(crate) use overlay::render_file_transfer_overlay;
pub(crate) use types::{
    ActiveUpload, FileTransferState, PendingSave, PendingUpload, RecentTransfer, TransferInfo,
};

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use parking_lot::Mutex;

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

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
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
                            id: ft.id,
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
                    id: upload.id,
                    filename: upload.filename.clone(),
                    direction: TransferDirection::Upload,
                    bytes_transferred,
                    total_bytes: Some(upload.file_size),
                    started_at: upload.started_at,
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
    ///
    /// Creates a tar.gz archive of the selected file, base64-encodes it, and spawns
    /// a background thread to write the data to the PTY in chunks. This avoids both:
    /// - The response_buffer deadlock (reader thread blocked waiting for PTY input)
    /// - UI freezing from large synchronous PTY writes
    fn process_upload_dialog(&mut self, _pending: PendingUpload) {
        self.file_transfer_state.dialog_open = true;

        let result = rfd::FileDialog::new().pick_file();

        self.file_transfer_state.dialog_open = false;

        if let Some(path) = result {
            match std::fs::read(&path) {
                Ok(data) => {
                    let file_size = data.len();
                    let size_str = format_bytes(file_size);
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

                    // Create tar.gz archive (iTerm2 tgz format)
                    let tgz_data = match create_tgz_archive(&path, &data) {
                        Ok(d) => d,
                        Err(e) => {
                            crate::debug_info!(
                                "FILE_TRANSFER",
                                "Failed to create tar.gz archive: {}",
                                e
                            );
                            self.deliver_notification(
                                "Upload Failed",
                                &format!("Failed to create archive: {}", e),
                            );
                            self.cancel_upload_direct();
                            return;
                        }
                    };

                    // Base64 encode and format as single line + newline
                    use base64::Engine;
                    let encoded = base64::engine::general_purpose::STANDARD.encode(&tgz_data);
                    let response = format!("{}\n", encoded);
                    let total_wire_bytes = response.len();

                    // Set up shared progress tracking
                    let bytes_written = Arc::new(AtomicUsize::new(0));
                    let completed = Arc::new(AtomicBool::new(false));
                    let error: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
                    let upload_id = now_millis();

                    self.file_transfer_state.active_uploads.push(ActiveUpload {
                        id: upload_id,
                        filename: filename.clone(),
                        file_size,
                        total_wire_bytes,
                        bytes_written: Arc::clone(&bytes_written),
                        completed: Arc::clone(&completed),
                        error: Arc::clone(&error),
                        started_at: upload_id,
                    });

                    // Spawn background thread to write in chunks without blocking UI
                    if let Some(terminal_arc) =
                        self.with_active_tab(|tab| Arc::clone(&tab.terminal))
                    {
                        let response_bytes = response.into_bytes();

                        std::thread::Builder::new()
                            .name(format!("upload-{}", filename))
                            .spawn(move || {
                                let mut offset = 0;
                                while offset < response_bytes.len() {
                                    let end =
                                        (offset + UPLOAD_CHUNK_SIZE).min(response_bytes.len());
                                    let chunk = &response_bytes[offset..end];

                                    // Acceptable risk: blocking_lock() from std thread (not Tokio).
                                    // See docs/CONCURRENCY.md for mutex strategy.
                                    let term = terminal_arc.blocking_write();
                                    match term.write(chunk) {
                                        Ok(()) => {
                                            drop(term);
                                            offset = end;
                                            bytes_written.store(offset, Ordering::Relaxed);
                                        }
                                        Err(e) => {
                                            drop(term);
                                            *error.lock() =
                                                Some(format!("PTY write failed: {}", e));
                                            completed.store(true, Ordering::Relaxed);
                                            return;
                                        }
                                    }
                                }
                                completed.store(true, Ordering::Relaxed);
                            })
                            .ok();
                    }

                    self.request_redraw();
                }
                Err(e) => {
                    crate::debug_info!("FILE_TRANSFER", "Failed to read upload file: {}", e);
                    self.deliver_notification(
                        "Upload Failed",
                        &format!("Failed to read file: {}", e),
                    );
                    self.cancel_upload_direct();
                }
            }
        } else {
            crate::debug_info!("FILE_TRANSFER", "Upload file picker cancelled");
            self.cancel_upload_direct();
        }
    }

    /// Cancel an upload by writing abort directly to the PTY.
    fn cancel_upload_direct(&self) {
        self.with_active_tab(|tab| {
            // Acceptable risk: blocking_lock() from sync winit event loop.
            // See docs/CONCURRENCY.md for mutex strategy.
            let term = tab.terminal.blocking_write();
            let _ = term.write(b"abort\n");
        });
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

/// Create a tar.gz archive containing a single file.
///
/// Returns the compressed archive bytes suitable for base64-encoding
/// and sending as an iTerm2 upload response.
fn create_tgz_archive(path: &Path, data: &[u8]) -> std::io::Result<Vec<u8>> {
    let filename = path.file_name().unwrap_or_default().to_string_lossy();

    let compressed = Vec::new();
    let encoder = flate2::write::GzEncoder::new(compressed, flate2::Compression::default());
    let mut archive = tar::Builder::new(encoder);

    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    header.set_mtime(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    );
    header.set_cksum();

    archive.append_data(&mut header, &*filename, data)?;

    let encoder = archive.into_inner()?;
    encoder.finish()
}
