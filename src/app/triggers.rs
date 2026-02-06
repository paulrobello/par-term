//! Trigger action dispatch and sound playback.
//!
//! This module handles polling trigger action results from the core library
//! and executing frontend-handled actions: RunCommand, PlaySound, SendText.

use std::io::BufReader;
use std::path::PathBuf;

use par_term_emu_core_rust::terminal::ActionResult;

use super::window_state::WindowState;

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

        // Poll action results from core terminal
        let action_results = if let Ok(term) = tab.terminal.try_lock() {
            term.poll_action_results()
        } else {
            return;
        };

        if action_results.is_empty() {
            return;
        }

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
                    log::info!(
                        "Trigger {} firing MarkLine: row={} label={:?} color={:?}",
                        trigger_id,
                        row,
                        label,
                        color
                    );
                    // Convert grid row to absolute line for scrollbar mark positioning
                    if let Some(tab) = self.tab_manager.active_tab_mut() {
                        let scrollback_len = tab.cache.scrollback_len;
                        let absolute_line = scrollback_len + row;
                        tab.trigger_marks
                            .push(crate::scrollback_metadata::ScrollbackMark {
                                line: absolute_line,
                                exit_code: None,
                                start_time: None,
                                duration_ms: None,
                                command: label,
                                color,
                            });
                    }
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
