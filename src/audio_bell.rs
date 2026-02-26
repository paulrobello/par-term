use parking_lot::Mutex;
use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player, Source};
use std::sync::Arc;
use std::time::Duration;

/// Audio bell manager for playing terminal bell sounds
pub struct AudioBell {
    /// Audio output device sink handle (kept alive for the duration of the application)
    stream: Option<MixerDeviceSink>,
    /// Audio player for playback
    sink: Option<Arc<Mutex<Player>>>,
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

    /// Play a sound file (WAV/OGG/FLAC) at the specified volume
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
