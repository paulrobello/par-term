//! Trigger action dispatch and sound playback.
//!
//! This module handles polling trigger action results from the core library
//! and executing frontend-handled actions: RunCommand, PlaySound, SendText.

use std::collections::HashMap;
use std::io::BufReader;
use std::path::PathBuf;

use par_term_emu_core_rust::terminal::ActionResult;

use super::window_state::WindowState;

/// (grid_row, label, color) tuple for a pending MarkLine action.
type MarkLineEntry = (usize, Option<String>, Option<(u8, u8, u8)>);

impl WindowState {
    /// Check for trigger action results and dispatch them.
    ///
    /// Called each frame after check_bell(). Polls the core library for
    /// ActionResult events and executes the appropriate frontend action.
    pub(crate) fn check_trigger_actions(&mut self) {
        let tab = if let Some(t) = self.tab_manager.active_tab() {
            t
        } else {
            return;
        };

        // Poll action results from core terminal.
        // Also grab the current scrollback_len so our absolute line calculations
        // are consistent with the row values the trigger system produced.
        let (action_results, current_scrollback_len) = if let Ok(term) = tab.terminal.try_lock() {
            (term.poll_action_results(), term.scrollback_len())
        } else {
            return;
        };

        if action_results.is_empty() {
            return;
        }

        // Collect MarkLine events for batch deduplication (processed after the loop).
        // Between frames, the core may fire the same trigger multiple times for the
        // same physical line (once per PTY read). Each scan records a different grid
        // row because scrollback grows between scans, but we only get the scrollback_len
        // at poll time. Batch dedup clusters these into one mark per physical line.
        let mut pending_marks: HashMap<u64, Vec<MarkLineEntry>> = HashMap::new();

        for action in action_results {
            match action {
                ActionResult::RunCommand {
                    trigger_id,
                    command,
                    args,
                } => {
                    log::info!(
                        "Trigger {} firing RunCommand: {} {:?}",
                        trigger_id,
                        command,
                        args
                    );
                    match std::process::Command::new(&command).args(&args).spawn() {
                        Ok(_) => log::debug!("RunCommand spawned successfully"),
                        Err(e) => {
                            log::error!("RunCommand failed to spawn '{}': {}", command, e)
                        }
                    }
                }
                ActionResult::PlaySound {
                    trigger_id,
                    sound_id,
                    volume,
                } => {
                    log::info!(
                        "Trigger {} firing PlaySound: '{}' at volume {}",
                        trigger_id,
                        sound_id,
                        volume
                    );
                    if sound_id == "bell" || sound_id.is_empty() {
                        if let Some(tab) = self.tab_manager.active_tab()
                            && let Some(ref audio_bell) = tab.bell.audio
                        {
                            audio_bell.play(volume);
                        }
                    } else {
                        Self::play_sound_file(&sound_id, volume);
                    }
                }
                ActionResult::SendText {
                    trigger_id,
                    text,
                    delay_ms,
                } => {
                    log::info!(
                        "Trigger {} firing SendText: '{}' (delay={}ms)",
                        trigger_id,
                        text,
                        delay_ms
                    );
                    if let Some(tab) = self.tab_manager.active_tab() {
                        if delay_ms == 0 {
                            if let Ok(term) = tab.terminal.try_lock()
                                && let Err(e) = term.write(text.as_bytes())
                            {
                                log::error!("SendText write failed: {}", e);
                            }
                        } else {
                            let terminal = std::sync::Arc::clone(&tab.terminal);
                            let text_owned = text;
                            std::thread::spawn(move || {
                                std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                                if let Ok(term) = terminal.try_lock()
                                    && let Err(e) = term.write(text_owned.as_bytes())
                                {
                                    log::error!("Delayed SendText write failed: {}", e);
                                }
                            });
                        }
                    }
                }
                ActionResult::Notify {
                    trigger_id,
                    title,
                    message,
                } => {
                    log::info!(
                        "Trigger {} firing Notify: '{}' - '{}'",
                        trigger_id,
                        title,
                        message
                    );
                    // Trigger notifications always deliver (bypass focus suppression)
                    // since the user explicitly configured them
                    self.deliver_notification_force(&title, &message);
                }
                ActionResult::MarkLine {
                    trigger_id,
                    row,
                    label,
                    color,
                } => {
                    pending_marks
                        .entry(trigger_id)
                        .or_default()
                        .push((row, label, color));
                }
            }
        }

        // Process collected MarkLine events with deduplication.
        if !pending_marks.is_empty() {
            self.apply_mark_line_results(pending_marks, current_scrollback_len);
        }
    }

    /// Deduplicate and apply MarkLine trigger results.
    ///
    /// Between frames, the same trigger can scan the same physical line multiple
    /// times (once per PTY read cycle). Each scan sees the line at a different
    /// grid row because scrollback grows between scans. For example, a single
    /// match might produce rows [10, 9, 8, 7] across 4 scans.
    ///
    /// This method clusters consecutive rows per trigger_id (they represent the
    /// same physical line shifting), keeps only the smallest row from each cluster
    /// (most current, consistent with `current_scrollback_len`), then updates or
    /// adds marks using trigger_id + proximity matching.
    fn apply_mark_line_results(
        &mut self,
        pending_marks: HashMap<u64, Vec<MarkLineEntry>>,
        current_scrollback_len: usize,
    ) {
        let tab = if let Some(t) = self.tab_manager.active_tab_mut() {
            t
        } else {
            return;
        };

        for (trigger_id, mut entries) in pending_marks {
            // Sort by row ascending, then cluster consecutive rows.
            // Each cluster represents the same physical line seen at different
            // scroll positions. Keep the first (smallest row) from each cluster
            // — it's the most recent and matches current_scrollback_len.
            entries.sort_by_key(|(row, _, _)| *row);
            let mut deduped: Vec<MarkLineEntry> = Vec::new();
            let mut prev_row: Option<usize> = None;
            for (row, label, color) in entries {
                if let Some(prev) = prev_row
                    && row <= prev + 1
                {
                    // Same cluster (consecutive row) — skip, keep the first
                    prev_row = Some(row);
                    continue;
                }
                // New cluster starts here
                deduped.push((row, label, color));
                prev_row = Some(row);
            }

            for (row, label, color) in deduped {
                let absolute_line = current_scrollback_len + row;
                log::info!(
                    "Trigger {} MarkLine: row={} abs={} label={:?}",
                    trigger_id,
                    row,
                    absolute_line,
                    label
                );

                // Find existing mark with same trigger_id within a proximity window.
                // The window accounts for frame-to-frame drift of ±5 lines.
                const PROXIMITY: usize = 5;
                if let Some(existing) = tab.trigger_marks.iter_mut().find(|m| {
                    m.trigger_id == Some(trigger_id) && absolute_line.abs_diff(m.line) <= PROXIMITY
                }) {
                    existing.line = absolute_line;
                    existing.command = label;
                    existing.color = color;
                } else {
                    tab.trigger_marks
                        .push(crate::scrollback_metadata::ScrollbackMark {
                            line: absolute_line,
                            exit_code: None,
                            start_time: None,
                            duration_ms: None,
                            command: label,
                            color,
                            trigger_id: Some(trigger_id),
                        });
                }
            }
        }
    }

    /// Play a sound file from the par-term sounds directory.
    fn play_sound_file(sound_id: &str, volume: u8) {
        let sounds_dir = Self::sounds_dir();
        let path = sounds_dir.join(sound_id);

        if !path.exists() {
            log::warn!("Sound file not found: {}", path.display());
            return;
        }

        let volume_f32 = (volume as f32 / 100.0).clamp(0.0, 1.0);

        std::thread::spawn(move || {
            let file = match std::fs::File::open(&path) {
                Ok(f) => f,
                Err(e) => {
                    log::error!("Failed to open sound file '{}': {}", path.display(), e);
                    return;
                }
            };
            let stream = match rodio::OutputStreamBuilder::open_default_stream() {
                Ok(s) => s,
                Err(e) => {
                    log::error!("Failed to open audio output: {}", e);
                    return;
                }
            };
            let sink = rodio::Sink::connect_new(stream.mixer());
            let source = match rodio::Decoder::new(BufReader::new(file)) {
                Ok(s) => s,
                Err(e) => {
                    log::error!("Failed to decode sound file '{}': {}", path.display(), e);
                    return;
                }
            };
            sink.set_volume(volume_f32);
            sink.append(source);
            sink.sleep_until_end();
        });
    }

    /// Get the sounds directory path.
    fn sounds_dir() -> PathBuf {
        if let Some(config_dir) = dirs::config_dir() {
            config_dir.join("par-term").join("sounds")
        } else {
            PathBuf::from("sounds")
        }
    }
}
