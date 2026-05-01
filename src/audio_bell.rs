// ---------------------------------------------------------------------------
// Full rodio-backed implementation (feature = "audio" enabled)
// ---------------------------------------------------------------------------

#[cfg(feature = "audio")]
use parking_lot::Mutex;
#[cfg(feature = "audio")]
use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player, Source};
#[cfg(feature = "audio")]
use std::sync::Arc;
#[cfg(feature = "audio")]
use std::time::Duration;

/// Audio bell manager for playing terminal bell sounds.
///
/// When the `audio` feature is enabled this uses `rodio` for real audio
/// playback. When disabled, all methods are no-ops so callers never need
/// conditional code.
pub struct AudioBell {
    /// Audio output device sink handle (kept alive for the duration of the application)
    #[cfg(feature = "audio")]
    stream: Option<MixerDeviceSink>,
    /// Audio player for playback
    #[cfg(feature = "audio")]
    sink: Option<Arc<Mutex<Player>>>,
}

#[cfg(feature = "audio")]
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

// ---- audio feature enabled -------------------------------------------------

#[cfg(feature = "audio")]
impl AudioBell {
    /// Create a new audio bell manager
    pub fn new() -> Result<Self, String> {
        let stream = DeviceSinkBuilder::open_default_sink()
            .map_err(|e| format!("Failed to open audio stream: {}", e))?;

        let sink = Player::connect_new(stream.mixer());

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
        self.play_tone(volume, 800.0, 100);
    }

    /// Play a tone with configurable frequency and duration
    ///
    /// # Arguments
    /// * `volume` - Volume level from 0 to 100. A value of 0 disables the sound.
    /// * `frequency` - Frequency in Hz (e.g. 800.0 for standard bell)
    /// * `duration_ms` - Duration in milliseconds
    pub fn play_tone(&self, volume: u8, frequency: f32, duration_ms: u64) {
        if volume == 0 {
            return;
        }

        let sink_arc = match &self.sink {
            Some(s) => s,
            None => return, // Audio disabled
        };

        // Clamp volume to 0-100 range and convert to 0.0-1.0
        let volume_f32 = (volume.min(100) as f32) / 100.0;

        let source = rodio::source::SineWave::new(frequency)
            .take_duration(Duration::from_millis(duration_ms))
            .amplify(volume_f32 * 0.3); // Scale down to avoid being too loud

        let sink = sink_arc.lock();
        sink.append(source);
    }

    /// Play a sound file (WAV/MP3/OGG/FLAC) at the specified volume
    ///
    /// # Arguments
    /// * `volume` - Volume level from 0 to 100
    /// * `path` - Path to the sound file
    pub fn play_file(&self, volume: u8, path: &std::path::Path) {
        if volume == 0 {
            return;
        }

        let sink_arc = match &self.sink {
            Some(s) => s,
            None => return,
        };

        let file = match std::fs::File::open(path) {
            Ok(f) => f,
            Err(e) => {
                log::warn!("Failed to open alert sound file {:?}: {}", path, e);
                return;
            }
        };

        let reader = std::io::BufReader::new(file);
        let source = match rodio::Decoder::new(reader) {
            Ok(s) => s,
            Err(e) => {
                log::warn!("Failed to decode alert sound file {:?}: {}", path, e);
                return;
            }
        };

        let volume_f32 = (volume.min(100) as f32) / 100.0;
        let source = source.amplify(volume_f32 * 0.5);

        let sink = sink_arc.lock();
        sink.append(source);
    }

    /// Play an alert sound using the given configuration
    pub fn play_alert(&self, config: &crate::config::AlertSoundConfig) {
        if !config.enabled || config.volume == 0 {
            return;
        }

        if let Some(ref sound_file) = config.sound_file {
            let path = std::path::Path::new(sound_file);
            // Expand ~ to home directory
            let expanded = if sound_file.starts_with('~') {
                if let Some(home) = dirs::home_dir() {
                    home.join(&sound_file[2..])
                } else {
                    path.to_path_buf()
                }
            } else {
                path.to_path_buf()
            };
            self.play_file(config.volume, &expanded);
        } else {
            self.play_tone(config.volume, config.frequency, config.duration_ms);
        }
    }
}

#[cfg(feature = "audio")]
impl Default for AudioBell {
    fn default() -> Self {
        Self::new().unwrap_or_else(|e| {
            log::warn!("Failed to initialize audio bell: {}", e);
            Self::disabled()
        })
    }
}

// ---- audio feature disabled (no-op stub) -----------------------------------

#[cfg(not(feature = "audio"))]
impl AudioBell {
    /// Audio is disabled at compile time; always returns the no-op stub.
    pub fn new() -> Result<Self, String> {
        Ok(Self::disabled())
    }

    /// Create a no-op audio bell (no audio hardware is used).
    pub fn disabled() -> Self {
        Self {}
    }

    /// No-op — audio feature is disabled.
    pub fn play(&self, _volume: u8) {}

    /// No-op — audio feature is disabled.
    pub fn play_tone(&self, _volume: u8, _frequency: f32, _duration_ms: u64) {}

    /// No-op — audio feature is disabled.
    pub fn play_file(&self, _volume: u8, _path: &std::path::Path) {}

    /// No-op — audio feature is disabled.
    pub fn play_alert(&self, _config: &crate::config::AlertSoundConfig) {}
}

#[cfg(not(feature = "audio"))]
impl Default for AudioBell {
    fn default() -> Self {
        Self::disabled()
    }
}

#[cfg(all(test, feature = "audio"))]
mod tests_audio {
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

#[cfg(all(test, not(feature = "audio")))]
mod tests_no_audio {
    use super::*;

    #[test]
    fn test_noop_bell_new() {
        let bell = AudioBell::new();
        assert!(bell.is_ok());
    }

    #[test]
    fn test_noop_bell_default() {
        let _bell = AudioBell::default();
    }

    #[test]
    fn test_noop_bell_play() {
        let bell = AudioBell::disabled();
        bell.play(50);
        bell.play_tone(50, 800.0, 100);
        bell.play_file(50, std::path::Path::new("/nonexistent"));
        bell.play_alert(&crate::config::AlertSoundConfig::default());
    }
}
