//! Shader metadata parsing and caching.
//!
//! Parses embedded YAML metadata from shader files in the format:
//!
//! ```glsl
//! /*! par-term shader metadata
//! name: "CRT Effect"
//! author: "Timothy Lottes"
//! description: "Classic CRT monitor simulation"
//! version: "1.0.0"
//!
//! defaults:
//!   animation_speed: 1.0
//!   brightness: 0.85
//!   channel0: "textures/noise.png"
//! */
//! ```

use super::types::ShaderMetadata;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Marker string that identifies the start of shader metadata block
const METADATA_MARKER: &str = "/*! par-term shader metadata";

/// Parse shader metadata from GLSL source code.
///
/// Looks for a `/*! par-term shader metadata ... */` block at the top of the file
/// and parses the YAML content within.
///
/// # Arguments
/// * `source` - The GLSL shader source code
///
/// # Returns
/// * `Some(ShaderMetadata)` if metadata was found and parsed successfully
/// * `None` if no metadata block was found or parsing failed
pub fn parse_shader_metadata(source: &str) -> Option<ShaderMetadata> {
    // Find the metadata block marker
    let start_marker = source.find(METADATA_MARKER)?;

    // Find the start of YAML content (after the marker line)
    let yaml_start = source[start_marker + METADATA_MARKER.len()..]
        .find('\n')
        .map(|i| start_marker + METADATA_MARKER.len() + i + 1)?;

    // Find the closing */
    let yaml_end = source[yaml_start..].find("*/")?;
    let yaml_content = &source[yaml_start..yaml_start + yaml_end];

    // Trim trailing whitespace from each line and the whole block
    let yaml_trimmed = yaml_content.trim();

    // Parse the YAML
    match serde_yaml::from_str(yaml_trimmed) {
        Ok(metadata) => {
            log::debug!("Parsed shader metadata: {:?}", metadata);
            Some(metadata)
        }
        Err(e) => {
            log::warn!("Failed to parse shader metadata YAML: {}", e);
            log::debug!("YAML content was:\n{}", yaml_trimmed);
            None
        }
    }
}

/// Parse shader metadata from a file path.
///
/// # Arguments
/// * `path` - Path to the shader file
///
/// # Returns
/// * `Some(ShaderMetadata)` if the file was read and metadata was parsed successfully
/// * `None` if reading failed or no metadata was found
pub fn parse_shader_metadata_from_file(path: &Path) -> Option<ShaderMetadata> {
    match std::fs::read_to_string(path) {
        Ok(source) => parse_shader_metadata(&source),
        Err(e) => {
            log::warn!("Failed to read shader file '{}': {}", path.display(), e);
            None
        }
    }
}

/// Serialize shader metadata to a YAML string (without the comment wrapper).
///
/// # Arguments
/// * `metadata` - The metadata to serialize
///
/// # Returns
/// The YAML representation of the metadata
pub fn serialize_metadata_to_yaml(metadata: &ShaderMetadata) -> Result<String, String> {
    serde_yaml::to_string(metadata).map_err(|e| format!("Failed to serialize metadata: {}", e))
}

/// Format shader metadata as a complete comment block ready to insert into a shader.
///
/// # Arguments
/// * `metadata` - The metadata to format
///
/// # Returns
/// The formatted metadata block including the `/*! par-term shader metadata ... */` wrapper
pub fn format_metadata_block(metadata: &ShaderMetadata) -> Result<String, String> {
    let yaml = serialize_metadata_to_yaml(metadata)?;
    Ok(format!("{}\n{}\n*/", METADATA_MARKER, yaml.trim_end()))
}

/// Update or insert metadata in shader source code.
///
/// If the shader already has a metadata block, it will be replaced.
/// If not, the metadata block will be inserted at the beginning of the file.
///
/// # Arguments
/// * `source` - The original shader source code
/// * `metadata` - The new metadata to insert/update
///
/// # Returns
/// The updated shader source code
pub fn update_shader_metadata(source: &str, metadata: &ShaderMetadata) -> Result<String, String> {
    let new_block = format_metadata_block(metadata)?;

    // Check if there's an existing metadata block
    if let Some(start_pos) = source.find(METADATA_MARKER) {
        // Find the end of the metadata block
        if let Some(end_offset) = source[start_pos..].find("*/") {
            let end_pos = start_pos + end_offset + 2; // Include the */
            // Replace the existing block
            let mut result = String::with_capacity(source.len());
            result.push_str(&source[..start_pos]);
            result.push_str(&new_block);
            result.push_str(&source[end_pos..]);
            return Ok(result);
        }
    }

    // No existing block, insert at the beginning
    Ok(format!("{}\n\n{}", new_block, source))
}

/// Update metadata in a shader file.
///
/// # Arguments
/// * `path` - Path to the shader file
/// * `metadata` - The new metadata to write
///
/// # Returns
/// Ok(()) if successful, Err with error message otherwise
pub fn update_shader_metadata_file(path: &Path, metadata: &ShaderMetadata) -> Result<(), String> {
    // Read the current file content
    let source = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read shader file '{}': {}", path.display(), e))?;

    // Update the metadata
    let updated_source = update_shader_metadata(&source, metadata)?;

    // Write back to the file
    std::fs::write(path, updated_source)
        .map_err(|e| format!("Failed to write shader file '{}': {}", path.display(), e))?;

    log::info!("Updated metadata in shader file: {}", path.display());
    Ok(())
}

/// Cache for parsed shader metadata.
///
/// Avoids re-parsing shader files on every access while still allowing
/// invalidation for hot reload scenarios.
#[derive(Debug, Default)]
pub struct ShaderMetadataCache {
    /// Cached metadata by shader filename (not full path)
    cache: HashMap<String, Option<ShaderMetadata>>,
    /// The shaders directory path
    shaders_dir: Option<PathBuf>,
}

impl ShaderMetadataCache {
    /// Create a new empty metadata cache.
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new metadata cache with a specific shaders directory.
    pub fn with_shaders_dir(shaders_dir: PathBuf) -> Self {
        Self {
            cache: HashMap::new(),
            shaders_dir: Some(shaders_dir),
        }
    }

    /// Set the shaders directory path.
    #[allow(dead_code)]
    pub fn set_shaders_dir(&mut self, shaders_dir: PathBuf) {
        self.shaders_dir = Some(shaders_dir);
    }

    /// Get metadata for a shader, loading and caching if necessary.
    ///
    /// # Arguments
    /// * `shader_name` - Filename of the shader (e.g., "crt.glsl")
    ///
    /// # Returns
    /// * `Some(&ShaderMetadata)` if metadata was found
    /// * `None` if no metadata was found or the shader couldn't be read
    pub fn get(&mut self, shader_name: &str) -> Option<&ShaderMetadata> {
        // Check if already cached
        if self.cache.contains_key(shader_name) {
            return self.cache.get(shader_name).and_then(|m| m.as_ref());
        }

        // Load and cache
        let metadata = self.load_metadata(shader_name);
        self.cache.insert(shader_name.to_string(), metadata);
        self.cache.get(shader_name).and_then(|m| m.as_ref())
    }

    /// Get metadata without caching (always reads from disk).
    ///
    /// Useful for hot reload scenarios where you want fresh data.
    #[allow(dead_code)]
    pub fn get_fresh(&self, shader_name: &str) -> Option<ShaderMetadata> {
        self.load_metadata(shader_name)
    }

    /// Load metadata from a shader file.
    fn load_metadata(&self, shader_name: &str) -> Option<ShaderMetadata> {
        let path = self.resolve_shader_path(shader_name)?;
        parse_shader_metadata_from_file(&path)
    }

    /// Resolve a shader name to its full path.
    fn resolve_shader_path(&self, shader_name: &str) -> Option<PathBuf> {
        let shader_path = PathBuf::from(shader_name);

        // If it's an absolute path, use it directly
        if shader_path.is_absolute() && shader_path.exists() {
            return Some(shader_path);
        }

        // Otherwise, resolve relative to shaders directory
        if let Some(ref shaders_dir) = self.shaders_dir {
            let full_path = shaders_dir.join(shader_name);
            if full_path.exists() {
                return Some(full_path);
            }
        }

        // Try the default shaders directory
        let default_path = crate::config::Config::shader_path(shader_name);
        if default_path.exists() {
            return Some(default_path);
        }

        None
    }

    /// Invalidate cached metadata for a specific shader.
    ///
    /// Call this when a shader file has been modified (hot reload).
    #[allow(dead_code)]
    pub fn invalidate(&mut self, shader_name: &str) {
        self.cache.remove(shader_name);
        log::debug!("Invalidated metadata cache for: {}", shader_name);
    }

    /// Invalidate all cached metadata.
    ///
    /// Call this when the shaders directory might have changed.
    #[allow(dead_code)]
    pub fn invalidate_all(&mut self) {
        self.cache.clear();
        log::debug!("Invalidated all metadata cache entries");
    }

    /// Check if metadata is cached for a shader.
    #[allow(dead_code)]
    pub fn is_cached(&self, shader_name: &str) -> bool {
        self.cache.contains_key(shader_name)
    }

    /// Get the number of cached entries.
    #[allow(dead_code)]
    pub fn cache_size(&self) -> usize {
        self.cache.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_metadata_basic() {
        let source = r#"/*! par-term shader metadata
name: "Test Shader"
author: "Test Author"
description: "A test shader"
version: "1.0.0"
*/

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    fragColor = vec4(1.0);
}
"#;

        let metadata = parse_shader_metadata(source).expect("Should parse metadata");
        assert_eq!(metadata.name, Some("Test Shader".to_string()));
        assert_eq!(metadata.author, Some("Test Author".to_string()));
        assert_eq!(metadata.description, Some("A test shader".to_string()));
        assert_eq!(metadata.version, Some("1.0.0".to_string()));
    }

    #[test]
    fn test_parse_metadata_with_defaults() {
        let source = r#"/*! par-term shader metadata
name: "CRT Effect"
defaults:
  animation_speed: 0.5
  brightness: 0.85
  full_content: true
  channel0: "textures/noise.png"
*/

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    fragColor = vec4(1.0);
}
"#;

        let metadata = parse_shader_metadata(source).expect("Should parse metadata");
        assert_eq!(metadata.name, Some("CRT Effect".to_string()));
        assert_eq!(metadata.defaults.animation_speed, Some(0.5));
        assert_eq!(metadata.defaults.brightness, Some(0.85));
        assert_eq!(metadata.defaults.full_content, Some(true));
        assert_eq!(
            metadata.defaults.channel0,
            Some("textures/noise.png".to_string())
        );
    }

    #[test]
    fn test_parse_metadata_not_found() {
        let source = r#"// Regular shader without metadata
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    fragColor = vec4(1.0);
}
"#;

        let metadata = parse_shader_metadata(source);
        assert!(metadata.is_none());
    }

    #[test]
    fn test_parse_metadata_partial() {
        let source = r#"/*! par-term shader metadata
name: "Minimal Shader"
*/

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    fragColor = vec4(1.0);
}
"#;

        let metadata = parse_shader_metadata(source).expect("Should parse metadata");
        assert_eq!(metadata.name, Some("Minimal Shader".to_string()));
        assert!(metadata.author.is_none());
        assert!(metadata.description.is_none());
        assert!(metadata.defaults.animation_speed.is_none());
    }

    #[test]
    fn test_cache_basic() {
        let mut cache = ShaderMetadataCache::new();

        // Initially nothing is cached
        assert!(!cache.is_cached("test.glsl"));
        assert_eq!(cache.cache_size(), 0);

        // After calling get (even if file doesn't exist), it gets cached as None
        let _ = cache.get("nonexistent.glsl");
        assert!(cache.is_cached("nonexistent.glsl"));
        assert_eq!(cache.cache_size(), 1);

        // Invalidate removes from cache
        cache.invalidate("nonexistent.glsl");
        assert!(!cache.is_cached("nonexistent.glsl"));
        assert_eq!(cache.cache_size(), 0);
    }

    #[test]
    fn test_update_metadata_existing_block() {
        let source = r#"/*! par-term shader metadata
name: "Old Name"
version: "1.0.0"
*/

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    fragColor = vec4(1.0);
}
"#;

        let new_metadata = ShaderMetadata {
            name: Some("New Name".to_string()),
            author: Some("New Author".to_string()),
            version: Some("2.0.0".to_string()),
            ..Default::default()
        };

        let result = super::update_shader_metadata(source, &new_metadata).unwrap();

        // Should contain the new metadata
        assert!(result.contains("New Name"));
        assert!(result.contains("New Author"));
        assert!(result.contains("2.0.0"));
        // Should NOT contain the old metadata
        assert!(!result.contains("Old Name"));
        // Should still contain the shader code
        assert!(result.contains("void mainImage"));
    }

    #[test]
    fn test_update_metadata_no_existing_block() {
        let source = r#"// Simple shader without metadata
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    fragColor = vec4(1.0);
}
"#;

        let new_metadata = ShaderMetadata {
            name: Some("New Shader".to_string()),
            version: Some("1.0.0".to_string()),
            ..Default::default()
        };

        let result = super::update_shader_metadata(source, &new_metadata).unwrap();

        // Should contain the new metadata at the beginning
        assert!(result.starts_with("/*! par-term shader metadata"));
        assert!(result.contains("New Shader"));
        // Should still contain the shader code
        assert!(result.contains("void mainImage"));
        assert!(result.contains("// Simple shader without metadata"));
    }

    #[test]
    fn test_format_metadata_block() {
        let metadata = ShaderMetadata {
            name: Some("Test Shader".to_string()),
            author: Some("Test Author".to_string()),
            description: Some("A test shader".to_string()),
            version: Some("1.0.0".to_string()),
            defaults: Default::default(),
        };

        let block = super::format_metadata_block(&metadata).unwrap();

        assert!(block.starts_with("/*! par-term shader metadata"));
        assert!(block.ends_with("*/"));
        assert!(block.contains("Test Shader"));
        assert!(block.contains("Test Author"));
    }
}
