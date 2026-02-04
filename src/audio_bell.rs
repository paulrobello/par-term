use parking_lot::Mutex;
use rodio::{OutputStream, OutputStreamBuilder, Sink, Source};
use std::sync::Arc;
use std::time::Duration;

/// Audio bell manager for playing terminal bell sounds
pub struct AudioBell {
    /// Audio output stream handle (kept alive for the duration of the application)
    stream: Option<OutputStream>,
    /// Audio sink for playback
    sink: Option<Arc<Mutex<Sink>>>,
}

impl Drop for AudioBell {
    fn drop(&mut self) {
        // Stop and clear the sink BEFORE forgetting the stream
        // This prevents use-after-free when sink tries to access the forgotten stream's mixer
        // Note: If Arc has other references, they'll clean up on their own (try_unwrap fails)
        if let Some(sink_arc) = self.sink.take()
            && let Ok(sink) = Arc::try_unwrap(sink_arc)
        {
            let sink = sink.into_inner();
            sink.stop();
        }

        // Suppress 'Dropping OutputStream' message by forgetting the stream
        // Safe now that sink is stopped/dropped
        if let Some(stream) = self.stream.take() {
            std::mem::forget(stream);
        }
    }
}

impl AudioBell {
    /// Create a new audio bell manager
    pub fn new() -> Result<Self, String> {
        let stream = OutputStreamBuilder::open_default_stream()
            .map_err(|e| format!("Failed to open audio stream: {}", e))?;

        let sink = Sink::connect_new(stream.mixer());

        Ok(Self {
            stream: Some(stream),
            sink: Some(Arc::new(Mutex::new(sink))),
        })
    }

    /// Create a dummy/disabled audio bell (safe fallback)
    pub fn disabled() -> Self {
        Self {
            stream: None,
            sink: None,
        }
    }

    /// Play a bell sound with the specified volume (0-100)
    ///
    /// # Arguments
    /// * `volume` - Volume level from 0 to 100. A value of 0 disables the bell sound.
    pub fn play(&self, volume: u8) {
        if volume == 0 {
            return;
        }

        let sink_arc = match &self.sink {
            Some(s) => s,
            None => return, // Audio disabled
        };

        // Clamp volume to 0-100 range and convert to 0.0-1.0
        let volume_f32 = (volume.min(100) as f32) / 100.0;

        // Generate a simple beep: 800 Hz sine wave for 100ms
        let source = rodio::source::SineWave::new(800.0)
            .take_duration(Duration::from_millis(100))
            .amplify(volume_f32 * 0.3); // Scale down to avoid being too loud

        let sink = sink_arc.lock();
        sink.append(source);
    }
}

impl Default for AudioBell {
    fn default() -> Self {
        Self::new().unwrap_or_else(|e| {
            log::warn!("Failed to initialize audio bell: {}", e);
            Self::disabled()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_bell_creation() {
        // Just ensure we can create the audio bell without panicking
        let bell = AudioBell::new();
        assert!(bell.is_ok() || bell.is_err());
    }

    #[test]
    fn test_audio_bell_default() {
        // Should not panic even if audio setup fails
        let _bell = AudioBell::default();
    }

    #[test]
    fn test_audio_bell_play_zero_volume() {
        if let Ok(bell) = AudioBell::new() {
            // Should not panic with zero volume
            bell.play(0);
        }
    }

    #[test]
    fn test_audio_bell_play_max_volume() {
        if let Ok(bell) = AudioBell::new() {
            // Should not panic with max volume
            bell.play(100);
        }
    }

    #[test]
    fn test_audio_bell_play_over_max_volume() {
        if let Ok(bell) = AudioBell::new() {
            // Should clamp to max volume without panicking
            bell.play(150);
        }
    }

    #[test]
    fn test_disabled_bell() {
        let bell = AudioBell::disabled();
        // Should simply do nothing, not panic
        bell.play(50);
    }
}
