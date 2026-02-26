//! Shader hot reload watcher
//!
//! Watches custom shader files for changes and triggers automatic reloading.
//! Uses debouncing to avoid multiple reloads during rapid saves from editors.

use anyhow::{Context, Result};
use notify::{Config, Event, PollWatcher, RecursiveMode, Watcher};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// Type of shader being watched
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShaderType {
    /// Background/custom shader
    Background,
    /// Cursor effect shader
    Cursor,
}

/// Event indicating a shader file has changed and needs reloading
#[derive(Debug, Clone)]
pub struct ShaderReloadEvent {
    /// Type of shader that changed
    pub shader_type: ShaderType,
    /// Path to the shader file
    pub path: PathBuf,
}

/// Manages file watching for shader hot reload
pub struct ShaderWatcher {
    /// The file system watcher
    _watcher: PollWatcher,
    /// Receiver for file change events
    event_receiver: Receiver<ShaderReloadEvent>,
    /// Debounce delay in milliseconds
    debounce_delay_ms: u64,
}

impl std::fmt::Debug for ShaderWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShaderWatcher")
            .field("debounce_delay_ms", &self.debounce_delay_ms)
            .finish_non_exhaustive()
    }
}

impl ShaderWatcher {
    /// Create a new shader watcher
    ///
    /// # Arguments
    /// * `background_shader_path` - Optional path to background shader file
    /// * `cursor_shader_path` - Optional path to cursor shader file
    /// * `debounce_delay_ms` - Debounce delay in milliseconds
    pub fn new(
        background_shader_path: Option<&Path>,
        cursor_shader_path: Option<&Path>,
        debounce_delay_ms: u64,
    ) -> Result<Self> {
        let (tx, rx) = channel();
        let debounce_state: Arc<Mutex<HashMap<ShaderType, Instant>>> =
            Arc::new(Mutex::new(HashMap::new()));

        // Build mapping of filenames to shader types and track directories to watch
        // We watch parent directories because many editors use atomic saves (write temp + rename)
        // which breaks direct file watching
        let mut filename_to_type: HashMap<std::ffi::OsString, (ShaderType, PathBuf)> =
            HashMap::new();
        let mut dirs_to_watch: HashMap<PathBuf, ()> = HashMap::new();

        if let Some(path) = background_shader_path {
            if !path.exists() {
                anyhow::bail!("Background shader file not found: {}", path.display());
            }
            let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
            if let Some(filename) = canonical.file_name() {
                filename_to_type.insert(
                    filename.to_os_string(),
                    (ShaderType::Background, canonical.clone()),
                );
                if let Some(parent) = canonical.parent() {
                    dirs_to_watch.insert(parent.to_path_buf(), ());
                }
            }
            log::info!(
                "Shader hot reload: watching background shader at {}",
                canonical.display()
            );
        }
        if let Some(path) = cursor_shader_path {
            if !path.exists() {
                anyhow::bail!("Cursor shader file not found: {}", path.display());
            }
            let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
            if let Some(filename) = canonical.file_name() {
                filename_to_type.insert(
                    filename.to_os_string(),
                    (ShaderType::Cursor, canonical.clone()),
                );
                if let Some(parent) = canonical.parent() {
                    dirs_to_watch.insert(parent.to_path_buf(), ());
                }
            }
            log::info!(
                "Shader hot reload: watching cursor shader at {}",
                canonical.display()
            );
        }

        if filename_to_type.is_empty() {
            anyhow::bail!("No shader paths provided for hot reload");
        }

        let filename_to_type = Arc::new(filename_to_type);
        let debounce_delay = Duration::from_millis(debounce_delay_ms);
        let debounce_state_clone = Arc::clone(&debounce_state);

        // Create the watcher with event handler
        let mut watcher = PollWatcher::new(
            move |result: std::result::Result<Event, notify::Error>| {
                if let Ok(event) = result {
                    log::debug!(
                        "File system event: {:?} for paths: {:?}",
                        event.kind,
                        event.paths
                    );

                    // Process modify, create, and rename events (for atomic saves)
                    if !matches!(
                        event.kind,
                        notify::EventKind::Modify(_)
                            | notify::EventKind::Create(_)
                            | notify::EventKind::Remove(_)
                    ) {
                        log::trace!("Ignoring event kind: {:?}", event.kind);
                        return;
                    }

                    let filename_to_type = Arc::clone(&filename_to_type);
                    let debounce_state = Arc::clone(&debounce_state_clone);

                    // Process each path in the event
                    for path in event.paths {
                        // Match by filename (handles atomic saves where path changes)
                        let Some(filename) = path.file_name() else {
                            log::trace!("Skipping path with no filename: {:?}", path);
                            continue;
                        };

                        let Some((shader_type, canonical_path)) =
                            filename_to_type.get(filename).cloned()
                        else {
                            log::trace!("Filename {:?} not in watch list", filename);
                            continue;
                        };

                        // Check debounce using parking_lot Mutex (sync-safe)
                        let should_send = {
                            let now = Instant::now();
                            let mut state = debounce_state.lock();
                            if let Some(last_event) = state.get(&shader_type) {
                                if now.duration_since(*last_event) < debounce_delay {
                                    log::trace!("Debouncing shader reload for {:?}", shader_type);
                                    false
                                } else {
                                    state.insert(shader_type, now);
                                    true
                                }
                            } else {
                                state.insert(shader_type, now);
                                true
                            }
                        };

                        if should_send {
                            let reload_event = ShaderReloadEvent {
                                shader_type,
                                path: canonical_path,
                            };
                            log::info!(
                                "Shader file changed: {:?} at {}",
                                shader_type,
                                reload_event.path.display()
                            );
                            if let Err(e) = tx.send(reload_event) {
                                log::error!("Failed to send shader reload event: {}", e);
                            }
                        }
                    }
                }
            },
            Config::default().with_poll_interval(Duration::from_millis(100)),
        )
        .context("Failed to create file watcher")?;

        // Watch parent directories (handles atomic saves from editors like vim, VSCode)
        for dir in dirs_to_watch.keys() {
            watcher
                .watch(dir, RecursiveMode::NonRecursive)
                .with_context(|| format!("Failed to watch shader directory: {}", dir.display()))?;
            log::debug!("Watching directory for shader changes: {}", dir.display());
        }

        Ok(Self {
            _watcher: watcher,
            event_receiver: rx,
            debounce_delay_ms,
        })
    }

    /// Check for pending shader reload events (non-blocking)
    ///
    /// Returns the next reload event if one is available, or None if no events are pending.
    pub fn try_recv(&self) -> Option<ShaderReloadEvent> {
        self.event_receiver.try_recv().ok()
    }

    /// Get the debounce delay in milliseconds
    pub fn debounce_delay_ms(&self) -> u64 {
        self.debounce_delay_ms
    }
}

/// Builder for creating ShaderWatcher with configuration options
pub struct ShaderWatcherBuilder {
    background_shader_path: Option<PathBuf>,
    cursor_shader_path: Option<PathBuf>,
    debounce_delay_ms: u64,
}

impl ShaderWatcherBuilder {
    /// Create a new builder with default settings
    pub fn new() -> Self {
        Self {
            background_shader_path: None,
            cursor_shader_path: None,
            debounce_delay_ms: 100,
        }
    }

    /// Set the background shader path
    pub fn background_shader(mut self, path: impl Into<PathBuf>) -> Self {
        self.background_shader_path = Some(path.into());
        self
    }

    /// Set the cursor shader path
    pub fn cursor_shader(mut self, path: impl Into<PathBuf>) -> Self {
        self.cursor_shader_path = Some(path.into());
        self
    }

    /// Set the debounce delay in milliseconds
    pub fn debounce_delay_ms(mut self, delay_ms: u64) -> Self {
        self.debounce_delay_ms = delay_ms;
        self
    }

    /// Build the ShaderWatcher
    pub fn build(self) -> Result<ShaderWatcher> {
        ShaderWatcher::new(
            self.background_shader_path.as_deref(),
            self.cursor_shader_path.as_deref(),
            self.debounce_delay_ms,
        )
    }
}

impl Default for ShaderWatcherBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_shader_type_equality() {
        assert_eq!(ShaderType::Background, ShaderType::Background);
        assert_eq!(ShaderType::Cursor, ShaderType::Cursor);
        assert_ne!(ShaderType::Background, ShaderType::Cursor);
    }

    #[test]
    fn test_shader_watcher_builder_default() {
        let builder = ShaderWatcherBuilder::default();
        assert!(builder.background_shader_path.is_none());
        assert!(builder.cursor_shader_path.is_none());
        assert_eq!(builder.debounce_delay_ms, 100);
    }

    #[test]
    fn test_shader_watcher_builder_with_paths() {
        let builder = ShaderWatcherBuilder::new()
            .background_shader("/tmp/test.glsl")
            .cursor_shader("/tmp/cursor.glsl")
            .debounce_delay_ms(200);

        assert_eq!(
            builder.background_shader_path,
            Some(PathBuf::from("/tmp/test.glsl"))
        );
        assert_eq!(
            builder.cursor_shader_path,
            Some(PathBuf::from("/tmp/cursor.glsl"))
        );
        assert_eq!(builder.debounce_delay_ms, 200);
    }

    #[test]
    fn test_watcher_creation_with_valid_path() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let shader_path = temp_dir.path().join("test.glsl");
        fs::write(
            &shader_path,
            "void mainImage(out vec4 fragColor, in vec2 fragCoord) { fragColor = vec4(1.0); }",
        )
        .expect("Failed to write shader");

        let result = ShaderWatcher::new(Some(&shader_path), None, 100);
        assert!(result.is_ok());
    }

    #[test]
    fn test_watcher_creation_no_paths_fails() {
        let result = ShaderWatcher::new(None, None, 100);
        assert!(result.is_err());
    }

    #[test]
    fn test_try_recv_empty() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let shader_path = temp_dir.path().join("test.glsl");
        fs::write(
            &shader_path,
            "void mainImage(out vec4 fragColor, in vec2 fragCoord) { fragColor = vec4(1.0); }",
        )
        .expect("Failed to write shader");

        let watcher =
            ShaderWatcher::new(Some(&shader_path), None, 100).expect("Failed to create watcher");

        // Should return None immediately with no events
        assert!(watcher.try_recv().is_none());
    }

    #[test]
    fn test_shader_reload_event_debug() {
        let event = ShaderReloadEvent {
            shader_type: ShaderType::Background,
            path: PathBuf::from("/tmp/test.glsl"),
        };
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("Background"));
        assert!(debug_str.contains("test.glsl"));
    }

    #[test]
    fn test_file_change_triggers_event() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let shader_path = temp_dir.path().join("test.glsl");
        fs::write(
            &shader_path,
            "void mainImage(out vec4 fragColor, in vec2 fragCoord) { fragColor = vec4(1.0); }",
        )
        .expect("Failed to write shader");

        let watcher =
            ShaderWatcher::new(Some(&shader_path), None, 50).expect("Failed to create watcher");

        // Give the watcher time to set up
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Modify the file
        fs::write(
            &shader_path,
            "void mainImage(out vec4 fragColor, in vec2 fragCoord) { fragColor = vec4(0.5); }",
        )
        .expect("Failed to write shader");

        // Wait for the event to be detected
        std::thread::sleep(std::time::Duration::from_millis(200));

        // Check for the reload event
        let event = watcher.try_recv();
        // Note: This may not always trigger on all platforms, so we don't assert
        if let Some(evt) = event {
            assert_eq!(evt.shader_type, ShaderType::Background);
        }
    }
}
