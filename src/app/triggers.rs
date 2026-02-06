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

    /// Apply MarkLine trigger results using a rebuild strategy.
    ///
    /// Between frames, the core fires trigger scans on every PTY read. Each
    /// scan records the match at a different grid row (because scrollback grows
    /// between reads), and the batch may contain rows like [10, 8, 6, 4] for a
    /// single physical line. Trying to cluster these is fragile.
    ///
    /// Instead, we use a rebuild approach:
    /// 1. Keep historical marks that have scrolled into scrollback (they won't
    ///    be re-scanned, so we must preserve them).
    /// 2. Discard stale marks in the visible grid for each trigger_id present
    ///    in the current batch (these will be rebuilt from fresh results).
    /// 3. Add new marks using only the smallest row per trigger_id (most
    ///    current, consistent with `current_scrollback_len`).
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

        // Remove stale visible-grid marks for trigger_ids that have fresh results.
        // Marks in scrollback (line < current_scrollback_len) are historical and
        // must be preserved since the trigger scanner only scans the visible grid.
        let trigger_ids_in_batch: Vec<u64> = pending_marks.keys().copied().collect();
        tab.trigger_marks.retain(|m| {
            if let Some(tid) = m.trigger_id
                && trigger_ids_in_batch.contains(&tid)
            {
                // Keep only if in scrollback (historical)
                return m.line < current_scrollback_len;
            }
            true // Keep marks from other triggers or shell integration
        });

        // For each trigger, deduplicate rows from the batch. The last scan
        // (producing the smallest rows) has row values consistent with
        // current_scrollback_len. We use a HashSet of rows to eliminate exact
        // duplicates, then add marks for each unique row.
        for (trigger_id, entries) in pending_marks {
            // Deduplicate: keep only unique rows, preferring the entry with
            // the smallest row (from the most recent scan).
            let mut seen_rows = std::collections::HashSet::new();
            let mut unique: Vec<MarkLineEntry> = Vec::new();
            // Process in reverse so the last (smallest-row) entry for each
            // physical line wins.
            for (row, label, color) in entries.into_iter().rev() {
                if seen_rows.insert(row) {
                    unique.push((row, label, color));
                }
            }

            for (row, label, color) in unique {
                let absolute_line = current_scrollback_len + row;
                log::info!(
                    "Trigger {} MarkLine: row={} abs={} label={:?}",
                    trigger_id,
                    row,
                    absolute_line,
                    label
                );
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
