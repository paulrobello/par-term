use crate::audio_bell::AudioBell;
use std::time::Instant;

/// State related to audio and visual bells
pub struct BellState {
    pub audio: Option<AudioBell>, // Audio bell for terminal bell sounds
    pub last_count: u64, // Last bell event count from terminal
    pub visual_flash: Option<Instant>, // When visual bell flash started (None = not flashing)
}

impl BellState {
    pub fn new() -> Self {
        Self {
            audio: {
                match AudioBell::new() {
                    Ok(bell) => {
                        log::info!("Audio bell initialized successfully");
                        Some(bell)
                    }
                    Err(e) => {
                        log::warn!("Failed to initialize audio bell: {}", e);
                        None
                    }
                }
            },
            last_count: 0,
            visual_flash: None,
        }
    }
}
