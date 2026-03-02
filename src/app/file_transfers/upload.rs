//! File upload dialog and background upload thread management.
//!
//! Extracted from `file_transfers/mod` to keep that file under 500 lines.
//!
//! Contains:
//! - `process_upload_dialog` — show native file picker and spawn background upload thread
//! - `cancel_upload_direct`  — write abort sequence directly to the PTY
//! - `create_tgz_archive`    — build an iTerm2-compatible tar.gz payload

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use parking_lot::Mutex;

use super::types::{ActiveUpload, PendingUpload};
use super::{UPLOAD_CHUNK_SIZE, format_bytes};
use crate::app::window_state::WindowState;

impl WindowState {
    /// Show a native file picker for an upload request, read the file, and send data.
    ///
    /// Creates a tar.gz archive of the selected file, base64-encodes it, and spawns
    /// a background thread to write the data to the PTY in chunks. This avoids both:
    /// - The response_buffer deadlock (reader thread blocked waiting for PTY input)
    /// - UI freezing from large synchronous PTY writes
    pub(super) fn process_upload_dialog(&mut self, _pending: PendingUpload) {
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

                    self.file_transfer_state.active_uploads.push(ActiveUpload {
                        filename: filename.clone(),
                        file_size,
                        total_wire_bytes,
                        bytes_written: Arc::clone(&bytes_written),
                        completed: Arc::clone(&completed),
                        error: Arc::clone(&error),
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
    pub(super) fn cancel_upload_direct(&self) {
        self.with_active_tab(|tab| {
            // Acceptable risk: blocking_lock() from sync winit event loop.
            // See docs/CONCURRENCY.md for mutex strategy.
            let term = tab.terminal.blocking_write();
            let _ = term.write(b"abort\n");
        });
    }
}

/// Create a tar.gz archive containing a single file.
///
/// Returns the compressed archive bytes suitable for base64-encoding
/// and sending as an iTerm2 upload response.
pub(super) fn create_tgz_archive(path: &Path, data: &[u8]) -> std::io::Result<Vec<u8>> {
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
