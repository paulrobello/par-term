//! Generic metadata cache for parsed shader YAML blocks.
//!
//! [`MetadataCache<T>`] wraps an in-memory `HashMap` keyed by shader filename
//! and re-parses from disk only on cache misses or explicit invalidation.
//! The two concrete types used by par-term are exposed as type aliases:
//!
//! - [`ShaderMetadataCache`] — background shaders
//! - [`CursorShaderMetadataCache`] — cursor shaders

use super::parsing::extract_yaml_block;
use crate::types::{CursorShaderMetadata, ShaderMetadata};
use std::collections::HashMap;
use std::path::PathBuf;

/// Generic cache for parsed shader metadata.
///
/// Avoids re-parsing shader files on every access while still allowing
/// invalidation for hot reload scenarios.
///
/// `T` must be deserializable from YAML via serde. Use the type aliases
/// [`ShaderMetadataCache`] and [`CursorShaderMetadataCache`] for the two
/// concrete cache types used by par-term.
#[derive(Debug)]
pub struct MetadataCache<T>
where
    T: for<'de> serde::Deserialize<'de>,
{
    /// Cached metadata by shader filename (not full path).
    cache: HashMap<String, Option<T>>,
    /// The shaders directory path.
    shaders_dir: Option<PathBuf>,
}

impl<T> Default for MetadataCache<T>
where
    T: for<'de> serde::Deserialize<'de>,
{
    fn default() -> Self {
        Self {
            cache: HashMap::new(),
            shaders_dir: None,
        }
    }
}

impl<T> MetadataCache<T>
where
    T: for<'de> serde::Deserialize<'de>,
{
    /// Create a new empty metadata cache.
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
    pub fn set_shaders_dir(&mut self, shaders_dir: PathBuf) {
        self.shaders_dir = Some(shaders_dir);
    }

    /// Get metadata for a shader, loading and caching if necessary.
    ///
    /// # Arguments
    /// * `shader_name` - Filename of the shader (e.g., "crt.glsl")
    ///
    /// # Returns
    /// * `Some(&T)` if metadata was found
    /// * `None` if no metadata was found or the shader couldn't be read
    pub fn get(&mut self, shader_name: &str) -> Option<&T>
    where
        T: std::fmt::Debug,
    {
        if self.cache.contains_key(shader_name) {
            return self.cache.get(shader_name).and_then(|m| m.as_ref());
        }

        let metadata = self.load_metadata(shader_name);
        self.cache.insert(shader_name.to_string(), metadata);
        self.cache.get(shader_name).and_then(|m| m.as_ref())
    }

    /// Get metadata without caching (always reads from disk).
    ///
    /// Useful for hot reload scenarios where you want fresh data.
    pub fn get_fresh(&self, shader_name: &str) -> Option<T> {
        self.load_metadata(shader_name)
    }

    /// Load metadata from a shader file by parsing YAML from its embedded block.
    fn load_metadata(&self, shader_name: &str) -> Option<T> {
        let path = self.resolve_shader_path(shader_name)?;
        let source = std::fs::read_to_string(&path)
            .map_err(|e| {
                log::warn!("Failed to read shader file '{}': {}", path.display(), e);
            })
            .ok()?;
        let yaml_trimmed = extract_yaml_block(&source)?;
        match serde_yaml_ng::from_str(yaml_trimmed) {
            Ok(metadata) => Some(metadata),
            Err(e) => {
                log::warn!(
                    "Failed to parse shader metadata YAML from '{}': {}",
                    path.display(),
                    e
                );
                None
            }
        }
    }

    /// Resolve a shader name to its full path.
    fn resolve_shader_path(&self, shader_name: &str) -> Option<PathBuf> {
        let shader_path = PathBuf::from(shader_name);

        if shader_path.is_absolute() && shader_path.exists() {
            return Some(shader_path);
        }

        if let Some(ref shaders_dir) = self.shaders_dir {
            let full_path = shaders_dir.join(shader_name);
            if full_path.exists() {
                return Some(full_path);
            }
        }

        let default_path = crate::config::Config::shader_path(shader_name);
        if default_path.exists() {
            return Some(default_path);
        }

        None
    }

    /// Invalidate cached metadata for a specific shader.
    ///
    /// Call this when a shader file has been modified (hot reload).
    pub fn invalidate(&mut self, shader_name: &str) {
        self.cache.remove(shader_name);
        log::debug!("Invalidated metadata cache for: {}", shader_name);
    }

    /// Invalidate all cached metadata.
    ///
    /// Call this when the shaders directory might have changed.
    pub fn invalidate_all(&mut self) {
        self.cache.clear();
        log::debug!("Invalidated all metadata cache entries");
    }

    /// Check if metadata is cached for a shader.
    pub fn is_cached(&self, shader_name: &str) -> bool {
        self.cache.contains_key(shader_name)
    }

    /// Get the number of cached entries.
    pub fn cache_size(&self) -> usize {
        self.cache.len()
    }
}

// ============================================================================
// Type aliases — preserve the original public API
// ============================================================================

/// Cache for parsed background shader metadata.
///
/// See [`MetadataCache`] for full documentation.
pub type ShaderMetadataCache = MetadataCache<ShaderMetadata>;

/// Cache for parsed cursor shader metadata.
///
/// See [`MetadataCache`] for full documentation.
pub type CursorShaderMetadataCache = MetadataCache<CursorShaderMetadata>;
