//! Data types for file transfer state tracking.
//!
//! Extracted from `file_transfers.rs` (R-42) to isolate type definitions from
//! the logic and overlay rendering layers.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize};

use parking_lot::Mutex;

use par_term_emu_core_rust::terminal::file_transfer::TransferDirection;

use std::collections::VecDeque;

/// UI-friendly information about an active file transfer
#[derive(Debug, Clone)]
pub(crate) struct TransferInfo {
    /// Display filename
    pub filename: String,
    /// Transfer direction
    pub direction: TransferDirection,
    /// Bytes transferred so far
    pub bytes_transferred: usize,
    /// Total expected bytes (None if unknown)
    pub total_bytes: Option<usize>,
}

/// A completed download awaiting the save dialog
#[derive(Debug)]
pub(crate) struct PendingSave {
    /// Suggested filename
    pub filename: String,
    /// The downloaded data
    pub data: Vec<u8>,
}

/// An upload request awaiting the file picker
#[derive(Debug)]
pub(crate) struct PendingUpload {}

/// Tracks a background upload being written to the PTY in chunks.
pub(crate) struct ActiveUpload {
    /// Unique identifier
    pub id: u64,
    /// Display filename
    pub filename: String,
    /// Original file size (for user-facing display)
    pub file_size: usize,
    /// Total bytes to write to the PTY (base64-encoded size)
    pub total_wire_bytes: usize,
    /// Bytes written to the PTY so far (updated atomically by writer thread)
    pub bytes_written: Arc<AtomicUsize>,
    /// Whether the write has finished (success or error)
    pub completed: Arc<AtomicBool>,
    /// Error message if the write failed
    pub error: Arc<Mutex<Option<String>>>,
    /// When the upload started (unix millis)
    pub started_at: u64,
}

/// A recently-completed transfer shown briefly in the overlay.
/// Covers both uploads and downloads that completed too fast for
/// the active_transfers polling to catch.
pub(crate) struct RecentTransfer {
    /// Display filename
    pub filename: String,
    /// File size in bytes
    pub size: usize,
    /// Transfer direction
    pub direction: TransferDirection,
    /// When the transfer completed
    pub completed_at: std::time::Instant,
}

/// Tracks all file transfer UI state
#[derive(Default)]
pub(crate) struct FileTransferState {
    /// Currently active transfers (for overlay display)
    pub active_transfers: Vec<TransferInfo>,
    /// Completed downloads waiting to be saved
    pub pending_saves: VecDeque<PendingSave>,
    /// Upload requests waiting for the file picker
    pub pending_uploads: VecDeque<PendingUpload>,
    /// Background uploads being written to the PTY
    pub active_uploads: Vec<ActiveUpload>,
    /// Recently completed transfers (shown briefly in overlay)
    pub recent_transfers: Vec<RecentTransfer>,
    /// Whether a modal dialog (save/open) is currently showing
    pub dialog_open: bool,
    /// When the last transfer completed (for auto-hiding the overlay)
    pub last_completion_time: Option<std::time::Instant>,
}
