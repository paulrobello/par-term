//! Shader configuration resolution.
//!
//! Handles merging of per-shader configurations from multiple sources:
//! 1. User overrides (from config.yaml shader_configs)
//! 2. Shader metadata defaults (from embedded YAML in shader files)
//! 3. Global defaults (from defaults.rs / Config struct)

use crate::config::Config;
use crate::types::{
    CursorShaderConfig, CursorShaderMetadata, ResolvedCursorShaderConfig, ResolvedShaderConfig,
    ShaderConfig, ShaderMetadata,
};
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
/// Priority (highest to lowest):
/// 1. User overrides from config.yaml cursor_shader_configs
/// 2. Defaults embedded in cursor shader metadata
/// 3. Global defaults from Config
///
/// # Arguments
/// * `user_override` - Optional user overrides from config.yaml
/// * `metadata` - Optional cursor shader metadata with embedded defaults
/// * `config` - Global config for fallback values
///
/// # Returns
/// A fully resolved cursor shader configuration with all values filled in
pub fn resolve_cursor_shader_config(
    user_override: Option<&CursorShaderConfig>,
    metadata: Option<&CursorShaderMetadata>,
    config: &Config,
) -> ResolvedCursorShaderConfig {
    // Extract metadata defaults if available
    let meta_defaults = metadata.map(|m| &m.defaults);

    // Helper to resolve a cursor-specific value through the priority chain
    macro_rules! resolve_cursor {
        ($field:ident, $global:expr) => {
            user_override
                .and_then(|o| o.$field)
                .or_else(|| meta_defaults.and_then(|m| m.$field))
                .unwrap_or($global)
        };
    }

    // Resolve base shader settings (animation_speed comes from base)
    let animation_speed = user_override
        .and_then(|o| o.base.animation_speed)
        .or_else(|| meta_defaults.and_then(|m| m.base.animation_speed))
        .unwrap_or(config.cursor_shader_animation_speed);

    // Build a minimal resolved base config for cursor shader
    // (cursor shaders don't use most of the base shader features)
    let base = ResolvedShaderConfig {
        animation_speed,
        brightness: 1.0,
        text_opacity: 1.0,
        full_content: true, // Cursor shaders always use full content
        channel0: None,
        channel1: None,
        channel2: None,
        channel3: None,
        cubemap: None,
        cubemap_enabled: false,
        use_background_as_channel0: false,
    };

    // Resolve cursor-specific values
    let hides_cursor = resolve_cursor!(hides_cursor, config.cursor_shader_hides_cursor);
    let disable_in_alt_screen = resolve_cursor!(
        disable_in_alt_screen,
        config.cursor_shader_disable_in_alt_screen
    );
    let glow_radius = resolve_cursor!(glow_radius, config.cursor_shader_glow_radius);
    let glow_intensity = resolve_cursor!(glow_intensity, config.cursor_shader_glow_intensity);
    let trail_duration = resolve_cursor!(trail_duration, config.cursor_shader_trail_duration);
    let cursor_color = user_override
        .and_then(|o| o.cursor_color)
        .or_else(|| meta_defaults.and_then(|m| m.cursor_color))
        .unwrap_or(config.cursor_shader_color);

    ResolvedCursorShaderConfig {
        base,
        hides_cursor,
        disable_in_alt_screen,
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
    /// * `metadata` - Optional cursor shader metadata
    /// * `config` - Global config
    pub fn for_shader(
        shader_name: &str,
        metadata: Option<&CursorShaderMetadata>,
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
        let shader_defaults = ShaderConfig {
            animation_speed: Some(0.5),
            brightness: Some(0.7),
            ..Default::default()
        };

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
        let user_override = ShaderConfig {
            animation_speed: Some(2.0),
            brightness: Some(0.9),
            ..Default::default()
        };

        let shader_defaults = ShaderConfig {
            animation_speed: Some(0.5), // Should be overridden
            text_opacity: Some(0.8),    // Should be used (no user override)
            ..Default::default()
        };

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
