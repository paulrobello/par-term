//! Shader metadata parsing, serialization, and source-level update helpers.
//!
//! These functions operate exclusively on string data (GLSL source text and
//! YAML fragments) and perform no file I/O — all disk operations live in
//! [`super::cache`] via [`super::MetadataCache`].

use crate::types::{CursorShaderMetadata, ShaderMetadata};
use std::path::Path;

/// Marker string that identifies the start of a shader metadata block.
pub(super) const METADATA_MARKER: &str = "/*! par-term shader metadata";

// ============================================================================
// Shared YAML extraction
// ============================================================================

/// Extract the YAML block content from a GLSL shader source string.
///
/// Looks for a `/*! par-term shader metadata ... */` block and returns the
/// trimmed YAML text inside it, or `None` if no such block is found.
///
/// # Arguments
/// * `source` - The GLSL shader source code
///
/// # Returns
/// * `Some(&str)` pointing into `source` with the trimmed YAML content
/// * `None` if no metadata block is present
pub(super) fn extract_yaml_block(source: &str) -> Option<&str> {
    let start_marker = source.find(METADATA_MARKER)?;

    let yaml_start = source[start_marker + METADATA_MARKER.len()..]
        .find('\n')
        .map(|i| start_marker + METADATA_MARKER.len() + i + 1)?;

    let yaml_end = source[yaml_start..].find("*/")?;
    let yaml_content = &source[yaml_start..yaml_start + yaml_end];

    Some(yaml_content.trim())
}

// ============================================================================
// Background Shader Metadata
// ============================================================================

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
    let yaml_trimmed = extract_yaml_block(source)?;

    match serde_yaml_ng::from_str(yaml_trimmed) {
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
pub fn serialize_metadata_to_yaml(metadata: &ShaderMetadata) -> Result<String, String> {
    serde_yaml_ng::to_string(metadata).map_err(|e| format!("Failed to serialize metadata: {}", e))
}

/// Format shader metadata as a complete comment block ready to insert into a shader.
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
pub fn update_shader_metadata(source: &str, metadata: &ShaderMetadata) -> Result<String, String> {
    let new_block = format_metadata_block(metadata)?;

    if let Some(start_pos) = source.find(METADATA_MARKER)
        && let Some(end_offset) = source[start_pos..].find("*/")
    {
        let end_pos = start_pos + end_offset + 2; // Include the */
        let mut result = String::with_capacity(source.len());
        result.push_str(&source[..start_pos]);
        result.push_str(&new_block);
        result.push_str(&source[end_pos..]);
        return Ok(result);
    }

    Ok(format!("{}\n\n{}", new_block, source))
}

/// Update metadata in a shader file.
pub fn update_shader_metadata_file(path: &Path, metadata: &ShaderMetadata) -> Result<(), String> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read shader file '{}': {}", path.display(), e))?;

    let updated_source = update_shader_metadata(&source, metadata)?;

    std::fs::write(path, updated_source)
        .map_err(|e| format!("Failed to write shader file '{}': {}", path.display(), e))?;

    log::info!("Updated metadata in shader file: {}", path.display());
    Ok(())
}

// ============================================================================
// Cursor Shader Metadata
// ============================================================================

/// Parse cursor shader metadata from GLSL source code.
///
/// Uses the same `/*! par-term shader metadata ... */` format as background shaders,
/// but deserializes to `CursorShaderMetadata` which includes cursor-specific settings.
pub fn parse_cursor_shader_metadata(source: &str) -> Option<CursorShaderMetadata> {
    let yaml_trimmed = extract_yaml_block(source)?;

    match serde_yaml_ng::from_str(yaml_trimmed) {
        Ok(metadata) => {
            log::debug!("Parsed cursor shader metadata: {:?}", metadata);
            Some(metadata)
        }
        Err(e) => {
            log::warn!("Failed to parse cursor shader metadata YAML: {}", e);
            log::debug!("YAML content was:\n{}", yaml_trimmed);
            None
        }
    }
}

/// Parse cursor shader metadata from a file path.
pub fn parse_cursor_shader_metadata_from_file(path: &Path) -> Option<CursorShaderMetadata> {
    match std::fs::read_to_string(path) {
        Ok(source) => parse_cursor_shader_metadata(&source),
        Err(e) => {
            log::warn!(
                "Failed to read cursor shader file '{}': {}",
                path.display(),
                e
            );
            None
        }
    }
}

/// Serialize cursor shader metadata to a YAML string (without the comment wrapper).
pub fn serialize_cursor_metadata_to_yaml(
    metadata: &CursorShaderMetadata,
) -> Result<String, String> {
    serde_yaml_ng::to_string(metadata).map_err(|e| format!("Failed to serialize metadata: {}", e))
}

/// Format cursor shader metadata as a complete comment block ready to insert into a shader.
pub fn format_cursor_metadata_block(metadata: &CursorShaderMetadata) -> Result<String, String> {
    let yaml = serialize_cursor_metadata_to_yaml(metadata)?;
    Ok(format!("{}\n{}\n*/", METADATA_MARKER, yaml.trim_end()))
}

/// Update or insert cursor shader metadata in shader source code.
pub fn update_cursor_shader_metadata(
    source: &str,
    metadata: &CursorShaderMetadata,
) -> Result<String, String> {
    let new_block = format_cursor_metadata_block(metadata)?;

    if let Some(start_pos) = source.find(METADATA_MARKER)
        && let Some(end_offset) = source[start_pos..].find("*/")
    {
        let end_pos = start_pos + end_offset + 2; // Include the */
        let mut result = String::with_capacity(source.len());
        result.push_str(&source[..start_pos]);
        result.push_str(&new_block);
        result.push_str(&source[end_pos..]);
        return Ok(result);
    }

    Ok(format!("{}\n\n{}", new_block, source))
}

/// Update cursor shader metadata in a shader file.
pub fn update_cursor_shader_metadata_file(
    path: &Path,
    metadata: &CursorShaderMetadata,
) -> Result<(), String> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read shader file '{}': {}", path.display(), e))?;

    let updated_source = update_cursor_shader_metadata(&source, metadata)?;

    std::fs::write(path, updated_source)
        .map_err(|e| format!("Failed to write shader file '{}': {}", path.display(), e))?;

    log::info!("Updated cursor shader metadata in file: {}", path.display());
    Ok(())
}
