//! Shader utility functions and state management for the settings UI.

use super::SettingsUI;

impl SettingsUI {
    /// Scan the shaders folder and return a list of shader filenames.
    pub(super) fn scan_shaders_folder() -> Vec<String> {
        let shaders_dir = crate::config::Config::shaders_dir();
        let mut shaders = Vec::new();

        // Create the shaders directory if it doesn't exist
        if !shaders_dir.exists()
            && let Err(e) = std::fs::create_dir_all(&shaders_dir)
        {
            log::warn!("Failed to create shaders directory: {}", e);
            return shaders;
        }

        // Read all .glsl files from the shaders directory
        if let Ok(entries) = std::fs::read_dir(&shaders_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file()
                    && let Some(ext) = path.extension()
                    && (ext == "glsl" || ext == "frag" || ext == "shader")
                    && let Some(name) = path.file_name()
                {
                    shaders.push(name.to_string_lossy().to_string());
                }
            }
        }

        shaders.sort();
        shaders
    }

    /// Refresh the list of available shaders.
    pub fn refresh_shaders(&mut self) {
        self.available_shaders = Self::scan_shaders_folder();
    }

    /// Get background shaders (excludes cursor_* shaders).
    pub(crate) fn background_shaders(&self) -> Vec<String> {
        self.available_shaders
            .iter()
            .filter(|s| !s.starts_with("cursor_"))
            .cloned()
            .collect()
    }

    /// Get cursor shaders (only cursor_* shaders).
    pub(crate) fn cursor_shaders(&self) -> Vec<String> {
        self.available_shaders
            .iter()
            .filter(|s| s.starts_with("cursor_"))
            .cloned()
            .collect()
    }

    /// Set shader compilation error (called from app when shader fails to compile).
    pub fn set_shader_error(&mut self, error: Option<String>) {
        self.shader_editor_error = error;
    }

    /// Clear shader error.
    pub fn clear_shader_error(&mut self) {
        self.shader_editor_error = None;
    }

    /// Set cursor shader compilation error.
    pub fn set_cursor_shader_error(&mut self, error: Option<String>) {
        self.cursor_shader_editor_error = error;
    }

    /// Clear cursor shader error.
    pub fn clear_cursor_shader_error(&mut self) {
        self.cursor_shader_editor_error = None;
    }

    /// Check if cursor shader editor is visible.
    #[allow(dead_code)]
    pub fn is_cursor_shader_editor_visible(&self) -> bool {
        self.cursor_shader_editor_visible
    }

    /// Open the shader editor directly (without opening settings).
    ///
    /// Returns true if the editor was opened, false if no shader path is configured.
    pub fn open_shader_editor(&mut self) -> bool {
        if self.temp_custom_shader.is_empty() {
            log::warn!("Cannot open shader editor: no shader path configured");
            return false;
        }

        // Load shader source from file
        let shader_path = crate::config::Config::shader_path(&self.temp_custom_shader);
        match std::fs::read_to_string(&shader_path) {
            Ok(source) => {
                self.shader_editor_source = source.clone();
                self.shader_editor_original = source;
                self.shader_editor_error = None;
                self.shader_editor_visible = true;
                log::info!("Shader editor opened for: {}", shader_path.display());
                true
            }
            Err(e) => {
                self.shader_editor_error = Some(format!(
                    "Failed to read shader file '{}': {}",
                    shader_path.display(),
                    e
                ));
                self.shader_editor_visible = true; // Show editor with error
                log::error!("Failed to load shader: {}", e);
                true
            }
        }
    }

    /// Update search matches based on current query.
    pub(super) fn update_shader_search_matches(&mut self) {
        self.shader_search_matches.clear();
        self.shader_search_current = 0;

        if self.shader_search_query.is_empty() {
            return;
        }

        let query_lower = self.shader_search_query.to_lowercase();
        let source_lower = self.shader_editor_source.to_lowercase();

        let mut start = 0;
        while let Some(pos) = source_lower[start..].find(&query_lower) {
            self.shader_search_matches.push(start + pos);
            start += pos + query_lower.len();
        }
    }

    /// Move to next search match.
    pub(super) fn shader_search_next(&mut self) {
        if !self.shader_search_matches.is_empty() {
            self.shader_search_current =
                (self.shader_search_current + 1) % self.shader_search_matches.len();
        }
    }

    /// Move to previous search match.
    pub(super) fn shader_search_previous(&mut self) {
        if !self.shader_search_matches.is_empty() {
            if self.shader_search_current == 0 {
                self.shader_search_current = self.shader_search_matches.len() - 1;
            } else {
                self.shader_search_current -= 1;
            }
        }
    }

    /// Get the current match position (byte offset) if any.
    pub(super) fn shader_search_current_pos(&self) -> Option<usize> {
        if self.shader_search_matches.is_empty() {
            None
        } else {
            Some(self.shader_search_matches[self.shader_search_current])
        }
    }

    /// Check if shader editor is visible.
    pub fn is_shader_editor_visible(&self) -> bool {
        self.shader_editor_visible
    }
}
