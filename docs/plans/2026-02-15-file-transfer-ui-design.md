# File Transfer Frontend UI Design

**Date**: 2026-02-15
**Issue**: #154
**Status**: Approved

## Overview

Implement frontend UI for file transfer support (download save dialog, upload file picker, progress overlay). The core library (`par-term-emu-core-rust` v0.38+) has full file transfer support — this design covers the missing frontend layer.

## Architecture: Polling-Based (Approach A)

Follows the existing `check_notifications()` / `check_bell()` pattern. A new `check_file_transfers()` method polls the `TerminalManager` each frame in `about_to_wait()`, processing completed downloads and upload requests.

### Data Flow

```
Core FileTransferManager → TerminalManager wrappers → check_file_transfers()
  → FileTransferState (UI state) → egui overlay (progress bars)
  → rfd native dialogs (save/open) → file I/O + notifications
```

## Data Model

### Configuration (`config.rs`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DownloadSaveLocation {
    Downloads,   // ~/Downloads (default)
    LastUsed,    // Remember last directory
    Cwd,         // Shell's current working directory
    Custom(String), // User-specified path
}

// Config fields:
download_save_location: DownloadSaveLocation  // default: Downloads
last_download_directory: Option<String>        // internal, persisted
```

### UI State (`src/app/file_transfers.rs`)

```rust
struct FileTransferState {
    active_transfers: Vec<TransferInfo>,        // For progress overlay
    pending_saves: VecDeque<PendingSave>,       // Downloads awaiting save dialog
    pending_uploads: VecDeque<PendingUpload>,   // Uploads awaiting file picker
    show_overlay: bool,                         // Overlay visibility
    overlay_fade_start: Option<Instant>,        // For auto-hide after completion
    dialog_open: bool,                          // Prevent concurrent dialogs
}

struct TransferInfo {
    id: u64,
    filename: String,
    direction: TransferDirection,
    bytes_transferred: usize,
    total_bytes: Option<usize>,
    started_at: Instant,
}

struct PendingSave { id: u64, filename: String, data: Vec<u8> }
struct PendingUpload { format: String }
```

## Event Loop Integration

In `about_to_wait()` (handler.rs), after existing checks:

```rust
self.check_file_transfers();
```

### `check_file_transfers()` Logic

1. `try_lock()` active tab's terminal
2. Poll `get_active_transfers()` → update overlay state
3. Poll `get_completed_transfers()` → `take_completed_transfer(id)` for each download → queue to `pending_saves`
4. Poll for `UploadRequested` events → queue to `pending_uploads`
5. Process `pending_saves`: show `rfd::FileDialog::save_file()` → write bytes → notify
6. Process `pending_uploads`: show `rfd::FileDialog::pick_file()` → read file → `send_upload_data()` → notify
7. On cancel: discard data (download) or `cancel_upload()` (upload)

### Dialog Handling

- `rfd::FileDialog` is synchronous/modal (matches existing settings UI usage)
- Only one dialog at a time (`dialog_open` flag)
- Save dialog: pre-fill filename from transfer metadata, default directory from config
- File picker: no filename pre-fill, default to configured location

## TerminalManager Wrappers

New methods in `src/terminal/mod.rs`:

```rust
pub fn get_active_transfers(&self) -> Vec<FileTransfer>
pub fn get_completed_transfers(&self) -> Vec<FileTransfer>
pub fn take_completed_transfer(&self, id: u64) -> Option<FileTransfer>
pub fn cancel_file_transfer(&self, id: u64) -> bool
pub fn send_upload_data(&self, data: &[u8])
pub fn cancel_upload(&self)
pub fn has_pending_upload_request(&self) -> bool
pub fn take_upload_requests(&self) -> Vec<String>
```

Pattern: `pty_session.lock() → terminal().lock() → call method`

## Progress Overlay (egui)

- Semi-transparent panel anchored bottom-right
- Auto-shows when transfers are active, auto-hides 2s after last completion
- Each transfer row: direction icon + filename + progress bar + percentage + cancel button
- Determinate bar when `total_bytes` is known, indeterminate pulse when unknown
- Rendered in the egui pass after settings UI

## Settings UI

Add "File Transfers" section to Advanced tab (or new transfers section):
- Dropdown: `download_save_location` with 4 options
- Path picker for Custom directory
- Search keywords: `download`, `upload`, `transfer`, `save location`, `file transfer`

## Files Changed

### New
- `src/app/file_transfers.rs` — FileTransferState, check_file_transfers(), overlay rendering (~300-400 lines)

### Modified
- `src/app/handler.rs` — add `check_file_transfers()` call
- `src/app/window_state.rs` — add `FileTransferState` field
- `src/terminal/mod.rs` — add wrapper methods
- `src/config.rs` — add `DownloadSaveLocation` enum and fields
- `src/settings_ui/advanced_tab.rs` — add transfer settings UI
- `src/settings_ui/sidebar.rs` — add search keywords

## Error Handling

- File write failures → desktop notification with error message
- File read failures (upload) → desktop notification + `cancel_upload()`
- Transfer size limit exceeded → handled by core (emits `FileTransferFailed`)
- Dialog cancelled → discard download data / call `cancel_upload()`
