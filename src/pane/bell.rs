use crate::audio_bell::AudioBell;
use std::time::Instant;

/// State related to audio and visual bells
pub struct BellState {
    /// Shared process-wide audio output (see [`crate::audio_bell::shared`]);
    /// `None` when no device could be opened.
    pub(crate) audio: Option<&'static AudioBell>,
    pub(crate) last_count: u64, // Last bell event count from terminal
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
            audio: crate::audio_bell::shared(),
            last_count: 0,
            visual_flash: None,
        }
    }
}
