//! Sound playback for trigger actions.
//!
//! Provides `play_sound_file` (fire-and-forget audio) and `sounds_dir`
//! (path resolution for sound files installed with par-term).

use std::io::BufReader;
use std::path::PathBuf;

use crate::app::window_state::WindowState;

impl WindowState {
    /// Play a sound file. Absolute paths are used directly; relative names
    /// are resolved against the par-term sounds directory.
    pub(super) fn play_sound_file(sound_id: &str, volume: u8) {
        let candidate = std::path::Path::new(sound_id);
        let path = if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            Self::sounds_dir().join(sound_id)
        };

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
            let stream = match rodio::DeviceSinkBuilder::open_default_sink() {
                Ok(s) => s,
                Err(e) => {
                    log::error!("Failed to open audio output: {}", e);
                    return;
                }
            };
            let sink = rodio::Player::connect_new(stream.mixer());
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
    pub(super) fn sounds_dir() -> PathBuf {
        if let Some(config_dir) = dirs::config_dir() {
            config_dir.join("par-term").join("sounds")
        } else {
            PathBuf::from("sounds")
        }
    }
}
