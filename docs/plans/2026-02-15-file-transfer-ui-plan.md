# File Transfer Frontend UI Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add native file dialogs and egui progress overlay for iTerm2 OSC 1337 file transfers (download save dialog, upload file picker, real-time progress).

**Architecture:** Polling-based approach matching existing `check_notifications()`/`check_bell()` pattern. New `check_file_transfers()` runs in `about_to_wait()`, polls `FileTransferManager` for completed downloads and upload requests, shows native `rfd` dialogs, and renders an egui progress overlay.

**Tech Stack:** Rust, rfd (0.17.2, already in deps), egui (existing), par-term-emu-core-rust `FileTransferManager` API

---

## Task 1: Add `DownloadSaveLocation` enum to config types

**Files:**
- Modify: `src/config/types.rs` (after `BackgroundImageMode` enum, ~line 184)

**Step 1: Add the enum**

Add after the `BackgroundImageMode` enum in `src/config/types.rs`:

```rust
/// Default save location for downloaded files
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DownloadSaveLocation {
    /// Save to ~/Downloads (default)
    #[default]
    Downloads,
    /// Remember and re-use the last directory the user saved to
    LastUsed,
    /// Use the shell's current working directory
    Cwd,
    /// Use a custom directory path
    Custom(String),
}

impl DownloadSaveLocation {
    /// Get all non-Custom variants for settings UI dropdown
    pub fn variants() -> &'static [DownloadSaveLocation] {
        &[
            DownloadSaveLocation::Downloads,
            DownloadSaveLocation::LastUsed,
            DownloadSaveLocation::Cwd,
        ]
    }

    /// Display name for settings UI
    pub fn display_name(&self) -> &str {
        match self {
            DownloadSaveLocation::Downloads => "Downloads folder",
            DownloadSaveLocation::LastUsed => "Last used directory",
            DownloadSaveLocation::Cwd => "Current working directory",
            DownloadSaveLocation::Custom(_) => "Custom directory",
        }
    }
}
```

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: PASS (enum is defined but not yet used)

**Step 3: Commit**

```bash
git add src/config/types.rs
git commit -m "feat(config): add DownloadSaveLocation enum for file transfer settings"
```

---

## Task 2: Add file transfer config fields to `Config`

**Files:**
- Modify: `src/config/mod.rs` — add fields to `Config` struct and `Default` impl

**Step 1: Add config fields**

Find the `Config` struct in `src/config/mod.rs`. Add these fields in a new section (after the background image section, around line 350):

```rust
    // File Transfer Settings

    /// Default save location for downloaded files
    #[serde(default)]
    pub download_save_location: DownloadSaveLocation,

    /// Last used download directory (persisted internally)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_download_directory: Option<String>,
```

Make sure `DownloadSaveLocation` is imported from `types.rs` (check the existing imports at top of mod.rs).

**Step 2: Add to Default impl**

Find the `Default` impl for `Config`. Add:

```rust
            download_save_location: DownloadSaveLocation::default(),
            last_download_directory: None,
```

**Step 3: Verify it compiles**

Run: `cargo check`
Expected: PASS

**Step 4: Commit**

```bash
git add src/config/mod.rs
git commit -m "feat(config): add file transfer download save location settings"
```

---

## Task 3: Add file transfer wrapper methods to `TerminalManager`

**Files:**
- Modify: `src/terminal/mod.rs` — add methods after existing `poll_action_results()` (~line 955)

**Step 1: Add wrapper methods**

Add these methods to `TerminalManager` after `poll_action_results()`:

```rust
    // === File Transfer Methods ===

    /// Get all active (in-progress) file transfers
    pub fn get_active_transfers(
        &self,
    ) -> Vec<par_term_emu_core_rust::terminal::file_transfer::FileTransfer> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.get_active_transfers()
    }

    /// Get all completed file transfers (without removing them)
    pub fn get_completed_transfers(
        &self,
    ) -> Vec<par_term_emu_core_rust::terminal::file_transfer::FileTransfer> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let term = terminal.lock();
        term.get_completed_transfers()
    }

    /// Take a completed transfer by ID, removing it from the manager
    pub fn take_completed_transfer(
        &self,
        id: u64,
    ) -> Option<par_term_emu_core_rust::terminal::file_transfer::FileTransfer> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.take_completed_transfer(id)
    }

    /// Cancel an active file transfer
    pub fn cancel_file_transfer(&self, id: u64) -> bool {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.cancel_file_transfer(id)
    }

    /// Send data for an active upload (iTerm2 base64 format)
    pub fn send_upload_data(&self, data: &[u8]) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.send_upload_data(data);
    }

    /// Cancel the current upload (sends Ctrl-C)
    pub fn cancel_upload(&self) {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.cancel_upload();
    }
```

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: PASS

**Step 3: Commit**

```bash
git add src/terminal/mod.rs
git commit -m "feat(terminal): add file transfer wrapper methods to TerminalManager"
```

---

## Task 4: Create `FileTransferState` and `check_file_transfers()` module

**Files:**
- Create: `src/app/file_transfers.rs`
- Modify: `src/app/mod.rs` — add module declaration

**Step 1: Add module declaration**

In `src/app/mod.rs`, add after line 25 (`mod notifications;`):

```rust
mod file_transfers;
```

**Step 2: Create the file transfer module**

Create `src/app/file_transfers.rs`:

```rust
//! File transfer UI handling for downloads and uploads.
//!
//! This module provides:
//! - Polling for completed downloads and upload requests
//! - Native save dialog for downloads (via rfd)
//! - Native file picker for uploads (via rfd)
//! - egui overlay for transfer progress display

use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::Instant;

use par_term_emu_core_rust::terminal::file_transfer::{TransferDirection, TransferStatus};

use super::window_state::WindowState;
use crate::config::types::DownloadSaveLocation;

/// Tracks UI state for file transfers (progress overlay, pending dialogs)
#[derive(Debug)]
pub(crate) struct FileTransferState {
    /// Active transfers being displayed in the progress overlay
    pub active_transfers: Vec<TransferInfo>,
    /// Completed downloads awaiting save dialog
    pub pending_saves: VecDeque<PendingSave>,
    /// Pending upload requests awaiting file picker
    pub pending_uploads: VecDeque<PendingUpload>,
    /// Whether a native dialog is currently open (prevent concurrent dialogs)
    pub dialog_open: bool,
    /// Time when the last transfer completed (for auto-hide)
    pub last_completion_time: Option<Instant>,
}

/// Info about an active transfer for the progress overlay
#[derive(Debug, Clone)]
pub(crate) struct TransferInfo {
    pub id: u64,
    pub filename: String,
    pub direction: TransferDirection,
    pub bytes_transferred: usize,
    pub total_bytes: Option<usize>,
    pub started_at: Instant,
}

/// A completed download waiting for the user to choose a save location
#[derive(Debug)]
pub(crate) struct PendingSave {
    pub id: u64,
    pub filename: String,
    pub data: Vec<u8>,
}

/// An upload request waiting for the user to pick a file
#[derive(Debug)]
pub(crate) struct PendingUpload {
    pub format: String,
}

impl Default for FileTransferState {
    fn default() -> Self {
        Self {
            active_transfers: Vec::new(),
            pending_saves: VecDeque::new(),
            pending_uploads: VecDeque::new(),
            dialog_open: false,
            last_completion_time: None,
        }
    }
}

impl WindowState {
    /// Poll for file transfer events and process pending dialogs.
    ///
    /// Called every frame from `about_to_wait()`. This method:
    /// 1. Updates the active transfer list for the progress overlay
    /// 2. Collects completed downloads into the pending saves queue
    /// 3. Shows native save dialogs for completed downloads
    /// 4. Shows native file pickers for upload requests
    pub(crate) fn check_file_transfers(&mut self) {
        let tab = if let Some(t) = self.tab_manager.active_tab() {
            t
        } else {
            return;
        };

        if let Ok(term) = tab.terminal.try_lock() {
            // 1. Update active transfers for the progress overlay
            let active = term.get_active_transfers();
            self.file_transfer_state.active_transfers = active
                .iter()
                .map(|t| {
                    let (bytes_transferred, total_bytes) = match &t.status {
                        TransferStatus::InProgress {
                            bytes_transferred,
                            total_bytes,
                        } => (*bytes_transferred, *total_bytes),
                        _ => (0, None),
                    };
                    TransferInfo {
                        id: t.id,
                        filename: t.filename.clone(),
                        direction: t.direction,
                        bytes_transferred,
                        total_bytes,
                        started_at: Instant::now(), // Approximate — we don't have the real start
                    }
                })
                .collect();

            // 2. Collect completed downloads
            let completed = term.get_completed_transfers();
            for transfer in &completed {
                if transfer.direction == TransferDirection::Download
                    && matches!(transfer.status, TransferStatus::Completed)
                {
                    if let Some(taken) = term.take_completed_transfer(transfer.id) {
                        self.file_transfer_state
                            .pending_saves
                            .push_back(PendingSave {
                                id: taken.id,
                                filename: taken.filename.clone(),
                                data: taken.data,
                            });
                        self.deliver_notification(
                            "File Download Complete",
                            &format!("Ready to save: {}", taken.filename),
                        );
                    }
                }

                // Notify on failures
                if let TransferStatus::Failed(reason) = &transfer.status {
                    let _ = term.take_completed_transfer(transfer.id);
                    self.deliver_notification(
                        "File Transfer Failed",
                        &format!(
                            "{}: {}",
                            transfer.filename,
                            reason
                        ),
                    );
                    self.file_transfer_state.last_completion_time = Some(Instant::now());
                }
            }
        }

        // 3. Process pending saves (show native save dialog)
        if !self.file_transfer_state.dialog_open
            && let Some(pending) = self.file_transfer_state.pending_saves.pop_front()
        {
            self.file_transfer_state.dialog_open = true;
            self.process_save_dialog(pending);
            self.file_transfer_state.dialog_open = false;
            self.file_transfer_state.last_completion_time = Some(Instant::now());
        }

        // 4. Process pending uploads (show native file picker)
        if !self.file_transfer_state.dialog_open
            && let Some(pending) = self.file_transfer_state.pending_uploads.pop_front()
        {
            self.file_transfer_state.dialog_open = true;
            self.process_upload_dialog(pending);
            self.file_transfer_state.dialog_open = false;
            self.file_transfer_state.last_completion_time = Some(Instant::now());
        }
    }

    /// Show a native save dialog and write the downloaded file data.
    fn process_save_dialog(&mut self, pending: PendingSave) {
        let default_dir = self.resolve_download_directory();
        let filename = if pending.filename.is_empty() {
            "download".to_string()
        } else {
            pending.filename.clone()
        };

        let mut dialog = rfd::FileDialog::new().set_file_name(&filename);

        if let Some(dir) = &default_dir {
            dialog = dialog.set_directory(dir);
        }

        if let Some(path) = dialog.save_file() {
            match std::fs::write(&path, &pending.data) {
                Ok(()) => {
                    crate::debug_info!(
                        "FILE_TRANSFER",
                        "Saved download '{}' to {:?} ({} bytes)",
                        pending.filename,
                        path,
                        pending.data.len()
                    );
                    self.deliver_notification(
                        "File Saved",
                        &format!(
                            "{} saved ({} bytes)",
                            path.file_name()
                                .unwrap_or_default()
                                .to_string_lossy(),
                            pending.data.len()
                        ),
                    );

                    // Update last-used directory
                    if let Some(parent) = path.parent() {
                        self.config.last_download_directory =
                            Some(parent.to_string_lossy().to_string());
                    }
                }
                Err(e) => {
                    log::error!("Failed to save file {:?}: {}", path, e);
                    self.deliver_notification(
                        "File Save Failed",
                        &format!("Could not save {}: {}", pending.filename, e),
                    );
                }
            }
        } else {
            crate::debug_info!(
                "FILE_TRANSFER",
                "User cancelled save dialog for '{}'",
                pending.filename
            );
        }
    }

    /// Show a native file picker and send the selected file for upload.
    fn process_upload_dialog(&mut self, _pending: PendingUpload) {
        let dialog = rfd::FileDialog::new();

        if let Some(path) = dialog.pick_file() {
            match std::fs::read(&path) {
                Ok(data) => {
                    let tab = if let Some(t) = self.tab_manager.active_tab() {
                        t
                    } else {
                        return;
                    };

                    if let Ok(term) = tab.terminal.try_lock() {
                        term.send_upload_data(&data);
                        crate::debug_info!(
                            "FILE_TRANSFER",
                            "Sent upload data from {:?} ({} bytes)",
                            path,
                            data.len()
                        );
                        self.deliver_notification(
                            "File Uploaded",
                            &format!(
                                "{} sent ({} bytes)",
                                path.file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy(),
                                data.len()
                            ),
                        );
                    }
                }
                Err(e) => {
                    log::error!("Failed to read file {:?}: {}", path, e);
                    // Cancel the upload since we can't read the file
                    if let Some(tab) = self.tab_manager.active_tab() {
                        if let Ok(term) = tab.terminal.try_lock() {
                            term.cancel_upload();
                        }
                    }
                    self.deliver_notification(
                        "File Upload Failed",
                        &format!(
                            "Could not read {}: {}",
                            path.file_name()
                                .unwrap_or_default()
                                .to_string_lossy(),
                            e
                        ),
                    );
                }
            }
        } else {
            // User cancelled — send upload cancellation
            if let Some(tab) = self.tab_manager.active_tab() {
                if let Ok(term) = tab.terminal.try_lock() {
                    term.cancel_upload();
                }
            }
            crate::debug_info!("FILE_TRANSFER", "User cancelled upload dialog");
        }
    }

    /// Resolve the default directory for the save dialog based on config.
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
                        if let Some(cwd) = term.current_working_directory() {
                            return Some(PathBuf::from(cwd));
                        }
                    }
                }
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

    /// Render the file transfer progress overlay using egui.
    ///
    /// Call this from the egui rendering pass.
    pub(crate) fn render_file_transfer_overlay(&self, ctx: &egui::Context) {
        let state = &self.file_transfer_state;

        // Determine if overlay should be visible
        let has_active = !state.active_transfers.is_empty();
        let has_pending = !state.pending_saves.is_empty() || !state.pending_uploads.is_empty();
        let recently_completed = state
            .last_completion_time
            .is_some_and(|t| t.elapsed().as_secs() < 2);

        if !has_active && !has_pending && !recently_completed {
            return;
        }

        egui::Window::new("File Transfers")
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-10.0, -10.0))
            .collapsible(false)
            .resizable(false)
            .title_bar(true)
            .default_width(300.0)
            .show(ctx, |ui| {
                for transfer in &state.active_transfers {
                    ui.horizontal(|ui| {
                        // Direction icon
                        let icon = match transfer.direction {
                            TransferDirection::Download => "\u{2B07}", // ⬇
                            TransferDirection::Upload => "\u{2B06}",   // ⬆
                        };
                        ui.label(icon);

                        // Filename (truncated)
                        let name = if transfer.filename.len() > 30 {
                            format!("...{}", &transfer.filename[transfer.filename.len() - 27..])
                        } else {
                            transfer.filename.clone()
                        };
                        ui.label(&name);
                    });

                    // Progress bar
                    if let Some(total) = transfer.total_bytes {
                        let progress = if total > 0 {
                            transfer.bytes_transferred as f32 / total as f32
                        } else {
                            0.0
                        };
                        let text = format!(
                            "{} / {}",
                            format_bytes(transfer.bytes_transferred),
                            format_bytes(total)
                        );
                        ui.add(
                            egui::ProgressBar::new(progress)
                                .text(text)
                                .animate(false),
                        );
                    } else {
                        // Indeterminate progress
                        let text = format!("{} (unknown total)", format_bytes(transfer.bytes_transferred));
                        ui.add(
                            egui::ProgressBar::new(0.0)
                                .text(text)
                                .animate(true),
                        );
                    }

                    ui.add_space(4.0);
                }

                // Show pending saves count
                if !state.pending_saves.is_empty() {
                    ui.label(format!(
                        "{} download(s) ready to save",
                        state.pending_saves.len()
                    ));
                }

                // Show pending uploads count
                if !state.pending_uploads.is_empty() {
                    ui.label(format!(
                        "{} upload(s) pending",
                        state.pending_uploads.len()
                    ));
                }
            });
    }
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
```

**Step 3: Verify it compiles**

Run: `cargo check`
Expected: May need to add `dirs` crate if not already present. Check Cargo.toml.

**Step 4: Commit**

```bash
git add src/app/file_transfers.rs src/app/mod.rs
git commit -m "feat(app): add file transfer state and check_file_transfers handler"
```

---

## Task 5: Wire `check_file_transfers()` into event loop and add state field

**Files:**
- Modify: `src/app/window_state.rs` — add `file_transfer_state` field (~line 274, ~line 403)
- Modify: `src/app/handler.rs` — add `check_file_transfers()` call (~line 795)

**Step 1: Add field to WindowState struct**

In `src/app/window_state.rs`, add before the closing brace of `WindowState` struct (before line 275):

```rust
    // File transfers
    /// File transfer UI state (progress overlay, pending dialogs)
    pub(crate) file_transfer_state: crate::app::file_transfers::FileTransferState,
```

**Step 2: Initialize in constructor**

In the `Self { ... }` block in `WindowState::new()`, add before the closing brace (before line 404):

```rust
            file_transfer_state: crate::app::file_transfers::FileTransferState::default(),
```

**Step 3: Add check to event loop**

In `src/app/handler.rs`, after `self.check_notifications();` (line 795), add:

```rust
        // Check for file transfer events (downloads, uploads, progress)
        self.check_file_transfers();
```

**Step 4: Verify it compiles**

Run: `cargo check`
Expected: PASS

**Step 5: Commit**

```bash
git add src/app/window_state.rs src/app/handler.rs
git commit -m "feat(app): wire file transfer checking into event loop"
```

---

## Task 6: Handle `UploadRequested` events

**Files:**
- Modify: `src/app/file_transfers.rs` — add upload request detection to `check_file_transfers()`

The `UploadRequested` event is emitted as a `TerminalEvent` and flows through the observer system. We need to detect it. The simplest approach: add a method to `TerminalManager` that drains `UploadRequested` events from the terminal's event queue, following the `poll_cwd_events()` pattern.

**Step 1: Add `poll_upload_requests()` to core library**

In `/Users/probello/Repos/par-term-emu-core-rust/src/terminal/mod.rs`, after `poll_cwd_events()` (~line 2302), add:

```rust
    /// Poll for upload request events
    ///
    /// Returns all pending UploadRequested events and removes them from the queue.
    pub fn poll_upload_requests(&mut self) -> Vec<String> {
        let events = std::mem::take(&mut self.terminal_events);
        let mut upload_formats = Vec::new();
        let mut remaining = Vec::new();

        for event in events {
            if let TerminalEvent::UploadRequested { format } = event {
                upload_formats.push(format);
            } else {
                remaining.push(event);
            }
        }

        self.terminal_events = remaining;
        upload_formats
    }
```

**Step 2: Add wrapper to TerminalManager**

In `src/terminal/mod.rs`, add to the file transfer methods section:

```rust
    /// Poll for pending upload requests from the terminal
    pub fn poll_upload_requests(&self) -> Vec<String> {
        let pty = self.pty_session.lock();
        let terminal = pty.terminal();
        let mut term = terminal.lock();
        term.poll_upload_requests()
    }
```

**Step 3: Update check_file_transfers to poll upload requests**

In `src/app/file_transfers.rs`, inside the `if let Ok(term) = tab.terminal.try_lock()` block, after collecting completed downloads, add:

```rust
            // 3. Poll for upload requests
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
```

And renumber the existing comments (save dialog becomes 4, upload dialog becomes 5).

**Step 4: Verify it compiles**

Run: `cargo check`
Expected: PASS

**Step 5: Commit**

```bash
# In core library
cd /Users/probello/Repos/par-term-emu-core-rust
git add src/terminal/mod.rs
git commit -m "feat(terminal): add poll_upload_requests() for frontend upload handling"

# In par-term
cd /Users/probello/Repos/par-term
git add src/terminal/mod.rs src/app/file_transfers.rs
git commit -m "feat(app): handle UploadRequested events in file transfer handler"
```

---

## Task 7: Wire egui progress overlay into render pass

**Files:**
- Modify: The file where egui rendering happens — find where `egui_renderer` or `settings_window` rendering occurs

**Step 1: Find the egui render location**

Search for where egui rendering is called. Look for `egui` rendering in the render pass — likely in `src/app/handler.rs` or `src/renderer/mod.rs`. The overlay needs to render after the settings UI overlay.

The call will look something like:

```rust
// In the egui rendering section
self.render_file_transfer_overlay(&egui_ctx);
```

Add this call right after the settings window rendering in the egui pass. The exact location depends on where `egui_ctx.run()` is invoked — look for patterns like `self.settings_window.show()` or `egui_renderer.render()`.

**Step 2: Verify it compiles and renders**

Run: `cargo check`
Expected: PASS

**Step 3: Commit**

```bash
git add <modified files>
git commit -m "feat(renderer): wire file transfer progress overlay into egui render pass"
```

---

## Task 8: Add file transfer settings to Settings UI

**Files:**
- Modify: `src/settings_ui/advanced_tab.rs` — add file transfer section
- Modify: `src/settings_ui/sidebar.rs` — add search keywords

**Step 1: Add settings section to advanced tab**

In `src/settings_ui/advanced_tab.rs`, add a new collapsible section for file transfer settings. Follow the existing pattern of `section_matches()` + `collapsing_section()`:

```rust
    // File Transfer section
    if section_matches(query, "File Transfers", &["download", "upload", "transfer", "save"]) {
        collapsing_section(ui, collapsed, "File Transfers", |ui| {
            ui.label("Download save location:");

            let current = &settings.config.download_save_location;
            let mut selected_idx = match current {
                DownloadSaveLocation::Downloads => 0,
                DownloadSaveLocation::LastUsed => 1,
                DownloadSaveLocation::Cwd => 2,
                DownloadSaveLocation::Custom(_) => 3,
            };

            let labels = [
                "Downloads folder",
                "Last used directory",
                "Current working directory",
                "Custom directory",
            ];

            egui::ComboBox::from_label("")
                .selected_text(labels[selected_idx])
                .show_ui(ui, |ui| {
                    for (i, label) in labels.iter().enumerate() {
                        if ui.selectable_value(&mut selected_idx, i, *label).changed() {
                            settings.config.download_save_location = match i {
                                0 => DownloadSaveLocation::Downloads,
                                1 => DownloadSaveLocation::LastUsed,
                                2 => DownloadSaveLocation::Cwd,
                                3 => DownloadSaveLocation::Custom(String::new()),
                                _ => unreachable!(),
                            };
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    }
                });

            // Show custom path picker when Custom is selected
            if let DownloadSaveLocation::Custom(ref mut path) = settings.config.download_save_location {
                ui.horizontal(|ui| {
                    let response = ui.text_edit_singleline(path);
                    if response.changed() {
                        settings.has_changes = true;
                        *changes_this_frame = true;
                    }
                    if ui.button("Browse...").clicked() {
                        if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                            *path = dir.to_string_lossy().to_string();
                            settings.has_changes = true;
                            *changes_this_frame = true;
                        }
                    }
                });
            }
        });
    }
```

**Step 2: Add search keywords**

In `src/settings_ui/sidebar.rs`, find the `tab_search_keywords()` function and add keywords for the Advanced tab (where file transfer settings live). In the `SettingsTab::Advanced` match arm, add:

```rust
            "download",
            "upload",
            "transfer",
            "file transfer",
            "save location",
            "save directory",
```

**Step 3: Verify it compiles**

Run: `cargo check`
Expected: PASS

**Step 4: Commit**

```bash
git add src/settings_ui/advanced_tab.rs src/settings_ui/sidebar.rs
git commit -m "feat(settings): add file transfer download location settings UI"
```

---

## Task 9: Add `dirs` crate dependency (if needed)

**Files:**
- Modify: `Cargo.toml` — add `dirs` dependency for `dirs::download_dir()`

**Step 1: Check if `dirs` is already a dependency**

Search Cargo.toml for `dirs`. If not present, add:

```toml
dirs = "6"
```

If the project uses `directories` or another crate for dir resolution, use that instead.

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: PASS

**Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore(deps): add dirs crate for download directory resolution"
```

---

## Task 10: Build and test the full feature

**Step 1: Run full build**

Run: `cargo build`
Expected: PASS with no errors

**Step 2: Run clippy**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: PASS (fix any warnings)

**Step 3: Run tests**

Run: `cargo test`
Expected: All existing tests pass

**Step 4: Run format check**

Run: `cargo fmt -- --check`
Expected: PASS (run `cargo fmt` if needed)

**Step 5: Commit any fixes**

```bash
git add -A
git commit -m "fix: address clippy warnings and formatting in file transfer code"
```

---

## Task 11: Update MATRIX.md

**Files:**
- Modify: `MATRIX.md` — update section 33 file transfer rows

**Step 1: Update file transfer status**

Find section 33 (Image Protocol Enhancements) in MATRIX.md. Update the file transfer rows:
- Download (33.3): Change from 🔶 to ✅ (Core ✅, Frontend ✅)
- Upload (33.4): Change from 🔶 to ✅ (Core ✅, Frontend ✅)

**Step 2: Commit**

```bash
git add MATRIX.md
git commit -m "docs(matrix): update file transfer status to complete"
```

---

## Task 12: Update CHANGELOG.md

**Files:**
- Modify: `CHANGELOG.md`

**Step 1: Add entry**

Add under the appropriate version section (or Unreleased):

```markdown
### Added
- File transfer frontend UI: native save dialog for downloads, file picker for uploads (#154)
- Real-time file transfer progress overlay with multi-transfer support
- Configurable default save location for downloads (Downloads folder, last used, CWD, custom)
- Desktop notifications for file transfer lifecycle events (start, complete, fail)
```

**Step 2: Commit**

```bash
git add CHANGELOG.md
git commit -m "docs(changelog): add file transfer UI feature entry"
```

---

## Task 13: Create pull request

**Step 1: Push branch**

```bash
git push -u origin feat/file-transfer-ui
```

**Step 2: Create PR**

```bash
gh pr create --title "feat: add file transfer frontend UI for downloads and uploads" --body "$(cat <<'EOF'
## Summary
- Native save dialog when file downloads complete via iTerm2 OSC 1337 protocol
- Native file picker when remote application requests file upload
- Real-time egui progress overlay showing all active transfers with progress bars
- Configurable default save location (Downloads, last used, CWD, custom path)
- Desktop notifications for transfer lifecycle events
- Settings UI for download save location preference

Closes #154

## Test plan
- [ ] Test download: use `it2dl` or equivalent iTerm2 download command
- [ ] Verify save dialog appears with correct filename pre-filled
- [ ] Verify file is written correctly to chosen location
- [ ] Test upload: trigger iTerm2 upload request
- [ ] Verify file picker appears and selected file is sent
- [ ] Test cancel: cancel save dialog, verify no crash
- [ ] Test cancel upload: cancel file picker, verify upload cancellation sent
- [ ] Test progress overlay: verify it appears during active transfers
- [ ] Test settings: change download save location in Settings > Advanced
- [ ] Run `cargo test` — all tests pass
- [ ] Run `cargo clippy` — no warnings
EOF
)"
```
