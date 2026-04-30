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
//!
//! # Role in the Three-Tier Resolution Chain
//!
//! This module implements **Tier 2** of the shader configuration resolution chain
//! documented in [`crate::shader_config`]:
//!
//! ```text
//! Tier 1 — User override  (config.yaml → shader_configs)
//!     ↓
//! Tier 2 — Shader metadata  (THIS MODULE — embedded YAML in .glsl files)
//!     ↓
//! Tier 3 — Global defaults  (Config struct fields)
//! ```
//!
//! Parsed metadata is supplied as `Option<&ShaderMetadata>` / `Option<&CursorShaderMetadata>`
//! to [`crate::shader_config::resolve_shader_config`] and
//! [`crate::shader_config::resolve_cursor_shader_config`], which merge it with
//! the user override (Tier 1) and global defaults (Tier 3).
//!
//! ## Sub-modules
//!
//! - [`parsing`] — pure text operations: YAML extraction, parse/serialize/format/update
//! - [`cache`] — `MetadataCache<T>` generic cache with disk I/O

pub mod cache;
pub mod parsing;

// Re-export the full public surface so downstream crates keep working with
// paths like `crate::shader_metadata::parse_shader_metadata`, etc.
pub use cache::{CursorShaderMetadataCache, MetadataCache, ShaderMetadataCache};
pub use parsing::{
    format_cursor_metadata_block, format_metadata_block, parse_cursor_shader_metadata,
    parse_cursor_shader_metadata_from_file, parse_shader_metadata, parse_shader_metadata_from_file,
    serialize_cursor_metadata_to_yaml, serialize_metadata_to_yaml, update_cursor_shader_metadata,
    update_cursor_shader_metadata_file, update_shader_metadata, update_shader_metadata_file,
};

#[cfg(test)]
mod tests {
    use super::*;
    use parsing::extract_yaml_block;

    #[test]
    fn test_extract_yaml_block_present() {
        let source = "/*! par-term shader metadata\nname: test\n*/\nvoid main() {}";
        let yaml = extract_yaml_block(source);
        assert_eq!(yaml, Some("name: test"));
    }

    #[test]
    fn test_extract_yaml_block_absent() {
        let source = "// no metadata\nvoid main() {}";
        assert!(extract_yaml_block(source).is_none());
    }

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
    fn test_parse_metadata_with_custom_uniform_defaults() {
        let source = r#"/*! par-term shader metadata
name: "Controlled Shader"
defaults:
  uniforms:
    iGlow: 0.5
    iEnabled: true
*/

void mainImage(out vec4 fragColor, in vec2 fragCoord) {}
"#;

        let metadata = parse_shader_metadata(source).expect("Should parse metadata");
        assert_eq!(
            metadata.defaults.uniforms.get("iGlow"),
            Some(&crate::types::shader::ShaderUniformValue::Float(0.5))
        );
        assert_eq!(
            metadata.defaults.uniforms.get("iEnabled"),
            Some(&crate::types::shader::ShaderUniformValue::Bool(true))
        );
    }

    #[test]
    fn test_parse_metadata_with_custom_uniform_color_hex_defaults() {
        let source = r##"/*! par-term shader metadata
name: "Controlled Color Shader"
defaults:
  uniforms:
    iTint: "#ff8800"
    iOverlay: "#ff8800cc"
*/

void mainImage(out vec4 fragColor, in vec2 fragCoord) {}
"##;

        let metadata = parse_shader_metadata(source).expect("Should parse metadata");
        assert_eq!(
            metadata.defaults.uniforms.get("iTint"),
            Some(&crate::types::shader::ShaderUniformValue::Color(
                crate::types::shader::ShaderColorValue([1.0, 136.0 / 255.0, 0.0, 1.0])
            ))
        );
        assert_eq!(
            metadata.defaults.uniforms.get("iOverlay"),
            Some(&crate::types::shader::ShaderUniformValue::Color(
                crate::types::shader::ShaderColorValue([1.0, 136.0 / 255.0, 0.0, 204.0 / 255.0])
            ))
        );
    }

    #[test]
    fn test_parse_metadata_with_custom_uniform_color_array_defaults() {
        let source = r#"/*! par-term shader metadata
name: "Controlled Color Shader"
defaults:
  uniforms:
    iTint: [1.0, 0.5, 0.0]
    iOverlay: [1.0, 0.5, 0.0, 0.8]
*/

void mainImage(out vec4 fragColor, in vec2 fragCoord) {}
"#;

        let metadata = parse_shader_metadata(source).expect("Should parse metadata");
        assert_eq!(
            metadata.defaults.uniforms.get("iTint"),
            Some(&crate::types::shader::ShaderUniformValue::Color(
                crate::types::shader::ShaderColorValue([1.0, 0.5, 0.0, 1.0])
            ))
        );
        assert_eq!(
            metadata.defaults.uniforms.get("iOverlay"),
            Some(&crate::types::shader::ShaderUniformValue::Color(
                crate::types::shader::ShaderColorValue([1.0, 0.5, 0.0, 0.8])
            ))
        );
    }

    #[test]
    fn test_parse_metadata_skips_invalid_custom_uniform_default() {
        let source = r#"/*! par-term shader metadata
name: "Controlled Color Shader"
defaults:
  uniforms:
    iTint: [1.0, 0.5, 0.0, 0.8, 0.1]
    iGlow: 0.5
*/

void mainImage(out vec4 fragColor, in vec2 fragCoord) {}
"#;

        let metadata = parse_shader_metadata(source).expect("Should parse metadata");
        assert!(!metadata.defaults.uniforms.contains_key("iTint"));
        assert_eq!(
            metadata.defaults.uniforms.get("iGlow"),
            Some(&crate::types::shader::ShaderUniformValue::Float(0.5))
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

        let new_metadata = crate::types::ShaderMetadata {
            name: Some("New Name".to_string()),
            author: Some("New Author".to_string()),
            version: Some("2.0.0".to_string()),
            ..Default::default()
        };

        let result = update_shader_metadata(source, &new_metadata).unwrap();

        assert!(result.contains("New Name"));
        assert!(result.contains("New Author"));
        assert!(result.contains("2.0.0"));
        assert!(!result.contains("Old Name"));
        assert!(result.contains("void mainImage"));
    }

    #[test]
    fn test_update_metadata_no_existing_block() {
        let source = r#"// Simple shader without metadata
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    fragColor = vec4(1.0);
}
"#;

        let new_metadata = crate::types::ShaderMetadata {
            name: Some("New Shader".to_string()),
            version: Some("1.0.0".to_string()),
            ..Default::default()
        };

        let result = update_shader_metadata(source, &new_metadata).unwrap();

        assert!(result.starts_with("/*! par-term shader metadata"));
        assert!(result.contains("New Shader"));
        assert!(result.contains("void mainImage"));
        assert!(result.contains("// Simple shader without metadata"));
    }

    #[test]
    fn test_format_metadata_block() {
        let metadata = crate::types::ShaderMetadata {
            name: Some("Test Shader".to_string()),
            author: Some("Test Author".to_string()),
            description: Some("A test shader".to_string()),
            version: Some("1.0.0".to_string()),
            defaults: Default::default(),
            safety_badges: Vec::new(),
        };

        let block = format_metadata_block(&metadata).unwrap();

        assert!(block.starts_with("/*! par-term shader metadata"));
        assert!(block.ends_with("*/"));
        assert!(block.contains("Test Shader"));
        assert!(block.contains("Test Author"));
    }

    // ========================================================================
    // Cursor Shader Metadata Tests
    // ========================================================================

    #[test]
    fn test_parse_cursor_metadata_basic() {
        let source = r#"/*! par-term shader metadata
name: "Cursor Glow"
author: "Test Author"
description: "A cursor glow effect"
version: "1.0.0"
*/

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    fragColor = vec4(1.0);
}
"#;

        let metadata = parse_cursor_shader_metadata(source).expect("Should parse cursor metadata");
        assert_eq!(metadata.name, Some("Cursor Glow".to_string()));
        assert_eq!(metadata.author, Some("Test Author".to_string()));
        assert_eq!(
            metadata.description,
            Some("A cursor glow effect".to_string())
        );
        assert_eq!(metadata.version, Some("1.0.0".to_string()));
    }

    #[test]
    fn test_parse_cursor_metadata_with_defaults() {
        let source = r#"/*! par-term shader metadata
name: "Cursor Trail"
defaults:
  animation_speed: 2.0
  glow_radius: 100.0
  glow_intensity: 0.5
  trail_duration: 1.0
  cursor_color: [255, 128, 0]
*/

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    fragColor = vec4(1.0);
}
"#;

        let metadata = parse_cursor_shader_metadata(source).expect("Should parse cursor metadata");
        assert_eq!(metadata.name, Some("Cursor Trail".to_string()));
        assert_eq!(metadata.defaults.base.animation_speed, Some(2.0));
        assert_eq!(metadata.defaults.glow_radius, Some(100.0));
        assert_eq!(metadata.defaults.glow_intensity, Some(0.5));
        assert_eq!(metadata.defaults.trail_duration, Some(1.0));
        assert_eq!(metadata.defaults.cursor_color, Some([255, 128, 0]));
    }

    #[test]
    fn test_cursor_shader_cache_basic() {
        let mut cache = CursorShaderMetadataCache::new();

        assert!(!cache.is_cached("cursor_test.glsl"));
        assert_eq!(cache.cache_size(), 0);

        let _ = cache.get("nonexistent_cursor.glsl");
        assert!(cache.is_cached("nonexistent_cursor.glsl"));
        assert_eq!(cache.cache_size(), 1);

        cache.invalidate("nonexistent_cursor.glsl");
        assert!(!cache.is_cached("nonexistent_cursor.glsl"));
        assert_eq!(cache.cache_size(), 0);
    }

    #[test]
    fn test_update_cursor_metadata_existing_block() {
        let source = r#"/*! par-term shader metadata
name: "Old Cursor"
version: "1.0.0"
*/

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    fragColor = vec4(1.0);
}
"#;

        let new_metadata = crate::types::CursorShaderMetadata {
            name: Some("New Cursor".to_string()),
            author: Some("New Author".to_string()),
            version: Some("2.0.0".to_string()),
            ..Default::default()
        };

        let result = update_cursor_shader_metadata(source, &new_metadata).unwrap();

        assert!(result.contains("New Cursor"));
        assert!(result.contains("New Author"));
        assert!(result.contains("2.0.0"));
        assert!(!result.contains("Old Cursor"));
        assert!(result.contains("void mainImage"));
    }

    #[test]
    fn test_update_cursor_metadata_no_existing_block() {
        let source = r#"// Cursor shader without metadata
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    fragColor = vec4(1.0);
}
"#;

        let new_metadata = crate::types::CursorShaderMetadata {
            name: Some("New Cursor Shader".to_string()),
            version: Some("1.0.0".to_string()),
            ..Default::default()
        };

        let result = update_cursor_shader_metadata(source, &new_metadata).unwrap();

        assert!(result.starts_with("/*! par-term shader metadata"));
        assert!(result.contains("New Cursor Shader"));
        assert!(result.contains("void mainImage"));
        assert!(result.contains("// Cursor shader without metadata"));
    }

    #[test]
    fn test_format_cursor_metadata_block() {
        use crate::CursorShaderConfig;

        let metadata = crate::types::CursorShaderMetadata {
            name: Some("Test Cursor".to_string()),
            author: Some("Test Author".to_string()),
            description: Some("A test cursor shader".to_string()),
            version: Some("1.0.0".to_string()),
            defaults: CursorShaderConfig {
                glow_radius: Some(50.0),
                glow_intensity: Some(0.8),
                ..Default::default()
            },
        };

        let block = format_cursor_metadata_block(&metadata).unwrap();

        assert!(block.starts_with("/*! par-term shader metadata"));
        assert!(block.ends_with("*/"));
        assert!(block.contains("Test Cursor"));
        assert!(block.contains("Test Author"));
        assert!(block.contains("glow_radius"));
        assert!(block.contains("glow_intensity"));
    }
}
