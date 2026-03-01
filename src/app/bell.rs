use crate::audio_bell::AudioBell;
use std::time::Instant;

/// State related to audio and visual bells
pub(crate) struct BellState {
    pub(crate) audio: Option<AudioBell>, // Audio bell for terminal bell sounds
    pub(crate) last_count: u64,          // Last bell event count from terminal
    pub(crate) visual_flash: Option<Instant>, // When visual bell flash started (None = not flashing)
}

impl Default for BellState {
    fn default() -> Self {
        Self::new()
    }
}

impl BellState {
    pub(crate) fn new() -> Self {
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
