//! Shader configuration resolution.
//!
//! Handles merging of per-shader configurations from multiple sources:
//! 1. User overrides (from config.yaml shader_configs)
//! 2. Shader metadata defaults (from embedded YAML in shader files)
//! 3. Global defaults (from defaults.rs / Config struct)

use super::types::{
    CursorShaderConfig, ResolvedCursorShaderConfig, ResolvedShaderConfig, ShaderConfig,
    ShaderMetadata,
};
use crate::config::Config;
use std::path::PathBuf;

/// Resolve a shader configuration by merging sources in priority order.
///
/// Priority (highest to lowest):
/// 1. User overrides from config.yaml
/// 2. Defaults embedded in shader metadata
/// 3. Global defaults from Config
///
/// # Arguments
/// * `user_override` - Optional user overrides from config.yaml
/// * `metadata` - Optional shader metadata with embedded defaults
/// * `config` - Global config for fallback values
///
/// # Returns
/// A fully resolved configuration with all values filled in
#[allow(dead_code)]
pub fn resolve_shader_config(
    user_override: Option<&ShaderConfig>,
    metadata: Option<&ShaderMetadata>,
    config: &Config,
) -> ResolvedShaderConfig {
    // Extract metadata defaults if available
    let meta_defaults = metadata.map(|m| &m.defaults);

    // Helper to resolve a single value through the priority chain
    macro_rules! resolve {
        ($field:ident, $global:expr) => {
            user_override
                .and_then(|o| o.$field.clone())
                .or_else(|| meta_defaults.and_then(|m| m.$field.clone()))
                .unwrap_or($global)
        };
    }

    // Helper for Option<String> -> Option<PathBuf> with path resolution
    // An explicit empty string means "no texture" (don't fall back to defaults)
    macro_rules! resolve_path {
        ($field:ident, $global:expr) => {{
            // Check for user override first
            if let Some(override_val) = user_override.and_then(|o| o.$field.clone()) {
                if override_val.is_empty() {
                    None // User explicitly cleared this channel
                } else {
                    Some(Config::resolve_texture_path(&override_val))
                }
            } else {
                // No user override, fall back to metadata then global
                let path_str: Option<String> =
                    meta_defaults.and_then(|m| m.$field.clone()).or($global);
                path_str
                    .filter(|p| !p.is_empty())
                    .map(|p| Config::resolve_texture_path(&p))
            }
        }};
    }

    ResolvedShaderConfig {
        animation_speed: resolve!(animation_speed, config.custom_shader_animation_speed),
        brightness: resolve!(brightness, config.custom_shader_brightness),
        text_opacity: resolve!(text_opacity, config.custom_shader_text_opacity),
        full_content: resolve!(full_content, config.custom_shader_full_content),
        channel0: resolve_path!(channel0, config.custom_shader_channel0.clone()),
        channel1: resolve_path!(channel1, config.custom_shader_channel1.clone()),
        channel2: resolve_path!(channel2, config.custom_shader_channel2.clone()),
        channel3: resolve_path!(channel3, config.custom_shader_channel3.clone()),
        cubemap: resolve_path!(cubemap, config.custom_shader_cubemap.clone()),
        cubemap_enabled: resolve!(cubemap_enabled, config.custom_shader_cubemap_enabled),
        use_background_as_channel0: resolve!(
            use_background_as_channel0,
            config.custom_shader_use_background_as_channel0
        ),
    }
}

/// Resolve a cursor shader configuration by merging sources in priority order.
///
/// # Arguments
/// * `user_override` - Optional user overrides from config.yaml
/// * `metadata` - Optional shader metadata with embedded defaults
/// * `config` - Global config for fallback values
///
/// # Returns
/// A fully resolved cursor shader configuration with all values filled in
#[allow(dead_code)]
pub fn resolve_cursor_shader_config(
    user_override: Option<&CursorShaderConfig>,
    metadata: Option<&ShaderMetadata>,
    config: &Config,
) -> ResolvedCursorShaderConfig {
    // Resolve base shader config first
    let base_override = user_override.map(|o| &o.base);
    let base = resolve_shader_config(base_override, metadata, config);

    // Extract cursor-specific values from user override
    let glow_radius = user_override
        .and_then(|o| o.glow_radius)
        .unwrap_or(config.cursor_shader_glow_radius);

    let glow_intensity = user_override
        .and_then(|o| o.glow_intensity)
        .unwrap_or(config.cursor_shader_glow_intensity);

    let trail_duration = user_override
        .and_then(|o| o.trail_duration)
        .unwrap_or(config.cursor_shader_trail_duration);

    let cursor_color = user_override
        .and_then(|o| o.cursor_color)
        .unwrap_or(config.cursor_shader_color);

    ResolvedCursorShaderConfig {
        base,
        glow_radius,
        glow_intensity,
        trail_duration,
        cursor_color,
    }
}

#[allow(dead_code)]
impl ResolvedShaderConfig {
    /// Resolve a shader config for a specific shader.
    ///
    /// This is a convenience method that looks up the user override and
    /// combines it with metadata and global config.
    ///
    /// # Arguments
    /// * `shader_name` - Name of the shader file (e.g., "crt.glsl")
    /// * `metadata` - Optional shader metadata
    /// * `config` - Global config
    pub fn for_shader(
        shader_name: &str,
        metadata: Option<&ShaderMetadata>,
        config: &Config,
    ) -> Self {
        let user_override = config.get_shader_override(shader_name);
        resolve_shader_config(user_override, metadata, config)
    }

    /// Get channel paths as an array suitable for passing to the renderer.
    pub fn channel_paths(&self) -> [Option<PathBuf>; 4] {
        [
            self.channel0.clone(),
            self.channel1.clone(),
            self.channel2.clone(),
            self.channel3.clone(),
        ]
    }

    /// Get the cubemap path if configured.
    pub fn cubemap_path(&self) -> Option<&PathBuf> {
        if self.cubemap_enabled {
            self.cubemap.as_ref()
        } else {
            None
        }
    }
}

#[allow(dead_code)]
impl ResolvedCursorShaderConfig {
    /// Resolve a cursor shader config for a specific shader.
    ///
    /// # Arguments
    /// * `shader_name` - Name of the cursor shader file
    /// * `metadata` - Optional shader metadata
    /// * `config` - Global config
    pub fn for_shader(
        shader_name: &str,
        metadata: Option<&ShaderMetadata>,
        config: &Config,
    ) -> Self {
        let user_override = config.get_cursor_shader_override(shader_name);
        resolve_cursor_shader_config(user_override, metadata, config)
    }
}

/// Default global values for shader configuration.
///
/// These are used when neither user override nor metadata provide a value.
#[allow(dead_code)]
pub mod global_defaults {
    pub const ANIMATION_SPEED: f32 = 1.0;
    pub const BRIGHTNESS: f32 = 1.0;
    pub const TEXT_OPACITY: f32 = 1.0;
    pub const FULL_CONTENT: bool = false;
    pub const CUBEMAP_ENABLED: bool = true;

    // Cursor shader defaults
    pub const GLOW_RADIUS: f32 = 80.0;
    pub const GLOW_INTENSITY: f32 = 0.3;
    pub const TRAIL_DURATION: f32 = 0.5;
    pub const CURSOR_COLOR: [u8; 3] = [255, 255, 255];
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::ShaderConfig;

    fn make_test_config() -> Config {
        Config::default()
    }

    #[test]
    fn test_resolve_with_no_overrides() {
        let config = make_test_config();
        let resolved = resolve_shader_config(None, None, &config);

        assert_eq!(
            resolved.animation_speed,
            config.custom_shader_animation_speed
        );
        assert_eq!(resolved.brightness, config.custom_shader_brightness);
        assert_eq!(resolved.text_opacity, config.custom_shader_text_opacity);
        assert_eq!(resolved.full_content, config.custom_shader_full_content);
    }

    #[test]
    fn test_resolve_with_metadata_defaults() {
        let config = make_test_config();
        let mut shader_defaults = ShaderConfig::default();
        shader_defaults.animation_speed = Some(0.5);
        shader_defaults.brightness = Some(0.7);

        let metadata = ShaderMetadata {
            name: Some("Test".to_string()),
            defaults: shader_defaults,
            ..Default::default()
        };

        let resolved = resolve_shader_config(None, Some(&metadata), &config);

        assert_eq!(resolved.animation_speed, 0.5);
        assert_eq!(resolved.brightness, 0.7);
        // Others should use global defaults
        assert_eq!(resolved.text_opacity, config.custom_shader_text_opacity);
    }

    #[test]
    fn test_resolve_with_user_override() {
        let config = make_test_config();
        let mut user_override = ShaderConfig::default();
        user_override.animation_speed = Some(2.0);
        user_override.brightness = Some(0.9);

        let mut shader_defaults = ShaderConfig::default();
        shader_defaults.animation_speed = Some(0.5); // Should be overridden
        shader_defaults.text_opacity = Some(0.8); // Should be used (no user override)

        let metadata = ShaderMetadata {
            name: Some("Test".to_string()),
            defaults: shader_defaults,
            ..Default::default()
        };

        let resolved = resolve_shader_config(Some(&user_override), Some(&metadata), &config);

        // User override takes priority
        assert_eq!(resolved.animation_speed, 2.0);
        assert_eq!(resolved.brightness, 0.9);
        // Metadata default used when no user override
        assert_eq!(resolved.text_opacity, 0.8);
    }

    #[test]
    fn test_channel_paths() {
        let resolved = ResolvedShaderConfig {
            channel0: Some(PathBuf::from("/path/to/tex0.png")),
            channel1: None,
            channel2: Some(PathBuf::from("/path/to/tex2.png")),
            channel3: None,
            ..Default::default()
        };

        let paths = resolved.channel_paths();
        assert!(paths[0].is_some());
        assert!(paths[1].is_none());
        assert!(paths[2].is_some());
        assert!(paths[3].is_none());
    }

    #[test]
    fn test_cubemap_path_respects_enabled() {
        let mut resolved = ResolvedShaderConfig {
            cubemap: Some(PathBuf::from("/path/to/cubemap")),
            cubemap_enabled: true,
            ..Default::default()
        };

        assert!(resolved.cubemap_path().is_some());

        resolved.cubemap_enabled = false;
        assert!(resolved.cubemap_path().is_none());
    }
}
